# Rust Geçiş Notları — Karşılaşılan Hatalar ve Çözümleri

> **Amaç:** Rust `manager-bin`'in RetroBat kurulumunda canlıya alınması sırasında
> karşılaşılan tüm hatalar, kök nedenleri ve çözümlerini tek yerde toplamak.
> Göçü tekrar yapan veya geri döndüren bir geliştirici için kontrol listesi niteliğindedir.
>
> **Tarih:** 2026-08-13 · **Kapsam:** Faz 10c/3 (Rust WebUI sidecar) + Faz 10c/3/4 (indirme proxy)
>
> **Mimari özet:** Rust `manager-bin` (port 5010) bir **strangler/proxy**'dir —
> gerçek veri ve indirme **Python manager'da** (port 5001) yaşar. Rust yalnızca WebUI'yi
> sunar ve katalog/indirme route'larını Python'a devreder (`RGSX_PYTHON_MANAGER_URL`).
> Bu yüzden **Python manager kapalıysa Rust WebUI boş/hatalı görünür** — bu tasarım gereğidir.

---

## 1. Hata: WebUI placeholder / boş görünüyor

**Belirti:** `http://127.0.0.1:5010/` yalnızca placeholder HTML döner; gerçek arayüz yok.

**Kök neden:** `static_root` yok — `manager-bin` static klasörünü bulamıyor.
`static_root` env değişkeninden okunur:

| Env | Anlam |
|---|---|
| `RGSX_WEBUI_DIR` | static root (WebUI HTML/CSS/JS klasörü) |
| `RGSX_MANAGER_SCRIPT` | torrent bridge script (`qbittorrent_backend.py`) |
| `RGSX_PYTHON_MANAGER_URL` | Python manager proxy base (`http://127.0.0.1:<port>`) |
| `RGSX_TORRENT_ENGINE` | `python` (bridge) veya `librqbit` (embedded) |
| `RGSX_NO_AUTOSTART` | `1` → registry autostart kaydı yazma |

**Çözüm:** `RGSX_WEBUI_DIR`'i `static/` klasörüne işaret et. RetroBat kopya kurulumu için
`RGSX Retrobat.bat` içinde ayarlandı (bkz. bölüm 6).

---

## 2. Hata: `/api/platforms` → `count: 0` (proxy boş dönüyor)

**Belirti:** WebUI yükleniyor ama platform listesi boş. Rust health OK, Python health OK.

**Kök neden:** **Port uyuşmazlığı.** Python manager'ın gerçek portu `rgsx_settings.json`
içindeki `manager_port` değeridir — bu makinede **5001**. Test script'leri ve ilk `.bat`
kodu sabit **5000** kullanıyordu → Rust proxy yanlış porta gidip boş yanıt aldı.

**Çözüm:** Portu sabit kodlama, `rgsx_settings.json`'dan **dinamik oku**:

```bat
for /f "usebackq delims=" %%p in (`powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "(Get-Content -Raw -LiteralPath '!RGSX_SETTINGS!' | ConvertFrom-Json).manager_port"`) do set "RGSX_PY_PORT=%%p"
if not defined RGSX_PY_PORT set "RGSX_PY_PORT=5000"
set "RGSX_PYTHON_MANAGER_URL=http://127.0.0.1:!RGSX_PY_PORT!"
```

**Kural:** `manager_port` değiştiğinde Rust proxy'nin base'i de aynı değere işaret etmelidir.

---

## 3. Hata: Sidecar `RGSXManager` registry kaydını eziyor

**Belirti:** Windows açılışında Rust binary Python manager'ın boot autostart kaydını
üzerine yazıyor / çift manager başlıyor.

**Kök neden:** Rust `manager-windows` autostart kodu Python ile **aynı registry anahtarını**
kullanıyor: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\RGSXManager`.
Launcher'ın spawn ettiği sidecar, boot kaydını ezdi.

**Çözüm (`main.rs`):** Autostart bloğu öncesi env kontrolü:

```rust
if std::env::var("RGSX_NO_AUTOSTART").map(|v| v == "1").unwrap_or(false) {
    // launcher sidecar: registry'ye dokunma — boot autostart Python'da kalır
}
```

Launcher `.bat`'ında `RGSX_NO_AUTOSTART=1` set edilir. Böylece Python boot autostart
kaydı korunur; Rust yalnızca oturum boyunca çalışır.

---

## 4. Hata: İndirme "İndirme başladı" diyor ama hiçbir şey olmuyor — `/api/download` "Index de jeu invalide"

**Belirti:** WebUI indirme butonu `❌ Index de jeu invalide: 0` hata gösteriyor.
Python 5001 aynı istekle **çalışıyor**, Rust 5010 **çalışmıyor**.

**Kök neden:** WebUI (`static/js/app.js`) indirme isteğini her zaman
`{ platform, game_index: <number> }` olarak gönderir (nadiren `game_name`).
Rust `/api/download` handler'ı yalnızca `direct_url` destekliyordu; `game_index`'i
hiç çözümlemiyordu → yerel placeholder `json_err("Index de jeu invalide")` düşüyordu.

**Çözüm (`api.rs::download`):** Handler başına katalog proxy ekle — `download_batch`
deseniyle aynı. `catalog` varsa isteği olduğu gibi Python'a ilet (game_index/game_name
çözümü Python'da); yoksa yerel `direct_url` + bridge/librqbit yolu korunur:

```rust
if let Some(c) = &state.catalog {
    if let Ok(v) = c.post_json("/api/download", &body).await {
        return ok(v);
    }
}
```

**Kural:** `catalog` (proxy) aktifken indirme yetkisi **her zaman Python'dadır**.
Yerel `direct_url`/bridge yolu yalnızca `catalog=None` (pure Rust) için korunur.

---

## 5. Hata: İndirme kuyruğa giriyor ama Kuyruk/Aktif/Geçmiş hep boş

**Belirti:** İndirme butonu başarılı (toast), ama WebUI panelleri "Devam eden indirme yok",
"Kuyrukta öğe yok", "Tamamlanmış indirme yok" gösteriyor.

**Kök neden:** Bölüm 4'te `/api/download` proxy'e alındı → indirme durumu **Python
tarafında** yaşamaya başladı. Ama GET `/api/progress`, GET `/api/history`, GET `/api/queue`
hâlâ **Rust'ın kendi boş in-memory state'ini** okuyordu. WebUI bunları poll ettiğinden
hepsi boş görünüyordu.

**Çözüm (`api.rs`):** Üç GET handler'ına da katalog proxy ekle (önce proxy, başarısızsa
yerel placeholder):

| Handler | Proxy route |
|---|---|
| `progress` | `/api/progress` |
| `history` | `/api/history` |
| `queue` | `/api/queue` |

```rust
pub async fn queue(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/queue").await {
            return ok(v);
        }
    }
    // yerel placeholder (catalog=None)
}
```

**Kural:** İndirme akışına dokunan **tüm** state GET'leri proxy olmalıdır
(`game-status` zaten proxy'ydi; `progress`, `history`, `queue` eksikti).
"Bir endpoint'i proxy'e al, diğerlerini unut" en sinsi hata kaynağıdır — **eşleşmeyi**
`FAZ10C3_CONTRACT_MAP.md` tablosundan her seferinde doğrula.

---

## 6. Hata: Rust sidecar launcher'dan çalışmıyor (RetroBat kopya kurulum)

**Belirti:** `.bat` TVUI'yi açıyor ama 5010'da Rust yok / Python manager yok.

**Kök neden (kopya kuruluma özgü):**
- Deployed kopyada (`C:\RetroBat - Kopya\roms\ports\RGSX`) **`rust_daemon.py` yok** ve
  `rgsx_manager.py`'de `RGSX_RUST_DAEMON` entegrasyonu yok (yalnızca kaynak
  `C:\Users\lv\RGSX\RGSX\ports\RGSX\` içinde var) → Python-supervised (daemon) yol
  kopyada çalışmaz.
- Çözüm: `.bat` launcher'a **doğrudan sidecar bloğu** eklendi.

**Çözüm (`RGSX Retrobat.bat`, satır ~578-607):**
1. `RGSX_PY_PORT`'u settings JSON'dan oku (bölüm 2).
2. Eski/çakışan instance'ı temizle: `taskkill /F /IM manager-bin.exe`.
3. Env set et: `RGSX_WEBUI_DIR`, `RGSX_MANAGER_SCRIPT`, `RGSX_TORRENT_ENGINE=python`,
   `RGSX_NO_AUTOSTART=1`, `RGSX_PYTHON_MANAGER_URL` (bölüm 1 tablosu).
4. Gizli başlat: `Start-Process -FilePath '!RUST_MANAGER_BIN!' -WindowStyle Hidden`.
5. Binary yoksa blok sessizce atlanır (`if exist`) → python-only akış bozulmaz.

**Blok sıralaması:** display/windowed seçiminden **sonra**, `:: Log environnement`
bloğundan **önce** durur (Python değişkenleri tanımlanmadan, ancak arg'lar işlendikten sonra).

---

## 7. Hata: Windows'ta Rust testi başarısız — `test_download_with_bridge_forwards_to_engine`

**Belirti:** `cargo test` → `assertion left == right failed` satır 545:
`dest` beklendiği gibi `/tmp/fake_downloads/rom.zip` değil.

**Kök neden:** Test, Unix yolu `/tmp/fake_downloads/rom.zip` ile **sabit string** kıyası
yapıyordu. `PathBuf::join` Windows'ta **ters bölü** (`\`) üretir → Windows'ta her zaman
kırılırdı (Linux'ta geçer, platforma bağımlı hata).

**Çözüm (`tests/contract.rs:545`):** Kıyası OS-bağımsız yap:

```rust
let expected = std::path::Path::new("/tmp/fake_downloads").join("rom.zip");
assert_eq!(dest, &expected.display().to_string());
```

**Kural:** Dosya yolu kıyaslayan testler asla sabit `/` veya `\` string kullanmasın;
`Path::join` üretip karşılaştır.

---

## 8. Hata: Binary kopyalanamıyor (dosya kilitli)

**Belirti:** `Copy-Item manager-bin.exe` → "dosya başka bir işlem tarafından kullanılıyor".

**Kök neden:** Çalışan `manager-bin.exe` process'i dosyayı kilitliyor.

**Çözüm:** Kopyalamadan önce durdur:

```powershell
taskkill /F /IM manager-bin.exe
Start-Sleep 1
Copy-Item "$target\debug\manager-bin.exe" "<deploy>\manager-bin.exe" -Force
```

---

## 9. Hata: Python manager yokken Rust indirme yapamıyor

**Belirti:** `/api/download` → "Index de jeu invalide" veya proxy sessizce placeholder'a
düşüyor; Python 5001 health vermiyor.

**Kök neden:** Tasarım gereği Rust proxy'dir; Python yoksa katalog da yok → `catalog=None`
→ indirme yerel placeholder'a düşer (WebUI'nin `game_index` isteğini çözemez).

**Çözüm:** Python manager'ı başlat ve health doğrula, sonra Rust'ı başlat:

```powershell
Start-Process pythonw -ArgumentList 'rgsx_manager.py','--minimized' -WorkingDirectory $rgsx
# bekle → http://127.0.0.1:5001/api/health 200 + manager:true
# sonra manager-bin.exe
```

`.bat` akışı Python manager'ı kendisi ayağa kaldırdığı için bu sıralama launcher'da
otomatik sağlanır.

---

## 10. Doğrulama kontrol listesi (her değişiklikte)

1. Python manager başlat: port 5001 health 200 (`manager:true`).
2. Rust başlat (`.bat` env değerleriyle): port 5010 health 200.
3. `GET /api/platforms` → `count > 0` (~148).
4. `GET /api/translations` → `success:true` (~600 anahtar).
5. `GET /` → tam HTML (placeholder DEĞİL).
6. `POST /api/download` `{platform, game_index:0}` → `queued:true` (Python ile aynı).
7. `GET /api/queue` → proxy'den geliyor; `GET /api/history` → geçmiş dolu.
8. `GET /api/health` 5010 → `manager_state:"RUNNING"`.
9. Registry: `RGSXManager` hâlâ **Python** komutu (Rust `RGSX_NO_AUTOSTART=1` ile yazmamalı).
10. `cargo test` → 0 failed (Windows'ta bile).

---

## 11. Hata: librqbit indirme yolu canlıda hiç çalışmıyordu (proxy bypass)

**Belirti:** `RGSX_TORRENT_ENGINE=librqbit` (varsayılan) olsa bile canlı RetroBat
kurulumunda `POST /api/download` gerçekten librqbit'e uğramıyor; indirme hep
Python'a düşüyordu.

**Kök neden:** `manager-http/src/api.rs` `download()` handler'ı en üstte
`state.catalog.is_some()` kontrolüyle **tüm** isteği Python'a proxy edip erken
dönüyordu. Canlıda `RGSX_PYTHON_MANAGER_URL` set olduğundan `catalog` daima
`Some` → librqbit `download_torrent` **asla çağrılmıyordu**. Engine kendi başına
sağlamdı (`examples/live_torrent.rs` ile kanıtlı) ama manager HTTP akışında
bypass edilmişti. Katalog çözümü (game_index→url) ile indirme motoru seçimi
(RGSX_TORRENT_ENGINE) kavramları birbirine bağlanmıştı — oysa ortogonal.

**Çözüm (TASK-002l):** İstek **doğrudan çözülmüş torrent URL'i** taşıyorsa ve bir
bridge (librqbit varsayılan) mevcutsa proxy **atlanır**, indirme engine'e
yönlendirilir — `catalog` var olsa bile. Torrent şeması: `magnet:`,
`rgsx+torrent:`, `.torrent`. Torrent OLMAYAN düz http URL'ler ve çözülmemiş
`game_index`/`game_name` istekleri eskisi gibi Python'a proxy edilir.
Doğrulama: `manager-http/tests/contract.rs` (offline, catalog+bridge senaryo) +
`manager-http/tests/live_download.rs` (`#[ignore]`, gerçek Sintel torrent).

---

## Karar tarihçesi

| Tarih | Karar | Gerekçe |
|---|---|---|
| 2026-08-13 | `.bat` launcher'a Rust sidecar bloğu ekle | Deployed kopyada `rust_daemon.py` yok; Python-supervised yol kopyada çalışmaz |
| 2026-08-13 | `RGSX_TORRENT_ENGINE=python` (bridge) | Tek indirme altyapısı korunur; qBittorrent WebUI/şifre migration bozulmaz |
| 2026-08-13 | İndirme state GET'leri proxy (`progress`/`history`/`queue`) | İndirme Python'da yaşıyor; Rust state'i kopyası boş kaldığı için boş paneller görünüyordu |
| 2026-08-13 | `RGSX_NO_AUTOSTART=1` | Rust sidecar Python ile aynı registry anahtarını kullanıyor; boot autostart Python'da kalmalı |

> Güncel kaynaklar: `FAZ10C3_CONTRACT_MAP.md` (route durum tablosu) ·
> `PROJECT_MAP.md` (Rust workspace) · bu dosya (hata/çözüm günlüğü).
