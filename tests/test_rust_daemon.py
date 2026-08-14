# -*- coding: utf-8 -*-
"""Faz 10c/1 — rust_daemon sidecar süpervizör birim testleri."""

import os

os.environ.setdefault("RGSX_HEADLESS", "1")

import rust_daemon


class _FakeProc:
    def __init__(self, pid=1234):
        self.pid = pid
        self._alive = True

    def poll(self):
        return None if self._alive else 0

    def terminate(self):
        self._alive = False


class _FakeResp:
    def __init__(self, status=200, payload=None):
        self.status = status
        import json

        self._body = json.dumps(payload if payload is not None else {"success": True, "manager": True}).encode()

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def read(self):
        return self._body


def _reset(monkeypatch):
    monkeypatch.setattr(rust_daemon, "_PROC", None)
    monkeypatch.setattr(rust_daemon, "_DAEMON_PORT", rust_daemon.DEFAULT_PORT)
    monkeypatch.setattr(rust_daemon, "_STOP", __import__("threading").Event())


def test_enabled_flag(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.delenv("RGSX_RUST_DAEMON", raising=False)
    assert rust_daemon.enabled() is False
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    assert rust_daemon.enabled() is True
    monkeypatch.setenv("RGSX_RUST_DAEMON", "true")
    assert rust_daemon.enabled() is True
    monkeypatch.setenv("RGSX_RUST_DAEMON", "yes")
    assert rust_daemon.enabled() is True
    monkeypatch.setenv("RGSX_RUST_DAEMON", "0")
    assert rust_daemon.enabled() is False


def test_resolve_bin_explicit(monkeypatch, tmp_path):
    _reset(monkeypatch)
    fake = tmp_path / "manager-bin"
    fake.write_bytes(b"")
    monkeypatch.setenv("RGSX_MANAGER_BIN_PATH", str(fake))
    assert rust_daemon._resolve_bin() == str(fake)


def test_start_disabled_no_spawn(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.delenv("RGSX_RUST_DAEMON", raising=False)
    calls = []
    monkeypatch.setattr(
        rust_daemon.subprocess, "Popen", lambda *a, **k: calls.append((a, k)) or _FakeProc()
    )
    assert rust_daemon.start() is None
    assert calls == []  # flag kapalıyken hiç subprocess açılmaz


def test_start_spawns_when_enabled(monkeypatch, tmp_path):
    _reset(monkeypatch)
    fake = tmp_path / "manager-bin"
    fake.write_bytes(b"")
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setenv("RGSX_MANAGER_BIN_PATH", str(fake))
    monkeypatch.delenv("RGSX_TORRENT_ENGINE", raising=False)
    monkeypatch.delenv("RGSX_MANAGER_BIN_PORT", raising=False)
    calls = []
    monkeypatch.setattr(
        rust_daemon.subprocess,
        "Popen",
        lambda *a, **k: calls.append((a, k)) or _FakeProc(),
    )
    proc = rust_daemon.start()
    assert proc is not None
    assert calls, "Popen çağrılmalıydı"
    args, kwargs = calls[0]
    assert str(fake) in args[0]  # binary yolu
    assert "--port" in args[0]
    assert "5010" in args[0]  # varsayılan port
    # RGSX_TORRENT_ENGINE set edilmediyse librqbit'e default'lansın.
    assert kwargs["env"].get("RGSX_TORRENT_ENGINE") == "librqbit"
    assert rust_daemon._PROC is proc


def test_start_missing_binary_returns_none(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    # _resolve_bin'in binary bulamadığı senaryosunu simüle et (kandırmadan bağımsız).
    monkeypatch.setattr(rust_daemon, "_resolve_bin", lambda: None)
    calls = []
    monkeypatch.setattr(
        rust_daemon.subprocess, "Popen", lambda *a, **k: calls.append((a, k)) or _FakeProc()
    )
    assert rust_daemon.start() is None
    assert calls == []  # binary yoksa spawn yok, Python-only devam


def test_healthy(monkeypatch):
    _reset(monkeypatch)

    def _ok(*a, **k):
        return _FakeResp(200, {"success": True, "manager": True})

    def _bad(*a, **k):
        return _FakeResp(200, {"success": False, "manager": False})

    def _raise(*a, **k):
        raise OSError("conn refused")

    monkeypatch.setattr(rust_daemon.urllib.request, "urlopen", _ok)
    assert rust_daemon.healthy() is True
    monkeypatch.setattr(rust_daemon.urllib.request, "urlopen", _bad)
    assert rust_daemon.healthy() is False
    monkeypatch.setattr(rust_daemon.urllib.request, "urlopen", _raise)
    assert rust_daemon.healthy() is False


def test_torrent_delegate_flag(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.delenv("RGSX_RUST_TORRENT", raising=False)
    assert rust_daemon.torrent_delegate_enabled() is False
    monkeypatch.setenv("RGSX_RUST_TORRENT", "1")
    assert rust_daemon.torrent_delegate_enabled() is True
    monkeypatch.setenv("RGSX_RUST_TORRENT", "off")
    assert rust_daemon.torrent_delegate_enabled() is False


class _FakeCancel:
    def __init__(self):
        self._set = False

    def is_set(self):
        return self._set


def test_download_torrent_delegates(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "_DAEMON_PORT", 5010)
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    posted = {}

    def _post(port, path, body):
        posted[(path, port)] = body
        return {}

    monkeypatch.setattr(rust_daemon, "_post_json", _post)
    # İlk poll Downloading, ikinci poll tamamlandı.
    states = [{"status": "Downloading", "progress": 40}, {"status": "Download_OK", "progress": 100}]
    monkeypatch.setattr(rust_daemon, "_poll_progress", lambda port, url: states.pop(0))

    meta = {"source_url": "magnet:?xt=foo", "size_bytes": 123}
    ok, msg = rust_daemon.download_torrent(
        meta, "/roms/snes", "/roms/snes/foo.zip", "t1", _FakeCancel(),
        "Foo", "snes", "rgsx+torrent://x?source=magnet:?xt=foo",
    )
    assert ok is True
    body = posted[("/api/download", 5010)]
    assert body["url"] == "magnet:?xt=foo"
    assert body["dest_path"] == "/roms/snes/foo.zip"
    assert body["game_name"] == "Foo"


def test_download_torrent_falls_back_when_unhealthy(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "healthy", lambda: False)
    meta = {"source_url": "magnet:?xt=foo"}
    try:
        rust_daemon.download_torrent(
            meta, "/d", "/d/foo", "t1", _FakeCancel(), "Foo", "snes", "u"
        )
        assert False, "RustDaemonError bekleniyordu"
    except rust_daemon.RustDaemonError:
        pass


def test_download_torrent_missing_source(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    try:
        rust_daemon.download_torrent(
            {}, "/d", "/d/foo", "t1", _FakeCancel(), "Foo", "snes", "u"
        )
        assert False, "RustDaemonError bekleniyordu"
    except rust_daemon.RustDaemonError:
        pass


def test_download_torrent_mirrors_progress(monkeypatch):
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    monkeypatch.setattr(rust_daemon, "_post_json", lambda port, path, body: {})
    states = [
        {"status": "Downloading", "progress": 50, "downloaded_size": 500, "total_size": 1000},
        {"status": "Download_OK", "progress": 100},
    ]
    monkeypatch.setattr(rust_daemon, "_poll_progress", lambda port, url: states.pop(0))
    import history as _history

    monkeypatch.setattr(_history, "save_history", lambda h: None)
    import config as _cfg

    _cfg.download_progress = {}
    _cfg.history = [{"url": "orig", "status": "Downloading", "progress": 0}]

    rust_daemon.download_torrent(
        {"source_url": "magnet:?xt=foo"}, "/d", "/d/foo", "t1", _FakeCancel(),
        "Foo", "snes", "orig",
    )
    assert _cfg.download_progress["orig"]["progress_percent"] == 100
    assert _cfg.history[0]["status"] == "Download_OK"


class _FakePauseEv:
    """is_set() döndüren ve clear edilebilen pause event kuklası."""

    def __init__(self, states):
        self._states = list(states)
        self._idx = 0

    def is_set(self):
        self._idx += 1
        return self._states[min(self._idx, len(self._states)) - 1]


def test_download_torrent_sends_task_id_in_body(monkeypatch):
    """Gap-2: /api/download gövdesi `task_id`'yi de taşır (pause eşleşmesi için)."""
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    posted = {}

    def _post(port, path, body):
        posted[(path, port)] = body
        return {}

    monkeypatch.setattr(rust_daemon, "_post_json", _post)
    states = [{"status": "Download_OK", "progress": 100}]
    monkeypatch.setattr(rust_daemon, "_poll_progress", lambda port, url: states.pop(0))

    rust_daemon.download_torrent(
        {"source_url": "magnet:?xt=foo"}, "/d", "/d/foo", "task_42", _FakeCancel(),
        "Foo", "snes", "u",
    )
    body = posted[("/api/download", 5010)]
    assert body["task_id"] == "task_42"


def test_download_torrent_pause_resume_cycle(monkeypatch):
    """Gap-2: pause_ev set → /api/pause, clear → /api/resume (task_id ile)."""
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "_DAEMON_PORT", 5010)
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    posted = []

    def _post(port, path, body):
        posted.append((path, port, body))
        return {}

    monkeypatch.setattr(rust_daemon, "_post_json", _post)
    # Poll sırası: pause (evet) → resume (hayır) → tamam.
    pause_states = [True, False, False]
    states = [
        {"status": "Downloading", "progress": 10},
        {"status": "Downloading", "progress": 30},
        {"status": "Download_OK", "progress": 100},
    ]
    monkeypatch.setattr(rust_daemon, "_poll_progress", lambda port, url: states.pop(0))

    rust_daemon.download_torrent(
        {"source_url": "magnet:?xt=foo"}, "/d", "/d/foo", "task_p", _FakeCancel(),
        "Foo", "snes", "u",
        pause_ev=_FakePauseEv(pause_states),
    )
    paths = [p for (p, _, _) in posted]
    assert "/api/pause" in paths
    assert "/api/resume" in paths
    pause_body = next(b for (p, _, b) in posted if p == "/api/pause")
    resume_body = next(b for (p, _, b) in posted if p == "/api/resume")
    assert pause_body["task_id"] == "task_p"
    assert resume_body["task_id"] == "task_p"


def test_download_torrent_pause_ev_none_no_pause_posts(monkeypatch):
    """Gap-2: pause_ev yoksa pause/resume POST'u hiç gönderilmez (geriye uyum)."""
    _reset(monkeypatch)
    monkeypatch.setenv("RGSX_RUST_DAEMON", "1")
    monkeypatch.setattr(rust_daemon, "healthy", lambda: True)
    posted = []

    def _post(port, path, body):
        posted.append(path)
        return {}

    monkeypatch.setattr(rust_daemon, "_post_json", _post)
    states = [{"status": "Download_OK", "progress": 100}]
    monkeypatch.setattr(rust_daemon, "_poll_progress", lambda port, url: states.pop(0))

    rust_daemon.download_torrent(
        {"source_url": "magnet:?xt=foo"}, "/d", "/d/foo", "t1", _FakeCancel(),
        "Foo", "snes", "u",
    )
    assert "/api/pause" not in posted
    assert "/api/resume" not in posted
