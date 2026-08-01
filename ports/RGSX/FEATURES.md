# RGSX Özellikler ve Değişiklik Günlüğü

## Yapılan Özelleştirmeler (RetroBat Entegrasyonu)

### v2.6.4.9-TR1 - Oyun Listesi Durum Göstergeleri

**Dosya:** `display.py`

Oyun listesinde indirme durumu renkli göstergelerle gösterilir:

| Durum | Prefix | Renk | Açıklama |
|-------|--------|------|----------|
| İndirilmiş | `[>]` | Yeşil `(100, 255, 100)` | `is_game_downloaded()` ile doğrulanmış |
| İndiriliyor | `[~] %sayı` | Sarı `(255, 200, 0)` | `config.download_tasks` + `download_progress` |
| Başarısız | `[X]` | Kırmızı `(255, 80, 80)` | `config.history` son deneme `Erreur`/`Error` |
| Normal | yok | Tema rengi | Henüz indirilmemiş |

**Örnek:** `[~] 45% After Burner` → sarı renkte, %45 indirilmiş.

**Mantık:**
- `config.download_tasks` → Aktif indirme görevleri (task_id → (task, url, game_name, platform))
- `config.download_progress` → İndirme ilerlemesi (url → {status, progress_percent, ...})
- `config.history` → Geçmiş indirmeler (başarısız olanlar kontrol edilir)

---

### v2.6.4.9-TR1 - Türkçe Dil Desteği

**Dosyalar:** `language.py`, `languages/tr.json`, `static/js/app.js`

- `language.py`: `get_language_name()` fonksiyonuna `"tr": "Türkçe"` eklendi
- `languages/tr.json`: Tam Türkçe çeviri dosyası (337+ anahtar)
- `static/js/app.js`: Web arayüzünde Türkçe dil seçeneği eklendi

**Desteklenen diller:** FR, EN, ES, DE, IT, PT, JA, ZH, RU, **TR**

---

### v2.6.4.9-TR1 - Performans Optimizasyonu

**Dosya:** `display.py`

Gradient ve grain texture önbellek (cache) sistemi eklendi:

```python
_gradient_cache = {"surface": None, "top": None, "bottom": None, "size": None}
_grain_cache = {"surface": None, "size": None}
```

- `_build_grain_surface()`: Grain texture'sını sabit seed (42) ile bir kez oluşturur
- `draw_gradient()`: Aynı parametrelerle her frame'de yeniden çizim yerine cache'den okur
- Büyük ekranlarda belirgin performans artışı sağlar

---

## Orijinal RGSX Özellikleri

### v2.6.4.9

- Akıllı Sistem Tespiti (`es_systems.cfg` otomatik okuma)
- Akıllı Arşiv Yönetimi (ZIP desteklenmiyorsa otomatik çıkarma)
- Premium Kaynak Desteği (1Fichier API + AllDebrid/Debrid-Link/Real-Debrid/TorBox)
- Özelleştirilebilir Arayüz (3×3 - 4×4 layout, fontlar, diller)
- Kontrolcü Desteği (otomatik eşleme + özel yeniden eşleme)
- Gelişmiş Filtreleme (isme göre arama, platform filtreleme)
- İndirme Yönetimi (kuyruk, geçmiş, ilerleme bildirimleri)
- Erişilebilirlik (ayrı font ölçekleme, klavye modu)
- Web Arayüzü (Batocera/Knulli - uzaktan indirme)
- Arka Plan Müzik Desteği
- Symlink/Copy Seçenekleri
- Otomatik Güncelleme

---

## Gelecek Planlar (Roadmap)

### v2.6.5.0 - Arka Plan İndirme Servisi

**Problem:** TV UI (Pygame) ve indirme motoru aynı process içinde. TV UI kapatıldığında tüm indirmeler öldürülüyor.

**Hedef:** TV UI kapatılsa bile indirmeler arka planda devam etsin.

**Mimari:** Service/Worker Pattern
- `rgsx_service.py`: Bağımsız indirme servisi (daemon)
- REST API (localhost:6999/api/downloads)
- TV UI ve Web UI istemci olarak bağlanır

**Durum:** [ ] Tasarım tamamlandı, [ ] Uygulanacak
