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


def enabled() -> bool:
    """RGSX_RUST_DAEMON=1/true/yes ise sidecar etkin."""
    return os.environ.get("RGSX_RUST_DAEMON", "").lower() in ("1", "true", "yes")


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
