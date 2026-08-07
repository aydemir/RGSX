# WebUI Filtre Kullanım Kılavuzu

## Genel Bakış

WebUI (Flask tabanlı arayüz) oyun listelerinde gelişmiş filtreleme destekler. Filtreler oyun listesi sayfasının üstündeki **Filtre** panelinde bulunur.

## Filtre Paneline Erişim

1. WebUI'da bir platform seçin (`http://localhost:5000` → Platform seç)
2. Oyun listesi yüklendiğinde sayfa üstündeki **Filtre** bölümünü görün
3. Filtreler anlık uygulanır (auto-apply)

## Filtre Seçenekleri

### 1. Bölge Filtreleri (9 Bölge)

Her bölge için 3 durum seçilebilir:

| Durum | Renk | Açıklama |
|-------|------|----------|
| **Include** | Yeşil | Bu bölgedeki oyunlar gösterilir |
| **Exclude** | Kırmızı | Bu bölgedeki oyunlar gizlenir |
| **None** | Gri | Bu bölge filtresinde yok (varsayılan) |

**Bölgeler:** USA, Canada, Europe, France, Germany, Japan, Korea, World, Other

### 2. Diğer Seçenekler

| Seçenek | Açıklama | Varsayılan |
|---------|----------|------------|
| **Hide Demos/Betas/Protos** | `[Demo]`, `[Beta]`, `[Proto]` etiketli oyunları gizler | Kapalı |
| **🆕 Hide Downloaded ROMs** | **HDD'de zaten yüklü (indirilmiş) oyunları listeden gizler** | Kapalı |
| **One ROM Per Game** | Aynı oyunun birden fazla ROM'u varsa en yüksek öncelikli bölgesini gösterir | Kapalı |
| **Regex Mode** | Arama kutusunda regex pattern kullanımını aktifleştirir | Kapalı |
| **Priority Order** | "One ROM Per Game" için bölge öncelik sırasını yapılandırır (⚙️ butonu) | USA → Canada → World → Europe → Japan → Other |

### 🆕 **Hide Downloaded ROMs / Yüklü ROM'ları Gizle** (Yeni!)

| Özellik | Detay |
|---------|-------|
| **Amaç** | HDD'de zaten indirilmiş/kurulu oyunları listeden gizler |
| **Kaynak** | API response `downloaded: true` (server-side `is_game_downloaded()`) |
| **Etki** | Checkbox açık: İndirilmiş oyunlar listede **görünmez**<br>Checkbox kapalı: İndirilmiş oyunlar **yeşil `[✓]` ile gösterilir** |
| **Yeşil işaretleme** | Filtre kapalıyken indirilmiş oyunlar yeşil `[✓]` badge ile gösterilir (korunur) |
| **Kapsam** | Mevcut platform (platform-specific API response) |

**Kullanım Senaryosu:** "Hangi oyunları henüz indirmemişim?" diye filtrelemek için idealdir.

## Filtre Uygulama ve Kalıcılık

| Özellik | Davranış |
|---------|----------|
| **Anlık Uygulama** | Checkbox değiştiğinde anlık filtrelenir (auto-apply) |
| **Kaydetme** | "Kaydet" butonu veya checkbox değişiminde backend'e POST `/api/save_filters` |
| **Kalıcılık** | `rgsx_settings.json`'a kaydedilir (`game_filters` anahtarı) |
| **Yükleme** | Sayfa yüklendiğinde `GET /api/settings` → `loadSavedFilters()` |

## Arama ve Sıralama

| Özellik | Açıklama |
|---------|----------|
| **Arama Kutusu** | Oyun adına göre filtreleme (regex mode ile regex pattern) |
| **Sıralama** | A-Z, Z-A, Boyut Küçük-Büyük, Boyut Büyük-Küçük |
| **Regex Mode** | Açıkken arama kutusu regex pattern kabul eder |

## Klavye Kısayolları

| Eylem | Kısayol |
|-------|---------|
| Arama fokus | `/` veya `Ctrl+F` |
| Filtre paneli toggle | `F` |
| Sıralama değiştir | `S` |
| Sayfa yenile | `R` / `F5` |

## API Endpoints (Filtreler)

| Endpoint | Metot | Açıklama |
|----------|-------|----------|
| `GET /api/settings` | GET | Tüm ayarları getir (game_filters dahil) |
| `POST /api/save_filters` | POST | Sadece filtreleri kaydet |
| `POST /api/settings` | POST | Tüm ayarları kaydet (game_filters dahil) |

**Payload Örneği (`POST /api/save_filters`):**
```json
{
  "region_filters": {
    "USA": "include",
    "Japan": "exclude",
    "Europe": "none"
  },
  "hide_non_release": true,
  "one_rom_per_game": false,
  "hide_downloaded": true,
  "regex_mode": false,
  "region_priority": ["USA", "Canada", "World", "Europe", "Japan", "Other"]
}
```

## Çeviriler (7 Dil)

| Dil Kodu | Dil | `web_filter_hide_downloaded` |
|----------|-----|------------------------------|
| `tr` | Türkçe | **Yüklü ROM'ları Gizle** |
| `en` | English | **Hide Downloaded ROMs** |
| `fr` | Français | **Masquer les ROMs téléchargées** |
| `de` | Deutsch | **Heruntergeladene ROMs ausblenden** |
| `es` | Español | **Ocultar ROMs descargadas** |
| `it` | Italiano | **Nascondi ROMs scaricate** |
| `pt` | Português | **Ocultar ROMs baixadas** |

## İpuçları

1. **"Hide Downloaded ROMs" + "One ROM Per Game"** kombinasyonu: Sadece henüz indirilmemiş, benzersiz oyunları gösterir
2. Bölge filtresinde **Exclude** kullanarak istemediğiniz bölgeleri hızla elleyin
3. "Hide Non-Release" + "Hide Downloaded" = Sadece indirilecek tam sürüm, henüz indirilmemiş oyunlar
4. **Regex Mode** açıldığında arama kutusunda `^Super.*Bros` gibi pattern'ler kullanılabilir

---

*Son güncelleme: 2026-08-07 (v2.6.5.6+ "Yüklü ROM'ları Gizle / Hide Downloaded ROMs" eklendi)*