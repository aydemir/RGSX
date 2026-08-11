# -*- coding: utf-8 -*-
"""Faz 7 - Characterization tests: WebUI + Manager HTTP API contract.

Handler'lar gercek soket olmadan (object.__new__ + mock wfile/rfile/headers)
dogrudan cagrilir. Amac: Rust portu oncesi mevcut davranisi sabitlemek.
"""
import email
import io
import json
import os
import queue as queue_module
import zipfile

import pytest

import config
import rgsx_manager
import utils
from rgsx_web.handlers import RGSXHandler
from rgsx_manager import ManagerHandler


# ---------------------------------------------------------------------------
# Helpers
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


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def isolated(tmp_path, monkeypatch):
    """Tum config dosya/save yollari tmp_path'e yonlendirilir."""
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


@pytest.fixture
def no_network(monkeypatch, isolated):
    """Ag/indirme cagrilari susturulur."""
    import rgsx_web.handlers_settings as hs
    monkeypatch.setattr("urllib.request.urlretrieve", lambda *a, **k: (_ for _ in ()).throw(RuntimeError("no network")))
    monkeypatch.setattr(utils, "check_extension_before_download", lambda url, platform, name: (url, platform, name, False))
    monkeypatch.setattr(utils, "restart_application", lambda code: None)
    monkeypatch.setattr(utils, "check_web_service_status", lambda: {"running": False})
    monkeypatch.setattr(utils, "check_custom_dns_status", lambda: {"enabled": False})
    monkeypatch.setattr(utils, "load_api_keys", lambda: {})
    return isolated


# ---------------------------------------------------------------------------
# GET / - page d'accueil
# ---------------------------------------------------------------------------

class TestIndex:
    def test_root_returns_html(self, isolated):
        status, head, payload = invoke(RGSXHandler, "/")
        assert status == 200
        assert b"text/html" in head
        assert b"RGSX" in payload


# ---------------------------------------------------------------------------
# GET /api/* - formes de reponse
# ---------------------------------------------------------------------------

class TestWebGet:
    def test_platforms_empty(self, isolated):
        status, head, payload = invoke(RGSXHandler, "/api/platforms")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["count"] == 0
        assert body["platforms"] == []

    def test_search_empty_query(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/search?q=")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["search_term"] == ""
        assert body["results"] == {"platforms": [], "games": []}

    def test_search_with_term_empty_sources(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/search?q=zelda")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["search_term"] == "zelda"
        assert body["results"]["platforms"] == []
        assert body["results"]["games"] == []

    def test_translations_shape(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/translations")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert "language" in body
        assert isinstance(body["language"], str)
        assert "_language" in body["translations"]

    def test_translations_reloads_from_disk(self, isolated, monkeypatch):
        # Bayat TRANSLATIONS yerine diskten taze okuma: dosyaya yeni anahtar eklendiğinde
        # /api/translations restart olmadan yeni anahtarı servis etmeli.
        monkeypatch.setattr(config, "LANGUAGES_FOLDER", str(isolated / "langs"))
        langs = isolated / "langs"
        langs.mkdir(exist_ok=True)
        lang_file = langs / "en.json"
        lang_file.write_text(json.dumps({"custom_key": "one"}), encoding="utf-8")

        status, _, payload = invoke(RGSXHandler, "/api/translations")
        assert status == 200
        body = as_json(payload)
        assert body["translations"]["custom_key"] == "one"

        # Simülasyon: dil dosyası güncellendi (örn. yeni anahtar eklendi)
        lang_file.write_text(json.dumps({"custom_key": "one", "custom_key_2": "two"}), encoding="utf-8")
        status, _, payload = invoke(RGSXHandler, "/api/translations")
        assert status == 200
        body = as_json(payload)
        assert body["translations"]["custom_key_2"] == "two"

    def test_games_empty_platform(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/games/NES")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["platform"] == "NES"
        assert body["count"] == 0
        assert body["games"] == []

    def test_progress_empty(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/progress")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["downloads"] == {}

    def test_game_status_empty(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/game-status")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["statuses"] == {}

    def test_history_empty(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/history")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["count"] == 0
        assert body["history"] == []

    def test_history_strips_error_message_noise(self, isolated):
        noisy = ("Download error Crazy Cars ++.zip: Accès refusé (HTTP 500). "
                 "Fichiers disponibles exemples: ['Addams Family.zip', 'After Burner II.zip', "
                 "'Aladdin.zip', 'Amiga 500 Tutorial.mp4']")
        full = ("Download error Crazy Cars ++.zip: Accès refusé (HTTP 500). "
                "Fichiers disponibles exemples: ['Addams Family.zip', 'After Burner II.zip', "
                "'Aladdin.zip', 'Amiga 500 Tutorial.mp4']")
        entry = {
            "game_name": "Crazy Cars ++.zip",
            "platform": "Amiga OCS ECS (Archive)",
            "status": "Erreur",
            "message": noisy,
            "url": "https://archive.org/download/amiga-500-Collection/Crazy%20Cars%20%2B%2B.zip",
            "timestamp": "2026-08-11 02:42:48",
        }
        (isolated / "history.json").write_text(json.dumps([entry]), encoding="utf-8")

        status, _, payload = invoke(RGSXHandler, "/api/history")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["count"] == 1
        got = body["history"][0]
        assert got["message"] == "Accès refusé (HTTP 500)"
        # history.json'a ham mesaj korunmali (TVUI detay ekrani tam metin gosterir)
        stored = json.loads((isolated / "history.json").read_text(encoding="utf-8"))
        assert stored[0]["message"] == full

    def test_queue_get(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/queue")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["active"] is False
        assert body["queue"] == []
        assert body["queue_size"] == 0

    def test_settings_get(self, no_network):
        status, _, payload = invoke(RGSXHandler, "/api/settings")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert "settings" in body

    def test_system_info(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/system_info")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert "system_info" in body

    def test_browse_directories_root(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/browse-directories")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert "current_path" in body
        assert "directories" in body

    def test_browse_directories_missing_path(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/browse-directories?path=/chemin/nonexistant-xyz")
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Le chemin spécifié n'existe pas"

    def test_platform_image_not_found(self, isolated):
        status, head, payload = invoke(RGSXHandler, "/api/image/NES")
        assert status == 404
        assert b"image/png" in head
        assert payload.startswith(b"\x89PNG")

    def test_favicon_served(self, isolated):
        status, head, payload = invoke(RGSXHandler, "/api/favicon")
        assert status == 200
        assert b"image/x-icon" in head

    def test_static_missing_file_404(self, isolated):
        status, _, _ = invoke(RGSXHandler, "/static/js/does_not_exist.js")
        assert status == 404

    def test_unknown_route_404(self, isolated):
        status, _, payload = invoke(RGSXHandler, "/api/inconnue")
        assert status == 404
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Route non trouvée"
        assert body["path"] == "/api/inconnue"

    def test_update_cache_no_files(self, no_network):
        status, _, payload = invoke(RGSXHandler, "/api/update-cache")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert "deleted" in body


# ---------------------------------------------------------------------------
# POST /api/* - validation et formes de reponse
# ---------------------------------------------------------------------------

class TestWebPost:
    def test_download_missing_params(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/download", {})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètres manquants: platform et (game_index ou game_name) requis"

    def test_download_invalid_index(self, no_network, monkeypatch):
        import rgsx_web.handlers_download as hd
        monkeypatch.setattr(hd, "get_cached_games", lambda platform: ([], None, None))
        status, _, payload = post_json(RGSXHandler, "/api/download", {"platform": "NES", "game_index": 0})
        assert status == 400
        body = as_json(payload)
        assert body["error"] == "Index de jeu invalide: 0"

    def test_download_game_name_not_found(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/download", {"platform": "NES", "game_name": "Introuvable"})
        assert status == 400
        body = as_json(payload)
        assert body["error"] == "Jeu non trouvé: Introuvable"

    def test_cancel_missing_url(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/cancel", {})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètre manquant: url requis"

    def test_cancel_unknown_url(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/cancel", {"url": "https://exemple.invalid/rom.zip"})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["message"] == "Téléchargement annulé"
        assert body["url"] == "https://exemple.invalid/rom.zip"
        assert body["task_id"] is None

    def test_queue_post(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/queue", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["queue_size"] == 0

    def test_queue_clear_empty(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/queue/clear", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["cleared_count"] == 0
        assert body["message"] == "0 éléments supprimés de la queue"

    def test_queue_remove_missing_task_id(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/queue/remove", {})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètre manquant: task_id requis"

    def test_queue_remove_not_found(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/queue/remove", {"task_id": "xyz"})
        assert status == 404
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Élément non trouvé: xyz"

    def test_queue_remove_found(self, no_network, monkeypatch):
        monkeypatch.setattr(config, "download_queue", [{"task_id": "t1", "game_name": "Jeu"}])
        status, _, payload = post_json(RGSXHandler, "/api/queue/remove", {"task_id": "t1"})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["task_id"] == "t1"
        assert config.download_queue == []

    def test_settings_missing_param(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/settings", {})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètre \"settings\" manquant"

    def test_settings_post(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/settings", {"settings": {"dummy": 1}})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True

    def test_save_filters(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/save_filters", {"region_filters": {}})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["message"] == "Filtres sauvegardés"

    def test_clear_history(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/clear-history", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True

    def test_restart(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/restart", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["message"] == "Redémarrage en cours..."

    def test_support_zip(self, no_network, monkeypatch):
        tmp_path = no_network
        (tmp_path / "controls.json").write_text("{}", encoding="utf-8")
        (tmp_path / "history.json").write_text("[]", encoding="utf-8")
        (tmp_path / "rgsx_settings.json").write_text("{}", encoding="utf-8")
        monkeypatch.setattr(config, "log_file", str(tmp_path / "RGSX.log"))
        (tmp_path / "RGSX.log").write_text("log", encoding="utf-8")

        status, head, payload = invoke(RGSXHandler, "/api/support", method="POST")
        assert status == 200
        assert b"application/zip" in head
        assert b"filename=" in head
        zf = zipfile.ZipFile(io.BytesIO(payload))
        names = zf.namelist()
        assert "README.txt" in names
        assert "controls.json" in names

    def test_post_unknown_route_404(self, no_network):
        status, _, payload = post_json(RGSXHandler, "/api/inconnue", {})
        assert status == 404
        body = as_json(payload)
        assert body["success"] is False
        assert body["path"] == "/api/inconnue"


# ---------------------------------------------------------------------------
# Manager (rgsx_manager.ManagerHandler) endpoints
# ---------------------------------------------------------------------------

class TestManager:
    def test_health(self, isolated):
        status, _, payload = invoke(ManagerHandler, "/api/health")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["status"] == "ok"
        assert body["manager"] is True
        assert body["pid"] > 0
        assert isinstance(body["manager_state"], str)

    def test_password_status(self, isolated, monkeypatch):
        import qbittorrent_backend
        monkeypatch.setattr(
            qbittorrent_backend,
            "get_password_status",
            lambda: {"available": True, "using_default": True, "webui_url": "http://127.0.0.1:18572"},
        )
        status, _, payload = invoke(ManagerHandler, "/api/qbittorrent/password-status")
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["available"] is True
        assert body["using_default"] is True
        assert "webui_url" in body

    def test_download_missing_params(self, isolated):
        status, _, payload = post_json(ManagerHandler, "/api/download", {})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètres manquants: platform et (game_index ou game_name) requis"

    def test_download_invalid_index(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "get_cached_games", lambda platform: ([], None, None))
        status, _, payload = post_json(ManagerHandler, "/api/download", {"platform": "NES", "game_index": 0})
        assert status == 400
        body = as_json(payload)
        assert body["error"] == "Index de jeu invalide: 0"

    def test_download_direct_url_success(self, no_network):
        status, _, payload = post_json(
            ManagerHandler,
            "/api/download",
            {"url": "https://exemple.invalid/rom.zip", "game_name": "Rom", "platform": "NES"},
        )
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["queued"] is True
        assert body["game_name"] == "Rom"
        assert body["platform"] == "NES"
        assert body["task_id"].startswith("web_")
        assert len(config.download_queue) == 1
        assert config.download_queue[0]["game_name"] == "Rom"
        assert any(e.get("game_name") == "Rom" for e in config.history)

    def test_download_direct_url_missing_game_name(self, no_network):
        status, _, payload = post_json(ManagerHandler, "/api/download", {"url": "https://exemple.invalid/rom.zip", "platform": "NES"})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["error"] == "Paramètre manquant: game_name requis avec url"

    def test_shutdown(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "_trigger_shutdown", lambda: None)
        status, _, payload = post_json(ManagerHandler, "/api/shutdown", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True

    def test_pause(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "pause_all_downloads", lambda: 2)
        status, _, payload = post_json(ManagerHandler, "/api/pause", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["paused"] == 2

    def test_resume(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager, "resume_all_downloads", lambda: 3)
        status, _, payload = post_json(ManagerHandler, "/api/resume", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["resumed"] == 3

    def test_qbittorrent_start(self, isolated, monkeypatch):
        import qbittorrent_backend
        monkeypatch.setattr(qbittorrent_backend, "ensure_running", lambda timeout=30: True)
        monkeypatch.setattr(qbittorrent_backend, "get_webui_url", lambda: "http://127.0.0.1:18572")
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/start", {})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["ready"] is True
        assert body["url"] == "http://127.0.0.1:18572"

    def test_qbittorrent_change_password_ok(self, isolated, monkeypatch):
        import qbittorrent_backend
        monkeypatch.setattr(qbittorrent_backend, "change_webui_password", lambda pw: (True, "ok"))
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/change-password", {"password": "nouveau-mdp-123"})
        assert status == 200
        body = as_json(payload)
        assert body["success"] is True
        assert body["message"] == "ok"

    def test_qbittorrent_change_password_failure(self, isolated, monkeypatch):
        import qbittorrent_backend
        monkeypatch.setattr(qbittorrent_backend, "change_webui_password", lambda pw: (False, "password_too_short"))
        status, _, payload = post_json(ManagerHandler, "/api/qbittorrent/change-password", {"password": "x"})
        assert status == 400
        body = as_json(payload)
        assert body["success"] is False
        assert body["message"] == "password_too_short"


# ---------------------------------------------------------------------------
# SSE
# ---------------------------------------------------------------------------

class TestSse:
    def test_sse_event_format(self):
        event = rgsx_manager._sse_event("snapshot", {"active": False})
        assert event.startswith("event: snapshot\n")
        assert "data: " in event
        data_part = event.split("data: ", 1)[1].strip()
        assert json.loads(data_part) == {"active": False}
        assert event.endswith("\n\n")

    def test_handle_sse_sends_snapshot_then_exits(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager.STOP, "is_set", lambda: True)
        status, head, payload = invoke(ManagerHandler, "/api/events")
        assert status == 200
        assert b"text/event-stream" in head
        text = payload.decode("utf-8")
        assert "event: snapshot" in text
        data_part = text.split("data: ", 1)[1].split("\n")[0]
        snapshot = json.loads(data_part)
        for key in ("history", "queue", "active", "progress", "downloaded"):
            assert key in snapshot

    def test_handle_sse_cleans_subscribers(self, isolated, monkeypatch):
        monkeypatch.setattr(rgsx_manager.STOP, "is_set", lambda: True)
        before = set(rgsx_manager.SUBSCRIBERS)
        invoke(ManagerHandler, "/api/events")
        assert rgsx_manager.SUBSCRIBERS == before

    def test_broadcast_puts_raw_event(self, isolated):
        q = queue_module.Queue()
        with rgsx_manager.SUBSCRIBERS_LOCK:
            rgsx_manager.SUBSCRIBERS.add(q)
        try:
            rgsx_manager._broadcast("hello", {"x": 1})
            item = q.get_nowait()
            assert item["type"] == "hello"
            assert item["raw"].startswith("event: hello\n")
        finally:
            with rgsx_manager.SUBSCRIBERS_LOCK:
                rgsx_manager.SUBSCRIBERS.discard(q)
