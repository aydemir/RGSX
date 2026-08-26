# TASK-016 — Rust tabanlı Docker imajı (ihtiyaç halinde)

- **id:** TASK-016
- **title:** Native manager-bin için opsiyonel Docker imajı
- **status:** todo (tetikleyici: gerçek ihtiyaç doğduğunda)
- **priority:** P2
- **created:** 2026-08-26
- **environment:** linux
- **tags:** docker, native-only, packaging

## Kaynak

- TASK-012-gap-02 (`tasks/done/TASK-012-gap-02-python-cleanup.md`) — eski `docker/`
  içeriği tamamen Python imajıydı (`FROM python:3.11-slim`, `COPY ports/RGSX/`) ve
  kullanıcı kararıyla silindi. "Rust imaj gerekirse ayrı görev" notunun kendisi bu
  dosyadır.

## Açıklama

Python döneminde Docker desteği vardı; native-only geçişte kaldırıldı. Gerçek bir
kullanım senaryosu doğarsa (sunucu/konteyner dağıtımı, self-host WebUI) Rust tabanlı
yeni bir imaj yazılmalıdır:

- **Multi-stage öneri:** `rust:1.xx` builder (manager-bin release derlemesi; SDL2
  dev paketleri TVUI istenirse) → `debian:bookworm-slim` runtime (yalnız binary +
  `webui/dist` + katalog verisi hacimleri).
- Eski `docker-entrypoint.sh`'in ENV sözleşmesi referans alınabilir
  (`RGSX_HEADLESS`, `RGSX_APP_DIR`, `RGSX_CONFIG_DIR`, `RGSX_DATA_DIR` — paths.rs
  env'leriyle eşleştirilmeli).
- TVUI konteyner içinde anlamsızdır (display yok) → imaj headless WebUI odaklı olur.
- Önce soru: hangi dağıtım hedefi (unraid/Synology/K8s?) ve kim bakıyor?

## Kapsam / Dosyalar

- `docker/Dockerfile` (yeni), `docker/docker-compose.example.yml`, `README-DOCKER.md`
- Gerekirse `manager-bin`'e headless davranış ince ayarı (tray yok zaten Linux'ta)

## Doğrulama

- `docker build` yeşil; `docker run -p 5000:5000 -v data:/data` ile WebUI :5000 canlı,
  katalog OTA bootstrap çalışıyor, indirme akışı DDL+torrent canlı.

---

## İlerleme

- 2026-08-26 — Görev oluşturuldu (gap-02 docker silme kararının takibi; tetikleyici
  bekliyor).
