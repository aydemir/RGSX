# Python → Rust İş Akışı Haritası (Rust Refaktörü Öncesi)

> **Amaç:** `ports/RGSX/network/queue.py`, `ports/RGSX/qbittorrent_backend.py` ve
> `ports/RGSX/rgsx_manager.py` üzerindeki *tam* indirme/iş akışını, her karar noktası
> ayrı bir düğüm olacak şekilde çıkarmak; ardından Rust'a (`manager-rs`) şu ana kadar
> taşınan fonksiyonlarla eşleyip **Rust karşılığı OLMAYAN** düğümleri (nadiren tetiklenen
> dallar dahil) tespit etmek.
>
> Refaktör "satır satır okuyarak" değil, "tam iş akışı karşısında doğrulanarak"
> ilerlesin diye hazırlandı. Her düğüm ID'si (`W1`, `T4`, `H9`, `F2`, `Q6`, ...)
> alttaki Rust eşleme tablosu ve `tasks/gap/*.md` dosyalarıyla çapraz bağıntılanır.
>
> Kaynak: `codegraph` + doğrudan dosya okuma (queue.py 1805 satır, qbittorrent_backend.py
> 1853 satır, rgsx_manager.py 1072 satır). Tarih: 2026-08-14.

---

## 1. Mermaid Flowchart

```mermaid
flowchart TD
    %% ===================== QUEUE WORKER =====================
    subgraph QW["download_queue_worker (queue.py:91)"]
        W0["Başlat (while True)"] --> W1{"active < max_dl<br/>AND download_queue?"}
        W1 -- "hayır" --> Ws["time.sleep(1)"] --> W0
        W1 -- "evet" --> W2{"is_1fichier_url(url)?"}
        W2 -- "evet" --> W3a["Thread: download_from_1fichier"]
        W2 -- "hayır" --> W3b["Thread: download_rom"]
        W3a --> Ws
        W3b --> Ws
        W0 -. "except" .-> We["log error; sleep(2)"] --> W0
    end

    %% ===================== DOWNLOAD_ROM =====================
    subgraph DR["download_rom (queue.py:629, async)"]
        D0["torrent_meta çöz:<br/>rgsx+torrent:// ise yeniden üret,<br/>değilse parse_torrent_download_url"] --> D1{"url urls_in_progress içinde?<br/>(duplicate)"}
        D1 -- "evet" --> D1w["wait url_done_events (≤1800s)<br/>→ cache/True döner"]
        D1 -- "hayır" --> D1m["url'i in_progress'a ekle"]
        D1w --> DEnd1([return])
        D1m --> D2{"history'de entry var mı?<br/>(url eşleşmesi)"}
        D2 -- "var" --> D2a["status=Downloading,<br/>entity_state=DOWNLOADING,<br/>progress=0, total_size güncelle"]
        D2 -- "yok" --> D2b["yeni history entry oluştur<br/>(status=Downloading)"]
        D2a --> D3
        D2b --> D3
        D3["dest_dir çöz:<br/>custom_path? → platform_folder?<br/>→ BIOS ise USERDATA_FOLDER"] --> D4{"makedirs + yazma<br/>yetkisi?"}
        D4 -- "yok" --> D4e["PermissionError<br/>→ D6ex"] 
        D3 --> D5{"disk alanı yeterli mi?<br/>(expected_size)"} 
        D5 -- "hayır" --> D5e["InsufficientDiskSpaceError<br/>→ D6ex"]
        D5 -- "evet" --> D6{"torrent_meta<br/>None değil mi?"}

        %% ---------- TORRENT PATH ----------
        D6 -- "evet (TORRENT)" --> T1{"_is_arm_device?"}
        T1 -- "evet" --> T1e["RuntimeError (ARM desteklenmez)<br/>→ D6ex"]
        T1 -- "hayır" --> T2{"dest_path var ve<br/>boyut eşleşiyor mu?"}
        T2 -- "evet" --> T2r["zaten var → history Download_OK,<br/>toast, return"] --> DEnd2([return True])
        T2 -- "hayır" --> T3{"_should_prefer_qbittorrent_backend?"}
        T3 -- "hayır" --> T3e["BackendUnavailableError<br/>→ D6ex"]
        T3 -- "evet" --> T4{"rust_daemon.torrent_delegate_enabled()<br/>AND healthy()?"}
        T4 -- "evet" --> T4r["rust_daemon.download_torrent()<br/>(delegasyon)"]
        T4r -- "başarı + dosya var" --> T4ok["chmod 644, cleanup, return"] --> DEnd3([return])
        T4r -. "hata/timeout" .-> T4f["log, qBittorrent'e FALLBACK"]
        T4 -- "hayır" --> T5
        T4f --> T5["qbittorrent_backend.download_torrent_via_qbittorrent()"]
        T5 --> T5ok["chmod 644 (başarıysa)"] --> D7

        %% ---------- HTTP PATH ----------
        D6 -- "hayır (HTTP)" --> H0{"'vimm.net' in url?"}
        H0 -- "evet" --> H0a["fetch vimm info + file size,<br/>gerçek dosya adıyla dest_path güncelle"]
        H0 -- "hayır" --> H1
        H0a --> H1{"dest_path zaten var mı?"}
        H1 -- "evet, boyut farklı" --> H1d["eksik dosyayı sil → indirme devam"]
        H1 -- "evet (lolroms, doğrulanamaz)" --> H1l["sil → indirme devam"]
        H1 -- "evet, boyut eşleşiyor" --> H1r["zaten var → history Download_OK,<br/>toast, return"] --> DEnd4([return True])
        H1 -- "yok" --> H2{"aynı taban, farklı uzantı<br/>dosya var mı?"}
        H2 -- "evet, boyut eşleşiyor" --> H2r["zaten var → return"] --> DEnd5([return True])
        H2 -- "evet, farklı" --> H2c["indirme devam"]
        H2 -- "yok" --> H3{"_is_lolroms_url?"}
        H3 -- "evet" --> H3a["_download_lolroms_with_external_tool"]
        H3a -- "başarı" --> H3ok["external_lolroms_downloaded=True"] --> D7
        H3a -- "başarısız" --> H3f["fallback: requests/stream"] --> H4
        H3 -- "hayır" --> H4["archive.org ise cookie/metadata/<br/>alt-URL hazırlığı"]
        H4 --> H5{"HTTP retry döngüsü<br/>(header variantları + 429 backoff +<br/>browser-challenge)"}
        H5 -- "browser challenge" --> H5b["raise (erişim engellendi)"] --> D6ex
        H5 -- "401/403" --> H5c["sonraki variant dene"]
        H5 -- "429" --> H5d["Retry-After / exp backoff, bekle"] --> H5
        H5 -- "timeout/conn" --> H5t["bekle, retry"] --> H5
        H5 -- "başarılı response" --> H6{"response None mı?<br/>(tüm denemeler başarısız)"}
        H6 -- "evet" --> H6a["archive alt-URL dene /<br/>is_dark kontrolü / hata mesajı"] --> D6ex
        H6 -- "hayır" --> H7{"content-type HTML mi?<br/>(vimm)"}
        H7 -- "evet" --> H7e["raise (HTML yerine arşiv)"] --> D6ex
        H7 -- "hayır" --> H8{"disk alanı yeterli mi?<br/>(announced_total_size)"}
        H8 -- "hayır" --> H8e["InsufficientDiskSpaceError<br/>→ D6ex"]
        H8 -- "evet" --> H9["_stream_response_to_path<br/>(Range resume desteği)"]
        H9 -- "downloaded <= 0" --> H9e["dosyayı sil, raise (boş)"] --> D6ex
        H9 -- "archive alt-URL ile kurtar" --> H9a["_try_archive_org_alternate_urls"] --> H10
        H9 -- "tamamlandı" --> H10{"arşiv imza kontrolleri<br/>(.7z/.zip/.rar)"}
        H10 -- "HTML/challenge" --> H10a["sil, raise"] --> D6ex
        H10 -- "imza yok" --> H10b["sil, raise"] --> D6ex
        H10 -- "kısmi kabul edilemez" --> H10c["sil, raise"] --> D6ex
        H10 -- "geçti" --> H11{"download_canceled?"}
        H11 -- "evet" --> H11c["notify, return"] --> DEnd6([return])
        H11 -- "hayır" --> H12{"force_extract?<br/>(BIOS / PS3 redump / zip)"}
        H12 -- "evet" --> H12e["status=Extracting,<br/>_postprocess_downloaded_file"]
        H12 -- "hayır" --> H12ok["result = (True, ok)"]
        H12e --> D7
        H12ok --> D7

        %% ---------- EXCEPTION MERGE ----------
        D4e --> D6ex["result = (False, error)"]
        D5e --> D6ex
        T1e --> D6ex
        T3e --> D6ex
        H5b --> D6ex
        H6a --> D6ex
        H7e --> D6ex
        H8e --> D6ex
        H9e --> D6ex
        H10a --> D6ex
        H10b --> D6ex
        H10c --> D6ex
        D6ex --> D7["result[0] ise progress=100%,<br/>Completed, sleep(1.5)"]
        D7 --> D8["progress_queues.put((task_id, success, msg))"]
        D8 --> DLoop["ANA LOOP: thread.is_alive() iken<br/>progress queue'dan oku"]
        DLoop --> DFin{"queue verisi bool mu?<br/>(final sonuç)"}
        DFin -- "evet" --> F0["_finalize_download_result()"]
        DFin -- "hayır" --> DProg["config.download_progress +<br/>history güncelle (5% pale)"]
        DProg --> DLoop
        DLoop -- "CancelledError" --> DEndc([break + cleanup])
    end

    %% ===================== FINALIZE / RETRY =====================
    subgraph FR["_finalize_download_result (queue.py:468)"]
        F0 --> F1{"success?"}
        F1 -- "evet" --> F1c["transition COMPLETED,<br/>history Download_OK,<br/>mark_game_as_downloaded,<br/>emit 'completed'"] --> FRc([return "completed"])
        F1 -- "hayır" --> F2{"classify_error == transient<br/>AND retry_count < max?"}
        F2 -- "evet" --> F2r["transition → TRANSIENT_FAILURE<br/>→ RETRY_TRIGGERED → RETRY_SCHEDULED,<br/>_schedule_download_retry(delay)"] --> FRr([return "retry_scheduled"])
        F2 -- "hayır (kalıcı / hak bitti)" --> F2f["transition PERMANENT_FAILURE,<br/>history 'Erreur',<br/>emit 'failed_permanent'"] --> FRf([return "failed"])
    end

    subgraph RR["_schedule_download_retry (queue.py:572)"]
        R0["runner thread: deadline'a kadar bekle<br/>(_app_shutting_down / cancel kontrol)"] --> R1{"slot var mı?<br/>(active < max_dl)"}
        R1 -- "hayır" --> R1w["sleep(1)"] --> R1
        R1 -- "evet" --> R2{"_retry_in_flight dedup<br/>(url zaten retry'de?)"}
        R2 -- "evet" --> R2skip([atla])
        R2 -- "hayır" --> R3["active_download_count++,<br/>yeni task_id, download_rom()"] --> REnd([finally: url'i discard et])
    end

    %% ===================== PAUSE / RESUME / CANCEL =====================
    subgraph PRC["Pause / Resume / Cancel (queue.py)"]
        P0{"toggle_pause_download(task_id)<br/>event var mı?"}
        P0 -- "set (pause)" --> P0p["return True (paused)"]
        P0 -- "clear (resume)" --> P0r["return False (resumed)"]
        P1["pause_all_downloads():<br/>thread+history(Downloading/…) → pause_events.set,<br/>bulk history='Paused'"]
        P2["resume_all_downloads():<br/>thread+history('Paused') → pause_events.clear,<br/>bulk history='Downloading'"]
        P3["request_cancel(task_id):<br/>cancel_events[task_id].set()"]
        P4["cancel_all_downloads():<br/>tüm cancel_events.set, thread join,<br/>queue temizle, history 'Queued'→'Canceled'"]
        P5["cleanup_torrent_temp / _cleanup_torrent_resume_artifacts<br/>/ _cleanup_seeder_local_artifacts / stop_active_seeder"]
        P6["shutdown_downloads():<br/>_app_shutting_down=True, queue temizle,<br/>qbittorrent_backend.shutdown()"]
        P7["qBittorrent indirme döngüsünde pause_ev set ise:<br/>API pause → raporla → pause_ev clear olana dek bekle<br/>→ API resume (T qBittorrent path)"]
    end

    %% ===================== QBITTORRENT BACKEND =====================
    subgraph QB["qbittorrent_backend.py"]
        Q1["_ensure_qbittorrent_running()<br/>state: STOPPED→STARTING→PORT_RESOLVING→<br/>WEBUI_AUTH_WAIT→RUNNING ⇄ UNRESPONSIVE→RESTARTING"]
        Q2{"binary bulundu mu?<br/>(bundled/installed/registry)"}
        Q2 -- "hayır" --> Q2s["state=STOPPED, return None"]
        Q2 -- "evet" --> Q3{"port serbest mi?<br/>(port aralığı)?"}
        Q3 -- "hayır (tükendi)" --> Q3s["state=STOPPED, return None"]
        Q3 -- "evet" --> Q4["preseed profile (win/linux),<br/>Popen, pencere gizle (win)"]
        Q4 --> Q5{"wait_for_webui + login<br/>+ setPreferences +<br/>password migration?"}
        Q5 -- "başarısız" --> Q5f["return None (UNRESPONSIVE/RESTARTING)"]
        Q5 -- "başarı" --> Q6["download_torrent_via_qbittorrent():<br/>torrent ekle, file-selection,<br/>indirme döngüsü"]
        Q6 --> Q6a{"cancel_ev set?"}
        Q6a -- "evet" --> Q6c["torrent+temp sil (başka ref yoksa), raise Canceled"]
        Q6a -- "hayır" --> Q6b{"pause_ev set?"}
        Q6b -- "evet" --> Q6p["API pause, raporla, bekle, API resume"] --> Q6
        Q6b -- "hayır" --> Q6c2{"state hata /<br/>tamamlandı?"}
        Q6c2 -- "error state" --> Q6e["raise RuntimeError"]
        Q6c2 -- "file_completed / done" --> Q6ok["dosyayı resolve et,<br/>link/copy → dest_path,<br/>_promote_active_download_to_seed"]
        Q6ok --> Q7["_seed_status_worker başlat:<br/>seeding peers/ul_speed'ı history'ye yazar"]
        Q7 --> Q8["has_active_seed / stop_seed:<br/>kullanıcı isteyince torrent+file sil"]
        Q9["_find_stray_torrent_temp_roots:<br/>eski platform klasörlerinde<br/>orphan .rgsx_torrent temizliği"]
        Q10["--bridge JSON-RPC sunucusu:<br/>ping/status/ensure_running/get_webui_url/<br/>get_password_status/change_webui_password/<br/>get_app_paths/shutdown (Rust manager-bin için)"]
    end

    %% ===================== RUST DAEMON =====================
    subgraph RD["rust_daemon.py + manager-rs (Faz 10)"]
        RD1["rust_daemon.start():<br/>manager-bin sidecar (RGSX_RUST_DAEMON)"]
        RD2["healthy() + supervisor()<br/>(RestartLimiter, 3/3600s)"]
        RD3["rust_daemon.download_torrent():<br/>HTTP /api/download → /api/progress poll<br/>→ cancel /api/cancel"]
        RD4["LibrqbitEngine.download_torrent_source():<br/>add → wait_until_completed →<br/>en büyük dosyayı resolve → link/copy"]
    end

    %% ===================== MANAGER / WATCHDOG =====================
    subgraph MG["rgsx_manager.py"]
        M0["_resume_interrupted_downloads():<br/>history(Téléchargement/Downloading/Paused)<br/>→ queue'ya 'Queued' geri ekle"]
        M1["_watchdog_loop():<br/>HysteresisMonitor + RestartLimiter<br/>→ UNRESPONSIVE→RESTARTING→spawn restart<br/>/ CRASHED"]
        M2["_trigger_shutdown():<br/>shutdown_downloads + cancel_all + STOP"]
    end

    %% ---- ÇAPRAZ BAĞLANTILAR ----
    T4r -.delegasyon.-> RD3
    T5 ==> Q6
    Q6ok ==> Q7
    Q7 ==> Q8
    P5 -.çağrır.-> Q9
    F2r ==> R0
    P7 -.qBittorrent path.-> Q6
    M0 -.başlangıç.-> W0
    M1 -.state.-> Q1
    RD1 ==> RD3
```

---

## 2. Rust Eşleme Tablosu

| Düğüm(ler) | Python fonksiyon | Rust karşılığı | Durum |
|---|---|---|---|
| `Q1` (BackendState), `M1` (ManagerState), `F0/F1/F2/F3` (DownloadState/Event) | `watchdog.py`, `download_state.py` | `manager-core/src/state.rs` (ManagerState, BackendState, DownloadState, DownloadEvent, `transition`, `state_from_legacy`, `legacy_history_status`, `IllegalTransitionError`) | ✅ **TASK-002a** |
| `M1` (hysteresis/restart) | `watchdog.py` HysteresisMonitor/RestartLimiter | `manager-core/src/watchdog.rs` | ✅ **TASK-002a** |
| `RD1`, `RD2`, `RD3` | `rust_daemon.start/supervisor/download_torrent` | `rust_daemon.py` (Python supervisor) + `manager-bin` | ✅ **TASK-002i/002j** |
| `T4r` (torrent byte indirme) | `qbittorrent_backend.download_torrent_via_qbittorrent` | `manager-torrent/src/lib.rs` `LibrqbitEngine.download_torrent_source` | ⚠️ **kısmi** (TASK-002f/002g/002l): file-selection, pause/resume, seed, retry, cancel **yok** |
| `Q10` (bridge protokol) | `qbittorrent_backend._BRIDGE_METHODS` | `manager-bridge` + Python `_bridge_serve_loop` | ✅ **TASK-002c/002k-5** (Python tarafı Rust bin'e servis eder) |

---

## 3. Rust Karşılığı **OLMAYAN** Düğümler (Eksikler)

Aşağıdaki düğüm kümelerinin Rust'ta (ne `manager-core`, ne `manager-torrent`, ne `manager-bin`)
henüz bir karşılığı yok. Nadir/hatalı dallar özellikle işaretlendi.

| # | Eksik düğüm(ler) | Kritiklik | Görev dosyası |
|---|---|---|---|
| 1 | `F1→F2→R0..R3` — transient hata sınıflandırma, `_retry_backoff`, `_schedule_download_retry`, `_retry_in_flight` dedup | P1 | `tasks/gap/TASK-002-gap-1-retry-engine.md` |
| 2 | `P0..P2`, `P7` — pause/resume orkestrasyonu (toggle, pause_all, resume_all, pause_ev → backend) | **P0** | `tasks/gap/TASK-002-gap-2-pause-resume.md` |
| 3 | `P3..P6`, `Q9`, `Q8` (kısmi) — cancel + yarım kalan dosya/torrent temp-root/seeder artifact temizliği | **P0** | `tasks/gap/TASK-002-gap-3-cancel-cleanup.md` |
| 4 | `H0..H12` (tüm HTTP-alt ağacı) — vimm/archive.org/lolroms/1fichier, header variantları, browser-challenge, 429 backoff, Range resume, arşiv imza kontrolleri | **P0** | `tasks/gap/TASK-002-gap-4-http-direct.md` |
| 5 | `D5`, `H8` — disk alanı ön-kontrolü (`InsufficientDiskSpaceError`) | P1 | `tasks/gap/TASK-002-gap-5-disk-space.md` |
| 6 | `H12`, `H12e` — arşiv auto-extract / post-process (BIOS, PS3 redump force) | P2 | `tasks/gap/TASK-002-gap-6-extract.md` |
| 7 | `Q6ok→Q7→Q8`, `Q6c` — seed lifecycle (promote-to-seed, `_seed_status_worker`, `has_active_seed`, `stop_seed`) + password migration | P1 | `tasks/gap/TASK-002-gap-7-seed-lifecycle.md` |
| 8 | `Q9` — stray torrent temp-root temizliği (`_find_stray_torrent_temp_roots`) | P2 | `tasks/gap/TASK-002-gap-8-stray-temp.md` |
| 9 | `M0` — restart sonrası yarıda kalan indirmeyi sürdürme (Rust librqbit session ephemral → torrent resume kaybolur) | P1 | `tasks/gap/TASK-002-gap-9-resume-interrupted.md` |
| 10 | `F0` (mark_game_as_downloaded, emit_state_event, bulk history), `D2/D7` history kayıtları — daemon içinde history/SSE sonlandırma | P1 | `tasks/gap/TASK-002-gap-10-history-sse.md` |

> Not: Orkestratörün kendisi (`W0..W3`, `D0..D8` dış sarmalayıcı) şu an Python'da kalıyor ve
> Rust'a yalnızca *torrent byte indirme* (`T4r`/`RD4`) devrediliyor. Yukarıdaki 10 madde, Python
> kaldırıldığında/doğrulanırken **davranışsal olarak kaybolacak** düğümlerdir; refaktör bu düğümleri
> ya Rust'a taşımalı ya da daemon sözleşmesine eklemelidir.

---

## 4. Nadir / Hata Yolları Kontrol Listesi

- [x] `T1` ARM cihazda torrent reddi (RuntimeError)
- [x] `T2` torrent dosyası zaten mevcutsa atlama (boyut eşleşmesi)
- [x] `T4r` Rust delegasyon hatası → qBittorrent fallback (risk sıfır)
- [x] `Q2s`/`Q3s` qBittorrent binary yok / port tükendi → STOPPED
- [x] `Q6a` indirme sırasında cancel → torrent+temp sil (başka ref yoksa)
- [x] `Q6b`/`P7` gerçek qBittorrent pause (sadece polling durmaz, torrent askıya alınır)
- [x] `Q6c2` error state → RuntimeError
- [x] `H5b` browser challenge → raise (interactive tarayıcı gerekli)
- [x] `H5d` 429 rate-limit → Retry-After / exp backoff
- [x] `H9` boş response → dosya sil, raise
- [x] `H10` arşiv imza/HTML/challenge guards (kısmi kabul)
- [x] `F2` transient vs kalıcı hata ayrımı + retry hakkı
- [x] `M1` UNRESPONSIVE → RESTARTING → CRASHED (restart limiti)
- [ ] `Q9` stray temp-root temizliği — **Rust yok**
- [ ] `M0` restart sonrası resume — **Rust session ephemral, torrent resume kaybolur**
- [ ] `Q6ok→Q7` seed sonrası temizlik/iptal — **Rust yok**
- [ ] `H0..H12` HTTP-direct tüm alt ağaç — **Rust yok**
```
