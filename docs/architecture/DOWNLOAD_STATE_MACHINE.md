# İndirme Durum Makinesi (Faz 8) — Referans

> Modül: `ports/RGSX/network/download_state.py` (415 satır). Saf modül — `config`/`network`
> eyleme bağımlılığı yoktur; retry/SSE yan etkileri `queue.py` ve `rgsx_manager.py` tarafından
> bağlanır. `tests/test_download_state.py` (57 test) bu belgedeki her kuralı tutar.

## Amaç

İndirme öğesinin yaşam döngüsünü serbest sözlük yerine resmî bir modelle yönetmek.
`DownloadState` (anlık durum), `DownloadEvent` (tetikleyici) ve `transition()` (yan etkili
geçiş) üçlüsü, geçersiz kombinasyonlarda `IllegalTransitionError` fırlatır — kopyala-yapıştır
status string'leri ve kontrolsüz state değişimini ortadan kaldırır. history.json'a yazım
geriye dönük uyumludur.

## Durumlar (`DownloadState`)

| State | Açıklama |
|---|---|
| `QUEUED` | Kuyrukta bekliyor |
| `DOWNLOADING` | İndiriliyor |
| `PAUSED` | Duraklatıldı |
| `VERIFYING` | Doğrulanıyor (postprocess öncesi) |
| `EXTRACTING` | Ayıklanıyor / dönüştürülüyor |
| `RETRY_SCHEDULED` | Backoff bekliyor, yeniden denenecek |
| `FAILED_TRANSIENT` | Geçici hata (retry hakkı var) |
| `FAILED_PERMANENT` | Kalıcı hata (retry yok / tükendi) |
| `COMPLETED` | Tamamlandı |
| `CANCELED` | İptal edildi |

## Olaylar (`DownloadEvent`)

`STARTED`, `PROGRESS`, `PAUSE_REQUESTED`, `RESUME_REQUESTED`, `TRANSIENT_FAILURE`,
`PERMANENT_FAILURE`, `RETRY_TRIGGERED`, `RETRY_EXHAUSTED`, `TRANSITIONED`, `COMPLETED`,
`CANCEL_REQUESTED`.

## Geçiş tablosu (`_TRANSITIONS`, download_state.py:66)

| Kaynak | Olay | Hedef |
|---|---|---|
| `QUEUED` | `STARTED` | `DOWNLOADING` |
| `DOWNLOADING` | `PAUSE_REQUESTED` | `PAUSED` |
| `PAUSED` | `RESUME_REQUESTED` | `DOWNLOADING` |
| `PAUSED` | `CANCEL_REQUESTED` | `CANCELED` |
| `DOWNLOADING` | `TRANSITIONED` | `VERIFYING` |
| `VERIFYING` | `TRANSITIONED` | `EXTRACTING` |
| `VERIFYING` | `COMPLETED` | `COMPLETED` |
| `EXTRACTING` | `COMPLETED` | `COMPLETED` |
| `DOWNLOADING` | `COMPLETED` | `COMPLETED` |
| `DOWNLOADING` | `TRANSIENT_FAILURE` | `FAILED_TRANSIENT` |
| `FAILED_TRANSIENT` | `RETRY_TRIGGERED` | `RETRY_SCHEDULED` |
| `RETRY_SCHEDULED` | `STARTED` | `DOWNLOADING` |
| `FAILED_TRANSIENT` | `PERMANENT_FAILURE` | `FAILED_PERMANENT` |
| `FAILED_TRANSIENT` | `RETRY_EXHAUSTED` | `FAILED_PERMANENT` |
| `RETRY_SCHEDULED` | `PERMANENT_FAILURE` | `FAILED_PERMANENT` |
| `RETRY_SCHEDULED` | `CANCEL_REQUESTED` | `CANCELED` |
| `DOWNLOADING` | `PERMANENT_FAILURE` | `FAILED_PERMANENT` |
| `DOWNLOADING` | `CANCEL_REQUESTED` | `CANCELED` |
| `VERIFYING` | `CANCEL_REQUESTED` | `CANCELED` |
| `EXTRACTING` | `CANCEL_REQUESTED` | `CANCELED` |
| `FAILED_TRANSIENT` | `CANCEL_REQUESTED` | `CANCELED` |

Tablo dışı herhangi bir `(state, event)` kombinasyonu `IllegalTransitionError` fırlatır.

### Kritik akışlar

```
BAŞARI:  QUEUED ─STARTED→ DOWNLOADING ─COMPLETED→ COMPLETED
         (postprocess): DOWNLOADING ─TRANSITIONED→ VERIFYING ─TRANSITIONED→ EXTRACTING ─COMPLETED→ COMPLETED

GEÇİCİ:  DOWNLOADING ─TRANSIENT_FAILURE→ FAILED_TRANSIENT ─RETRY_TRIGGERED→ RETRY_SCHEDULED ─STARTED→ DOWNLOADING
         (retry tükendi): FAILED_TRANSIENT ─RETRY_EXHAUSTED→ FAILED_PERMANENT

KALICI:  DOWNLOADING ─PERMANENT_FAILURE→ FAILED_PERMANENT

İPTAL:   (her aktif durum) ─CANCEL_REQUESTED→ CANCELED
```

## `transition()` sözleşmesi (download_state.py:351)

```python
transition(job, event, effects=None) -> DownloadState
```

- `job.state` güncellenir; geçersiz kombinasyon → `IllegalTransitionError`.
- `effects(job, old_state, new_state, event)` geri çağrısı persist/emit gibi yan etkiler
  içindir (kullanıcıya bırakılır). Effects hatası log'lanır, geçişi bozmaz.
- `queue.py` içinde bazı noktalarda `IllegalTransitionError` yakalanıp state doğrudan set
  edilir (stale/melez eski kayıtlarda yumuşak geçiş) — model serttir ama veri onarımına
  izin verir.

## Yardımcılar

| Fonksiyon | Rol |
|---|---|
| `is_active_state(state)` | Canlı/ilerliyor kümesi: `DOWNLOADING, PAUSED, VERIFYING, EXTRACTING, RETRY_SCHEDULED, FAILED_TRANSIENT` |
| `retryable(state)` | `FAILED_TRANSIENT` veya `RETRY_SCHEDULED` |
| `state_from_legacy(status)` | Eski history string → enum (`Téléchargement`, `Download_OK`, `Erreur`, `Try N/M` ...) |
| `legacy_history_status(state)` | Enum → TVUI/WebUI'nin anladığı string |

## Hata sınıflandırıcı (`classify_error`, download_state.py:195)

`True` = geçici (retry mantıklı), `False` = kalıcı. Sıralama:

1. `error_type` exception verildiyse: `InsufficientDiskSpaceError` → kalıcı;
   `Timeout`/`Connection` tip adı → geçici.
2. Metin yoksa → kalıcı (belirsiz).
3. **Kalıcı marker'lar her zaman öncelikli** (`_PERMANENT_MARKERS`): `"access denied"`,
   `"browser challenge"`, `"not a valid archive"`, `"file not found"`, `"password incorrect"`,
   `"insufficient disk space"`, `"removed for abuse"` vb.
4. Serbest 3 haneli HTTP kod taraması: `_TRANSIENT_HTTP_STATUS` =
   `{408,409,425,429,500,502,503,504,520..527}` → geçici;
   `_PERMANENT_HTTP_STATUS` = `{400..418,420..431,451}` → kalıcı.
5. `_TRANSIENT_MARKERS`: `"timeout"`, `"connection reset"`, `"rate limit"`,
   `"temporarily unavailable"`, `"limite les téléchargements"` vb. → geçici.
6. Hiçbiri eşleşmediyse → kalıcı (sonsuz retry döngüsünü önler).

## Retry backoff (download_state.py:250)

```python
retry_backoff_seconds(retry_count, base=5.0, max_wait=300.0) -> float
```

- `retry_count=1` → `base` (5 sn), `2` → `2*base`, `3` → `4*base` ... `max_wait` ile tavan.
- `DEFAULT_MAX_RETRIES = 3`. `queue.py` `_max_retries()`/`_retry_backoff()` bu modeli kullanır.

## `DownloadJob` modeli (download_state.py:260)

`@dataclass`: `id`, `url`, `destination`, `state`, `progress`, `retry_count`, `error`,
`task_id`, `platform`, `game_name`, `message`, `timestamp`, `is_zip_non_supported`,
`max_retries`, `retry_at`, `metadata`.

- `from_history_entry(entry)`: history.json satırı → job; `entity_state` alanı yoksa legacy
  `status`'tan `state_from_legacy` ile doldurur (eski format okumada özel iş gerekmez).
- `apply_to_history_entry(entry)`: mevcut alan adlarını korur, ek alanları overlay eder:
  `status` (legacy), `entity_state`, `retry_count`, `max_retries`, `error`, `retry_at`,
  `progress`, `message`.

## SSE yayını (isteğe bağlı emitter)

`set_state_emitter(fn)` / `emit_state_event(event_type, **data)` — emitter yoksa no-op.
`rgsx_manager.py` `_broadcast`'i kaydeder (`set_state_emitter(_broadcast)`), olaylar
`download_state` tipiyle SSE üzerinden yayınlanır: `completed`, `retry_scheduled`,
`failed_permanent`.

## Doğrulama

- `tests/test_download_state.py` — 57 test: tüm geçerli geçişler, illegal kombinasyonlar,
  classify_error (marker/HTTP kod/tip), backoff formülü, legacy mapping, history uyumu.
- Tam suite: **341 passed / 23 pre-existing** (bkz. `docs/guides/TESTING.md`).
