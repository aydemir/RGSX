"""Manager RGSX spawn + dış supervisor (daemon + tray + queue).

Faz 6-5: __main__.py'nin manager ile ilgili mantığı buraya taşındı. Davranış
birebir korunur; tvui (TVUI boot akışı) bu modülü import eder.

- `ensure_manager`: manager'ı garantiler / joystick varsa...
- `_start_manager_supervisor`: manager hard-crash / kalıcı yanıtsızlıkta respawn.
"""

import json
import logging
import os
import subprocess
import sys
import threading
import time

import config

from watchdog import (  # noqa: E402
    STATE_UNRESPONSIVE,
    HysteresisMonitor,
    RestartLimiter,
)

logger = logging.getLogger("manager_launcher")


# ===== GESTION DU MANAGER RGSX (daemon + tray + queue) =====

def _manager_healthy(port=None):
    """Vérifie qu'un manager RGSX répond sur /api/health."""
    port = port or getattr(config, 'manager_port', 5000)
    try:
        import urllib.request
        with urllib.request.urlopen(f'http://127.0.0.1:{port}/api/health', timeout=2) as resp:
            if resp.status != 200:
                return False
            data = json.loads(resp.read().decode('utf-8'))
            return bool(data.get('success') and data.get('manager'))
    except Exception:
        return False


def _spawn_manager_process(port):
    """rgsx_manager.py'yi arka planda spawn eder. Başarılıysa process döner."""
    manager_script = os.path.join(config.APP_FOLDER, 'rgsx_manager.py')
    if not os.path.exists(manager_script):
        logger.warning(f'Manager introuvable: {manager_script}, mode local')
        return None
    try:
        spawn_log = os.path.join(config.log_dir, 'rgsx_manager_spawn.log')
        with open(spawn_log, 'w', encoding='utf-8') as log_file:
            if config.OPERATING_SYSTEM == 'Windows':
                CREATE_NO_WINDOW = 0x08000000
                proc = subprocess.Popen(
                    [sys.executable, manager_script, f'--port={port}', '--minimized'],
                    stdout=log_file, stderr=subprocess.STDOUT,
                    cwd=config.APP_FOLDER, creationflags=CREATE_NO_WINDOW)
            else:
                proc = subprocess.Popen(
                    [sys.executable, manager_script, f'--port={port}', '--minimized'],
                    stdout=log_file, stderr=subprocess.STDOUT,
                    cwd=config.APP_FOLDER)
        return proc
    except Exception as e:
        logger.error(f'Erreur démarrage manager: {e}')
        return None


def _wait_for_manager_ready(proc=None, timeout=30):
    """Manager sağlıklı olana dek bekler; port settings'ten yeniden okunur
    (Faz 3 fallback'e geçerse yeni port yakalanır). True = hazır."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            from rgsx_settings import get_manager_port
            port = get_manager_port()
            config.manager_port = port
        except Exception:
            pass
        if _manager_healthy(port):
            return True
        if proc is not None and proc.poll() is not None:
            logger.warning(f'Manager arrêté immédiatement (code {proc.returncode}), mode local')
            return False
        time.sleep(0.5)
    logger.warning('Manager non prêt, mode local')
    return False


def ensure_manager():
    """Garantit qu'un manager RGSX est actif et expose config.manager_available.

    Retourne True si un manager est disponible (délégation HTTP des téléchargements),
    False en mode local (--ui-only / fallback : la TV UI gère sa propre queue).
    """
    try:
        from rgsx_settings import get_manager_port
        config.manager_port = get_manager_port()
    except Exception:
        config.manager_port = getattr(config, 'manager_port', 5000)
    config.manager_available = False

    # Mode local explicite: pas de manager, pas de délégation
    if '--ui-only' in sys.argv or os.environ.get('RGSX_NO_MANAGER') == '1':
        logger.info('Mode --ui-only : démarrage sans manager')
        return False

    port = config.manager_port

    if _manager_healthy(port):
        config.manager_available = True
        logger.info(f'✅ Manager RGSX déjà actif sur http://localhost:{port}')
        return True

    proc = _spawn_manager_process(port)
    if proc is None:
        return False

    if _wait_for_manager_ready(proc=proc, timeout=30):
        config.manager_available = True
        logger.info('✅ Manager RGSX démarré')
        return True
    return False


# ===== Faz 4 — Dış supervisor (TV UI) =====
# Tray, manager process'inin İÇİNDE yaşadığı için manager'ı supervise edemez
# (hard-crash'te ikisi birlikte ölür). Gerçek dış supervisor, manager'ı spawn
# eden TV UI sürecidir: manager'ın /api/health'i yanıtlamadığını görünce respawn
# eder. TV UI yoksa (--no-tray / daemon-only) Task Scheduler (Windows) / systemd
# (Linux) alternatifi ROADMAP_DOWNLOAD_MANAGER.md'de belgelenir.

_SUPERVISOR_POLL_SECONDS = 5.0
_SUPERVISOR_DEGRADE_THRESHOLD = 3
_SUPERVISOR_UNRESPONSIVE_THRESHOLD = 6
_SUPERVISOR_MAX_RESTARTS = 3
_SUPERVISOR_RESTART_WINDOW_SECONDS = 3600


def _start_manager_supervisor():
    """Manager hard-crash / kalıcı yanıtsızlık durumunda respawn eden daemon thread."""
    threading.Thread(target=_manager_supervisor_loop, daemon=True,
                     name="manager-supervisor").start()


def _manager_supervisor_loop():
    monitor = HysteresisMonitor(_SUPERVISOR_DEGRADE_THRESHOLD, _SUPERVISOR_UNRESPONSIVE_THRESHOLD)
    limiter = RestartLimiter(_SUPERVISOR_MAX_RESTARTS, _SUPERVISOR_RESTART_WINDOW_SECONDS)
    while True:
        time.sleep(_SUPERVISOR_POLL_SECONDS)
        if not getattr(config, 'manager_available', False):
            continue
        try:
            from rgsx_settings import get_manager_port
            port = get_manager_port()
            config.manager_port = port
        except Exception:
            port = getattr(config, 'manager_port', 5000)
        healthy = _manager_healthy(port)
        state = monitor.report(healthy)
        logger.debug(f"[SUPERVISOR] manager health={'ok' if healthy else 'fail'} → {state}")
        if state != STATE_UNRESPONSIVE:
            continue
        if limiter.record_restart():
            logger.error(f"[SUPERVISOR] manager yanıtsız ({port}) → RESTARTING (respawn)")
            if _spawn_manager_process(port) is None:
                logger.error("[SUPERVISOR] respawn başlatılamadı — manager kapalı kalabilir")
                continue
            # Respawn sonrası yeni portu bekle/yakala; başarısızsa bir sonraki
            # poll turunda tekrar denenecek (RestartLimiter sınırlar).
            if _wait_for_manager_ready(timeout=30):
                monitor.reset()
                logger.info("[SUPERVISOR] manager yeniden sağlıklı")
            else:
                logger.warning("[SUPERVISOR] respawn sonrası manager 30s içinde hazır olmadı")
        else:
            logger.error("[SUPERVISOR] restart limiti aşıldı → CRASHED, elle müdahale gerekli")


def stop_web_server():
    """Compatible avec l'ancien flux: le serveur web est désormais géré par le manager."""
    pass
