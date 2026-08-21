# TASK-012f — TVUI Cutover (Flag Kapat + Divergence-Note)

- **id:** TASK-012f
- **title:** TVUI cutover — `RGSX_TVUI=1` ile Python fallback kapat, divergence-notları yaz
- **status:** todo
- **priority:** P1
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, cutover, parity, faz12b

## Kaynak

- `plan.md` §5.6, `docs/roadmap/FAZ12_PARITY_STRATEGY.md` §2 Adım D + §4 (feature-flag geçiş)
- `docs/roadmap/ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` §0 (TVUI birleştirme kararı)

## Açıklama

Tüm çekirdek ekranlar/menüler/klavye/accessibility SPA'da parity ile tamamlanınca,
`RGSX_TVUI=1` varsayılan yaparak eski Python TVUI (`tvui.py` + `display/*` + `controls/*`)
fallback'ini kapatır. Her bilinçli ayrılma (pygame→SPA) **divergence-note** ile belgelenir.

**Behavior contract (parity):**
- Cutover sonrası kullanıcının gördüğü TVUI davranışı eski ile aynı (regression yok).
- 102 contract + SSE baseline iki modda da yeşil kalır.

## Kapsam / Dosyalar

- `manager-tvui/src/` — `RGSX_TVUI` default davranışı.
- `docs/roadmap/` — her ayrılma için divergence-note (TASK-012a..e görev dosyalarına eklenir).

## Doğrulama

- `RGSX_TVUI=1` → Python TVUI hiç yüklenmez; SPA `?mode=tv` tek kaynak.
- Tüm TVUI contract testleri yeşil; divergence-notları mevcut.
- Canlı: gamepad ile tam TVUI akışı (yükleme→platform→oyun→indir→menü→arama) sorunsuz.

---

## İlerleme

- 2026-08-21 — plan.md §5.6'dan çıkarıldı (TASK-012a..e tamamlandığında uygulanacak).
