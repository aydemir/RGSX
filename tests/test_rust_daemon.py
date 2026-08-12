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
