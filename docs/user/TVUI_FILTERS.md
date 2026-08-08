# TVUI Filtre Kullanım Kılavuzu

## Genel Bakış

TVUI (Pygame arayüzü) oyun listelerinde gelişmiş filtreleme destekler. Filtreler **Gelişmiş Filtre** menüsünden erişilebilir.

## Filtre Menüsüne Erişim

1. Oyun listesinde **Filtre** butonuna basın (veya ilgili tuş kombinasyonu)
2. **Gelişmiş Filtreleme** seçeneğini seçin
3. Filtre seçenekleri ızgarası açılır

## Filtre Seçenekleri

### 1. Bölge Filtreleri (9 Bölge)

| Bölge | Açıklama |
|-------|----------|
| **USA** | ABD yayınları |
| **Canada** | Kanada yayınları |
| **Europe** | Avrupa yayınları |
| **France** | Fransa yayınları |
| **Germany** | Almanya yayınları |
| **Japan** | Japonya yayınları |
| **Korea** | Kore yayınları |
| **World** | Dünya geneli yayınları |
| **Other** | Diğer bölgeler |

**Durumlar:**
- **Include** `[V]` (Yeşil) - Bu bölgedeki oyunlar gösterilir
- **Exclude** `[X]` (Kırmızı) - Bu bölgedeki oyunlar gizlenir
- Varsayılan: Tümü **Include**

**Navigasyon:** Ok tuşlarıyla 3×3 ızgarada gezinme, **Confirm** ile Include↔Exclude toggle.

### 2. Diğer Seçenekler

| Seçenek | Açıklama | Varsayılan |
|---------|----------|------------|
| **Demoları/Betaları/Prototipleri Gizle** | `[Demo]`, `[Beta]`, `[Proto]` etiketli oyunları gizler | Kapalı |
| **Oyun başına bir ROM** | Aynı oyunun birden fazla ROM'u varsa en yüksek öncelikli bölgesini gösterir | Kapalı |
| **🆕 Yüklü ROM'ları Gizle** | **HDD'de zaten yüklü (indirilmiş) oyunları listeden gizler** | Kapalı |
| **Öncelik Sırası** | "Oyun başına bir ROM" için bölge öncelik sırasını yapılandırır | USA → Canada → World → Europe → Japan → Other |

### 🆕 **Yüklü ROM'ları Gizle** (Yeni!)

| Özellik | Detay |
|---------|-------|
| **Amaç** | HDD'de zaten indirilmiş/kurulu oyunları listeden gizler |
| **Kaynak** | `config.downloaded_games` (HDD tarama + indirme geçmişi) |
| **Etki** | Tik açık: İndirilmiş oyunlar listede **görünmez**<br>Tik kapalı: İndirilmiş oyunlar **yeşil `[>]` ile gösterilir** |
| **Yeşil işaretleme** | Filtre kapalıyken indirilmiş oyunlar yeşil `[>]` prefix ile gösterilir (korunur) |
| **Kapsam** | Mevcut platform (config.current_platform) |

**Kullanım Senaryosu:** "Hangi oyunları henüz indirmemişim?" diye filtrelemek için idealdir.

## Filtre Uygulama

| Buton | Aksiyon |
|-------|---------|
| **Uygula** | Filtreleri kaydet ve oyun listesine uygula |
| **Sıfırla** | Tüm filtreleri varsayılana sıfırla (bölgeler Include, diğerleri kapalı) |
| **Geri** | Değişiklikleri kaydetmeden çık |

## Kalıcılık

- Filtreler `rgsx_settings.json`'a kaydedilir (`game_filters` anahtarı)
- Uygulama yeniden başlatıldığında son ayarlar yüklenir
- **Uygula** butonu basılmadıkça değişiklikler kalıcı olmaz

## Klavye/Kontrolcü Kısayolları

| Eylem | Klavye | Kontrolcü |
|-------|--------|-----------|
| Navigasyon | Ok tuşları | D-Pad / Left Stick |
| Seç/Toggle | Enter / Space | A / Cross |
| Geri/İptal | Escape / Backspace | B / Circle |
| Uygula | F5 / Ctrl+S | Start / Options |

## İpuçları

1. **"Yüklü ROM'ları Gizle" + "Oyun başına bir ROM"** kombinasyonu: Sadece henüz indirilmemiş, benzersiz oyunları gösterir
2. Bölge filtresinde **Exclude** kullanarak istemediğiniz bölgeleri hızla elleyin
3. "Demoları Gizle" + "Yüklü Gizle" = Sadece indirilecek tam sürüm oyunlar listelenir

---

*Son güncelleme: 2026-08-07 (v2.6.5.6+ "Yüklü ROM'ları Gizle" eklendi)*