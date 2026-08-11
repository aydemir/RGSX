"""rgsx_manager.py davranış testleri (TASK-001 — Faz 7 kapsam artırma).

Hedef: modül kapsamını %30 -> %55'e taşımak. Handler endpoint'leri gerçek
soket olmadan çağrılır; Windows-only yollar (winreg / pystray / autostart /
watchdog spawn) sahte modüllerle sürülür; süreç başlatma ve ağ çağrısı yapılmaz.
"""

import email
import io
import json
import os
import queue as queue_module
import sys
import time
import types

import pytest

import config
import rgsx_manager
import rgsx_settings
import rgsx_web
import utils
from rgsx_manager import ManagerHandler
from watchdog import STATE_CRASHED, STATE_INIT, STATE_RESTARTING, STATE_RUNNING


# ---------------------------------------------------------------------------
# Helpers (test_api_contract.py ile aynı desen — handler'lar soketsiz çağrılır)
# ---------------------------------------------------------------------------

class _FakeServer:
    server_version = "TestHTTP/1.0"
    sys_version = ""


def make_handler(handler_cls, path, method="GET", body=b"", extra_headers=None):
    handler = object.__new__(handler_cls)
    handler.command = method
    handler.requestline = f"{method} {path} HTTP/1.1"
    handler.path = path
    handler.request_version = "HTTP/1.1"
    handler.protocol_version = "HTTP/1.1"
    handler.server = _FakeServer()
    handler.rfile = io.BytesIO(body)
    handler.wfile = io.BytesIO()
    headers = ["Host: test", f"Content-Length: {len(body)}"]
    if extra_headers:
        headers.extend(extra_headers)
    handler.headers = email.message_from_string("\n".join(headers))
    return handler


def invoke(handler_cls, path, method="GET", body=b"", extra_headers=None):
    handler = make_handler(handler_cls, path, method, body, extra_headers)
    (handler.do_GET if method == "GET" else handler.do_POST)()
    raw = handler.wfile.getvalue()
    head, _, payload = raw.partition(b"\r\n\r\n")
    status_line = head.split(b"\r\n", 1)[0]
    status = int(status_line.split(b" ", 2)[1])
    return status, head, payload


def post_json(handler_cls, path, data):
    return invoke(handler_cls, path, method="POST", body=json.dumps(data).encode("utf-8"))


def as_json(payload):
    return json.loads(payload.decode("utf-8"))


@pytest.fixture
def isolated(tmp_path, monkeypatch):
    """Tüm config dosya/save yolları tmp_path'e yönlendirilir."""
    monkeypatch.setattr(config, "SAVE_FOLDER", str(tmp_path))
    monkeypatch.setattr(config, "HISTORY_PATH", str(tmp_path / "history.json"))
    monkeypatch.setattr(config, "RGSX_SETTINGS_PATH", str(tmp_path / "rgsx_settings.json"))
    monkeypatch.setattr(config, "DOWNLOADED_GAMES_PATH", str(tmp_path / "downloaded_games.json"))
    monkeypatch.setattr(config, "CONTROLS_CONFIG_PATH", str(tmp_path / "controls.json"))
    monkeypatch.setattr(config, "SOURCES_FILE", str(tmp_path / "systems_list.json"))
    monkeypatch.setattr(config, "GAMES_FOLDER", str(tmp_path / "games"))
    monkeypatch.setattr(config, "IMAGES_FOLDER", str(tmp_path / "images"))
    monkeypatch.setattr(config, "ROMS_FOLDER", str(tmp_path / "roms"))
    monkeypatch.setattr(config, "download_queue", [])
    monkeypatch.setattr(config, "download_active", False)
    monkeypatch.setattr(config, "download_progress", {})
    monkeypatch.setattr(config, "history", [])
    monkeypatch.setattr(config, "downloaded_games", {})
    return tmp_path


class _FakeGame:
    def __init__(self, name, url):
        self.name = name
        self.url = url


# ---------------------------------------------------------------------------
# Endpoint yüzeyi
# ---------------------------------------------------------------------------

class TestManagerEndpoints:
    def test_get_unknown_path_404(self, isolated):
        status, _, payload = invoke(ManagerHandler, "/api/does-not-exist")
        assert status == 404
        body = as_json(payload)
        assert body["success"] is False

    def test_password_status_error_500(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "get_password_status",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        status, _, payload = invoke(ManagerHandler, "/api/qbittorrent/password-status")
        assert status == 500
        assert as_json(payload)["success"] is False

    def test_batch_malformed_json_400(self, isolated):
        status, _, payload = invoke(
            ManagerHandler, "/api/download/batch", method="POST", body=b"{not json"
        )
        assert status == 400
        assert as_json(payload)["success"] is False

    def test_qbittorrent_start_error_500(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "ensure_running",
            lambda timeout=30: (_ for _ in ()).throw(RuntimeError("no qbt")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/start", {})
        assert status == 500
        assert as_json(payload)["success"] is False

    def test_qbittorrent_change_password_exception_500(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "change_webui_password",
            lambda pw: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/change-password", {"password": "x"})
        assert status == 500

    def test_qbittorrent_change_password_empty_body_400(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(qbittorrent_backend, "change_webui_password", lambda pw: (False, "password_too_short"))
        status, _, payload = invoke(
            ManagerHandler, "/api/qbittorrent/change-password", method="POST", body=b"{}"
        )
        assert status == 400
        assert as_json(payload)["message"] == "password_too_short"

    def test_qbittorrent_regenerate_password_ok(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "regenerate_qbittorrent_password",
            lambda: (True, "N3wR4nd0m"),
        )
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/regenerate-password", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["password"] == "N3wR4nd0m"

    def test_qbittorrent_regenerate_password_failure_500(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "regenerate_qbittorrent_password",
            lambda: (False, None),
        )
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/regenerate-password", {})
        assert status == 500
        assert as_json(payload)["message"] == "password_regeneration_failed"

    def test_qbittorrent_regenerate_password_exception_500(self, isolated, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "regenerate_qbittorrent_password",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/regenerate-password", {})
        assert status == 500

    def test_pause_exception_500(self, isolated, monkeypatch):
        monkeypatch.setattr(
            rgsx_manager, "pause_all_downloads",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/pause", {})
        assert status == 500

    def test_resume_exception_500(self, isolated, monkeypatch):
        monkeypatch.setattr(
            rgsx_manager, "resume_all_downloads",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/resume", {})
        assert status == 500


class TestDownloadWorker:
    def _setup(self, monkeypatch, games):
        monkeypatch.setattr(rgsx_manager, "get_cached_games", lambda platform: (games, None, None))
        monkeypatch.setattr(
            utils, "check_extension_before_download",
            lambda url, platform, name: (url, platform, name, False),
        )
        monkeypatch.setattr(rgsx_manager, "get_clean_display_name", lambda name, platform: name)

    def test_malformed_json_400(self, isolated):
        status, _, payload = invoke(
            ManagerHandler, "/api/download", method="POST", body=b"{not json"
        )
        assert status == 400
        assert as_json(payload)["success"] is False

    def test_direct_url_extension_check_fails_400(self, isolated, monkeypatch):
        monkeypatch.setattr(utils, "check_extension_before_download", lambda *a: None)
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "url": "https://ex.invalid/bad.exe", "game_name": "Rom", "platform": "NES",
        })
        assert status == 400
        assert "Extension" in as_json(payload)["error"]

    def test_by_index_success(self, isolated, monkeypatch):
        self._setup(monkeypatch, [_FakeGame("Alpha", "https://ex.invalid/a.zip")])
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "platform": "NES", "game_index": 0,
        })
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["game_name"] == "Alpha"
        assert config.download_queue[0]["game_name"] == "Alpha"
        assert config.download_queue[0]["url"] == "https://ex.invalid/a.zip"

    def test_by_name_success(self, isolated, monkeypatch):
        self._setup(monkeypatch, [
            _FakeGame("Alpha", "https://ex.invalid/a.zip"),
            _FakeGame("Beta", "https://ex.invalid/b.zip"),
        ])
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "platform": "NES", "game_name": "Beta",
        })
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["game_name"] == "Beta"

    def test_by_name_not_found_400(self, isolated, monkeypatch):
        self._setup(monkeypatch, [_FakeGame("Alpha", "https://ex.invalid/a.zip")])
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "platform": "NES", "game_name": "Ghost",
        })
        assert status == 400
        assert "Jeu non trouvé" in as_json(payload)["error"]

    def test_game_without_url_400(self, isolated, monkeypatch):
        self._setup(monkeypatch, [_FakeGame("Broken", None)])
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "platform": "NES", "game_index": 0,
        })
        assert status == 400
        assert as_json(payload)["success"] is False

    def test_check_extension_fail_400(self, isolated, monkeypatch):
        self._setup(monkeypatch, [_FakeGame("Alpha", "https://ex.invalid/a.zip")])
        monkeypatch.setattr(utils, "check_extension_before_download", lambda *a: None)
        status, _, payload = post_json(ManagerHandler, "/api/download", {
            "platform": "NES", "game_index": 0,
        })
        assert status == 400


class TestCancelWorker:
    def test_malformed_json_400(self, isolated):
        status, _, payload = invoke(ManagerHandler, "/api/cancel", method="POST", body=b"{bad")
        assert status == 400

    def test_missing_url_400(self, isolated):
        status, _, payload = post_json(ManagerHandler, "/api/cancel", {})
        assert status == 400
        assert as_json(payload)["error"] == "Paramètre manquant: url requis"

    def test_cancel_success(self, isolated, monkeypatch):
        history = [{
            "platform": "NES", "game_name": "Rom", "status": "Downloading",
            "url": "https://ex.invalid/a.zip", "task_id": "t1",
        }]
        monkeypatch.setattr(rgsx_manager, "load_history", lambda: list(history))
        monkeypatch.setattr(rgsx_manager, "save_history", lambda h: None)
        canceled = []
        monkeypatch.setattr(rgsx_manager, "request_cancel", lambda task_id: canceled.append(task_id))
        status, _, payload = post_json(ManagerHandler, "/api/cancel", {"url": "https://ex.invalid/a.zip"})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["task_id"] == "t1"
        assert canceled == ["t1"]

    def test_cancel_no_matching_entry(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "load_history", lambda: [])
        monkeypatch.setattr(rgsx_manager, "save_history", lambda h: None)
        status, _, payload = post_json(ManagerHandler, "/api/cancel", {"url": "https://x.invalid/none.zip"})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["task_id"] is None

    def test_cancel_error_500(self, isolated, monkeypatch):
        monkeypatch.setattr(
            rgsx_manager, "load_history",
            lambda: (_ for _ in ()).throw(OSError("locked")),
        )
        status, _, payload = post_json(ManagerHandler, "/api/cancel", {"url": "https://x.invalid/a.zip"})
        assert status == 500


class TestSse:
    def test_empty_timeout_sends_snapshot(self, isolated, monkeypatch):
        class _EmptyQ:
            def put_nowait(self, item):
                pass

            def get(self, timeout=0):
                raise queue_module.Empty

        monkeypatch.setattr(rgsx_manager.queue_module, "Queue", lambda: _EmptyQ())
        n = {"v": 0}

        def stop_is_set():
            n["v"] += 1
            return n["v"] > 1

        monkeypatch.setattr(rgsx_manager.STOP, "is_set", stop_is_set)
        status, head, payload = invoke(ManagerHandler, "/api/events")
        assert status == 200
        assert payload.count(b"event: snapshot") == 2

    def test_broadcaster_loop_broadcasts_diffs(self, isolated, monkeypatch):
        calls = []

        def fake_broadcast(event_type, data=None):
            calls.append((event_type, data))

        monkeypatch.setattr(rgsx_manager, "_broadcast", fake_broadcast)
        monkeypatch.setattr(rgsx_manager.time, "sleep", lambda s: None)
        clock = {"t": 0.0}

        def fake_time():
            clock["t"] += 40.0
            return clock["t"]

        monkeypatch.setattr(rgsx_manager.time, "time", fake_time)
        n = {"v": 0}

        def stop_is_set():
            n["v"] += 1
            return n["v"] > 1

        monkeypatch.setattr(rgsx_manager.STOP, "is_set", stop_is_set)
        rgsx_manager._broadcaster_loop()
        types_seen = {t for t, _ in calls}
        assert {"history", "queue", "progress", "downloaded", "snapshot"} <= types_seen


# ---------------------------------------------------------------------------
# Auto-start (Windows registry)
# ---------------------------------------------------------------------------

class _FakeWinreg:
    HKEY_CURRENT_USER = object()
    KEY_SET_VALUE = 0x2
    REG_SZ = 1

    def __init__(self):
        self.open_error = None
        self.query_result = ("value", 1)
        self.delete_error = None
        self.set_calls = []
        self.delete_calls = []

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False

    def OpenKey(self, key, subkey, *a, **k):
        if self.open_error:
            raise self.open_error
        return self

    def QueryValueEx(self, key, name):
        if self.query_result is None:
            raise OSError("not found")
        return self.query_result

    def SetValueEx(self, key, name, res, typ, value):
        self.set_calls.append((name, value))

    def DeleteValue(self, key, name):
        if self.delete_error:
            raise self.delete_error
        self.delete_calls.append(name)


@pytest.fixture
def winreg_fake(monkeypatch):
    fake = _FakeWinreg()
    monkeypatch.setitem(sys.modules, "winreg", fake)
    return fake


@pytest.fixture
def win_platform(monkeypatch):
    monkeypatch.setattr(os, "name", "nt")


class TestAutostart:
    def test_command_posix(self):
        cmd = rgsx_manager._autostart_command()
        assert cmd.startswith('"')
        assert cmd.endswith("--minimized")
        assert "--minimized" in cmd

    def test_command_windows_uses_pythonw(self, win_platform, monkeypatch):
        monkeypatch.setattr(rgsx_manager.os.path, "exists", lambda p: p.endswith("w.exe"))
        monkeypatch.setattr(sys, "executable", "C:/Python/python.exe")
        cmd = rgsx_manager._autostart_command()
        assert "pythonw.exe" in cmd
        assert "--minimized" in cmd

    def test_is_enabled_non_windows_false(self):
        assert rgsx_manager.is_autostart_enabled() is False

    def test_is_enabled_windows_true(self, win_platform, winreg_fake):
        assert rgsx_manager.is_autostart_enabled() is True

    def test_is_enabled_windows_missing_key(self, win_platform, winreg_fake):
        winreg_fake.query_result = None
        assert rgsx_manager.is_autostart_enabled() is False

    def test_install_non_windows_false(self):
        assert rgsx_manager.autostart_install() is False

    def test_install_windows_success(self, win_platform, winreg_fake):
        assert rgsx_manager.autostart_install() is True
        assert winreg_fake.set_calls[0][0] == rgsx_manager._AUTOSTART_NAME

    def test_install_windows_error(self, win_platform, winreg_fake):
        winreg_fake.open_error = RuntimeError("denied")
        assert rgsx_manager.autostart_install() is False

    def test_remove_non_windows_false(self):
        assert rgsx_manager.autostart_remove() is False

    def test_remove_windows_success(self, win_platform, winreg_fake):
        assert rgsx_manager.autostart_remove() is True
        assert rgsx_manager._AUTOSTART_NAME in winreg_fake.delete_calls

    def test_remove_windows_missing_key(self, win_platform, winreg_fake):
        winreg_fake.delete_error = FileNotFoundError("no key")
        assert rgsx_manager.autostart_remove() is False

    def test_remove_windows_error(self, win_platform, winreg_fake):
        winreg_fake.delete_error = OSError("denied")
        assert rgsx_manager.autostart_remove() is False

    def test_get_pref(self, monkeypatch):
        monkeypatch.setattr(rgsx_settings, "get_autostart_on_boot", lambda: False)
        assert rgsx_manager._get_autostart_pref() is False

    def test_get_pref_error_default_true(self, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "get_autostart_on_boot",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert rgsx_manager._get_autostart_pref() is True

    def test_set_pref(self, monkeypatch):
        recorded = []
        monkeypatch.setattr(rgsx_settings, "set_autostart_on_boot", lambda v: recorded.append(v) or v)
        assert rgsx_manager._set_autostart_pref(False) is False
        assert recorded == [False]

    def test_set_pref_error_returns_enabled(self, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "set_autostart_on_boot",
            lambda v: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert rgsx_manager._set_autostart_pref(True) is True


# ---------------------------------------------------------------------------
# System tray
# ---------------------------------------------------------------------------

class _FakeMenuItem:
    _instances = []

    def __init__(self, text, action=None, default=False, checked=None):
        self.text = text
        self.action = action
        self.checked = checked
        _FakeMenuItem._instances.append(self)


class _FakeMenu:
    SEPARATOR = object()

    def __init__(self, *items):
        self.items = items


class _FakeIcon:
    _instances = []

    def __init__(self, *args, **kwargs):
        self.args = args
        self.kwargs = kwargs
        self.notifications = []
        self.stopped = False
        _FakeIcon._instances.append(self)

    def run_detached(self):
        pass

    def stop(self):
        self.stopped = True

    def notify(self, msg, title=""):
        self.notifications.append((msg, title))


class _FakeImage:
    @staticmethod
    def open(path):
        raise FileNotFoundError(path)


class _FakePystrayModule:
    Menu = _FakeMenu
    MenuItem = _FakeMenuItem
    Icon = _FakeIcon


@pytest.fixture
def tray_env(monkeypatch, isolated):
    import language as lang

    monkeypatch.setitem(sys.modules, "pystray", _FakePystrayModule())
    monkeypatch.setitem(sys.modules, "PIL", types.SimpleNamespace(Image=_FakeImage))
    # Tray menü etiketleri yüklü dile bağlıdır; sabit 'en' etiketleri kullan.
    monkeypatch.setattr(lang, "translations", {})
    monkeypatch.setattr(lang, "current_language", "en")
    lang.load_language("en")
    _FakeMenuItem._instances.clear()
    _FakeIcon._instances.clear()
    monkeypatch.setattr(rgsx_manager, "_TRAY_ICON", None)
    return _FakeIcon._instances


def _menu_item(text):
    for item in _FakeMenuItem._instances:
        if item.text == text:
            return item
    raise AssertionError(f"menu item not found: {text}")


class TestTray:
    def test_no_tray_returns_none(self, isolated):
        assert rgsx_manager._setup_tray("icon.ico", 5000, no_tray=True) is None

    def test_missing_library_returns_none(self, isolated):
        assert rgsx_manager._setup_tray("icon.ico", 5000) is None

    def test_tray_starts(self, isolated, tray_env):
        icon = rgsx_manager._setup_tray("missing.ico", 5000)
        assert icon is not None
        assert rgsx_manager._TRAY_ICON is icon

    def test_open_web_ui(self, isolated, tray_env, monkeypatch):
        opened = []
        monkeypatch.setattr(rgsx_manager.webbrowser, "open", lambda url: opened.append(url))
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Open Web UI").action(icon, None)
        assert opened == ["http://localhost:5000"]

    def test_open_settings(self, isolated, tray_env, monkeypatch):
        opened = []
        monkeypatch.setattr(rgsx_manager.webbrowser, "open", lambda url: opened.append(url))
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Settings").action(_FakeIcon._instances[-1], None)
        assert opened == ["http://localhost:5000/settings"]

    def test_open_downloads_missing_folder_notifies(self, isolated, tray_env):
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Downloads folder").action(icon, None)
        assert icon.notifications == [("Downloads folder not found", "RGSX")]

    def test_open_downloads_posix(self, isolated, tray_env, monkeypatch):
        import config as cfg

        folder = isolated / "roms"
        folder.mkdir()
        monkeypatch.setattr(cfg, "ROMS_FOLDER", str(folder))
        opened = []
        monkeypatch.setattr(rgsx_manager.webbrowser, "open", lambda url: opened.append(url))
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Downloads folder").action(_FakeIcon._instances[-1], None)
        assert opened == [str(folder)]

    def test_open_downloads_windows(self, isolated, tray_env, monkeypatch, win_platform):
        import config as cfg

        folder = isolated / "roms"
        folder.mkdir()
        monkeypatch.setattr(cfg, "ROMS_FOLDER", str(folder))
        started = []
        monkeypatch.setattr(rgsx_manager.os, "startfile", lambda p: started.append(p), raising=False)
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Downloads folder").action(_FakeIcon._instances[-1], None)
        assert started == [str(folder)]

    def test_open_logs_missing_folder_notifies(self, isolated, tray_env, monkeypatch):
        import config as cfg

        monkeypatch.setattr(cfg, "log_dir", str(isolated / "no-such-logs"))
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Logs folder").action(icon, None)
        assert icon.notifications == [("Logs folder not found", "RGSX")]

    def test_open_logs_posix(self, isolated, tray_env, monkeypatch):
        import config as cfg

        monkeypatch.setattr(cfg, "log_dir", str(isolated))
        opened = []
        monkeypatch.setattr(rgsx_manager.webbrowser, "open", lambda url: opened.append(url))
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Logs folder").action(_FakeIcon._instances[-1], None)
        assert opened == [str(isolated)]

    def test_toggle_autostart_enable(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_autostart_enabled", lambda: False)
        installed = []
        monkeypatch.setattr(rgsx_manager, "autostart_install", lambda: installed.append(1) or True)
        prefs = []
        monkeypatch.setattr(rgsx_manager, "_set_autostart_pref", lambda v: prefs.append(v) or v)
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Auto-start on boot").action(icon, None)
        assert installed == [1]
        assert prefs == [True]

    def test_toggle_autostart_disable(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_autostart_enabled", lambda: True)
        removed = []
        monkeypatch.setattr(rgsx_manager, "autostart_remove", lambda: removed.append(1) or True)
        prefs = []
        monkeypatch.setattr(rgsx_manager, "_set_autostart_pref", lambda v: prefs.append(v) or v)
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Auto-start on boot").action(icon, None)
        assert removed == [1]
        assert prefs == [False]

    def test_autostart_checked_callback(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_autostart_enabled", lambda: True)
        rgsx_manager._setup_tray("x.ico", 5000)
        item = _menu_item("Auto-start on boot")
        assert item.checked(None) is True

    def test_toggle_pause_all_resumes(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_any_download_paused", lambda: True)
        monkeypatch.setattr(rgsx_manager, "resume_all_downloads", lambda: 2)
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Pause/Resume Downloads").action(icon, None)
        assert icon.notifications == [("2 indirme sürdürüldü", "RGSX")]

    def test_toggle_pause_all_pauses(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_any_download_paused", lambda: False)
        monkeypatch.setattr(rgsx_manager, "pause_all_downloads", lambda: 3)
        rgsx_manager._setup_tray("x.ico", 5000)
        icon = _FakeIcon._instances[-1]
        _menu_item("Pause/Resume Downloads").action(icon, None)
        assert icon.notifications == [("3 indirme durduruldu", "RGSX")]

    def test_toggle_pause_checked(self, isolated, tray_env, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "is_any_download_paused", lambda: False)
        rgsx_manager._setup_tray("x.ico", 5000)
        item = _menu_item("Pause/Resume Downloads")
        assert item.checked(None) is False

    def test_quit_triggers_shutdown(self, isolated, tray_env, monkeypatch):
        shutdown = []
        monkeypatch.setattr(rgsx_manager, "_trigger_shutdown", lambda: shutdown.append(1))
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Quit RGSX").action(_FakeIcon._instances[-1], None)
        assert shutdown == [1]

    def test_server_settings_saved(self, isolated, tray_env, monkeypatch):
        captured = {}
        monkeypatch.setattr(
            rgsx_manager, "open_server_settings_dialog",
            lambda **k: captured.update(k),
        )
        ports, hosts, autostarts = [], [], []
        monkeypatch.setattr(rgsx_settings, "set_manager_port", lambda p: ports.append(p) or p)
        monkeypatch.setattr(rgsx_settings, "set_manager_host", lambda h: hosts.append(h) or h)
        monkeypatch.setattr(rgsx_settings, "set_autostart_on_boot", lambda v: autostarts.append(v) or v)
        prefs = []
        monkeypatch.setattr(rgsx_manager, "_set_autostart_pref", lambda v: prefs.append(v) or v)
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Server Settings...").action(_FakeIcon._instances[-1], None)
        on_save = captured["on_save"]
        get_current = captured["get_current"]

        assert get_current() == {
            "port": 5000, "host": "0.0.0.0", "autostart": True,
        }

        on_save(None)
        assert ports == []

        on_save({"port": 8080, "host": "127.0.0.1", "autostart": False})
        assert ports == [8080]
        assert hosts == ["127.0.0.1"]
        assert autostarts == [False]
        assert prefs == [False]

    def test_server_settings_saved_with_restart(self, isolated, tray_env, monkeypatch):
        captured = {}
        monkeypatch.setattr(
            rgsx_manager, "open_server_settings_dialog",
            lambda **k: captured.update(k),
        )
        restarts = []
        monkeypatch.setattr(rgsx_manager, "_restart_manager_for_settings", lambda: restarts.append(1))

        class _FakeThread:
            def __init__(self, *a, **k):
                self.kwargs = k

            def start(self):
                self.kwargs["target"]()

        monkeypatch.setattr(rgsx_manager.threading, "Thread", _FakeThread)
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Server Settings...").action(_FakeIcon._instances[-1], None)
        captured["on_save"]({"port": 8080, "host": "0.0.0.0", "autostart": True, "restart": True})
        assert restarts == [1]

    def test_server_settings_save_error(self, isolated, tray_env, monkeypatch):
        captured = {}
        monkeypatch.setattr(
            rgsx_manager, "open_server_settings_dialog",
            lambda **k: captured.update(k),
        )
        monkeypatch.setattr(
            rgsx_settings, "set_manager_port",
            lambda p: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        rgsx_manager._setup_tray("x.ico", 5000)
        _menu_item("Server Settings...").action(_FakeIcon._instances[-1], None)
        captured["on_save"]({"port": 8080, "host": "0.0.0.0", "autostart": True})

    def test_tray_start_error_sets_none(self, isolated, tray_env, monkeypatch):
        class _BoomIcon:
            def __init__(self, *a, **k):
                pass

            def run_detached(self):
                raise RuntimeError("no tray server")

        class _BoomModule:
            Menu = _FakeMenu
            MenuItem = _FakeMenuItem
            Icon = _BoomIcon

        monkeypatch.setitem(sys.modules, "pystray", _BoomModule())
        icon = rgsx_manager._setup_tray("x.ico", 5000)
        assert icon is None
        assert rgsx_manager._TRAY_ICON is None


# ---------------------------------------------------------------------------
# Restart / resume / shutdown
# ---------------------------------------------------------------------------

class TestRestartManager:
    def test_restart_success(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_manager.time, "sleep", lambda s: None)
        spawned = []
        monkeypatch.setattr(rgsx_manager, "_spawn_manager", lambda args: spawned.append(args) or True)
        shut = []
        monkeypatch.setattr(rgsx_manager, "_trigger_shutdown", lambda: shut.append(1))
        rgsx_manager._restart_manager_for_settings()
        assert spawned == [["--port=5000", "--host=0.0.0.0"]]
        assert shut == [1]

    def test_restart_spawn_failure_skips_shutdown(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_manager.time, "sleep", lambda s: None)
        monkeypatch.setattr(rgsx_manager, "_spawn_manager", lambda args: False)
        shut = []
        monkeypatch.setattr(rgsx_manager, "_trigger_shutdown", lambda: shut.append(1))
        rgsx_manager._restart_manager_for_settings()
        assert shut == []

    def test_restart_exception(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_manager.time, "sleep", lambda s: None)
        monkeypatch.setattr(
            rgsx_settings, "get_manager_port",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        shut = []
        monkeypatch.setattr(rgsx_manager, "_trigger_shutdown", lambda: shut.append(1))
        rgsx_manager._restart_manager_for_settings()
        assert shut == []


class TestResumeInterrupted:
    def test_load_error_returns_zero(self, monkeypatch, isolated):
        monkeypatch.setattr(
            rgsx_manager, "load_history",
            lambda: (_ for _ in ()).throw(OSError("boom")),
        )
        assert rgsx_manager._resume_interrupted_downloads() == 0

    def test_no_interrupted_returns_zero(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_manager, "load_history", lambda: [{"status": "Completed"}])
        assert rgsx_manager._resume_interrupted_downloads() == 0

    def test_requeues_interrupted(self, monkeypatch, isolated):
        history = [
            {"platform": "NES", "game_name": "A", "status": "Downloading",
             "url": "https://x.invalid/a.zip", "task_id": "t1"},
            {"platform": "SNES", "game_name": "B", "status": "Téléchargement",
             "url": "https://x.invalid/b.zip"},
            {"platform": "N64", "game_name": "C", "status": "Paused",
             "url": None},
            {"platform": "GB", "game_name": "D", "status": "Completed",
             "url": "https://x.invalid/d.zip"},
        ]
        monkeypatch.setattr(rgsx_manager, "load_history", lambda: list(history))
        saved = []
        monkeypatch.setattr(rgsx_manager, "save_history", lambda h: saved.append(h))
        count = rgsx_manager._resume_interrupted_downloads()
        assert count == 2
        assert len(config.download_queue) == 2
        assert config.download_queue[0]["url"] == "https://x.invalid/a.zip"
        assert config.download_queue[1]["task_id"].startswith("resume_")
        assert saved[0][0]["status"] == "Queued"

    def test_save_error_still_returns_count(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_manager, "load_history", lambda: [{
            "platform": "NES", "game_name": "A", "status": "Paused",
            "url": "https://x.invalid/a.zip",
        }])
        monkeypatch.setattr(
            rgsx_manager, "save_history",
            lambda h: (_ for _ in ()).throw(OSError("boom")),
        )
        assert rgsx_manager._resume_interrupted_downloads() == 1


class TestTriggerShutdown:
    def test_shutdown_sequence(self, monkeypatch, isolated):
        stopped = []
        monkeypatch.setattr(rgsx_manager.STOP, "set", lambda: stopped.append(1))
        monkeypatch.setattr(
            rgsx_manager, "shutdown_downloads",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        monkeypatch.setattr(
            rgsx_manager, "cancel_all_downloads",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )

        httpd_shutdowns = []

        class _FakeHttpd:
            def shutdown(self):
                httpd_shutdowns.append(1)

        monkeypatch.setattr(rgsx_web, "CURRENT_HTTPD", _FakeHttpd())

        class _FakeThread:
            def __init__(self, *a, **k):
                self.kwargs = k

            def start(self):
                self.kwargs["target"]()

        monkeypatch.setattr(rgsx_manager.threading, "Thread", _FakeThread)

        icon = _FakeIcon()
        monkeypatch.setattr(rgsx_manager, "_TRAY_ICON", icon)
        rgsx_manager._trigger_shutdown()
        assert stopped == [1]
        assert httpd_shutdowns == [1]
        assert icon.stopped is True

    def test_shutdown_without_httpd_or_tray(self, monkeypatch, isolated):
        monkeypatch.setattr(rgsx_web, "CURRENT_HTTPD", None)
        monkeypatch.setattr(rgsx_manager, "_TRAY_ICON", None)
        monkeypatch.setattr(rgsx_manager, "shutdown_downloads", lambda: None)
        monkeypatch.setattr(rgsx_manager, "cancel_all_downloads", lambda: None)
        rgsx_manager._trigger_shutdown()


class TestManagerHealth:
    def _fake_urlopen(self, monkeypatch, response=None, exc=None):
        class _FakeResp:
            def __init__(self, status, body):
                self.status = status
                self._body = body

            def __enter__(self):
                return self

            def __exit__(self, *a):
                return False

            def read(self):
                return self._body

        def urlopen(url, timeout=2.0):
            if exc:
                raise exc
            return _FakeResp(*response)

        monkeypatch.setattr(rgsx_manager.urllib.request, "urlopen", urlopen)

    def test_healthy_true(self, monkeypatch):
        self._fake_urlopen(monkeypatch, response=(200, b'{"success": true, "manager": true}'))
        assert rgsx_manager.manager_healthy() is True

    def test_healthy_manager_false(self, monkeypatch):
        self._fake_urlopen(monkeypatch, response=(200, b'{"success": true, "manager": false}'))
        assert rgsx_manager.manager_healthy() is False

    def test_healthy_non_200(self, monkeypatch):
        self._fake_urlopen(monkeypatch, response=(500, b'{}'))
        assert rgsx_manager.manager_healthy() is False

    def test_healthy_exception(self, monkeypatch):
        self._fake_urlopen(monkeypatch, exc=OSError("refused"))
        assert rgsx_manager.manager_healthy() is False


class TestPortDelegation:
    def test_is_port_free_delegates(self, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(qbittorrent_backend, "_is_port_free", lambda port, host="0.0.0.0": False)
        assert rgsx_manager._is_port_free(5000) is False

    def test_find_available_port_delegates(self, monkeypatch):
        import qbittorrent_backend

        monkeypatch.setattr(
            qbittorrent_backend, "_find_available_port",
            lambda pref, host="0.0.0.0", max_attempts=100: pref + 5,
        )
        assert rgsx_manager._find_available_port(5000) == 5005


# ---------------------------------------------------------------------------
# Watchdog / state
# ---------------------------------------------------------------------------

class TestManagerState:
    def test_initial_state(self):
        assert rgsx_manager.get_manager_state() == STATE_INIT

    def test_set_state(self, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "MANAGER_STATE", STATE_INIT)
        rgsx_manager._set_manager_state(STATE_RUNNING, "test")
        assert rgsx_manager.get_manager_state() == STATE_RUNNING

    def test_set_same_state(self, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "MANAGER_STATE", STATE_RUNNING)
        rgsx_manager._set_manager_state(STATE_RUNNING, "test")
        assert rgsx_manager.get_manager_state() == STATE_RUNNING


class TestSpawnManager:
    def test_spawn_success(self, monkeypatch):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py"])
        popen_calls = []
        monkeypatch.setattr(
            rgsx_manager.subprocess, "Popen",
            lambda cmd, **kw: popen_calls.append((cmd, kw)) or object(),
        )
        assert rgsx_manager._spawn_manager(["--port=9000"]) is True
        cmd, kw = popen_calls[0]
        assert cmd[0] == sys.executable
        assert "--minimized" in cmd
        assert "--port=9000" in cmd

    def test_spawn_with_minimized_not_duplicated(self, monkeypatch):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py", "--minimized"])
        popen_calls = []
        monkeypatch.setattr(
            rgsx_manager.subprocess, "Popen",
            lambda cmd, **kw: popen_calls.append(cmd) or object(),
        )
        rgsx_manager._spawn_manager()
        cmd = popen_calls[0]
        assert cmd.count("--minimized") == 1

    def test_spawn_windows_creationflags(self, monkeypatch, win_platform):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py"])
        popen_calls = []
        monkeypatch.setattr(
            rgsx_manager.subprocess, "Popen",
            lambda cmd, **kw: popen_calls.append(kw) or object(),
        )
        rgsx_manager._spawn_manager()
        assert popen_calls[0]["creationflags"] == 0x08000000

    def test_spawn_failure_returns_false(self, monkeypatch):
        monkeypatch.setattr(
            rgsx_manager.subprocess, "Popen",
            lambda cmd, **kw: (_ for _ in ()).throw(OSError("no exec")),
        )
        assert rgsx_manager._spawn_manager() is False


class TestWatchdogLoop:
    def _run(self, monkeypatch, healthy, restart_action, is_set):
        monkeypatch.setattr(rgsx_manager.time, "sleep", lambda s: None)
        monkeypatch.setattr(rgsx_manager, "manager_healthy", lambda *a, **k: healthy)
        monkeypatch.setattr(rgsx_manager, "_restart_manager_for_settings", restart_action)
        monkeypatch.setattr(rgsx_manager, "MANAGER_STATE", STATE_INIT)
        monkeypatch.setattr(rgsx_manager.STOP, "is_set", is_set)
        rgsx_manager._watchdog_loop(5000)

    def test_healthy_exits(self, monkeypatch):
        n = {"v": 0}

        def is_set():
            n["v"] += 1
            return n["v"] > 1

        self._run(monkeypatch, True, lambda: None, is_set)
        assert rgsx_manager.get_manager_state() == STATE_RUNNING

    def test_unresponsive_restarts(self, monkeypatch):
        restarts = []
        self._run(monkeypatch, False, lambda: restarts.append(1), lambda: False)
        assert restarts == [1]
        assert rgsx_manager.get_manager_state() == STATE_RESTARTING

    def test_unresponsive_limit_crashes(self, monkeypatch):
        class _FakeLimiter:
            def __init__(self, *a, **k):
                pass

            def record_restart(self):
                return False

        monkeypatch.setattr(rgsx_manager, "RestartLimiter", _FakeLimiter)
        restarts = []
        self._run(monkeypatch, False, lambda: restarts.append(1), lambda: False)
        assert restarts == []
        assert rgsx_manager.get_manager_state() == STATE_CRASHED


class TestStartWatchdog:
    def test_starts_thread(self, monkeypatch):
        started = []

        class _FakeThread:
            def __init__(self, *a, **k):
                self.kwargs = k

            def start(self):
                started.append(self.kwargs["target"])

        monkeypatch.setattr(rgsx_manager.threading, "Thread", _FakeThread)
        thread = rgsx_manager._start_watchdog(5000)
        assert started == [rgsx_manager._watchdog_loop]


# ---------------------------------------------------------------------------
# main() entry point
# ---------------------------------------------------------------------------

class TestMain:
    def test_auto_start_install(self, monkeypatch, isolated):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py", "--auto-start-install"])
        monkeypatch.setattr(rgsx_manager, "autostart_install", lambda: True)
        prefs = []
        monkeypatch.setattr(rgsx_manager, "_set_autostart_pref", lambda v: prefs.append(v) or v)
        assert rgsx_manager.main() == 0
        assert prefs == [True]

    def test_auto_start_remove_failure(self, monkeypatch, isolated):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py", "--auto-start-remove"])
        monkeypatch.setattr(rgsx_manager, "autostart_remove", lambda: False)
        assert rgsx_manager.main() == 1

    def test_already_running(self, monkeypatch, isolated):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py"])
        monkeypatch.setattr(rgsx_manager, "manager_healthy", lambda *a, **k: True)
        assert rgsx_manager.main() == 0

    def test_no_port_available(self, monkeypatch, isolated):
        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py"])
        monkeypatch.setattr(rgsx_manager, "manager_healthy", lambda *a, **k: False)
        monkeypatch.setattr(rgsx_manager, "_find_available_port", lambda pref, host="0.0.0.0": 0)
        assert rgsx_manager.main() == 1

    def test_full_start_with_port_shift(self, monkeypatch, isolated):
        import config as cfg

        monkeypatch.setattr(sys, "argv", ["rgsx_manager.py"])
        monkeypatch.setattr(rgsx_manager, "manager_healthy", lambda *a, **k: False)
        monkeypatch.setattr(rgsx_manager, "_find_available_port", lambda pref, host="0.0.0.0": 5005)
        set_ports = []
        monkeypatch.setattr(rgsx_settings, "set_manager_port", lambda p: set_ports.append(p) or p)
        monkeypatch.setattr(rgsx_manager, "download_queue_worker", lambda: None)
        monkeypatch.setattr(rgsx_manager, "_broadcaster_loop", lambda: None)
        monkeypatch.setattr(rgsx_manager, "_start_watchdog", lambda port: None)
        monkeypatch.setattr(rgsx_manager, "_resume_interrupted_downloads", lambda: 0)
        monkeypatch.setattr(rgsx_manager, "_setup_tray", lambda *a, **k: None)
        monkeypatch.setattr(cfg, "queue_worker_running", True)

        import qbittorrent_backend
        from network import download_state

        monkeypatch.setattr(download_state, "set_state_emitter", lambda f: None)
        monkeypatch.setattr(qbittorrent_backend, "ensure_qbittorrent_password_secured", lambda: None)
        monkeypatch.setattr(rgsx_web, "run_server", lambda **k: None)

        assert rgsx_manager.main() == 0
        assert set_ports == [5005]
