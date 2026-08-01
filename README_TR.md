# 🎮 Retro Game Sets Xtra (RGSX) - Türkçe

**[Discord Desteği](https://discord.gg/Vph9jwg3VV)** • **[Kurulum](#-kurulum)** • **[İngilizce Dokümantasyon](https://github.com/RetroGameSets/RGSX)** • **[Sorun Giderme](#-sorun-giderme)** •

Batocera, Knulli ve RetroBat için ücretsiz, kullanımı kolay ROM indiricisi. Çoklu kaynak desteği.

<p align="center">
  <img width="69%" alt="main" src="https://github.com/user-attachments/assets/a98f1189-9a50-4cc3-b588-3f85245640d8" />
  <img width="30%" alt="controls help" src="https://github.com/user-attachments/assets/38cac7e6-14f2-4e83-91da-0679669822ee" />
</p>

---

## 🚀 Kurulum

### Hızlı Kurulum (Batocera / Knulli)

**SSH veya Terminal erişimi gerekli:**
```bash
curl -L bit.ly/rgsx-install | sh
```

Kurulumdan sonra:
1. Oyun listelerini güncelleyin: `Menü > Oyun Ayarları > Oyun listesini güncelle`
2. RGSX'i **PORTS** veya **Homebrew and ports** altında bulun

### Manuel Kurulum (Tüm Sistemler)
1. **İndirin**: [RGSX_full_latest.zip](https://github.com/RetroGameSets/RGSX/releases/latest/download/RGSX_full_latest.zip)
2. **Çıkarın**:
   - **Batocera/Knulli**: `ports` klasörünü `/roms/` altına çıkarın
   - **RetroBat**: Hem `ports` hem de `windows` klasörlerini `/roms/` altına çıkarın
3. **Yenileyin**: `Menü > Oyun Ayarları > Oyun listesini güncelle`

### RetroBat Kurulumu
1. RGSX'i indirin ve çıkarın
2. `ports` ve `windows` klasörlerini `C:\RetroBat\roms\` altına kopyalayın
3. RetroBat'ı başlatın
4. RGSX'i **PORTS** bölümünde bulun

**Kurulan yollar:**
- `/roms/ports/RGSX` (tüm sistemler)
- `/roms/windows/RGSX` (sadece RetroBat)

---

## 🎮 Kullanım

### İlk Çalıştırma

- Sistem görsellerini ve oyun listelerini otomatik indirir
- Kontrolcünüz tanınıyorsa kontrolleri otomatik yapılandırır
- **Kontroller çalışmıyor mu?** `/saves/ports/rgsx/controls.json` dosyasını silin ve yeniden başlatın

**Klavye Modu**: Kontrolcü algılanmadığında kontroller `[Tuş]` olarak gösterilir.

### Duraklatma Menü Yapısı

**Ana kategoriler**
- Oyunlar (indirmeler, taramalar, platform görünürlüğü)
- Dil (arayüz dilini değiştirme)
- Kontroller (yardım ve yeniden eşleme)
- Ekran (yerleşim, fontlar, monitör/mod, görsel seçenekler)
- Ayarlar (müzik, symlink, otomatik çıkarma, ağ ve API durumu)
- Destek (destek ZIP/günlük paketi oluşturma)
- Çıkış (çıkış veya yeniden başlatma)

### Oyun İndirme

1. Platformlara göz atın → Oyun seçin
2. **Doğrudan İndirme**: `Onayla` tuşuna basın
3. **Kuyruğa Ekleme**: `X` (Batı butonu) tuşuna basın
4. İlerlemeyi **Geçmiş** menüsünden veya açılır bildirimlerden takip edin

---

## ✨ Özellikler

- 🎯 **Akıllı Sistem Tespiti** – `es_systems.cfg` dosyasından desteklenen sistemleri otomatik keşfeder
- 📦 **Akıllı Arşiv Yönetimi** – Sistemler ZIP dosyasını desteklemiyorsa otomatik çıkarır
- 🔑 **Premium Kaynak Desteği** – 1Fichier API + AllDebrid/Debrid-Link/Real-Debrid/TorBox yedekleme
- 🎨 **Tam Özelleştirilebilir** – Yerleşim (3×3 - 4×4), fontlar, font boyutları, diller (EN/FR/DE/ES/IT/PT/JA/ZH/RU/**TR**)
- 🎮 **Kontrolcü Odaklı Tasarım** – Popüler kontrolcüler için otomatik eşleme + özel yeniden eşleme
- 🔍 **Gelişmiş Filtreleme** – İisme göre arama, desteklenmeyen sistemleri gizleme/gösterme, platform filtreleme
- 📊 **İndirme Yönetimi** – Kuyruk sistemi, geçmiş takibi, ilerleme bildirimleri
- ♿ **Erişilebilirlik** – UI ve alt bilgi için ayrı font ölçekleme, klavye modu desteği
- 🌐 **Web Arayüzü** – Batocera/Knulli için uzaktan indirme (port 5000)
- 🇹🇷 **Türkçe Dil Desteği** – Tam Türkçe arayüz ve çeviri
- 🟡 **Renkli Durum Göstergeleri** – Oyun listesinde indirme durumu renk kodları

### Renk Kodları (Oyun Listesi)

| Durum | Prefix | Renk | Açıklama |
|-------|--------|------|----------|
| İndirilmiş | `[>]` | 🟢 Yeşil | ROM dosyası mevcut |
| İndiriliyor | `[~] %sayı` | 🟡 Sarı | Aktif indirme |
| Başarısız | `[X]` | 🔴 Kırmızı | Son deneme başarısız |

> ### 🔑 API Anahtarı Yapıllandırma
> Sınırsız 1Fichier indirmeleri için API anahtarınızı `/saves/ports/rgsx/` dizinine ekleyin:
> - `1FichierAPI.txt` – 1Fichier API anahtarı (önerilen)
> - `AllDebridAPI.txt` – AllDebrid yedekleme (isteğe bağlı)
> - `DebridLinkAPI.txt` – Debrid-Link yedekleme (isteğe bağlı)
> - `RealDebridAPI.txt` – Real-Debrid yedekleme (isteğe bağlı)
> - `TorBoxAPI.txt` – TorBox yedekleme (isteğe bağlı)
>
> **Her dosya SADECE anahtarı içermeli, ekstra metin olmamalı.**

---

## 🌐 Web Arayüzü (Sadece Batocera/Knulli)

RGSX, ağınızdaki herhangi bir cihazdan uzaktan göz atma ve indirme için otomatik olarak başlatılan bir web arayüzü içerir.

### Web Arayüzüne Erişim

1. **Batocera IP adresinizi bulun**:
   - Batocera menüsünden: `Ayarlar > Ağ`
   - Veya terminalden: `ip addr show`

2. **Tarayıcıda açın**: `http://[BATOCERA_IP]:5000`
   - Örnek: `http://192.168.1.100:5000`

3. **Herhangi bir cihazdan erişilebilir**: Telefon, tablet, PC (aynı ağda)

### Web Arayüzü Özellikleri

- 📱 **Mobil Uyumlu** – Duyarlı tasarım tüm ekran boyutlarında çalışır
- 🔍 **Tüm Sistemlere Göz Atma** – Tüm platformları ve oyunları görüntüleme
- ⬇️ **Uzaktan İndirme** – Batocera'nıza doğrudan indirme kuyruğu
- 📊 **Gerçek Zamanlı Durum** – Aktif indirmeleri ve geçmişi görme

---

## 📁 Dosya Yapısı

```
/roms/
├── ports/
│   ├── RGSX/
│   │   ├── __main__.py                # Giriş noktası
│   │   ├── controls.py                # Giriş işleme
│   │   ├── display.py                 # Render motoru
│   │   ├── network.py                 # İndirme yöneticisi
│   │   ├── rgsx_settings.py           # Ayarlar yöneticisi
│   │   ├── languages/                 # Çeviriler (EN/FR/DE/ES/IT/PT/JA/ZH/RU/TR)
│   │   └── logs/RGSX.log              # Çalışma günlükleri
│   ├── gamelist.xml
│   ├── images/
│   └── videos/
└── windows/
    ├── RGSX Retrobat.bat              # Sadece Windows (RetroBat olmadan da kullanılabilir)
    ├── gamelist.xml
    ├── images/
    └── videos/

/saves/ports/rgsx/
├── rgsx_settings.json        # Kullanıcı tercihleri
├── controls.json             # Kontrol eşleme
├── history.json              # İndirme geçmişi
├── systems_list.json         # Algılanan sistemler
├── downloaded_games.json     # İndirilen oyunlar (yeşil işaret)
├── games/                    # Oyun veritabanları (platform başına)
├── images/                   # Platform görselleri
└── API anahtarı dosyaları
```

---

## 🛠️ Sorun Giderme

| Sorun | Çözüm |
|-------|-------|
| Kontroller çalışmıyor | `/saves/ports/rgsx/controls.json` silin + uygulamayı yeniden başlatın |
| Oyun yok mu? | Duraklatma Menüsü > Oyunlar > Oyun Önbelleğini Güncelle |
| Eksik sistemler? | Duraklatma Menüsü > Oyunlar > Desteklenmeyen Sistemleri Göster |
| Uygulama çöküyor | `/roms/ports/RGSX/logs/RGSX.log` dosyasını kontrol edin |
| Yerleşim değişikliği uygulanmadı | RGSX'i değiştirdikten sonra yeniden başlatın |
| Bazı oyunları indiremiyoruz? | Duraklatma Menüsü > Ayarlar > Bağlantı Durumu'nu kontrol edin |

**Yardıma mı ihtiyacınız var?** Günlükleri [Discord](https://discord.gg/Vph9jwg3VV) üzerinden paylaşın.

---

## 📝 Lisans

Ücretsiz ve açık kaynaklı yazılımdır. Özgürce kullanın, değiştirin ve dağıtın.

## Tüm katkıda bulunanlara ve uygulamayı takip edenlere teşekkürler

**Projemi desteklemek isterseniz bira ısmarlayabilirsiniz: https://bit.ly/donate-to-rgsx**

<a href="https://github.com/RetroGameSets/RGSX/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=RetroGameSets/RGSX" />
</a>

**Retro gaming topluluğu için ❤️ ile geliştirildi.**
