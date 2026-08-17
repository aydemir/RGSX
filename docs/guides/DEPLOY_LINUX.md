# RGSX Linux Canlıya Alma (Rust `manager-bin`) — Deploy Talimatı

Bu doküman, `manager-bin` binary'sini Linux'ta canlıya aldığında **hangi klasör
yapısının ve dosyaların deploy edilmesi gerektiğini** tanımlar. "Deploy et" denildiğinde
referans alınacak tek otoritatif kaynaktır.

## 1. Özet — deploy edilmesi gerekenler

```
<deploy>/                         # deploy kökü (örn. /opt/rgsx)
├── manager-bin                   # 1) derlenmiş Rust binary (ZORUNLU)
├── webui/                        # 2) BUILT SPA kökü (ZORUNLU)
│   ├── index.html               #    npm run build çıktısı (→ /static/assets/... işaret eder)
│   └── assets/                  #    index-*.js, index-*.css  (ZORUNLU)
├── languages/                    # 3) çeviri dosyaları (opsiyonel, native catalog için)
├── qbittorrent_backend.py        # 4) torrent backend (opsiyonel)
└── data/                         # 5) RGSX_DATA_DIR (otomatik türetilir, ilk çalıştırmada oluşur)
    ├── downloads/
    └── logs/
```

**Kritik kural:** `webui/` içinde `index.html` + `assets/` **built (derlenmiş)**
olmalıdır. `webui/` kaynak klasöründeki dev `index.html` (`/src/main.js` işaret eder)
KULLANILMAZ — sayfa boş görünür.

## 2. Build adımları (deploy öncesi)

### 2a. Rust binary
```bash
# termux/bionic ortamda statik lzma link hatası için:
LZMA_API_STATIC=1 cargo build -p manager-bin
# çıktı: manager-rs/target/debug/manager-bin  (veya release)
```

### 2b. WebUI (SPA) — ZORUNLU
```bash
cd webui
npm install
npm run build          # → webui/dist/  (index.html + assets/)
```

> `vite.config.js` `base: '/static/'` ayarlıdır; bu yüzden built `index.html`
> `/static/assets/...` işaret eder ve Rust router `nest("/static", ServeDir)`
> ile `webui_dir/assets/...` sunar. **`webui/dist` doğrudan `webui/` olarak
> deploy edilmelidir** (içindeki `dist/` alt klasörü değil).

## 3. Deploy yöntemleri

### Yöntem A — Env override (önerilen, RetroBat gerektirmez)
Binary'yi istediğin yere koy, SPA'yı `webui/dist` içeriğini `webui/`'ye kopyala,
sonra env ile göster:

```bash
mkdir -p /opt/rgsx/webui
cp -r webui/dist/. /opt/rgsx/webui/
cp manager-rs/target/debug/manager-bin /opt/rgsx/

cd /opt/rgsx
RGSX_WEBUI_DIR=/opt/rgsx/webui \
RGSX_DATA_DIR=/opt/rgsx/data \
./manager-bin
# log: "Ağ erişimi: http://<LAN_IP>:5000"
```

`RGSX_WEBUI_DIR` set edilirse `paths.rs` override devreye girer ve boş placeholder
servis edilmez.

### Yöntem B — RetroBat yerleşimi (resolver varsayılanı)
Binary'yi `roms/ports/RGSX/` altına koyarsan `rgsx_dir/webui` otomatik çözülür:
```
<retrobat>/
└── roms/ports/RGSX/        # rgsx_dir (binary burada)
    ├── manager-bin
    └── webui/              # = rgsx_dir/webui  (BUILT SPA)
        ├── index.html
        └── assets/
```
Anchor `roms/ports/RGSX` imzası bulunamazsa resolver `.parent()×3` fallback'e
düşer ve `webui_dir` yanlış yere işaret eder → boş sayfa. Bu yüzden Yöntem A
daha güvenlidir.

## 4. Çalışma zamanı env değişkenleri

| Env | Varsayılan (türetilen) | Açıklama |
|-----|------------------------|----------|
| `RGSX_WEBUI_DIR` | `rgsx_dir/webui` | **Built SPA kökü** — boş sayfa sorununun çözümü |
| `RGSX_DATA_DIR` | `root/saves/ports/rgsx` | indirme/geçmiş/log kökü |
| `RGSX_RUST_WEBUI` | `1` | saf-Rust webui (port 5000); `0` → eski UI (5010) |
| `RGSX_NATIVE_CATALOG` | `1` | Python'sız local katalog |
| `RGSX_1FICHIER_KEY` / `RGSX_REALDEBRID_KEY` | — | debrid (opsiyonel) |

Port: vars. `5000`, `0.0.0.0`'e bind (LAN'dan erişilir).

## 5. Doğrulama (canlı kanıt)
1. Tarayıcı: `http://<LAN_IP>:5000` → SPA yüklenmeli (boş `<h1>` DEĞİL).
2. `curl -f http://localhost:5000/api/health` → `{"success":true,...}`.
3. `curl -f http://localhost:5000/static/assets/$(ls webui/assets | head -1)` → JS dönmeli (404 DEĞİL).

Boş sayfa görülürse: `RGSX_WEBUI_DIR`'in built `index.html` + `assets/` içerdiğini
ve `webui/`'nin dev `index.html` olmadığını kontrol et.
