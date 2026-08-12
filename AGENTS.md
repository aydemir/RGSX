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
