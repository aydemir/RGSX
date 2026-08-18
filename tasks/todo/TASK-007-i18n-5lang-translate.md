# TASK-007 — 36 yeni webui i18n anahtarının fr/es/de/it/pt çevirilerinin eklenmesi

- **id:** TASK-007
- **title:** Yeni eklenen 36 webui i18n anahtarının fr/es/de/it/pt dilinde tamamlanması (7-dil parity)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both
- **tags:** i18n, webui, translations, parity, localization

## Kaynak

- `13e444b` (dil seçimi fix) ve `a150660` (Python JSON hizalaması) ile 36 yeni UI anahtarı eklendi. Bunlar şu an **yalnız `tr`+`en`** değerine sahip; `fr/es/de/it/pt` için `current→en→tr` fallback nedeniyle **İngilizce** gösteriyor. Tam 7-dil parity hedeflenmişti ama kapatılmadı.

## Açıklama

36 yeni anahtarın fr/es/de/it/pt karşılıklarını `ports/RGSX/languages/*.json` dosyalarına ekle. Anahtarlar:
`active_dl, tab_platforms, tab_downloaded, tab_queue, tab_history, games_label, filter_hide_dl, filter_hide_demo, filter_one_rom, filter_regex, pf_sort_name_asc, pf_sort_name_desc, pf_sort_size_asc, filter_reset, download_all, downloaded_empty, pause_all, resume_all, cancel, history_clear, history_empty, font_family, allow_unknown, source_mode, custom_url, symlink_label, linux_section, web_service_label, custom_dns_label, system_info, game_search_ph, default_placeholder, refresh_title, settings_title, dl_now_title, dl_queue_title`

## Kapsam / Dosyalar

- `ports/RGSX/languages/{fr,es,de,it,pt}.json` — yeni anahtar değerleri eklenir.
- `webui/scripts/gen-i18n.mjs` — değişmez (otomatik üretir).
- `webui/src/i18n.strings.js` — `npm run gen:i18n` ile yeniden üretilir.
- `webui/dist/*` — rebuild.

## Doğrulama

- `npm run gen:i18n` hatasız çalışır.
- Playwright: FR/DE/ES/IT/PT diline çevrilince yukarıdaki string'ler doğru dilde görünür (İngilizce fallback kalmamalı).
- `git diff` ile 5 JSON dosyasında yalnız yeni anahtarların eklendiği teyit edilir.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (dil fix + hizalama sonrası kalan 5-dil boşluğu).
