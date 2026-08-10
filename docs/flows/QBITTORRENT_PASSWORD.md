# qBittorrent WebUI Şifre Yönetimi Akışı

Bu doküman qBittorrent WebUI şifresinin RGSX tarafında nasıl güvence altına alındığını
ve WebUI'dan nasıl yönetildiğini açıklar.

## Arka plan

qBittorrent WebUI API'si mevcut şifreyi geri okumaz. RGSX şifreyi kendi settings'inde
tutar (`qbittorrent_webui_password` anahtarı) ve her qBittorrent açılışında
(`_ensure_qbittorrent_running` bootstrap) bu şifreyi `setPreferences` ile WebUI'a uygular.

qBittorrent lazy spawn olduğundan Migration v1 yalnızca qBittorrent ilk RUNNING
olduğunda çalışıyordu. "Varsayılan şifre kullanımda" durumunu imkânsız kılmak için artık
**manager açılışında** şifre güvence altına alınır.

## Açılış akışı

```
rgsx_manager.py main()
  └─ ensure_qbittorrent_password_secured()
       ├─ settings okunur (load_rgsx_settings)
       ├─ depolanmış şifre güvenliyse (boş değil ve KNOWN_DEFAULT_PASSWORDS'te değilse)
       │    → dokunulmaz, olduğu gibi döner
       ├─ yoksa/varayılandaysa → secrets.token_urlsafe(16) ile rastgele üretilir
       │    set_qbittorrent_webui_password + set_qbittorrent_password_mode("random")
       │    + set_qbittorrent_password_migration_done(True)
       └─ üretilen şifre bir sonraki qBittorrent spawn'ında WebUI'a uygulanır
```

## Şifre durumu (mode)

`get_qbittorrent_password_mode()` üç durum döndürür:

| mode | Anlam |
|---|---|
| `default` | Henüz güvence altına alınmamış (settings'te ne mode ne şifre kaydı var) |
| `random` | RGSX tarafından üretilmiş rastgele şifre kullanımda |
| `custom` | Kullanıcı tanımlı şifre (mode anahtarı yoksa depolanmış şifre varlığından çıkarım) |

`get_password_status()` (manager `GET /api/qbittorrent/password-status`):
`{available, using_default, secured, mode, webui_url}`. `secured = şifre varsayılan değil`.

## WebUI yönetimi

Settings → qBittorrent bölümündeki şifre kartı (durum `password-status`'tan yüklenir):

- **Rastgele Şifre Üret** (`POST /api/qbittorrent/regenerate-password`):
  `regenerate_qbittorrent_password()` çağırır. qBittorrent canlıysa şifre anında
  `setPreferences` ile uygulanır (mevcut WebUI oturumları geçersiz olur); değilse
  settings'e yazılır, bir sonraki bootstrap uygular. Yeni şifre kullanıcıya bir kez
  gösterilir.
- **Özel Şifre Belirle** (`POST /api/qbittorrent/change-password`): `change_webui_password`
  akışıyla yeni şifre belirlenir; mode `custom` olarak kaydedilir.
- **WebUI'ı Aç**: `password-status`'tan gelen `webui_url` yeni sekmede açılır.
- Şifrenin kendisi UI'da asla görüntülenmez; yalnızca üretildiği anda bir kez gösterilir.

## Testler

`tests/test_qbittorrent_backend.py` — `ensure_qbittorrent_password_secured`,
`regenerate_qbittorrent_password`, `get_password_status` (yeni `secured`/`mode`) ve
`get/set_qbittorrent_password_mode` contract'ları. Gerçek qBittorrent süreci başlatılmaz;
tüm dış bağımlılıklar monkeypatch ile izole edilir.

Not: `tests/test_qbittorrent_port.py` port testleri, makinede gerçek qBittorrent 18572'yi
dinliyorsa ortam kaynaklı başarısız olabilir (beklenen davranış).
