# -*- coding: utf-8 -*-
"""Faz 9 — Toplu indirme ("Tümünü İndir") testleri.

Kapsam:
  1. Web endpoint /api/download/batch — kuyruğa alma, dedupe, sayaçlar, kick.
  2. ManagerHandler yönlendirmesi — aynı payload queue-worker için kuyruğa basar.
  3. TV batch çekirdeği (controls.downloads.queue_download_batch) — sayaç döndürür,
     öğeleri QUEUED akışına sokar.
"""
import io
import json
import email
import threading

import pytest

import config
from config import Game
from rgsx_web.handlers import RGSXHandler
from rgsx_manager import ManagerHandler


# ---------------------------------------------------------------------------
# Helpers (test_api_contract.py ile aynı desen)
# ---------------------------------------------------------------------------

class _FakeServer:
    server_version = "TestHTTP/1.0"
    sys_version = ""


def make_handler(handler_cls, path, method="POST", body=b""):
    handler = object.__new__(handler_cls)
    handler.command = method
    handler.requestline = f"{method} {path} HTTP/1.1"
    handler.path = path
    handler.request_version = "HTTP/1.1"
    handler.protocol_version = "HTTP/1.1"
    handler.server = _FakeServer()
    handler.rfile = io.BytesIO(body)
    handler.wfile = io.BytesIO()
    handler.headers = email.message_from_string(f"Host: test\nContent-Length: {len(body)}")
    return handler


def post_json(handler_cls, path, data):
    handler = make_handler(handler_cls, path, method="POST", body=json.dumps(data).encode("utf-8"))
    handler.do_POST()
    raw = handler.wfile.getvalue()
    head, _, payload = raw.partition(b"\r\n\r\n")
    status_line = head.split(b"\r\n", 1)[0]
    status = int(status_line.split(b" ", 2)[1])
    return status, json.loads(payload.decode("utf-8"))


@pytest.fixture
def isolated(tmp_path, monkeypatch):
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
def batch_env(isolated, monkeypatch):
    """Web batch endpoint ortamı: oyun kataloğu + yaygın susturmalar."""
    import rgsx_web.handlers_download as hd
    import utils
    from rgsx_web.cache import get_cached_games

    games = [
        Game(name="Alpha.zip", url="https://ex.invalid/alpha.zip", size="1 MB", display_name="Alpha"),
        Game(name="Beta.zip", url="https://ex.invalid/beta.zip", size="2 MB", display_name="Beta"),
        Game(name="Gamma.7z", url="https://ex.invalid/gamma.7z", size="3 MB", display_name="Gamma"),
        Game(name="NoUrl.bin", url=None, size="1 MB", display_name="NoUrl"),
    ]
    monkeypatch.setattr(hd, "get_cached_games", lambda platform: (games, None, None))
    monkeypatch.setattr(utils, "check_extension_before_download",
                        lambda url, platform, name: (url, platform, name, False) if url else None)
    monkeypatch.setattr(utils, "check_web_service_status", lambda: {"running": False})
    monkeypatch.setattr(utils, "check_custom_dns_status", lambda: {"enabled": False})
    monkeypatch.setattr(utils, "load_api_keys", lambda: {})
    # Varsayılan: worker çalışıyor (production manager). Kick testleri False'a çeker.
    monkeypatch.setattr(config, "queue_worker_running", True)
    return games


class _NoopThread:
    calls = 0

    def __init__(self, *args, **kwargs):
        self.args = args
        self.kwargs = kwargs

    def start(self):
        type(self).calls += 1


@pytest.fixture
def noop_thread(monkeypatch):
    _NoopThread.calls = 0
    monkeypatch.setattr(threading, "Thread", _NoopThread)
    return _NoopThread


# ---------------------------------------------------------------------------
# 1. Web endpoint /api/download/batch
# ---------------------------------------------------------------------------

class TestWebBatch:
    def test_missing_platform(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch", {})
        assert status == 400
        assert body["success"] is False

    def test_missing_game_names(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch", {"platform": "NES"})
        assert status == 400
        assert body["success"] is False

    def test_empty_game_names(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch", {"platform": "NES", "game_names": []})
        assert status == 400
        assert body["success"] is False

    def test_full_batch_queues_all(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip", "Gamma.7z"]})
        assert status == 200
        assert body["success"] is True
        assert body["queued"] == 3
        assert body["skipped"] == 0
        assert set(q["game_name"] for q in config.download_queue) == {"Alpha.zip", "Beta.zip", "Gamma.7z"}
        for q in config.download_queue:
            assert q["status"] == "Queued"
            assert q["task_id"].startswith("batch_")
        queued_history = [e for e in config.history if e.get("status") == "Queued"]
        assert len(queued_history) == 3

    def test_unknown_game_skipped(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Mystere.zip", "Beta.zip"]})
        assert status == 200
        assert body["queued"] == 2
        assert body["skipped"] == 1
        assert len(body["errors"]) == 1
        assert "Mystere.zip" in body["errors"][0]

    def test_no_url_skipped(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["NoUrl.bin", "Alpha.zip"]})
        assert status == 200
        assert body["queued"] == 1
        assert body["skipped"] == 1
        assert config.download_queue[0]["game_name"] == "Alpha.zip"

    def test_duplicate_names_inside_batch(self, batch_env):
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Alpha.zip"]})
        assert status == 200
        assert body["queued"] == 1
        assert body["skipped"] == 1

    def test_already_queued_url_not_duplicated(self, batch_env, monkeypatch):
        config.download_queue.append({
            "url": "https://ex.invalid/alpha.zip", "platform": "NES", "game_name": "Alpha.zip",
            "is_zip_non_supported": False, "is_1fichier": False,
            "task_id": "old", "status": "Queued",
        })
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip"]})
        assert status == 200
        assert body["queued"] == 1
        assert body["skipped"] == 1
        assert len(config.download_queue) == 2  # mevcut + yeni

    def test_already_downloaded_counter(self, batch_env, monkeypatch):
        monkeypatch.setattr(config, "downloaded_games", {"NES": {"alpha": {"size": "1 MB", "timestamp": "x"}}})
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip"]})
        assert status == 200
        assert body["queued"] == 2
        assert body["already_downloaded"] == 1

    def test_worker_running_no_kick(self, batch_env, noop_thread, monkeypatch):
        # queue-worker çalışıyorsa legacy thread zinciri pop yapmaz
        monkeypatch.setattr(config, "queue_worker_running", True)
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip"]})
        assert body["queued"] == 2
        assert len(config.download_queue) == 2

    def test_standalone_kick_fills_free_slots(self, batch_env, noop_thread, monkeypatch):
        monkeypatch.setattr(config, "queue_worker_running", False)
        monkeypatch.setattr(config, "active_download_count", 0)
        monkeypatch.setattr(config, "max_simultaneous_downloads", 2)
        status, body = post_json(RGSXHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip", "Gamma.7z"]})
        assert status == 200
        assert body["queued"] == 3
        # 3 eklendi; 2 boş slot olduğu için 2 öğe pop edilip thread'e verildi
        assert len(config.download_queue) == 1
        assert noop_thread.calls == 2


# ---------------------------------------------------------------------------
# 2. Manager yönlendirmesi
# ---------------------------------------------------------------------------

class TestManagerBatch:
    def test_manager_download_batch_pushes_to_queue(self, batch_env, noop_thread, monkeypatch):
        monkeypatch.setattr(config, "queue_worker_running", True)  # manager process'te worker var
        status, body = post_json(ManagerHandler, "/api/download/batch",
                                 {"platform": "NES", "game_names": ["Alpha.zip", "Beta.zip", "Gamma.7z"]})
        assert status == 200
        assert body["success"] is True
        assert body["queued"] == 3
        assert len(config.download_queue) == 3
        assert all(q["status"] == "Queued" for q in config.download_queue)
        assert len([e for e in config.history if e.get("status") == "Queued"]) == 3


# ---------------------------------------------------------------------------
# 3. TV batch çekirdeği
# ---------------------------------------------------------------------------

class TestTvBatchCore:
    def test_queue_download_batch_counts(self, monkeypatch):
        import controls.downloads as cd

        calls = []
        monkeypatch.setattr(cd, "check_extension_before_download",
                            lambda url, platform, name: (url, platform, name, False))
        monkeypatch.setattr(cd, "is_game_downloaded", lambda plat, name: name == "Beta.zip")
        monkeypatch.setattr(cd, "_queue_download",
                            lambda url, platform, name, is_zip, defer_save=False: calls.append(
                                (url, platform, name, is_zip, defer_save)) or str(len(calls)))
        monkeypatch.setattr(cd, "_launch_next_queued_download", lambda *a, **k: None)

        games = [
            Game(name="Alpha.zip", url="https://ex.invalid/alpha.zip", size="1 MB", display_name="Alpha"),
            Game(name="Beta.zip", url="https://ex.invalid/beta.zip", size="2 MB", display_name="Beta"),
        ]
        queued, skipped, already, errors = cd.queue_download_batch(games, "NES")
        assert queued == 2
        assert skipped == 0
        assert already == 1
        assert errors == []
        assert calls == [
            ("https://ex.invalid/alpha.zip", "NES", "Alpha.zip", False, True),
            ("https://ex.invalid/beta.zip", "NES", "Beta.zip", False, True),
        ]

    def test_queue_download_batch_skips_unsupported(self, monkeypatch):
        import controls.downloads as cd

        monkeypatch.setattr(cd, "check_extension_before_download",
                            lambda url, platform, name: None)
        monkeypatch.setattr(cd, "is_game_downloaded", lambda plat, name: False)
        monkeypatch.setattr(cd, "_queue_download", lambda *a, **k: None)
        monkeypatch.setattr(cd, "_launch_next_queued_download", lambda *a, **k: None)
        monkeypatch.setattr(config, "download_queue", [])

        games = [Game(name="Alpha.bin", url="https://ex.invalid/a.bin", size="1", display_name="A")]
        queued, skipped, already, errors = cd.queue_download_batch(games, "NES")
        assert queued == 0
        assert skipped == 1
        assert len(errors) == 1
        assert "Alpha.bin" in errors[0]

    def test_queue_download_batch_dedupe_in_queue(self, monkeypatch):
        import controls.downloads as cd

        monkeypatch.setattr(cd, "check_extension_before_download",
                            lambda url, platform, name: (url, platform, name, False))
        monkeypatch.setattr(cd, "is_game_downloaded", lambda plat, name: False)
        monkeypatch.setattr(cd, "_queue_download", lambda *a, **k: None)
        monkeypatch.setattr(cd, "_launch_next_queued_download", lambda *a, **k: None)
        monkeypatch.setattr(config, "download_queue",
                            [{"url": "https://ex.invalid/alpha.zip", "status": "Queued"}])

        games = [
            Game(name="Alpha.zip", url="https://ex.invalid/alpha.zip", size="1", display_name="Alpha"),
            Game(name="Beta.zip", url="https://ex.invalid/beta.zip", size="1", display_name="Beta"),
        ]
        queued, skipped, already, _ = cd.queue_download_batch(games, "NES")
        assert queued == 1
        assert skipped == 1

    def test_trigger_filtered_batch_download_runs_async(self, monkeypatch):
        import controls.downloads as cd

        triggered = {}
        monkeypatch.setattr(config, "filter_active", True)
        monkeypatch.setattr(config, "filtered_games",
                            [Game(name="Alpha.zip", url="https://ex.invalid/alpha.zip", size="1", display_name="A")])
        monkeypatch.setattr(config, "games", [])
        monkeypatch.setattr(config, "platforms", {"nes": {"name": "NES"}})
        monkeypatch.setattr(config, "current_platform", "nes")

        def fake_batch(games, platform_label):
            triggered["games"] = len(games)
            triggered["platform"] = platform_label
            return (1, 0, 0, [])

        monkeypatch.setattr(cd, "queue_download_batch", fake_batch)

        from display import show_toast
        toasted = []
        monkeypatch.setattr(cd, "show_toast", lambda text: toasted.append(text))

        class _SyncThread:
            def __init__(self, *a, **k):
                self.target = k.get("target")
                self.args = a
                self.kwargs = {kk: vv for kk, vv in k.items() if kk != "target"}

            def start(self):
                self.target()

        monkeypatch.setattr(cd.threading, "Thread", _SyncThread)

        cd.trigger_filtered_batch_download()
        assert triggered == {"games": 1, "platform": "NES"}
        assert len(toasted) == 1