#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
RGSX Download Manager daemon.

Single background process that:
  * runs the download queue worker (network.download_queue_worker)
  * hosts the RGSX web UI + REST API on port 5000
  * pushes real-time events over SSE (/api/events)
  * lives in the system tray (Windows) so downloads keep running in the background

Run directly:            python rgsx_manager.py [--port N] [--no-tray]
Spawn helper:            python rgsx_manager.py --auto-start-install / --auto-start-remove
"""
import os
import sys

_APP_DIR = os.path.dirname(os.path.abspath(__file__))
if _APP_DIR not in sys.path:
    sys.path.insert(0, _APP_DIR)

# Headless: this process must never touch the pygame UI
os.environ.setdefault("RGSX_HEADLESS", "1")

import argparse
import datetime
import json
import logging
import queue as queue_module
import subprocess
import threading
import time
import urllib.parse
import urllib.request
import webbrowser

import config
import rgsx_web
from rgsx_web import RGSXHandler, get_cached_games, get_translation
from utils import get_clean_display_name
from settings_dialog import open_server_settings_dialog
from language import _

from network import (
    download_queue_worker,
    shutdown_downloads,
    cancel_all_downloads,
    request_cancel,
    pause_all_downloads,
    resume_all_downloads,
    is_any_download_paused,
)
from history import load_history, save_history
from watchdog import (
    STATE_CRASHED,
    STATE_DEGRADED,
    STATE_INIT,
    STATE_RESTARTING,
    STATE_RUNNING,
    STATE_UNRESPONSIVE,
    HysteresisMonitor,
    RestartLimiter,
)

logger = logging.getLogger("rgsx_manager")

# ---------------------------------------------------------------------------
# Globals
# ---------------------------------------------------------------------------
STOP = threading.Event()

SUBSCRIBERS = set()
SUBSCRIBERS_LOCK = threading.Lock()

_TRAY_ICON = None


# ---------------------------------------------------------------------------
# SSE
# ---------------------------------------------------------------------------
def _sse_event(event_type: str, data) -> str:
    payload = json.dumps(data, ensure_ascii=False, default=str)
    return f"event: {event_type}\ndata: {payload}\n\n"


def _build_snapshot() -> dict:
    try:
        history = list(getattr(config, "history", []) or [])
    except Exception:
        history = []
    try:
        queue_state = list(getattr(config, "download_queue", []) or [])
    except Exception:
        queue_state = []
    try:
        progress = dict(getattr(config, "download_progress", {}) or {})
    except Exception:
        progress = {}
    try:
        downloaded = dict(getattr(config, "downloaded_games", {}) or {})
    except Exception:
        downloaded = {}
    return {
        "history": history,
        "queue": queue_state,
        "active": bool(getattr(config, "download_active", False)),
        "progress": progress,
        "downloaded": downloaded,
    }


def _broadcast(event_type: str, data=None):
    if not SUBSCRIBERS:
        return
    msg = _sse_event(event_type, data if data is not None else {})
    with SUBSCRIBERS_LOCK:
        subs = list(SUBSCRIBERS)
    for q in subs:
        try:
            q.put_nowait({"type": event_type, "raw": msg})
        except Exception:
            pass


def _broadcaster_loop():
    """Diff config state every ~250ms and push changed sections over SSE."""
    last = {
        "history": None,
        "queue": None,
        "progress": None,
        "downloaded": None,
    }
    last_snapshot = 0.0
    while not STOP.is_set():
        time.sleep(0.25)
        try:
            hist = getattr(config, "history", None)
            fp = repr(hist)
            if fp != last["history"]:
                last["history"] = fp
                _broadcast("history", {"history": list(hist or [])})

            qstate = getattr(config, "download_queue", None)
            fp = repr(qstate)
            if fp != last["queue"]:
                last["queue"] = fp
                _broadcast("queue", {
                    "queue": list(qstate or []),
                    "active": bool(getattr(config, "download_active", False)),
                })

            prog = getattr(config, "download_progress", None)
            fp = repr(prog)
            if fp != last["progress"]:
                last["progress"] = fp
                _broadcast("progress", {
                    "progress": dict(prog or {}),
                    "active": bool(getattr(config, "download_active", False)),
                })

            down = getattr(config, "downloaded_games", None)
            fp = repr(down)
            if fp != last["downloaded"]:
                last["downloaded"] = fp
                _broadcast("downloaded", {"downloaded": dict(down or {})})

            now = time.time()
            if now - last_snapshot >= 30.0:
                last_snapshot = now
                _broadcast("snapshot", _build_snapshot())
        except Exception as e:
            logger.debug(f"[MANAGER] broadcast loop error: {e}")


# ---------------------------------------------------------------------------
# HTTP handler
# ---------------------------------------------------------------------------
class ManagerHandler(RGSXHandler):
    """RGSX web handler + manager-specific endpoints."""

    # -- GET ---------------------------------------------------------------
    def do_GET(self):
        parsed_path = urllib.parse.urlparse(self.path)
        path = parsed_path.path

        if path == "/api/health":
            self._send_json({
                "success": True,
                "status": "ok",
                "manager": True,
                "version": getattr(config, "app_version", ""),
                "pid": os.getpid(),
                "manager_state": get_manager_state(),
            })
            return

        if path == "/api/qbittorrent/password-status":
            try:
                from qbittorrent_backend import get_password_status
                status = get_password_status()
                self._send_json({"success": True, **status})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/qbittorrent/password-status: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        if path == "/api/events":
            self._handle_sse()
            return

        super().do_GET()

    # -- POST --------------------------------------------------------------
    def do_POST(self):
        parsed_path = urllib.parse.urlparse(self.path)
        path = parsed_path.path

        if path == "/api/download":
            self._handle_download_worker()
            return

        if path == "/api/download/batch":
            try:
                content_length = int(self.headers.get("Content-Length", 0))
                post_data = self.rfile.read(content_length)
                data = json.loads(post_data.decode("utf-8")) if content_length > 0 else {}
            except Exception as e:
                self._send_json({"success": False, "error": str(e)}, status=400)
                return
            # Faz 9: queue-worker tek tüketicidir → sadece kuyruğa bas (kick yok).
            self._api_download_batch(data)
            return

        if path == "/api/cancel":
            self._handle_cancel_worker()
            return

        if path == "/api/shutdown":
            self._send_json({"success": True, "message": "Shutdown en cours..."})
            threading.Thread(target=_trigger_shutdown, daemon=True).start()
            return

        if path == "/api/pause":
            try:
                n = pause_all_downloads()
                self._send_json({"success": True, "paused": n})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/pause: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        if path == "/api/qbittorrent/start":
            try:
                from qbittorrent_backend import ensure_running, get_webui_url
                ready = ensure_running(timeout=30)
                self._send_json({"success": ready, "ready": ready,
                                 "url": get_webui_url()})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/qbittorrent/start: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        if path == "/api/qbittorrent/change-password":
            try:
                content_length = int(self.headers.get("Content-Length", 0))
                post_data = self.rfile.read(content_length) if content_length > 0 else b"{}"
                body = json.loads(post_data.decode("utf-8")) if post_data else {}
                new_password = str(body.get("password") or "")
                from qbittorrent_backend import change_webui_password
                ok, message = change_webui_password(new_password)
                if not ok:
                    self._send_json({"success": False, "message": message}, status=400)
                    return
                self._send_json({"success": True, "message": "ok"})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/qbittorrent/change-password: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        if path == "/api/qbittorrent/regenerate-password":
            try:
                from qbittorrent_backend import regenerate_qbittorrent_password
                ok, password = regenerate_qbittorrent_password()
                if not ok:
                    self._send_json({"success": False, "message": "password_regeneration_failed"}, status=500)
                    return
                self._send_json({"success": True, "password": password})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/qbittorrent/regenerate-password: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        if path == "/api/resume":
            try:
                n = resume_all_downloads()
                self._send_json({"success": True, "resumed": n})
            except Exception as e:
                logger.warning(f"[MANAGER] /api/resume: {e}")
                self._send_json({"success": False, "message": str(e)}, status=500)
            return

        super().do_POST()

    # -- SSE ---------------------------------------------------------------
    def _handle_sse(self):
        try:
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream; charset=utf-8")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Connection", "keep-alive")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
        except Exception:
            return

        q = queue_module.Queue()
        with SUBSCRIBERS_LOCK:
            SUBSCRIBERS.add(q)

        try:
            self.wfile.write(_sse_event("snapshot", _build_snapshot()).encode("utf-8"))
            self.wfile.flush()
            while not STOP.is_set():
                try:
                    item = q.get(timeout=15)
                    self.wfile.write(item["raw"].encode("utf-8"))
                    self.wfile.flush()
                except queue_module.Empty:
                    self.wfile.write(_sse_event("snapshot", _build_snapshot()).encode("utf-8"))
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass
        finally:
            with SUBSCRIBERS_LOCK:
                SUBSCRIBERS.discard(q)

    # -- Worker-based /api/download ----------------------------------------
    def _handle_download_worker(self):
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data.decode("utf-8")) if content_length > 0 else {}
        except Exception as e:
            self._send_json({"success": False, "error": str(e)}, status=400)
            return

        platform = data.get("platform")
        game_index = data.get("game_index")
        game_name_param = data.get("game_name")
        direct_url = data.get("url")
        mode = data.get("mode", "now")

        if not platform or (game_index is None and not game_name_param and not direct_url):
            self._send_json({
                "success": False,
                "error": "Paramètres manquants: platform et (game_index ou game_name) requis",
            }, status=400)
            return

        game_name = None
        game_url = None

        if direct_url:
            # Délégation directe (TV UI / CLI): url + game_name déjà connus
            game_url = direct_url
            game_name = game_name_param
            if not game_name:
                self._send_json({"success": False, "error": "Paramètre manquant: game_name requis avec url"}, status=400)
                return
            from utils import check_extension_before_download
            check_result = check_extension_before_download(game_url, platform, game_name)
            if not check_result:
                self._send_json({
                    "success": False,
                    "error": "Extension non supportée ou erreur de vérification",
                }, status=400)
                return
            is_zip_non_supported = check_result[3] if len(check_result) > 3 else False
        else:
            games, _, _ = get_cached_games(platform)

            if game_name_param and game_index is None:
                game_index = None
                for idx, game in enumerate(games):
                    if game.name == game_name_param:
                        game_index = idx
                        break
                if game_index is None:
                    self._send_json({"success": False, "error": f"Jeu non trouvé: {game_name_param}"}, status=400)
                    return

            if game_index is None or game_index < 0 or game_index >= len(games):
                self._send_json({"success": False, "error": f"Index de jeu invalide: {game_index}"}, status=400)
                return

            game = games[game_index]
            game_name = game.name
            game_url = game.url

            if not game_url:
                self._send_json({
                    "success": False,
                    "error": get_translation("popup_torrent_in_maintenance", "torrent in maintenance"),
                }, status=400)
                return

            from utils import check_extension_before_download
            check_result = check_extension_before_download(game_url, platform, game_name)
            if not check_result:
                self._send_json({
                    "success": False,
                    "error": "Extension non supportée ou erreur de vérification",
                }, status=400)
                return

            is_zip_non_supported = check_result[3] if len(check_result) > 3 else False

        is_1fichier = "1fichier.com" in game_url
        task_id = f"web_{int(time.time() * 1000)}"

        # Push into the shared queue: the download_queue_worker picks it up
        # within ~1s (immediately if a slot is free).
        config.download_queue.append({
            "url": game_url,
            "platform": platform,
            "game_name": game_name,
            "is_zip_non_supported": is_zip_non_supported,
            "is_1fichier": is_1fichier,
            "task_id": task_id,
            "status": "Queued",
        })

        queue_history_entry = {
            "platform": platform,
            "game_name": game_name,
            "display_name": get_clean_display_name(game_name, platform),
            "status": "Queued",
            "url": game_url,
            "progress": 0,
            "message": get_translation("download_queued"),
            "timestamp": datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "downloaded_size": 0,
            "total_size": 0,
            "task_id": task_id,
        }
        config.history.append(queue_history_entry)
        try:
            save_history(config.history)
        except Exception as e:
            logger.warning(f"[MANAGER] save_history échec: {e}")

        logger.info(f"[MANAGER] {game_name} ajouté à la queue (mode={mode}, position={len(config.download_queue)})")
        self._send_json({
            "success": True,
            "message": f"{game_name} ajouté à la file d'attente",
            "task_id": task_id,
            "game_name": game_name,
            "platform": platform,
            "queued": True,
            "queue_position": len(config.download_queue),
        })

    # -- Cancel via worker semantics ----------------------------------------
    def _handle_cancel_worker(self):
        """Annule un téléchargement sans manipuler la queue manuellement.

        Le download_queue_worker est l'unique consommateur de config.download_queue:
        on ne fait donc PAS de pop() ni de spawn de _process_queued_download ici.
        Le worker enchaîne tout seul sur l'élément suivant une fois que
        notify_download_finished() libère le slot.
        """
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data.decode("utf-8")) if content_length > 0 else {}
        except Exception as e:
            self._send_json({"success": False, "error": str(e)}, status=400)
            return

        url = data.get("url")
        if not url:
            self._send_json({"success": False, "error": "Paramètre manquant: url requis"}, status=400)
            return

        try:
            history = load_history() or []
            task_id = None
            for entry in history:
                if entry.get("url") == url and entry.get("status") in ["Downloading", "Téléchargement", "Connecting"]:
                    entry["status"] = "Canceled"
                    entry["progress"] = 0
                    entry["message"] = get_translation("web_download_canceled")
                    task_id = entry.get("task_id")
                    break

            if task_id:
                request_cancel(task_id)

            save_history(history)
            if isinstance(getattr(config, "history", None), list):
                config.history = history
            self._send_json({
                "success": True,
                "message": "Téléchargement annulé",
                "url": url,
                "task_id": task_id,
            })
        except Exception as e:
            logger.error(f"[MANAGER] Erreur annulation: {e}")
            self._send_json({"success": False, "error": str(e)}, status=500)


# ---------------------------------------------------------------------------
# Auto-start (Windows registry)
# ---------------------------------------------------------------------------
_AUTOSTART_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
_AUTOSTART_NAME = "RGSXManager"


def _autostart_command() -> str:
    exe = sys.executable or "python"
    if os.name == "nt":
        base = os.path.splitext(exe)[0]
        pythonw = base + "w.exe"
        if os.path.exists(pythonw):
            exe = pythonw
    script = os.path.abspath(__file__)
    return f'"{exe}" "{script}" --minimized'


def is_autostart_enabled() -> bool:
    if os.name != "nt":
        return False
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, _AUTOSTART_KEY) as k:
            winreg.QueryValueEx(k, _AUTOSTART_NAME)
        return True
    except OSError:
        return False


def autostart_install() -> bool:
    if os.name != "nt":
        logger.warning("[MANAGER] Auto-start uniquement supporté sur Windows")
        return False
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, _AUTOSTART_KEY, 0, winreg.KEY_SET_VALUE) as k:
            winreg.SetValueEx(k, _AUTOSTART_NAME, 0, winreg.REG_SZ, _autostart_command())
        logger.info("[MANAGER] Auto-start installé")
        return True
    except Exception as e:
        logger.error(f"[MANAGER] Auto-start installation échouée: {e}")
        return False


def autostart_remove() -> bool:
    if os.name != "nt":
        return False
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, _AUTOSTART_KEY, 0, winreg.KEY_SET_VALUE) as k:
            winreg.DeleteValue(k, _AUTOSTART_NAME)
        logger.info("[MANAGER] Auto-start supprimé")
        return True
    except FileNotFoundError:
        return False
    except Exception as e:
        logger.error(f"[MANAGER] Auto-start suppression échouée: {e}")
        return False


def _get_autostart_pref() -> bool:
    """Préférence utilisateur persistée (défaut: True => auto-start au boot activé par défaut)."""
    try:
        from rgsx_settings import get_autostart_on_boot
        return bool(get_autostart_on_boot())
    except Exception:
        return True


def _set_autostart_pref(enabled: bool) -> bool:
    try:
        from rgsx_settings import set_autostart_on_boot
        return bool(set_autostart_on_boot(enabled))
    except Exception:
        return bool(enabled)


# ---------------------------------------------------------------------------
# System tray
# ---------------------------------------------------------------------------
def _setup_tray(icon_path: str, port: int, no_tray: bool = False):
    global _TRAY_ICON
    if no_tray:
        return None
    try:
        import pystray
        from PIL import Image
    except ImportError:
        logger.warning("[MANAGER] pystray/Pillow non installés, tray désactivé")
        return None

    try:
        image = Image.open(icon_path)
    except Exception as e:
        logger.warning(f"[MANAGER] Icône tray introuvable ({e}), icône par défaut")
        image = None

    def _open_ui(icon, item):
        webbrowser.open(f"http://localhost:{port}")

    def _open_downloads(icon, item):
        folder = getattr(config, "ROMS_FOLDER", "")
        if not folder or not os.path.isdir(folder):
            icon.notify("Downloads folder not found", "RGSX")
            return
        if os.name == "nt":
            os.startfile(folder)
        else:
            webbrowser.open(folder)

    def _open_logs(icon, item):
        log_dir = getattr(config, "log_dir", "")
        if not log_dir or not os.path.isdir(log_dir):
            icon.notify("Logs folder not found", "RGSX")
            return
        if os.name == "nt":
            os.startfile(log_dir)
        else:
            webbrowser.open(log_dir)

    def _toggle_autostart(icon, item):
        if is_autostart_enabled():
            autostart_remove()
            _set_autostart_pref(False)
            icon.notify("Auto-start disabled", "RGSX")
        else:
            autostart_install()
            _set_autostart_pref(True)
            icon.notify("Auto-start enabled", "RGSX")

    def _open_settings(icon, item):
        webbrowser.open(f"http://localhost:{port}/settings")

    def _get_current_server_cfg():
        from rgsx_settings import get_manager_port, get_manager_host, get_autostart_on_boot
        return {
            "port": get_manager_port(),
            "host": get_manager_host(),
            "autostart": get_autostart_on_boot(),
        }

    def _on_server_cfg_saved(cfg):
        if not cfg:
            return
        try:
            from rgsx_settings import (
                set_manager_port, set_manager_host, set_autostart_on_boot,
            )
            set_manager_port(cfg["port"])
            set_manager_host(cfg["host"])
            set_autostart_on_boot(cfg["autostart"])
            _set_autostart_pref(cfg["autostart"])
            logger.info(
                f"[MANAGER] Sunucu ayarları kaydedildi: port={cfg['port']} host={cfg['host']} "
                f"autostart={cfg['autostart']} restart={cfg.get('restart')}"
            )
            if cfg.get("restart"):
                threading.Thread(target=_restart_manager_for_settings, daemon=True).start()
        except Exception as e:
            logger.warning(f"[MANAGER] Ayar kaydı: {e}")

    def _open_server_settings(icon, item):
        open_server_settings_dialog(
            on_save=_on_server_cfg_saved,
            get_current=_get_current_server_cfg,
            app_dir=_APP_DIR,
        )

    def _toggle_pause_all(icon, item):
        try:
            if is_any_download_paused():
                n = resume_all_downloads()
                icon.notify(f"{n} indirme sürdürüldü", "RGSX")
            else:
                n = pause_all_downloads()
                icon.notify(f"{n} indirme durduruldu", "RGSX")
        except Exception as e:
            logger.warning(f"[MANAGER] pause toggle: {e}")

    def _quit(icon, item):
        _trigger_shutdown()

    menu = pystray.Menu(
        pystray.MenuItem(_("menu_open_web_ui", "Open Web UI"), _open_ui, default=True),
        pystray.MenuItem(_("menu_settings", "Ayarlar"), _open_settings),
        pystray.MenuItem(_("menu_server_settings", "Sunucu Ayarları..."), _open_server_settings),
        pystray.MenuItem(_("menu_toggle_downloads", "İndirmeleri Durdur/Sürdür"), _toggle_pause_all,
                         checked=lambda item: is_any_download_paused()),
        pystray.MenuItem(_("menu_downloads_folder", "Downloads folder"), _open_downloads),
        pystray.MenuItem(_("menu_logs_folder", "Logs folder"), _open_logs),
        pystray.MenuItem(_("menu_autostart", "Auto-start on boot"), _toggle_autostart,
                         checked=lambda item: is_autostart_enabled()),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem(_("menu_quit_app", "Exit"), _quit),
    )

    try:
        _TRAY_ICON = pystray.Icon("RGSX Manager", image, "RGSX Download Manager", menu)
        _TRAY_ICON.run_detached()
        logger.info("[MANAGER] Tray démarré")
    except Exception as e:
        logger.warning(f"[MANAGER] Tray impossible: {e}")
        _TRAY_ICON = None
    return _TRAY_ICON


def _restart_manager_for_settings():
    """Yeni port/host ayarlarıyla servisi yeniden başlat.

    Kısa bir gecikmeyle kendini tekrar spawn eder, ardından mevcut süreci kapatır.
    """
    try:
        time.sleep(0.8)
        from rgsx_settings import get_manager_port, get_manager_host
        new_port = get_manager_port()
        new_host = get_manager_host()
        if not _spawn_manager([f"--port={new_port}", f"--host={new_host}"]):
            return
        logger.info(f"[MANAGER] Restart: port={new_port} host={new_host}")
    except Exception as e:
        logger.warning(f"[MANAGER] Restart spawn hatası: {e}")
        return
    # Eski süreci kapat (yeni süreç portu serbest bulacak)
    try:
        _trigger_shutdown()
    except Exception as e:
        logger.warning(f"[MANAGER] Restart shutdown hatası: {e}")


def _resume_interrupted_downloads() -> int:
    """Après un redémarrage, remet en file les téléchargements interrompus/en pause.

    - Torrents: qBittorrent conserve les données partielles → reprise à l'endroit laissé.
    - HTTP direct: reprise depuis le début (pas de Range), mais la file est relancée.
    """
    try:
        history = load_history() or []
    except Exception as e:
        logger.warning(f"[RESUME] load_history: {e}")
        return 0

    interrupted = [
        e for e in history
        if e.get("status") in ("Téléchargement", "Downloading", "Paused")
        and e.get("url")
    ]
    if not interrupted:
        return 0

    for entry in interrupted:
        url = str(entry.get("url") or "").strip()
        if not url:
            continue
        task_id = entry.get("task_id") or f"resume_{int(time.time() * 1000)}"
        config.download_queue.append({
            "url": url,
            "platform": entry.get("platform", ""),
            "game_name": entry.get("game_name", ""),
            "is_zip_non_supported": bool(entry.get("is_zip_non_supported", False)),
            "task_id": task_id,
            "status": "Queued",
        })
        entry["status"] = "Queued"
        entry["message"] = "Queued (reprise après redémarrage)"

    try:
        save_history(history)
        if isinstance(getattr(config, "history", None), list):
            config.history = history
    except Exception as e:
        logger.warning(f"[RESUME] save_history: {e}")

    logger.info(f"[RESUME] {len(interrupted)} téléchargement(s) remis en file après redémarrage")
    return len(interrupted)


# ---------------------------------------------------------------------------
# Faz 4 — Watchdog / auto-restart
# ---------------------------------------------------------------------------
_WATCHDOG_POLL_SECONDS = 5.0
_WATCHDOG_HEALTH_TIMEOUT = 3.0
_WATCHDOG_DEGRADE_THRESHOLD = 3
_WATCHDOG_UNRESPONSIVE_THRESHOLD = 6
_WATCHDOG_MAX_RESTARTS = 3
_WATCHDOG_RESTART_WINDOW_SECONDS = 3600

MANAGER_STATE = STATE_INIT
MANAGER_STATE_LOCK = threading.Lock()


def get_manager_state() -> str:
    with MANAGER_STATE_LOCK:
        return MANAGER_STATE


def _set_manager_state(new_state: str, reason: str = "") -> None:
    global MANAGER_STATE
    with MANAGER_STATE_LOCK:
        previous = MANAGER_STATE
        MANAGER_STATE = new_state
    if previous != new_state:
        logger.warning(f"[WATCHDOG] manager {previous} → {new_state} ({reason})")


def _spawn_manager(extra_args: list[str] | None = None) -> bool:
    """Mevcut süreci aynı argümanlarla yeniden spawn eder.

    extra_args sona eklenir; argparse son değeri alır, bu yüzden --port/--host
    override edilebilir. Mevcut process'i kapatmak çağıranın işidir.
    """
    cmd = [sys.executable, os.path.abspath(__file__)]
    cmd.extend(sys.argv[1:])
    if "--minimized" not in cmd:
        cmd.append("--minimized")
    if extra_args:
        cmd.extend(extra_args)
    try:
        popen_kwargs = {
            "cwd": os.path.dirname(os.path.abspath(__file__)),
            "stdout": subprocess.DEVNULL,
            "stderr": subprocess.DEVNULL,
        }
        if os.name == "nt":
            popen_kwargs["creationflags"] = 0x08000000  # CREATE_NO_WINDOW
        subprocess.Popen(cmd, **popen_kwargs)
        logger.info(f"[WATCHDOG] spawn: {' '.join(cmd)}")
        return True
    except Exception as e:
        logger.warning(f"[MANAGER] Restart spawn hatası: {e}")
        return False


def _start_watchdog(port: int) -> threading.Thread:
    thread = threading.Thread(target=_watchdog_loop, args=(port,),
                              daemon=True, name="manager-watchdog")
    thread.start()
    return thread


def _watchdog_loop(port: int) -> None:
    monitor = HysteresisMonitor(_WATCHDOG_DEGRADE_THRESHOLD, _WATCHDOG_UNRESPONSIVE_THRESHOLD)
    limiter = RestartLimiter(_WATCHDOG_MAX_RESTARTS, _WATCHDOG_RESTART_WINDOW_SECONDS)
    _set_manager_state(STATE_RUNNING, "watchdog started")
    while not STOP.is_set():
        time.sleep(_WATCHDOG_POLL_SECONDS)
        healthy = manager_healthy("127.0.0.1", port, timeout=_WATCHDOG_HEALTH_TIMEOUT)
        state = monitor.report(healthy)
        _set_manager_state(
            state,
            f"health={'ok' if healthy else 'fail'} (#{monitor.consecutive_failures})",
        )
        if state == STATE_UNRESPONSIVE:
            _set_manager_state(
                STATE_RESTARTING,
                f"{_WATCHDOG_UNRESPONSIVE_THRESHOLD} ardışık health hatası",
            )
            if limiter.record_restart():
                logger.error("[WATCHDOG] UNRESPONSIVE tespit edildi → RESTARTING (spawn + kapanış)")
                _restart_manager_for_settings()
            else:
                _set_manager_state(
                    STATE_CRASHED,
                    "restart limiti aşıldı — dış supervisor gerekli (TV UI / Task Scheduler)",
                )
                logger.error("[WATCHDOG] restart limiti aşıldı → CRASHED, otomatik restart durduruldu")
            return


# ---------------------------------------------------------------------------
# Shutdown / health
# ---------------------------------------------------------------------------
def _trigger_shutdown():
    logger.info("[MANAGER] Arrêt demandé")
    try:
        shutdown_downloads()
    except Exception as e:
        logger.warning(f"[MANAGER] shutdown_downloads: {e}")
    try:
        cancel_all_downloads()
    except Exception as e:
        logger.warning(f"[MANAGER] cancel_all_downloads: {e}")
    STOP.set()

    httpd = getattr(rgsx_web, "CURRENT_HTTPD", None)
    if httpd is not None:
        try:
            threading.Thread(target=httpd.shutdown, daemon=True).start()
        except Exception:
            pass

    global _TRAY_ICON
    if _TRAY_ICON is not None:
        try:
            _TRAY_ICON.stop()
        except Exception:
            pass


def manager_healthy(host: str = "127.0.0.1", port: int = 5000, timeout: float = 2.0) -> bool:
    try:
        with urllib.request.urlopen(f"http://{host}:{port}/api/health", timeout=timeout) as resp:
            if resp.status != 200:
                return False
            data = json.loads(resp.read().decode("utf-8"))
            return bool(data.get("success") and data.get("manager"))
    except Exception:
        return False


def _is_port_free(port: int, host: str = "0.0.0.0") -> bool:
    """Delegué à qbittorrent_backend (Faz 3 — tek ortak implementasyon)."""
    from qbittorrent_backend import _is_port_free as _qbt_is_port_free
    return _qbt_is_port_free(port, host)


def _find_available_port(preferred: int, host: str = "0.0.0.0", max_attempts: int = 100) -> int:
    """Delegué à qbittorrent_backend (Faz 3 — tek ortak implementasyon)."""
    from qbittorrent_backend import _find_available_port as _qbt_find_available_port
    return _qbt_find_available_port(preferred, host, max_attempts)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="RGSX Download Manager")
    parser.add_argument("--host", default=None, help="Adresse d'écoute (défaut: rgsx_settings.json)")
    parser.add_argument("--port", type=int, default=None, help="Port HTTP (défaut: rgsx_settings.json)")
    parser.add_argument("--no-tray", action="store_true", help="Désactiver l'icône système")
    parser.add_argument("--minimized", action="store_true", help="Lancé en arrière-plan (auto-start)")
    parser.add_argument("--auto-start-install", action="store_true", help="Installer le démarrage auto puis quitter")
    parser.add_argument("--auto-start-remove", action="store_true", help="Supprimer le démarrage auto puis quitter")
    args = parser.parse_args()

    # CLI argümanı verilmediyse kalıcı ayarlardan oku (Sunucu Ayarları penceresi yazıyor).
    from rgsx_settings import get_manager_port, get_manager_host
    args.port = args.port if args.port is not None else get_manager_port()
    args.host = args.host if args.host else get_manager_host()

    if args.auto_start_install:
        ok = autostart_install()
        _set_autostart_pref(True)
        print("RGSX Manager: auto-start installed" if ok else "RGSX Manager: auto-start install FAILED")
        return 0 if ok else 1
    if args.auto_start_remove:
        ok = autostart_remove()
        _set_autostart_pref(False)
        print("RGSX Manager: auto-start removed" if ok else "RGSX Manager: auto-start remove FAILED")
        return 0 if ok else 1

    if manager_healthy("127.0.0.1", args.port):
        logger.info(f"[MANAGER] Un manager est déjà actif sur le port {args.port}")
        print(f"RGSX Manager already running on http://localhost:{args.port}")
        return 0

    # Faz 4: port doluysa (sağlıklı RGSX manager değil, başka bir process) 5000+N dene.
    final_port = _find_available_port(args.port, args.host or "0.0.0.0")
    if final_port == 0:
        logger.error(f"[MANAGER] Aucun port disponible à partir de {args.port} (100 essais)")
        print(f"RGSX Manager: aucun port disponible à partir de {args.port}")
        return 1
    if final_port != args.port:
        logger.info(f"[MANAGER] Port {args.port} occupé → utilisation de {final_port}")
        from rgsx_settings import set_manager_port
        try:
            set_manager_port(final_port)
        except Exception as e:
            logger.warning(f"[MANAGER] set_manager_port({final_port}) échec: {e}")
        args.port = final_port

    # Auto-start par défaut: si l'utilisateur ne l'a pas désactivé, l'installer au boot
    if os.name == "nt" and _get_autostart_pref() and not is_autostart_enabled():
        if autostart_install():
            logger.info("[MANAGER] Auto-start activé (préférence par défaut)")

    logger.info("=" * 60)
    logger.info("[MANAGER] RGSX Download Manager démarre")
    logger.info(f"[MANAGER] http://localhost:{args.port}")
    logger.info("=" * 60)

    config.queue_worker_running = True  # Faz 9: batch endpoint'i standalone kick'ini atlar
    threading.Thread(target=download_queue_worker, daemon=True, name="queue-worker").start()
    threading.Thread(target=_broadcaster_loop, daemon=True, name="sse-broadcaster").start()
    _start_watchdog(args.port)

    # Faz 8: indirme state makinesi SSE yayınını bu sürecin broadcast'ine bağla
    # (download_state.emit_state_event -> 'download_state' SSE event tipi).
    try:
        from network.download_state import set_state_emitter
        set_state_emitter(_broadcast)
    except Exception as e:
        logger.debug(f"[MANAGER] state emitter kaydı atlandı: {e}")

    # qBittorrent WebUI şifresini daha ilk açılışta güvence altına al: settings'te
    # güvenli şifre yoksa rastgele üretilip kaydedilir (spawn'da setPreferences ile
    # uygulanır). Böylece 'varsayılan şifre kullanımda' durumu pratikte oluşmaz.
    try:
        from qbittorrent_backend import ensure_qbittorrent_password_secured
        ensure_qbittorrent_password_secured()
    except Exception as e:
        logger.debug(f"[MANAGER] qBittorrent şifre güvence atlandı: {e}")

    try:
        _resume_interrupted_downloads()
    except Exception as e:
        logger.warning(f"[MANAGER] _resume_interrupted_downloads: {e}")

    icon_path = os.path.join(_APP_DIR, "assets", "images", "favicon_rgsx.ico")
    if not args.no_tray:
        _setup_tray(icon_path, args.port, no_tray=False)

    # Faz 10c/1: Rust `manager-bin` sidecar torrent daemon (RGSX_RUST_DAEMON flag'iyle;
    # binary yoksa Python-only devam eder). Sağlık süpervizörü ayrı thread'te izler.
    try:
        from rust_daemon import start as _rust_start, supervisor as _rust_supervisor
        _rust_proc = _rust_start()
        if _rust_proc is not None:
            threading.Thread(
                target=_rust_supervisor, daemon=True, name="rust-daemon-supervisor"
            ).start()
    except Exception as e:
        logger.debug(f"[MANAGER] rust daemon başlatma atlandı: {e}")

    try:
        rgsx_web.run_server(host=args.host, port=args.port, handler_class=ManagerHandler,
                            kill_conflicts=False)
    finally:
        STOP.set()
        httpd = getattr(rgsx_web, "CURRENT_HTTPD", None)
        if httpd is not None:
            try:
                httpd.server_close()
            except Exception:
                pass
        global _TRAY_ICON
        if _TRAY_ICON is not None:
            try:
                _TRAY_ICON.stop()
            except Exception:
                pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
