# -*- coding: utf-8 -*-
"""Faz 10c/1 — Rust `manager-bin` sidecar torrent daemon süpervizörü.

`rgsx_manager.py` bu modülü kullanarak Rust `manager-bin`'i supervised bir
yan süreç (sidecar) olarak başlatır ve sağlığını izler. Tüm davranış
`RGSX_RUST_DAEMON` env flag'iyle kapalıdır; binary bulunamazsa sessizce
Python-only moda düşer (risk sıfır — mevcut akış değişmez).

Bağımlılık: yalnızca standart kütüphane + (lazy) `watchdog.RestartLimiter`.
`config` yalnızca fonksiyon içinde lazy import edilir (üst-seviye döngü yok).
"""

import os
import subprocess
import threading
import time
import json
import logging
import urllib.request

logger = logging.getLogger("rust_daemon")

# Rust manager-bin varsayılan portu (Python manager portu 5000'den ayrı).
DEFAULT_PORT = 5010

# Modül-seviyesi durum.
_DAEMON_PORT = DEFAULT_PORT
_PROC = None
_STOP = threading.Event()

# Faz 10c/2: Python indirme akışını Rust daemon'a devretme opt-in flag'i.
_TORRENT_DELEGATE_TIMEOUT = 3600  # saniye — büyük torrentler için.
_TORRENT_DELEGATE_POLL = 2  # saniye — /api/progress poll aralığı.


class RustDaemonError(RuntimeError):
    """Torrent devri başarısız oldu (caller qBittorrent'e fallback eder)."""


def enabled() -> bool:
    """RGSX_RUST_DAEMON=1/true/yes ise sidecar etkin."""
    return os.environ.get("RGSX_RUST_DAEMON", "").lower() in ("1", "true", "yes")


def torrent_delegate_enabled() -> bool:
    """Faz 10c/2 — RGSX_RUST_TORRENT=1/true/yes ise Python torrent akışı Rust'a devredilir."""
    return os.environ.get("RGSX_RUST_TORRENT", "").lower() in ("1", "true", "yes")


def _resolve_bin():
    """manager-bin binary'sini bulur.

    Öncelik: RGSX_MANAGER_BIN_PATH env -> bilinen birkaç konum (repo kökü
    altında rust-target-sandbox / manager-rs target). Windows'ta .exe eklenir.
    """
    ext = ".exe" if os.name == "nt" else ""
    env = os.environ.get("RGSX_MANAGER_BIN_PATH")
    if env and os.path is not None and os.path.isfile(env):
        return env
    try:
        script_dir = os.path.dirname(os.path.abspath(__file__))
    except NameError:
        script_dir = os.getcwd()
    # ports/RGSX -> repo kökü iki seviye yukarı.
    repo_root = os.path.dirname(os.path.dirname(script_dir))
    candidates = [
        os.path.join(repo_root, "rust-target-sandbox", "debug", "manager-bin" + ext),
        os.path.join(repo_root, "manager-rs", "target", "debug", "manager-bin" + ext),
        os.path.join(script_dir, "rust-target-sandbox", "debug", "manager-bin" + ext),
    ]
    for c in candidates:
        if os.path.isfile(c):
            return c
    return None


def start():
    """Rust daemon'ı başlatır; başarılıysa Popen, aksi halde None döner.

    `RGSX_TORRENT_ENGINE` set edilmemişse `librqbit` (varsayılan) kullanılır.
    """
    global _PROC, _DAEMON_PORT
    if not enabled():
        return None
    bin_path = _resolve_bin()
    if not bin_path:
        logger.warning(
            "[RUST-DAEMON] manager-bin binary bulunamadı (RGSX_RUST_DAEMON=1 ama yok) "
            "— Python-only devam"
        )
        return None
    port = int(os.environ.get("RGSX_MANAGER_BIN_PORT", DEFAULT_PORT))
    _DAEMON_PORT = port
    env = dict(os.environ)
    env.setdefault("RGSX_TORRENT_ENGINE", "librqbit")
    # Faz 10c/2: librqbit output klasörünü Python ROMS_FOLDER'a çek (dest_path
    # verilmediğinde bile dosya Python'ın ROM klasöründe bitir). `config`
    # zaten import edilmişse kullan (zorla import -> yan etki tetiklememek için).
    try:
        import sys

        _cfg = sys.modules.get("config")
        if _cfg is not None:
            if getattr(_cfg, "ROMS_FOLDER", None):
                env.setdefault("RGSX_DOWNLOADS_FOLDER", _cfg.ROMS_FOLDER)
            _logs = getattr(_cfg, "LOGS_FOLDER", None)
            if _logs:
                env.setdefault("RGSX_LOGS_FOLDER", _logs)
    except Exception:
        pass
    try:
        proc = subprocess.Popen(
            [bin_path, "--port", str(port)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=env,
        )
        _PROC = proc
        logger.info(f"[RUST-DAEMON] manager-bin başlatıldı (pid={proc.pid}, port={port})")
        try:
            import config

            config.rust_daemon_available = True
        except Exception:
            pass
        return proc
    except Exception as e:
        logger.warning(f"[RUST-DAEMON] başlatılamadı: {e}")
        return None


def healthy(port: int | None = None) -> bool:
    """Daemon /api/health üzerinden sağlıklı mı?"""
    port = port or _DAEMON_PORT
    try:
        with urllib.request.urlopen(
            f"http://127.0.0.1:{port}/api/health", timeout=2
        ) as resp:
            if resp.status != 200:
                return False
            data = json.loads(resp.read().decode("utf-8"))
            return bool(data.get("success") and data.get("manager"))
    except Exception:
        return False


def request_stop() -> None:
    """Süpervizör döngüsünü durdurur ve süreci sonlandırır."""
    _STOP.set()
    proc = _PROC
    if proc is not None:
        try:
            proc.terminate()
        except Exception:
            pass


def supervisor() -> None:
    """Daemon'ı izler; çökerse sınırlı sayıda yeniden başlatır (RestartLimiter).

    `watchdog.RestartLimiter` 1 saatte en fazla 3 restart'a izin verir; limit
    dolunca döngüden çıkar. `_STOP` set edilirse usulca sonlanır.
    """
    global _PROC
    from watchdog import RestartLimiter

    limiter = RestartLimiter(3, 3600)
    while not _STOP.is_set():
        time.sleep(5)
        proc = _PROC
        alive = proc is not None and proc.poll() is None
        try:
            import config

            config.rust_daemon_available = healthy()
        except Exception:
            pass
        if healthy():
            continue
        if not alive:
            if limiter.record_restart():
                logger.warning("[RUST-DAEMON] sağlıksız/çöktü → yeniden başlatılıyor")
                start()
            else:
                logger.error("[RUST-DAEMON] restart limiti aşıldı, supervised bırakılıyor")
                break


def _post_json(port: int, path: str, body: dict) -> dict:
    """Rust daemon'a JSON POST; başarısızsa RustDaemonError."""
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        if resp.status >= 400:
            raise RustDaemonError(f"{path} HTTP {resp.status}")
        return json.loads(resp.read().decode("utf-8"))


def _poll_progress(port: int, url: str) -> dict | None:
    """Rust /api/progress'ten bu URL'nin ilerlemesini döner (yoksa None)."""
    try:
        with urllib.request.urlopen(
            f"http://127.0.0.1:{port}/api/progress", timeout=3
        ) as resp:
            if resp.status != 200:
                return None
            data = json.loads(resp.read().decode("utf-8"))
            downloads = data.get("downloads") or {}
            return downloads.get(url)
    except Exception:
        return None


def _mirror_progress(original_url, task_id, prog, game_name, platform) -> None:
    """Rust ilerlemesini Python config.download_progress + config.history'ye yazar."""
    if not prog:
        return
    try:
        import config as _cfg
    except Exception:
        return
    try:
        dp = dict(getattr(_cfg, "download_progress", {}) or {})
        dp[original_url] = {
            "downloaded_size": prog.get("downloaded_size", 0),
            "total_size": prog.get("total_size", 0),
            "status": prog.get("status", "Downloading"),
            "progress_percent": prog.get("progress", 0),
            "speed": prog.get("speed", 0),
            "game_name": game_name,
            "platform": platform,
        }
        _cfg.download_progress = dp
    except Exception:
        pass
    try:
        history = getattr(_cfg, "history", None) or []
        changed = False
        for entry in history:
            if entry.get("url") == original_url:
                entry["status"] = prog.get("status", entry.get("status"))
                if "progress" in prog:
                    entry["progress"] = prog["progress"]
                if prog.get("message"):
                    entry["message"] = prog["message"]
                changed = True
                break
        if changed:
            from history import save_history

            save_history(history)
    except Exception:
        pass


def download_torrent(
    torrent_meta,
    dest_dir,
    dest_path,
    task_id,
    cancel_ev,
    game_name,
    platform,
    original_url,
) -> tuple[bool, str]:
    """Faz 10c/2 — torrent indirmeyi Rust daemon'a devreder.

    `torrent_meta` (parse_torrent_download_url çıktısı) içindeki `source_url`'yi
    Rust `/api/download`'a gönderir; ilerlemeyi `/api/progress`'ten poll edip
    Python state'ine yansıtır; `cancel_ev` set edilirse iptal eder. Başarısızlık
    veya timeout → `RustDaemonError` (caller qBittorrent'e fallback eder).
    """
    if not (enabled() and healthy()):
        raise RustDaemonError("[RUST-DAEMON] daemon hazır değil (sağlıksız/kapalı)")
    source_url = (torrent_meta or {}).get("source_url")
    if not source_url:
        raise RustDaemonError("[RUST-DAEMON] torrent_meta'da source_url yok")
    port = _DAEMON_PORT

    # 1) İndirmeyi başlat (Rust arka planda koşar, hemen 'queued' döner).
    _post_json(
        port,
        "/api/download",
        {
            "platform": platform,
            "game_name": game_name,
            "url": source_url,
            "dest_path": dest_path,
        },
    )
    logger.info(f"[RUST-DAEMON] torrent devredildi: {game_name} -> {source_url}")

    # 2) İlerlemeyi izle.
    deadline = time.time() + _TORRENT_DELEGATE_TIMEOUT
    while time.time() < deadline:
        if cancel_ev is not None and cancel_ev.is_set():
            try:
                _post_json(port, "/api/cancel", {"url": source_url})
            except Exception:
                pass
            raise RustDaemonError("[RUST-DAEMON] torrent iptal edildi")
        prog = _poll_progress(port, source_url)
        _mirror_progress(original_url, task_id, prog, game_name, platform)
        status = (prog or {}).get("status")
        if status == "Download_OK":
            return True, f"{game_name} indirildi (Rust)"
        if status == "Erreur":
            return False, (prog or {}).get("message", "Rust torrent hatası")
        time.sleep(_TORRENT_DELEGATE_POLL)

    raise RustDaemonError("[RUST-DAEMON] torrent indirme timeout")
