# Faz 12 — Python Parity'den Kontrollü Ayrılma Stratejisi

> Bağlam: `ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` tüm native geçişi "strangler/proxy + flag-gated
> cutover" ile yürütüyor. Bu belge, geçiş sırasında **ne zaman** Python davranışından
> bilerek ayrılacağımızı ve ayrılmanın **nasıl** güvenli/biçimli yapılacağını tanımlar.
> Amaç: "her şeyi birebir kopyala" tuzağından çıkmak, ama stabiliteyi ve test
> edilebilirliği (102 contract + SSE baseline) kaybetmemek.

## 0. Temel Prensip

**Davranış parity'si zorunlu, yapı/kod parity'si serbest.**

- Kullanıcının gördüğü her şey Python ile **aynı** kalır: indirme sonucu, hata mesajı
  metni, pause/resume davranışı, SSE event isimleri + payload'u, RetroBat/Batocera klasör
  uyumu.
- İç implementasyon (queue yapısı, state machine, retry/backoff, error sınıflandırma)
  Rust idiom'larına göre serbesttir.

> Zihinsel kural: **"Python referans, emir değil."** "Birebir çevir" deme; "Rust'ta daha
> doğru/sağlam yol nedir, davranış aynı kalsın?" diye sor.

---

## 1. Ayrılma Kriterleri

Şu 3 sorudan **en az 2'si evet** ise Python'dan ayrıl:

| # | Soru | Evet ise |
|---|------|----------|
| Q1 | Python'daki çözüm tarihsel / kötü tasarım mı? | Ayrıl |
| Q2 | Rust'ta daha temiz, güvenli veya performanslı yol var mı? | Ayrıl |
| Q3 | Ayrılmak testleri veya kullanıcı deneyimini bozuyor mu? | **Ayrılma (geri dön)** |

Q3'ün evet olması, ayrılmanın *geri alınması* gerektiği anlamına gelir (regresyon).

### Örnek uygulama (mevcut duruma göre)

| Alan | Karar |
|------|-------|
| Queue + state machine (command channel) | **Ayrıl** — zaten yapıldı, doğru |
| SSE throttle / dirty flag | **Ayrıl** — zaten yapıldı |
| Hata mesajı metinleri | **Parity koru** — kullanıcı görüyor |
| Dosya yolları / klasör yapısı | **Parity koru** — RetroBat/Batocera uyumu |
| SSE event isimleri + payload | **Parity koru** — frontend sözleşmesi |
| Ağ durumu + retry/backoff | **Ayrıl** — Python'daki basit kontrol yetersiz |
| Error sınıflandırma (transient/permanent/user) | **Ayrıl** — Python'da zayıf |

---

## 2. Pratik Ayrılma Süreci

### Adım A — Gap'i ikiye böl
- `gap-XX-behavior`: Kullanıcıya görünen davranış (parity zorunlu)
- `gap-XX-impl`: İç implementasyon (serbest)

### Adım B — Davranış sözleşmesi yaz
Her kritik özellik için kısa, Python'dan bağımsız bir "contract" tut. Örnek:

```text
Pause  → tüm aktif HTTP + torrent durur
Resume → kaldığı yerden devam eder
Network drop → soft fail + retry (max 3, exponential backoff)
Hata   → aynı kullanıcıya görünen mesaj metni (parity)
```

Bu sözleşme `docs/roadmap/FAZ10C3_CONTRACT_MAP.md` ve 102 contract testiyle çapraz doğrulanır.

### Adım C — Testi davranışa bağla
Parity testi = "aynı input → aynı output / aynı state geçişi", **"aynı kod" değil**.
Yeni native dal (`RGSX_NATIVE_*=1`) ve Python proxy dalı aynı contract test suite'inden
geçmeli.

### Adım D — Ayrıldığın yeri belgele (divergence-note)
Her bilinçli ayrılma için `docs/roadmap/` altına veya ilgili görev dosyasına not:

```text
# divergence-note
Python: <modül>:<satır> X yapıyordu
Rust:   Y yapıyor çünkü Z daha doğru/güvenli
Davranış: kullanıcı açısından aynı (veya fark: <gerekçe>)
```

---

## 3. Öncelikli Ayrılma Adayları

1. **Ağ durumu + retry/backoff** → kesinlikle ayrıl (Python basit kontrol yetersiz).
2. **Error sınıflandırma** → transient / permanent / user-action-required ayrımı.
3. **Queue iç yapısı** → zaten ayrıldı (command channel + state machine), devam.
4. **SSE event modeli** → event isim/payload sabit, üretim serbest.
5. **Config / settings** → dosya formatı + anahtar isimleri parity, okuma/yazma serbest.

---

## 4. Güvenli Geçiş Taktiği (feature-flag)

- Önce `RGSX_PARITY_MODE=1` (veya mevcut `RGSX_NATIVE_*=0`) ile eski davranış korunur.
- Yeni implementasyon yazılır; aynı test suite'i iki modda da çalıştırılır.
- Davranış aynıysa flag kaldırılır ve divergence-note eklenir.
- Böylece "bir anda her şey bozuldu" riski azalır.

Mevcut flag'ler (`rgsx-faz12-migration` skill): `RGSX_NATIVE_CATALOG`,
`RGSX_NATIVE_DOWNLOAD`, `RGSX_TVUI`. Yeni ayrılma alanları için aynı desen uygulanır.

---

## 5. Entegrasyon Noktaları

- Contract test baseline: **102 yeşil** (flag kapalıyken davranış değişmez) —
  `rgsx-faz12-migration` SKILL.md.
- Bilinen sapmalar zaten `ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` §3 + skill "Bilinen sapmalar"
  altında; her yeni bilinçli ayrılma o listeye divergence-note ile işlenir.
- Rust gap'leri (Faz 13): `tasks/gap/TASK-002-gap-*.md` — parity stratejisi bu görevlere
  de uygulanır (özellikle Gap 4 HTTP-direct, Gap retry engine).
