RFC: Manager daemon refactor plan — add PLAN_download_manager_pattern.md

Kısa özet:
- RGSX için tek manager-daemon mimarisi planını ekler. Plan; hedefler, adımlar, riskler, yeni dosyalar ve kabul kriterlerini içerir.

Checklist:
- [ ] Mimari onayı alındı
- [ ] Önceliklendirme & sprint planı oluşturulsun
- [ ] Adım 1 (manager iskeleti) için tasklar açılsın

Ek notlar:
- SSE payload'larına version alanı eklenmesi önerisi
- Default bind: 127.0.0.1; uzak erişim gerekiyorsa TLS + token auth veya Unix socket seçeneği
- Örnek systemd / NSSM unit dosyası snippet'leri eklenecek (sonraki commit)
