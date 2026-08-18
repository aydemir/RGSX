# TASK-007 — 36 yeni webui i18n anahtarının fr/es/de/it/pt çevirilerinin eklenmesi

- **id:** TASK-007
- **title:** Yeni eklenen 36 webui i18n anahtarının fr/es/de/it/pt dilinde tamamlanması (7-dil parity)
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both
- **tags:** i18n, webui, translations, parity, localization

## Kaynak

- `13e444b` (dil seçimi fix) ve `a150660` (Python JSON hizalaması) ile 36 yeni UI anahtarı eklendi. Bunlar şu an **yalnız `tr`+`en`** değerine sahip; `fr/es/de/it/pt` için `current→en→tr` fallback nedeniyle **İngilizce** gösteriyordu. Tam 7-dil parity hedeflenmişti ama kapatılmamıştı.

## Açıklama

36 yeni anahtarın fr/es/de/it/pt karşılıklarını `ports/RGSX/languages/*.json` dosyalarına ekle. Anahtarlar:
`active_dl, tab_platforms, tab_downloaded, tab_queue, tab_history, games_label, filter_hide_dl, filter_hide_demo, filter_one_rom, filter_regex, pf_sort_name_asc, pf_sort_name_desc, pf_sort_size_asc, filter_reset, download_all, downloaded_empty, pause_all, resume_all, cancel, history_clear, history_empty, font_family, allow_unknown, source_mode, custom_url, symlink_label, linux_section, web_service_label, custom_dns_label, system_info, game_search_ph, default_placeholder, refresh_title, settings_title, dl_now_title, dl_queue_title`

## Kapsam / Dosyalar

- `ports/RGSX/languages/{fr,es,de,it,pt}.json` — 36 anahtarın tamamı gerçek çeviriyle dolduruldu.
- `webui/scripts/gen-i18n.mjs` — değişmez (otomatik üretir).
- `webui/src/i18n.strings.js` — `npm run gen:i18n` ile yeniden üretildi.
- `webui/dist/*` — rebuild + `/test/roms/ports/RGSX/webui/`'a deploy edildi.

## Doğrulama

- `npm run gen:i18n` hatasız (99 kullanılan anahtar işlendi), `npm run build` başarılı.
- Playwright: FR/DE/ES/IT/PT için 5 dilde de tab/ayar etiketleri doğru dilde göründü, **İngilizce fallback kalmadı** (`avoid=0`):
  - fr: Plateformes / Téléchargés / File d'attente / Historique / Paramètres ✓
  - de: Plattformen / Heruntergeladen / Warteschlange / Verlauf / Spiele / Einstellungen ✓
  - es: Plataformas / Descargados / Cola / Historial / Ajustes ✓
  - it: Piattaforme / Scaricati / Coda / Cronologia / Impostazioni ✓
  - pt: Plataformas / Transferidos / Fila / Histórico / Configurações ✓
- `filter_regex`="Regex" ve `linux_section`="Linux / Batocera" ortak terim/proper-noun olduğu için İngilizceyle aynı kaldı (kabul edilebilir); gerçek çevrilmemiş string yok.
- `git diff` ile 5 JSON dosyasında yalnız 36 anahtarın değerleri değişti (sıra/format korundu).

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (dil fix + hizalama sonrası kalan 5-dil boşluğu).
- 2026-08-18 — 36 anahtar fr/es/de/it/pt'ye çevrildi, gen:i18n + build + deploy, Playwright ile 5 dil PASS.
