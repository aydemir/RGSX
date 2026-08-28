## Memory & Decision Resolution Rules

1. Her mimari/tasarım kararı hafızaya şu formatta yazılır:
   `[YYYY-MM-DD HH:MM] DECISION: <konu> -> <karar> | REASON: <gerekçe> | SUPERSEDES: <önceki karar ID/özet ya da "none">`

2. Çelişen iki kayıt bulunduğunda: **timestamp'i en yakın olan geçerlidir.** Eski kaydı silme, "SUPERSEDED" olarak işaretle, tarihçe korunsun (rollback ve "neden değiştirdik" sorgusu için gerekli).

3. codegraph MCP'den kod-seviyeli sorgu (fonksiyon/sınıf/bağımlılık) geldiğinde bunu opencode-mem'deki kararla çapraz kontrol et — kod hâlâ eski kararı yansıtıyorsa bunu görev listesine "DRIFT: kod ile son karar uyuşmuyor" olarak düş.

4. Bir kararı sorgularken asla yalnız en eski/en detaylı kaydı döndürme; önce timestamp'e göre sırala, en son geçerli olanı başa al, çelişen eski kayıtları "(superseded, bkz. tarih)" notuyla altına ekle.

5. Belirsiz/timestamp'siz eski kayıt varsa (migration öncesi girilmiş), onu otomatik en düşük öncelikli kabul et, kullanıcıya "bu kayıt tarihsiz, hâlâ geçerli mi?" diye sor — sessizce üzerine yazma.

## Task Pickup Protocol

Kullanıcı "N numaralı göreve başla" dediğinde otomatik olarak:

1. `tasks/*/{N}-*.md` dosyasını bul (hangi klasörde olursa olsun)
2. todo ise in-progress'e taşı, status güncelle
3. opencode-mem'den ilgili geçmiş kararları çek
4. codegraph ile mevcut kod durumunu doğrula
5. Plan özeti sun, onay bekle, sonra implementasyona geç

## Task Sorgu Protokolü — jq zorunlu (otomatlar için)

Otomat `tasks/` sorgularında `grep -R tasks/` / `Grep pattern="status"` / `Glob tasks/**/*.md` ile tüm dosyaları taramak **varsayılan olarak YASAKTIR** — önce index denenir. Bu hem token israfı hem de yavaştır.

**Zorunlu akış:**

1. `read tasks/index.json` — yoksa `bash python3 scripts/build-tasks-index.py` ile üret (front-matter + legacy bullet her ikisini de parse eder).
2. `bash jq '...' tasks/index.json` ile filtrele — tek dosya, tek tool call (`jq` yoksa `python3 -c "import json; ..."` eşdeğeri kullanılabilir; CI `ubuntu-22.04` imajında `jq` preinstalled, `tasks-index.yml` bunu `which jq` ile doğrular; Windows'ta `scoop install jq` ile kurulu — şu an `jq-1.8.2` doğrulandı, kuruluysa `jq` tercih edilir).
3. Sadece eşleşen `id`'lerin dosyasını `read tasks/**/{ID}*.md` ile çek (gövde gerektiğinde).

**Fallback (yalnız bozuk/stale durumda):** `tasks/index.json` yok, JSON parse hatası veriyor veya `scripts/build-tasks-index.py` çökerse otomat kör kalmaz — tek seferlik `Grep`/`Glob` ile doğrudan `tasks/**/*.md` okuyabilir. Bu durumda hatayı logla ve imkân varsa index'i yeniden üretmeyi dene.

**jq örnekleri:**

```bash
jq '[.[] | select(.status=="in_progress")]' tasks/index.json
jq '[.[] | select(.priority=="high" or .priority=="P0")]' tasks/index.json
jq '[.[] | select(.labels | index("ci"))] | sort_by(.updated)' tasks/index.json
jq '[.[] | select(.depends_on | index("TASK-039"))]' tasks/index.json
jq -r '.[] | "\(.id) \(.status) \(.priority) \(.file)"' tasks/index.json
```

`tasks/index.json` front-matter'dan türetilmiş artefakt'tır, elle editlenmez. Stale ise `bash python3 scripts/build-tasks-index.py` ile yeniden üret. PR'da stale ise CI fail eder (bilinçli sürtünme — unutulan index'i yakalamak için); `push`'ta bot otomatik düzeltir. Legacy bullet format desteği geçiş dönemidir — tüm task'lar front-matter'a taşındıktan sonra script'ten kaldırılacak.

## Görev Dosyası Şablonu — environment zorunluluğu

Görev dosyası oluşturulurken environment alanı boş bırakılamaz.
Rust/Windows-rs/netsh/tray içeren işler -> windows.
Python/pytest/Termux/proot içeren işler -> linux.
Her ikisini de kapsayan entegrasyon işleri -> both.

## Proje Haritası

Statik harita: `docs/PROJECT_MAP.md` — hızlı navigasyon için ilk bakılacak yer.

Şu durumlarda haritayı güncelle (yeni bölüm ekleme, var olanı düzenle):
- Yeni bir crate/modül/görev klasörü oluşturulduğunda
- Bir dosya taşındığında veya yeniden adlandırıldığında
- Bir görev tamamlanıp mimari bir bağımlılık değiştiğinde
  (örn. TASK-002f gibi Python<->Rust köprüsü kurulduğunda)
- Haritadaki bir satır ile codegraph_explore sonucu çelişirse
  (harita eskimiş demektir, düzelt)

Güncellemeyi görevin normal akışı içinde yap (commit'e dahil et),
ayrı bir "harita güncelleme" görevi açmana gerek yok — küçük ve
sürekli tut.

Harita ile codegraph çelişirse HER ZAMAN codegraph'e güven, harita
sadece hız için bir özet, gerçek kaynak değil.

## Cross-Platform Standartları (Windows/Linux)

Rust workspace (`manager-rs/`) hem Linux (proot/CI) hem Windows (manager-windows, NSIS) hedeflerinde derlenmelidir. Aşağıdaki kurallar zorunludur:

1. **Platform izolasyonu:** Linux/POSIX özel kodlar (sinyaller, `/proc` okumaları, Unix socketler, `fork`, `nix` çağrıları) mutlaka `#[cfg(unix)]` veya `#[cfg(target_os = "linux")]` bloğuna alınmalıdır.
2. **Windows izolasyonu:** Windows özel kodlar `#[cfg(windows)]` altına yazılmalı, mümkünse platform bağımsız alternatif (`std::path`, `std::fs`) tercih edilmelidir.
3. **Yol birleştirme:** `Path` birleştirmelerinde asla elle `/` veya `\` string formatlaması yapılmamalı; `std::path::PathBuf` ve `.join()` kullanılmalıdır. Hardcoded POSIX yolları (`/var`, `/tmp`, `/proc`) Windows derlemesini kırar.
4. **Cargo.toml ayrımı:** Platform bağımlı crate'ler (`nix`, `signal-hook`, `procfs` vb.) `[target.'cfg(unix)'.dependencies]` ve `[target.'cfg(windows)'.dependencies]` altında ayrıştırılmalıdır; workspace ortak `[dependencies]`'e konmamalıdır.
5. **Çapraz derleme kontrolü:** Windows'a özgü bir değişiklik yapıldığında en azından `cargo check --target x86_64-pc-windows-gnu` (veya `cargo build` cross) ile doğrulanmalı; yalnız Linux testi geçti demek yeterli değildir.

## Yarım İş Bırakma Yasağı

Kod yazılırken "sonraki faza bırakıldı", `TODO`, ya da canlı akıştan kopuk bırakma yapılmayacaktır. Tüm modüller çağıran katmanlara (caller chain) bizzat bağlanacaktır; yarım imza, stub fonksiyon veya bağlanmamış public API kabul edilmez. Bir modülün gerçek bir tüketicisi yoksa o modül görev tamamlanmış sayılmaz.

## Analitik Problem Çözme & Yanıt Metodolojisi

Her görevde aşağıdaki disiplin uygulanır:

1. **Doğruluk > Onay.** Kullanıcıyı memnun etmek için abartılı/yanlış ifade kullanma; belirsizlik varsa belirsizliği açıkça söyle.
2. **Niyeti oku.** Talebi gerçek amaç açısından yorumla; kelimesiyle mi yoksa daha geniş amaç için mi soruluyor?
3. **Belirsizlik yönetimi.** Makul varsayımla ilerlenebiliyorsa varsayımı belirt ve devam et. Yanlış yön ciddi kayba yol açacaksa TEK bir netleştirici soru sor (çoklu soru yok).
4. **Ayrıştırma.** Karmaşık işi girdi/çıktı, kısıt, başarı kriterine böl; bağımlılık sırasına diz. Büyük çıktıda önce iskelet, sonra derinleştir.
5. **Doğrula, tahmin etme.** Güncel/doğrulanabilir her konu için (versiyon, API davranışı, kod durumu) codegraph/araçlarla teyit et. Çelişkiyi kullanıcıya bildir, kendi başına "çözme".
6. **Adım adım akıl yürüt.** Çok adımlı işte ara adımları kontrol et; pattern-matching otomatizmine düşme, gizli karmaşıklık ara. Kendi hipotezini çürütmeye çalış (strongest-evidence-against).
7. **Epistemik dürüstlik.** Aşırı onaylayıcılıktan kaçın; hata fark edince savunmaya geçmeden kabul et/düzelt. "Emin değilim" ile "bilmiyorum"u ayır, belirsiz kısmı spesifik belirt.
8. **Yanıt yapısı.** Cevapla başla, girizgâh yok. Format içeriğe göre: basit soru → 1-2 cümle, nasıl-yapılır → maddeler, kapsamlı → kısa paragraflar. Uzunluk talep edilen derinlikle orantılı.
9. **Araç stratejisi.** Bilgiyi kullanıcıdan istemeden araçlarla (codegraph, bash, read) topla. Geri dönüşü olmayan eylemlerden (silme/gönderme/düzenleme) önce onay al.
10. **Sınır farkındalığı.** Bilgi güncelliği sınırlıysa açıkça belirt; uzmanlık gerektiren (hukuk/finans/tıp) konularda kesin tavsiye yerine çerçeve sun, kararı kullanıcıya bırak.
