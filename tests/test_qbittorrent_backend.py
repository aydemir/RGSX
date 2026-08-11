# -*- coding: utf-8 -*-
"""Faz 7 - Characterization tests: qbittorrent_backend contract.

Migration mantigi test_password_migration.py'de kapsanmis. Bu dosya manager
API'sinin guvendigi yuzeyi (get_password_status / _get_configured_password /
change_webui_password) sabitler. Gercek qBittorrent sureci baslatilmaz.
"""

import io
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time as _time
import types

import pytest
import requests

import config
import qbittorrent_backend as qbt


class FakeSession:
    """_apply_webui_password/_ensure_qbittorrent_running icin sahte oturum."""

    def __init__(self):
        self.closed = False
        self.posts = []

    def post(self, url, **kwargs):
        self.posts.append((url, kwargs))
        return None

    def close(self):
        self.closed = True


class TestGetPasswordStatus:
    """Manager /api/qbittorrent/password-status'in dayandigi contract."""

    def test_shape_with_default_password(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(qbt, "is_available", lambda: True)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "RGSXqbt")
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_password_mode", lambda: "default")
        status = qbt.get_password_status()
        assert set(status.keys()) == {"available", "using_default", "secured", "mode", "webui_url"}
        assert status["available"] is True
        assert status["using_default"] is True
        assert status["secured"] is False
        assert status["mode"] == "default"
        assert status["webui_url"] == "http://localhost:18572/"

    def test_custom_password_not_default(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(qbt, "is_available", lambda: True)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp-2026")
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_password_mode", lambda: "custom")
        status = qbt.get_password_status()
        assert status["using_default"] is False
        assert status["secured"] is True
        assert status["mode"] == "custom"

    def test_random_mode_secured(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(qbt, "is_available", lambda: True)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_password_mode", lambda: "random")
        status = qbt.get_password_status()
        assert status["using_default"] is False
        assert status["secured"] is True
        assert status["mode"] == "random"

    def test_settings_failure_falls_back_to_default(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(qbt, "is_available", lambda: False)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_password_mode", lambda: "default")

        def boom():
            raise RuntimeError("settings erisilemez")

        monkeypatch.setattr(qbt, "_get_configured_password", boom)
        status = qbt.get_password_status()
        assert status["available"] is False
        assert status["using_default"] is True
        assert status["secured"] is False

    def test_config_constant_absence_defaults(self, monkeypatch):
        import rgsx_settings
        monkeypatch.delattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", raising=False)
        monkeypatch.setattr(qbt, "is_available", lambda: False)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "RGSXqbt")
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_password_mode", lambda: "default")
        status = qbt.get_password_status()
        assert status["using_default"] is True


class TestGetConfiguredPassword:
    """Sifre onceligi: settings > config sabiti."""

    def test_settings_value_wins(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_webui_password", lambda: "depuis-settings")
        monkeypatch.setattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", "depuis-config")
        assert qbt._get_configured_password() == "depuis-settings"

    def test_settings_error_falls_back_to_config(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", "depuis-config")

        def boom():
            raise RuntimeError("settings hata")

        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_webui_password", boom)
        assert qbt._get_configured_password() == "depuis-config"

    def test_settings_error_no_config_returns_empty(self, monkeypatch):
        import rgsx_settings
        monkeypatch.delattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", raising=False)

        def boom():
            raise RuntimeError("settings hata")

        monkeypatch.setattr(rgsx_settings, "get_qbittorrent_webui_password", boom)
        assert qbt._get_configured_password() == ""


class TestChangeWebuiPassword:
    """change_webui_password dönüs contract'i: tuple[bool, str]."""

    def test_empty_password_rejected(self):
        ok, message = qbt.change_webui_password("")
        assert ok is False
        assert message == "password_too_short"

    def test_short_password_rejected(self):
        ok, message = qbt.change_webui_password("12345")
        assert ok is False
        assert message == "password_too_short"

    def test_live_session_applies_and_closes(self, monkeypatch):
        session = FakeSession()

        def fake_apply(active_session, new_password):
            active_session.posts.append(("apply", new_password))

        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(qbt, "_apply_webui_password", fake_apply)
        ok, message = qbt.change_webui_password("NouveauMdp-123")
        assert ok is True
        assert message == "ok"
        assert session.posts == [("apply", "NouveauMdp-123")]
        assert session.closed is True

    def test_backend_exception_uses_settings_fallback(self, monkeypatch):
        def boom():
            raise RuntimeError("backend hatasi")

        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", boom)
        wrote = {}

        def fake_set(pw):
            wrote["pw"] = pw

        import rgsx_settings
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password", fake_set)
        ok, message = qbt.change_webui_password("FallbackMdp-123")
        assert ok is True
        assert message == "ok"
        assert wrote["pw"] == "FallbackMdp-123"


class TestEnsurePasswordSecured:
    """ensure_qbittorrent_password_secured: açılışta varsayılan şifre imkânsız."""

    def test_no_stored_password_generates_and_saves(self, monkeypatch):
        import rgsx_settings

        writes = {}

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", lambda: {})
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password",
                            lambda pw: writes.update(pw=pw))
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_mode",
                            lambda m: writes.update(mode=m))
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_migration_done",
                            lambda v: writes.update(done=v))
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_extract_temp_password", lambda lines: None)

        result = qbt.ensure_qbittorrent_password_secured()
        assert result == "rtG4_9Q2xLmZ7vPw"
        assert writes["pw"] == "rtG4_9Q2xLmZ7vPw"
        assert writes["mode"] == "random"
        assert writes["done"] is True

    def test_secure_stored_password_returned_untouched(self, monkeypatch):
        import rgsx_settings

        wrote = []

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings",
                            lambda: {"qbittorrent_webui_password": "monMdp-2026"})
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password",
                            lambda pw: wrote.append(pw))
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")

        result = qbt.ensure_qbittorrent_password_secured()
        assert result == "monMdp-2026"
        assert wrote == []

    def test_default_stored_password_regenerated(self, monkeypatch):
        import rgsx_settings

        writes = {}

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings",
                            lambda: {"qbittorrent_webui_password": "adminadmin"})
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password",
                            lambda pw: writes.update(pw=pw))
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_extract_temp_password", lambda lines: None)

        result = qbt.ensure_qbittorrent_password_secured()
        assert result == "rtG4_9Q2xLmZ7vPw"
        assert writes["pw"] == "rtG4_9Q2xLmZ7vPw"

    def test_settings_unreadable_falls_back_to_configured(self, monkeypatch):
        import rgsx_settings

        def boom():
            raise RuntimeError("settings erisilemez")

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", boom)
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "depuis-config")
        assert qbt.ensure_qbittorrent_password_secured() == "depuis-config"


class TestRegeneratePassword:
    """regenerate_qbittorrent_password: tuple[bool, str] dönüş contract'i."""

    def test_live_webui_applies_immediately(self, monkeypatch):
        applied = []
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_apply_webui_password",
                            lambda s, pw: applied.append((s is not None, pw)))
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572/")

        ok, password = qbt.regenerate_qbittorrent_password()
        assert ok is True
        assert password == "rtG4_9Q2xLmZ7vPw"
        assert applied == [(True, "rtG4_9Q2xLmZ7vPw")]

    def test_offline_falls_back_to_settings(self, monkeypatch):
        applied = []
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        monkeypatch.setattr(qbt, "_apply_webui_password",
                            lambda s, pw: applied.append((s is None, pw)))
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572/")

        ok, password = qbt.regenerate_qbittorrent_password()
        assert ok is True
        assert password == "rtG4_9Q2xLmZ7vPw"
        assert applied == [(True, "rtG4_9Q2xLmZ7vPw")]


class TestPasswordModeSettings:
    """rgsx_settings qbittorrent_password_mode get/set contract'i."""

    def test_get_mode_from_settings_key(self, monkeypatch):
        import rgsx_settings
        assert rgsx_settings.get_qbittorrent_password_mode({"qbittorrent_password_mode": "random"}) == "random"

    def test_get_mode_inferred_from_stored_password(self, monkeypatch):
        import rgsx_settings
        assert rgsx_settings.get_qbittorrent_password_mode({"qbittorrent_webui_password": "monMdp"}) == "custom"

    def test_get_mode_default_when_empty(self, monkeypatch):
        import rgsx_settings
        assert rgsx_settings.get_qbittorrent_password_mode({}) == "default"

    def test_get_mode_bogus_value_falls_to_default(self, monkeypatch):
        import rgsx_settings
        assert rgsx_settings.get_qbittorrent_password_mode({"qbittorrent_password_mode": "bogus"}) == "default"

    def test_set_mode_persists(self, monkeypatch):
        import rgsx_settings

        store = {}
        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", lambda: store)
        monkeypatch.setattr(rgsx_settings, "save_rgsx_settings", lambda s: None)
        rgsx_settings.set_qbittorrent_password_mode("random")
        assert store["qbittorrent_password_mode"] == "random"


class FakeHttpSession:
    """URL'e göre yanıt kuyruğu döndüren sahte HTTP oturumu."""

    def __init__(self, responses=None):
        self.cookies = types.SimpleNamespace(clear=lambda: None)
        self.calls = []
        self.responses = list(responses or [])

    def get(self, url, **kwargs):
        self.calls.append(("get", url))
        return self._next("get", url)

    def post(self, url, **kwargs):
        self.calls.append(("post", url))
        return self._next("post", url)

    def close(self):
        self.calls.append(("close",))

    def _next(self, method, url):
        for i, (m, u, r) in enumerate(self.responses):
            if m == method and (u == "*" or u == url or
                                (u.startswith("*/") and url.endswith(u[1:]))):
                return self.responses.pop(i)[2]
        return SimpleFakeResponse(404)


class SimpleFakeResponse:
    def __init__(self, status_code=200, text="", json_data=None, content=b""):
        self.status_code = status_code
        self.text = text
        self._json = json_data
        self.content = content

    def raise_for_status(self):
        if self.status_code >= 400:
            raise requests.exceptions.HTTPError(f"HTTP {self.status_code}")

    def json(self):
        return self._json


class StubEvent:
    """is_set() çağrılarında sırayla değer döner, tükenince False."""

    def __init__(self, values):
        self._values = list(values)

    def is_set(self):
        return self._values.pop(0) if self._values else False


class FakeProc:
    """_ensure_qbittorrent_running için sahte Popen benzeri."""

    def __init__(self, poll_result=None, wait_exc=None):
        self.pid = 4242
        self._poll = poll_result
        self._wait_exc = wait_exc
        self.stdout = io.StringIO("")
        self.terminated = False
        self.killed = False

    def poll(self):
        return self._poll

    def terminate(self):
        self.terminated = True

    def kill(self):
        self.killed = True

    def wait(self, timeout=None):
        if self._wait_exc:
            raise self._wait_exc
        return 0


def _fake_time(monkeypatch):
    fake = types.SimpleNamespace(time=lambda: 1000.0, sleep=lambda s: None)
    monkeypatch.setattr(qbt, "time", fake)
    return fake


class TestSmallHelpers:
    def test_generate_random_password_length(self):
        assert len(qbt.generate_random_password()) >= 16
        assert len(qbt.generate_random_password(32)) >= 32
        assert qbt.generate_random_password() != qbt.generate_random_password()

    def test_extract_temp_password_english(self):
        lines = ["blah", "Temporary password for WebUI administrator user: aBcD123"]
        assert qbt._extract_temp_password(lines) == "aBcD123"

    def test_extract_temp_password_french(self):
        lines = ["mot de passe temporaire pour le compte administrateur : XyZ789"]
        assert qbt._extract_temp_password(lines) == "XyZ789"

    def test_extract_temp_password_none(self):
        assert qbt._extract_temp_password(["no password here"]) is None

    def test_url_builder(self, monkeypatch):
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        assert qbt._url("/api/v2/app/version") == "http://127.0.0.1:18572/api/v2/app/version"

    def test_build_torrent_headers(self):
        headers = qbt._build_torrent_headers()
        assert "Mozilla" in headers["User-Agent"]
        assert headers["Accept"] == "*/*"


class TestEnsureIniSettings:
    def test_creates_new_file(self, tmp_path):
        ini = tmp_path / "qBittorrent.ini"
        qbt._ensure_ini_settings(str(ini), {
            "LegalNotice": {"Accepted": "true"},
            "Preferences": {"WebUI\\Port": "18572"},
        })
        content = ini.read_text()
        assert "[LegalNotice]" in content
        assert "Accepted=true" in content
        assert "[Preferences]" in content
        assert "WebUI\\Port=18572" in content

    def test_preserves_existing_keys_and_adds_missing(self, tmp_path):
        ini = tmp_path / "qBittorrent.ini"
        ini.write_text("[Preferences]\nWebUI\\Port=8080\nKeep=yes\n")
        qbt._ensure_ini_settings(str(ini), {
            "Preferences": {"WebUI\\Port": "18572", "WebUI\\Enabled": "true"},
            "LegalNotice": {"Accepted": "true"},
        })
        content = ini.read_text()
        assert "WebUI\\Port=8080" in content  # mevcut korunur
        assert "Keep=yes" in content
        assert "WebUI\\Enabled=true" in content  # eksik anahtar eklenir
        assert "[LegalNotice]" in content
        assert "Accepted=true" in content


class TestPreseedProfiles:
    def test_preseed_linux_profile_writes_port(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        qbt._preseed_linux_profile(18573)
        ini = tmp_path / "qBittorrent" / "config" / "qBittorrent.conf"
        content = ini.read_text()
        assert "WebUI\\Port=18573" in content
        assert "General\\Locale=en_US" in content

    def test_preseed_linux_migrates_localhost_only(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        ini = tmp_path / "qBittorrent" / "config" / "qBittorrent.conf"
        ini.parent.mkdir(parents=True)
        ini.write_text(
            "[Preferences]\n"
            "WebUI\\Address=127.0.0.1\n"
            "WebUI\\AuthSubnetWhitelist=127.0.0.1/32\n"
            "WebUI\\AuthSubnetWhitelistEnabled=true\n"
        )
        qbt._preseed_linux_profile(18572)
        content = ini.read_text()
        assert "WebUI\\Address=0.0.0.0" in content
        assert "WebUI\\AuthSubnetWhitelistEnabled=false" in content

    def test_preseed_windows_never_check_association(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        qbt._preseed_windows_profile(18574)
        ini = tmp_path / "data" / "profile" / "qBittorrent" / "config" / "qBittorrent.ini"
        content = ini.read_text()
        assert "WebUI\\Port=18574" in content
        assert "NeverCheckFileAssocation=true" in content


class TestFindExecutable:
    def test_linux_bundled_nox_chmod(self, tmp_path, monkeypatch):
        exe = tmp_path / "qbittorrent-nox_linux"
        exe.write_bytes(b"#!/bin/sh\n")
        monkeypatch.setattr(qbt, "_NOX_LINUX", str(exe))
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Android")
        result = qbt._find_qbittorrent_executable()
        assert result == str(exe)
        assert os.stat(exe).st_mode & 0o111

    def test_linux_which_fallback(self, monkeypatch):
        monkeypatch.setattr(qbt, "_NOX_LINUX", "/nonexistent/nox")
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Android")
        monkeypatch.setattr(shutil, "which", lambda name: "/usr/bin/qbittorrent-nox")
        assert qbt._find_qbittorrent_executable() == "/usr/bin/qbittorrent-nox"

    def test_linux_none(self, monkeypatch):
        monkeypatch.setattr(qbt, "_NOX_LINUX", "/nonexistent/nox")
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Android")
        monkeypatch.setattr(shutil, "which", lambda name: None)
        assert qbt._find_qbittorrent_executable() is None

    def test_windows_registry_install_dir(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Windows")
        monkeypatch.setattr(qbt, "_extract_portable_windows", lambda: None)
        monkeypatch.setattr(os, "environ", {
            "ProgramFiles": "C:\\PF",
            "LOCALAPPDATA": "C:\\LA",
        })
        monkeypatch.setattr(shutil, "which", lambda name: None)
        fake_reg = types.SimpleNamespace(stdout="InstallDir    REG_SZ    C:\\qbt\n")
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: fake_reg)
        exe_path = os.path.join("C:\\qbt", "qbittorrent.exe")
        monkeypatch.setattr(os.path, "isfile", lambda p: p == exe_path)
        assert qbt._find_qbittorrent_executable() == exe_path

    def test_windows_registry_exception(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Windows")
        monkeypatch.setattr(qbt, "_extract_portable_windows", lambda: None)
        monkeypatch.setattr(os, "environ", {"ProgramFiles": "C:\\PF"})
        monkeypatch.setattr(os.path, "isfile", lambda p: False)
        monkeypatch.setattr(shutil, "which", lambda name: None)

        def boom(*a, **k):
            raise OSError("reg yok")

        monkeypatch.setattr(subprocess, "run", boom)
        assert qbt._find_qbittorrent_executable() is None

    def test_windows_installed_candidate(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Windows")
        monkeypatch.setattr(qbt, "_extract_portable_windows", lambda: None)
        monkeypatch.setattr(os, "environ", {
            "ProgramFiles": "C:\\PF",
            "ProgramFiles(x86)": "C:\\PF86",
            "LOCALAPPDATA": "C:\\LA",
        })
        exe_path = os.path.join("C:\\PF86", "qBittorrent", "qbittorrent.exe")
        monkeypatch.setattr(os.path, "isfile", lambda p: p == exe_path)
        monkeypatch.setattr(shutil, "which", lambda name: None)
        assert qbt._find_qbittorrent_executable() == exe_path


class TestExtractPortableWindows:
    def test_launcher_already_present(self, tmp_path, monkeypatch):
        launcher = tmp_path / "qbittorrent-portable.exe"
        launcher.write_bytes(b"x")
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        assert qbt._extract_portable_windows() == str(launcher)

    def test_no_archive(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(tmp_path / "missing.7z"))
        assert qbt._extract_portable_windows() is None

    def test_extraction_failure(self, tmp_path, monkeypatch):
        archive = tmp_path / "q.7z"
        archive.write_bytes(b"7z")
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(archive))
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(SEVEN_Z_EXE="7z"))
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: types.SimpleNamespace(returncode=1, stdout="boom"))
        assert qbt._extract_portable_windows() is None

    def test_extraction_success(self, tmp_path, monkeypatch):
        archive = tmp_path / "q.7z"
        archive.write_bytes(b"7z")
        launcher = tmp_path / "qbittorrent-portable.exe"
        launcher.write_bytes(b"x")
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(archive))
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(SEVEN_Z_EXE="7z"))
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: types.SimpleNamespace(returncode=0, stdout=""))
        assert qbt._extract_portable_windows() == str(launcher)


class TestIsAvailable:
    def test_available(self, monkeypatch):
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: "/usr/bin/qbittorrent-nox")
        assert qbt.is_available() is True

    def test_unavailable(self, monkeypatch):
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: None)
        assert qbt.is_available() is False


class TestPorts:
    def test_is_port_open_true(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.listen(1)
        try:
            assert qbt._is_port_open("127.0.0.1", port) is True
        finally:
            s.close()

    def test_is_port_open_false(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()
        assert qbt._is_port_open("127.0.0.1", port) is False


class TestBackendState:
    def test_state_set_and_get(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_state", qbt.STATE_STOPPED)
        qbt._set_qbt_state(qbt.STATE_STARTING, "test")
        assert qbt.get_backend_state() == qbt.STATE_STARTING

    def test_state_same_no_change_log(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_state", qbt.STATE_STARTING)
        qbt._set_qbt_state(qbt.STATE_STARTING, "same")
        assert qbt.get_backend_state() == qbt.STATE_STARTING


class TestTerminateManagedProcess:
    def test_no_process(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_process", None)
        qbt._terminate_managed_process()  # no raise
        assert qbt._qbt_process is None

    def test_terminate_running_process(self, monkeypatch):
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt._terminate_managed_process()
        assert proc.terminated is True
        assert qbt._qbt_process is None

    def test_terminate_kill_on_timeout(self, monkeypatch):
        proc = FakeProc(wait_exc=subprocess.TimeoutExpired("qbt", 5))
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt._terminate_managed_process()
        assert proc.killed is True
        assert qbt._qbt_process is None

    def test_already_exited(self, monkeypatch):
        proc = FakeProc(poll_result=0)
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt._terminate_managed_process()
        assert proc.terminated is False


class TestLogin:
    def test_success_with_configured_password(self, monkeypatch):
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(200, "Ok."))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is True

    def test_success_temp_password_priority(self, monkeypatch):
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(200, "Ok."))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "")
        lines = ["temporary password: Temp1234"]
        assert qbt._login(session, "http://127.0.0.1:18572", lines) is True

    def test_banned_returns_false(self, monkeypatch):
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(403, "ip has been banned"))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is False

    def test_204_success(self, monkeypatch):
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(204))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is True

    def test_localhost_bypass_success(self, monkeypatch):
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(403, "denied")),
            ("get", "*", SimpleFakeResponse(200, "")),
        ])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is True

    def test_failure_and_timeout(self, monkeypatch):
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(403, "denied")),
            ("get", "*", SimpleFakeResponse(500, "")),
        ])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        _fake_time(monkeypatch)
        assert qbt._login(session, "http://127.0.0.1:18572", [], timeout=0.1) is False


class TestTerminateExistingProcesses:
    def test_linux_kills_matching_pids(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Android")
        fake_result = types.SimpleNamespace(stdout="123 qbittorrent-nox\n456 /usr/bin/qbittorrent\n789 python\n")
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: fake_result)
        kills = []

        def fake_kill(pid, sig):
            kills.append((pid, sig))
            if pid == 123 and sig == signal.SIGTERM:
                raise PermissionError()

        monkeypatch.setattr(os, "kill", fake_kill)
        _fake_time(monkeypatch)
        qbt._terminate_existing_qbittorrent_processes()
        pids = {k[0] for k in kills}
        assert 123 in pids
        assert 456 in pids
        assert 789 not in pids
        assert any(sig == signal.SIGKILL for _, sig in kills)

    def test_linux_ps_failure_ignored(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Android")

        def boom(*a, **k):
            raise OSError("ps yok")

        monkeypatch.setattr(subprocess, "run", boom)
        qbt._terminate_existing_qbittorrent_processes()  # no raise

    def test_windows_taskkill(self, monkeypatch):
        monkeypatch.setattr(config, "OPERATING_SYSTEM", "Windows")
        runs = []
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: runs.append(a) or types.SimpleNamespace(stdout=""))
        qbt._terminate_existing_qbittorrent_processes()
        assert runs and "taskkill" in runs[0][0]


class TestTorrentInfoHelpers:
    def test_torrent_info_by_tag(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[{"hash": "h1"}]))])
        assert qbt._torrent_info_by_tag(session, "tag1") == {"hash": "h1"}
        session2 = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[]))])
        assert qbt._torrent_info_by_tag(session2, "tag1") is None

    def test_torrent_info_by_tag_request_error(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(500))])
        assert qbt._torrent_info_by_tag(session, "tag1") is None

    def test_torrent_info_by_hash(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[{"hash": "abc"}]))])
        assert qbt._torrent_info_by_hash(session, "abc") == {"hash": "abc"}

    def test_find_existing_by_save_path(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[
            {"save_path": "/roms/.rgsx_torrent/abc"},
        ]))])
        found = qbt._find_existing_torrent_by_save_path(session, "/roms/.rgsx_torrent/abc")
        assert found is not None
        session2 = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[
            {"save_path": "/other"},
        ]))])
        assert qbt._find_existing_torrent_by_save_path(session2, "/roms/.rgsx_torrent/abc") is None

    def test_torrent_files(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data=[{"index": 1}]))])
        assert qbt._torrent_files(session, "h1") == [{"index": 1}]

    def test_resolve_target_file_index(self):
        files = [
            {"index": 0, "name": "a/b/c.iso"},
            {"index": 1, "name": "c.iso"},
            {"index": 2, "name": "x/y/z.iso"},
        ]
        assert qbt._resolve_target_file_index(files, "c.iso") == 1
        assert qbt._resolve_target_file_index(files, "a/b/c.iso") == 0
        assert qbt._resolve_target_file_index(files, "nothere.iso") is None

    def test_count_active_peers(self, monkeypatch):
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([("get", "*", SimpleFakeResponse(200, json_data={
            "peers": {"a": {"dl_speed": 500}, "b": {"dl_speed": 0}},
        }))])
        assert qbt._count_active_peers(session, "h1", "dl_speed") == 1


class TestApplyFileSelection:
    def _files(self):
        return [
            {"index": 0, "name": "a.iso", "size": 100},
            {"index": 1, "name": "b.iso", "size": 200},
        ]

    def test_empty_files(self, monkeypatch):
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: [])
        assert qbt._apply_file_selection(None, "h1", "b.iso") == (None, 0)

    def test_single_file(self, monkeypatch):
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: [{"index": 0, "size": 123}])
        assert qbt._apply_file_selection(None, "h1", "anything") == (0, 123)

    def test_multi_file_selection(self, monkeypatch):
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: self._files())
        monkeypatch.setattr(qbt, "_selected_file_indexes_by_hash", {})
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(200)),
            ("post", "*", SimpleFakeResponse(200)),
        ])
        idx, size = qbt._apply_file_selection(session, "h1", "b.iso")
        assert idx == 1
        assert size == 200
        posts = [c for c in session.calls if c[0] == "post"]
        assert len(posts) == 2

    def test_target_not_resolved(self, monkeypatch):
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: self._files())
        monkeypatch.setattr(qbt, "_selected_file_indexes_by_hash", {})
        assert qbt._apply_file_selection(None, "h1", "missing.iso") == (None, 0)


class TestGetTargetFileProgress:
    def test_no_index(self):
        assert qbt._get_target_file_progress([], None, 500) == (0, 500, False)

    def test_matched_file_partial(self):
        files = [{"index": 1, "size": 100, "progress": 0.5}]
        downloaded, size, done = qbt._get_target_file_progress(files, 1, 100)
        assert downloaded == 50
        assert size == 100
        assert done is False

    def test_matched_file_complete(self):
        files = [{"index": 1, "size": 100, "progress": 1.0}]
        assert qbt._get_target_file_progress(files, 1, 100)[2] is True

    def test_index_not_found(self):
        assert qbt._get_target_file_progress([{"index": 5, "size": 10}], 1, 500) == (0, 500, False)

    def test_malformed_entries(self):
        files = [{"index": "1", "size": "abc", "progress": "xyz"}]
        downloaded, size, done = qbt._get_target_file_progress(files, 1, 100)
        assert size == 100
        assert done is False


class TestActiveReferences:
    def test_lifecycle(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_downloads", {})
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
        qbt._register_active_download("t1", "hash1", "url1", 3)
        assert qbt._has_other_hash_references("hash1") is True
        assert qbt._has_other_hash_references("hash2") is False
        qbt._promote_active_download_to_seed("t1", {"hash": "hash1", "tag": "rgsx_t1"})
        assert qbt.has_active_seed("t1") is True
        task, entry = qbt._pop_active_reference(task_id="t1")
        assert task == "t1"
        assert entry["hash"] == "hash1"

    def test_pop_by_url(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_downloads", {})
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
        qbt._register_active_download("t2", "hash2", "url2", None)
        task, entry = qbt._pop_active_reference(original_history_url="url2")
        assert task == "t2"
        assert qbt._pop_active_reference(task_id="t2") == (None, None)

    def test_cleanup_hash_state(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_downloads", {})
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
        monkeypatch.setattr(qbt, "_selected_file_indexes_by_hash", {"hash1": {2}, "hash2": {1}})
        qbt._register_active_download("keep", "hash1", "url", None)
        qbt._cleanup_hash_state_if_unused("hash1")  # referans var → silinmez
        assert "hash1" in qbt._selected_file_indexes_by_hash
        qbt._cleanup_hash_state_if_unused("hash2")  # referans yok → silinir
        assert "hash2" not in qbt._selected_file_indexes_by_hash


class TestProcessStatus:
    def test_is_process_running(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_process", None)
        assert qbt.is_process_running() is False
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        assert qbt.is_process_running() is True


class TestPrewarmStartup:
    def test_success(self, monkeypatch):
        session = FakeSession()
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        assert qbt.prewarm_startup() is True
        assert session.closed is True

    def test_unavailable(self, monkeypatch):
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        assert qbt.prewarm_startup() is False

    def test_exception(self, monkeypatch):
        def boom():
            raise RuntimeError("hata")

        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", boom)
        assert qbt.prewarm_startup() is False


class TestEnsureRunning:
    def test_unavailable(self, monkeypatch):
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        assert qbt.ensure_running() is False

    def test_success(self, monkeypatch):
        session = FakeSession()
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        assert qbt.ensure_running() is True

    def test_timeout(self, monkeypatch):
        session = FakeSession()
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: False)
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        _fake_time(monkeypatch)
        assert qbt.ensure_running(timeout=0.1) is False


class TestPrewarmStartupAsync:
    def test_skips_when_running(self, monkeypatch):
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        monkeypatch.setattr(qbt, "_prewarm_thread", None)
        called = []
        monkeypatch.setattr(qbt, "prewarm_startup", lambda: called.append(1) or True)
        qbt.prewarm_startup_async()
        assert called == []

    def test_starts_single_thread(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_process", None)
        monkeypatch.setattr(qbt, "_prewarm_thread", None)
        called = []

        def slow_prewarm():
            called.append(1)
            _time.sleep(0.1)

        monkeypatch.setattr(qbt, "prewarm_startup", slow_prewarm)
        qbt.prewarm_startup_async()
        qbt.prewarm_startup_async()
        _time.sleep(0.2)
        assert len(called) == 1
        assert qbt._prewarm_thread is None


class TestHasActiveSeed:
    def test_by_task_id(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {"t1": {"original_history_url": "url1"}})
        assert qbt.has_active_seed(task_id="t1") is True
        assert qbt.has_active_seed(task_id="nope") is False

    def test_by_url(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {"t1": {"original_history_url": "url1"}})
        assert qbt.has_active_seed(original_history_url="url1") is True
        assert qbt.has_active_seed(original_history_url="url9") is False
        assert qbt.has_active_seed() is False


class TestStopSeed:
    def test_no_seed_no_process(self, monkeypatch):
        monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
        monkeypatch.setattr(qbt, "_qbt_process", None)
        assert qbt.stop_seed(task_id="t1") is False

    def test_detach_when_shared(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: True)
        fake_network = _fake_network_module(monkeypatch)
        stopped = []
        fake_network._stop_seeding_status = lambda url: stopped.append(url)
        assert qbt.stop_seed(task_id="t1") is True
        assert stopped == ["url1"]
        assert "t1" not in seeds

    def test_delete_torrent(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_url", lambda p: p)
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: {"hash": "h1"})
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        fake_network = _fake_network_module(monkeypatch)
        stopped = []
        fake_network._stop_seeding_status = lambda url: stopped.append(url)
        assert qbt.stop_seed(task_id="t1") is True
        assert stopped == ["url1"]

    def test_webui_unavailable(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        assert qbt.stop_seed(task_id="t1") is False

    def test_info_missing_returns_entry(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: None)
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        assert qbt.stop_seed(task_id="t1") is True


class TestShutdown:
    def test_no_process(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_process", None)
        qbt.shutdown()
        assert qbt._qbt_process is None

    def test_terminates_running(self, monkeypatch):
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt.shutdown()
        assert proc.terminated is True
        assert qbt._qbt_process is None

    def test_kill_on_wait_failure(self, monkeypatch):
        proc = FakeProc(wait_exc=subprocess.TimeoutExpired("qbt", 5))
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt.shutdown()
        assert proc.killed is True


class TestResolveDownloadedFile:
    def test_content_path_is_file(self, tmp_path):
        f = tmp_path / "game.iso"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file(str(f), str(tmp_path), "game.iso", "game.iso") == str(f)

    def test_expected_path(self, tmp_path):
        (tmp_path / "a" / "b").mkdir(parents=True)
        f = tmp_path / "a" / "b" / "game.iso"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file("", str(tmp_path), "a/b/game.iso", "game.iso") == str(f)

    def test_walk_fallback(self, tmp_path):
        sub = tmp_path / "sub"
        sub.mkdir()
        f = sub / "game.bin"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file("", str(tmp_path), "", "game.bin") == str(f)

    def test_not_found(self, tmp_path):
        assert qbt._resolve_downloaded_file("", str(tmp_path), "", "missing.iso") is None


class TestMaybeMigratePassword:
    def _settings(self, monkeypatch, settings, **patches):
        import rgsx_settings
        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", lambda: settings)
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_migration_done", lambda v: None)
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_mode", lambda m: None)
        monkeypatch.setattr(qbt, "_apply_webui_password", lambda s, pw: None)
        monkeypatch.setattr(qbt, "_notify_password_migrated", lambda: None)
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_extract_temp_password", lambda lines: None)
        for name, value in patches.items():
            monkeypatch.setattr(getattr(qbt, name), name, value)

    def test_settings_unreadable(self, monkeypatch):
        import rgsx_settings

        def boom():
            raise RuntimeError("erisilemez")

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", boom)
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "failed"

    def test_already_done(self, monkeypatch):
        self._settings(monkeypatch, {"migration_v1_done": True})
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "already_done"

    def test_no_stored_generates(self, monkeypatch):
        applied = []
        import rgsx_settings
        self._settings(monkeypatch, {})
        monkeypatch.setattr(qbt, "_apply_webui_password", lambda s, pw: applied.append(pw))
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "migrated"
        assert applied == ["rtG4_9Q2xLmZ7vPw"]

    def test_default_stored_rotated(self, monkeypatch):
        applied = []
        self._settings(monkeypatch, {"qbittorrent_webui_password": "adminadmin"})
        monkeypatch.setattr(qbt, "_apply_webui_password", lambda s, pw: applied.append(pw))
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "migrated"
        assert applied == ["rtG4_9Q2xLmZ7vPw"]

    def test_user_defined_noop(self, monkeypatch):
        import rgsx_settings
        done = []
        self._settings(monkeypatch, {"qbittorrent_webui_password": "monMdp-2026"})
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_migration_done",
                            lambda v: done.append(v))
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "noop"
        assert done == [True]


class TestNotifyPasswordMigrated:
    def test_broadcast_and_toast(self, monkeypatch):
        broadcast = []
        fake_manager = types.SimpleNamespace(
            SUBSCRIBERS=[1],
            _broadcast=lambda t, p: broadcast.append((t, p)),
        )
        monkeypatch.setitem(sys.modules, "rgsx_manager", fake_manager)
        toasts = []
        fake_display = types.SimpleNamespace(show_toast=lambda msg, duration=0: toasts.append(msg))
        monkeypatch.setitem(sys.modules, "display", fake_display)
        fake_web = types.SimpleNamespace(get_translation=lambda k: "qbt_password_migrated")
        monkeypatch.setitem(sys.modules, "rgsx_web", fake_web)
        qbt._notify_password_migrated()
        assert broadcast == [("toast", {"message": "qBittorrent WebUI password was automatically rotated for security."})]
        assert toasts == ["qBittorrent WebUI password was automatically rotated for security."]

    def test_no_subscribers(self, monkeypatch):
        broadcast = []
        fake_manager = types.SimpleNamespace(
            SUBSCRIBERS=[],
            _broadcast=lambda t, p: broadcast.append((t, p)),
        )
        monkeypatch.setitem(sys.modules, "rgsx_manager", fake_manager)
        toasts = []
        fake_display = types.SimpleNamespace(show_toast=lambda msg, duration=0: toasts.append(msg))
        monkeypatch.setitem(sys.modules, "display", fake_display)
        fake_web = types.SimpleNamespace(get_translation=lambda k: "Traduit")
        monkeypatch.setitem(sys.modules, "rgsx_web", fake_web)
        qbt._notify_password_migrated()
        assert broadcast == []
        assert toasts == ["Traduit"]


class TestEnsureQbittorrentRunning:
    def test_reuses_live_process(self, monkeypatch):
        _reset_process(monkeypatch)
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_recovered_after_retry(self, monkeypatch):
        _reset_process(monkeypatch)
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        waits = iter([False, True])
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: next(waits))
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        _fake_time(monkeypatch)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_no_binary(self, monkeypatch):
        _reset_process(monkeypatch)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: None)
        assert qbt._ensure_qbittorrent_running() is None
        assert qbt.get_backend_state() == qbt.STATE_STOPPED

    def test_reuses_existing_instance(self, monkeypatch):
        _reset_process(monkeypatch)
        existing = FakeHttpSession()
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([qbt._TARGET_PORT]))
        monkeypatch.setattr(qbt, "_probe_existing_webui_session", lambda port: existing)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        session = qbt._ensure_qbittorrent_running()
        assert session is existing
        assert qbt._base_url == f"http://127.0.0.1:{qbt._TARGET_PORT}"

    def test_port_range_exhausted(self, monkeypatch):
        _reset_process(monkeypatch)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: "/usr/bin/qbittorrent-nox")
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 0)
        assert qbt._ensure_qbittorrent_running() is None
        assert qbt.get_backend_state() == qbt.STATE_STOPPED

    def test_linux_full_start(self, monkeypatch, tmp_path):
        _reset_process(monkeypatch)
        preseeded = []
        proc = FakeProc()
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: str(tmp_path / "nox"))
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_preseed_linux_profile", lambda port: preseeded.append(port))
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 19000)
        monkeypatch.setattr(qbt, "_is_port_open", lambda host, port: False)
        monkeypatch.setattr(subprocess, "Popen", lambda cmd, **kw: proc)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert preseeded == [19000]
        assert qbt.get_backend_state() == qbt.STATE_RUNNING
        assert qbt._base_url == "http://127.0.0.1:19000"

    def test_linux_port_occupied_after_probe(self, monkeypatch):
        _reset_process(monkeypatch)
        ports = iter([18573, 0])
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: "/usr/bin/qbittorrent-nox")
        monkeypatch.setattr(qbt, "_is_port_open", lambda host, port: True)
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: next(ports))
        monkeypatch.setattr(qbt, "_terminate_existing_qbittorrent_processes", lambda: None)
        _fake_time(monkeypatch)
        assert qbt._ensure_qbittorrent_running() is None


class TestDownloadTorrentViaQbittorrent:
    def _meta(self, tmp_path):
        dest_dir = tmp_path / "dl"
        dest_dir.mkdir()
        meta = {
            "source_url": "http://src/x.torrent",
            "relative_path": "game.iso",
            "size_bytes": 1000,
        }
        return meta, dest_dir

    def _setup(self, monkeypatch, tmp_path, session_responses, info=None, progress_return=(100, 500, True), cancel=(False,), pause=(False,), add_status=200):
        import qbittorrent_backend as qbt
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session_responses[0])
        session = session_responses[0] if isinstance(session_responses, (list, tuple)) else FakeHttpSession([("post", "*", SimpleFakeResponse(200))])
        monkeypatch.setattr(qbt, "_find_existing_torrent_by_save_path", lambda s, p: {"hash": "hash1"})
        monkeypatch.setattr(qbt, "_apply_file_selection", lambda s, h, p: (2, 500))
        monkeypatch.setattr(qbt, "_torrent_info_by_hash", lambda s, h: info or {
            "state": "downloading", "dlspeed": 1024 * 1024, "size": 500,
            "num_leechs": 3, "num_seeds": 1, "content_path": "",
        })
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: [{"index": 2, "size": 500, "progress": 1.0}])
        monkeypatch.setattr(qbt, "_get_target_file_progress", lambda files, idx, size: progress_return)
        monkeypatch.setattr(qbt, "_count_active_peers", lambda s, h, f: 2)
        monkeypatch.setattr(qbt, "_seed_status_worker", lambda *a, **k: None)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        monkeypatch.setattr(requests, "get", lambda url, **kw: SimpleFakeResponse(content=b"torrent-bytes"))
        _fake_time(monkeypatch)
        return session

    def test_success(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        downloaded = temp_dir / "game.iso"
        downloaded.write_bytes(b"x" * 100)
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(200, "Ok."))])
        self._setup(monkeypatch, tmp_path, [session], info={
            "state": "downloading", "dlspeed": 1024 * 1024, "size": 500,
            "num_leechs": 3, "num_seeds": 1, "progress": 1.0,
            "content_path": str(downloaded),
        })
        progress = Recorder()
        ok, msg = qbt.download_torrent_via_qbittorrent(
            meta, str(dest_dir), dest_path, "t1", StubEvent([False]), progress, "")
        assert ok is True
        assert os.path.isfile(dest_path)
        assert "game.iso" in str(msg)
        assert len(progress.items) >= 2

    def test_canceled(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(200, "Ok.")),
            ("post", "*", SimpleFakeResponse(200, "Ok.")),
        ])
        self._setup(monkeypatch, tmp_path, [session])
        with pytest.raises(RuntimeError):
            qbt.download_torrent_via_qbittorrent(
                meta, str(dest_dir), dest_path, "t1", StubEvent([True]), Recorder(), "")

    def test_error_state(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(200, "Ok."))])
        self._setup(monkeypatch, tmp_path, [session], info={"state": "error"})
        with pytest.raises(RuntimeError, match="état d'erreur"):
            qbt.download_torrent_via_qbittorrent(
                meta, str(dest_dir), dest_path, "t1", StubEvent([False]), Recorder(), "")

    def test_pause_resume(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        downloaded = temp_dir / "game.iso"
        downloaded.write_bytes(b"x" * 100)
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(200, "Ok.")),
            ("post", "*", SimpleFakeResponse(200, "Ok.")),
            ("post", "*", SimpleFakeResponse(200, "Ok.")),
        ])
        self._setup(monkeypatch, tmp_path, [session], info={
            "state": "downloading", "dlspeed": 1024 * 1024, "size": 500,
            "num_leechs": 3, "num_seeds": 1, "progress": 1.0,
            "content_path": str(downloaded),
        })
        ok, _msg = qbt.download_torrent_via_qbittorrent(
            meta, str(dest_dir), dest_path, "t1",
            StubEvent([False]), Recorder(), "", StubEvent([True, False]))
        assert ok is True

    def test_backend_unavailable(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        meta, dest_dir = self._meta(tmp_path)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        with pytest.raises(qbt.BackendUnavailableError):
            qbt.download_torrent_via_qbittorrent(
                meta, str(dest_dir), str(dest_dir / "x.iso"), "t1",
                StubEvent([False]), [], "")

    def test_duplicate_409_reuses(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        session = FakeHttpSession([
            ("post", "*", SimpleFakeResponse(409)),
            ("post", "*", SimpleFakeResponse(200)),
        ])
        self._setup(monkeypatch, tmp_path, [session])
        with pytest.raises(RuntimeError):
            qbt.download_torrent_via_qbittorrent(
                meta, str(dest_dir), dest_path, "t1", StubEvent([True]), Recorder(), "")

    def test_add_not_confirmed(self, monkeypatch, tmp_path):
        import qbittorrent_backend as qbt
        _reset_refs(monkeypatch)
        meta, dest_dir = self._meta(tmp_path)
        dest_path = str(dest_dir / "game.iso")
        temp_dir = dest_dir / ".rgsx_torrent" / "abc"
        temp_dir.mkdir(parents=True)
        session = FakeHttpSession([("post", "*", SimpleFakeResponse(200, "Ok."))])
        self._setup(monkeypatch, tmp_path, [session])
        monkeypatch.setattr(qbt, "_find_existing_torrent_by_save_path", lambda s, p: None)
        clock = [10 ** 6]

        def _tick():
            clock[0] += 0.5
            return clock[0]

        fake_time_now = types.SimpleNamespace(time=_tick, sleep=lambda s: None)
        monkeypatch.setattr(qbt, "time", fake_time_now)
        with pytest.raises(RuntimeError, match="n'a pas confirmé"):
            qbt.download_torrent_via_qbittorrent(
                meta, str(dest_dir), dest_path, "t1", StubEvent([False]), Recorder(), "")


class TestSeedStatusWorker:
    def test_exits_when_entry_gone(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: None)
        monkeypatch.setattr(qbt, "_pop_active_reference", lambda **kw: ("t1", seeds["t1"]))
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        _fake_time(monkeypatch)
        qbt._seed_status_worker("t1", FakeHttpSession())  # sonsuz döngü değil

    def test_updates_seeding_status(self, monkeypatch):
        seeds = {"t1": {"hash": "h1", "tag": "rgsx_t1", "original_history_url": "url1"}}
        monkeypatch.setattr(qbt, "_active_qbt_seeds", seeds)
        info_sequence = iter([
            {"upspeed": 2048 * 1024, "state": "uploading"},
            None,
        ])
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: next(info_sequence))
        monkeypatch.setattr(qbt, "_count_active_peers", lambda s, h, f: 3)
        updates = []
        fake_network = _fake_network_module(monkeypatch)
        fake_network._update_seeding_status = lambda url, peers=0, ul_speed=0.0: updates.append((url, peers, ul_speed))
        monkeypatch.setattr(qbt, "_pop_active_reference", lambda **kw: ("t1", seeds["t1"]))
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        _fake_time(monkeypatch)
        qbt._seed_status_worker("t1", FakeHttpSession())
        assert updates and updates[0][0] == "url1"
        assert updates[0][1] == 3


# ---------------------------------------------------------------------------
# Faz 8 - Altyapi testleri: saf yardimcilar + HTTP katmani + yasam dongusu.
# Gercek qBittorrent sureci baslatilmaz; requests/subprocess tamamen mock'lanir.
# ---------------------------------------------------------------------------


class _FakeCookies:
    def clear(self):
        pass


class FakeHttpResponse:
    def __init__(self, status_code=200, text="Ok.", json_data=None):
        self.status_code = status_code
        self.text = text
        self._json = json_data
        self.content = b"payload"

    def raise_for_status(self):
        if self.status_code >= 400:
            raise requests.exceptions.HTTPError(f"HTTP {self.status_code}")

    def json(self):
        return self._json


class FakeHttpSession:
    """URL bazli yanit kuyrugu olan sahte HTTP oturumu. (m, url, yanit) uclusu."""

    def __init__(self, responses=None):
        self.cookies = _FakeCookies()
        self.calls = []
        self.responses = list(responses or [])

    def get(self, url, **kw):
        return self._pop("get", url)

    def post(self, url, **kw):
        return self._pop("post", url)

    def close(self):
        self.calls.append(("close",))

    def _pop(self, method, url):
        self.calls.append((method, url))
        for i, (m, u, r) in enumerate(self.responses):
            if m == method and (u == "*" or u == url or
                                (u.startswith("*/") and url.endswith(u[1:]))):
                return self.responses.pop(i)[2]
        return FakeHttpResponse(404)

    def posts(self):
        return [(m, u) for m, u in self.calls if m == "post"]


class StubEvent:
    """is_set() degerlerini sirayla tuke ten basit event."""

    def __init__(self, sequence):
        self._seq = list(sequence)

    def is_set(self):
        if not self._seq:
            return False
        return self._seq.pop(0)


class FakeProcess:
    def __init__(self, poll=None, wait_exc=None):
        self._poll = poll
        self.pid = 12345
        self.stdout = io.StringIO("")
        self._terminated = False
        self._killed = False
        self._wait_exc = wait_exc
        self.wait_count = 0

    def poll(self):
        return self._poll

    def terminate(self):
        self._terminated = True

    def kill(self):
        self._killed = True

    def wait(self, timeout=None):
        self.wait_count += 1
        if self._wait_exc and self.wait_count == 1:
            raise self._wait_exc
        return 0


class Recorder:
    def __init__(self):
        self.items = []

    def put(self, *args):
        self.items.append(args)


def _reset_globals(monkeypatch, base_url=None):
    """Module globallerini sabit baslangic noktasina ceker (test izolasyonu)."""
    monkeypatch.setattr(qbt, "_qbt_process", None)
    monkeypatch.setattr(qbt, "_qbt_state", qbt.STATE_STOPPED)
    monkeypatch.setattr(qbt, "_base_url",
                        base_url or f"http://127.0.0.1:{qbt._TARGET_PORT}")
    monkeypatch.setattr(qbt, "_active_qbt_downloads", {})
    monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
    monkeypatch.setattr(qbt, "_selected_file_indexes_by_hash", {})
    monkeypatch.setattr(qbt, "_url", lambda p: p)
    monkeypatch.setattr(qbt, "_base_url", base_url or "http://127.0.0.1:18572")


def _reset_process(monkeypatch):
    """TestEnsureQbittorrentRunning izolasyonu: surec/state globallerini sifirlar."""
    monkeypatch.setattr(qbt, "_qbt_process", None)
    monkeypatch.setattr(qbt, "_qbt_state", qbt.STATE_STOPPED)
    monkeypatch.setattr(qbt, "_base_url", f"http://127.0.0.1:{qbt._TARGET_PORT}")


def _reset_refs(monkeypatch):
    """Indirme/seed referans kayitlarini ve secili dosya haritasini sifirlar."""
    monkeypatch.setattr(qbt, "_active_qbt_downloads", {})
    monkeypatch.setattr(qbt, "_active_qbt_seeds", {})
    monkeypatch.setattr(qbt, "_selected_file_indexes_by_hash", {})


class TestPureHelpers:
    def test_generate_random_password_length_and_uniqueness(self):
        first = qbt.generate_random_password()
        second = qbt.generate_random_password()
        assert len(first) >= 16
        assert first != second

    def test_url_uses_base(self, monkeypatch):
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:9999")
        assert qbt._url("/api/x") == "http://127.0.0.1:9999/api/x"

    def test_extract_temp_password_english(self):
        lines = ["2026-01-01 log line",
                 "A temporary password is provided for this session: TempPw123"]
        assert qbt._extract_temp_password(lines) == "TempPw123"

    def test_extract_temp_password_french(self):
        lines = ["mot de passe temporaire : SecretFr45"]
        assert qbt._extract_temp_password(lines) == "SecretFr45"

    def test_extract_temp_password_missing(self):
        assert qbt._extract_temp_password(["nothing here"]) is None

    def test_build_torrent_headers(self):
        headers = qbt._build_torrent_headers()
        assert "User-Agent" in headers
        assert headers["Accept"] == "*/*"

    def test_is_process_running_with_global(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_process", None)
        assert qbt.is_process_running() is False
        monkeypatch.setattr(qbt, "_qbt_process", FakeProcess(poll=None))
        assert qbt.is_process_running() is True
        monkeypatch.setattr(qbt, "_qbt_process", FakeProcess(poll=0))
        assert qbt.is_process_running() is False

    def test_state_set_and_get(self, monkeypatch):
        monkeypatch.setattr(qbt, "_qbt_state", qbt.STATE_STOPPED)
        qbt._set_qbt_state(qbt.STATE_STARTING, "test")
        assert qbt.get_backend_state() == qbt.STATE_STARTING
        qbt._set_qbt_state(qbt.STATE_STARTING, "no change")
        assert qbt.get_backend_state() == qbt.STATE_STARTING


class TestEnsureIniSettings:
    def test_creates_file_with_sections(self, tmp_path):
        ini = tmp_path / "qBittorrent.ini"
        qbt._ensure_ini_settings(str(ini), {"LegalNotice": {"Accepted": "true"}})
        content = ini.read_text()
        assert "[LegalNotice]" in content
        assert "Accepted=true" in content

    def test_preserves_existing_values(self, tmp_path):
        ini = tmp_path / "qBittorrent.ini"
        ini.write_text("[LegalNotice]\nAccepted=false\n")
        qbt._ensure_ini_settings(str(ini), {
            "LegalNotice": {"Accepted": "true"},
            "Preferences": {"WebUI\\Port": "18573"},
        })
        content = ini.read_text()
        assert "Accepted=false" in content
        assert "[Preferences]" in content
        assert "WebUI\\Port=18573" in content

    def test_existing_section_key_kept_untouched(self, tmp_path):
        ini = tmp_path / "qBittorrent.ini"
        ini.write_text("[Preferences]\nWebUI\\Port=8080\n")
        qbt._ensure_ini_settings(str(ini), {
            "Preferences": {"WebUI\\Port": "18573", "WebUI\\Enabled": "true"},
        })
        content = ini.read_text()
        assert "WebUI\\Port=8080" in content
        assert "WebUI\\Enabled=true" in content


class TestPreseedProfiles:
    def test_linux_profile_writes_port(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        qbt._preseed_linux_profile(18573)
        ini = tmp_path / "qBittorrent" / "config" / "qBittorrent.conf"
        content = ini.read_text()
        assert "WebUI\\Port=18573" in content
        assert "WebUI\\Enabled=true" in content

    def test_linux_profile_migrates_localhost_only(self, tmp_path, monkeypatch):
        ini = tmp_path / "qBittorrent" / "config" / "qBittorrent.conf"
        ini.parent.mkdir(parents=True)
        ini.write_text(
            "[Preferences]\n"
            "WebUI\\Address=127.0.0.1\n"
            "WebUI\\AuthSubnetWhitelist=127.0.0.1/32\n"
            "WebUI\\AuthSubnetWhitelistEnabled=true\n"
        )
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        qbt._preseed_linux_profile(18572)
        content = ini.read_text()
        assert "WebUI\\Address=0.0.0.0" in content
        assert "WebUI\\AuthSubnetWhitelistEnabled=false" in content
        assert "WebUI\\Port=18572" in content

    def test_windows_profile_migrates_localhost_only(self, tmp_path, monkeypatch):
        ini = tmp_path / "data" / "profile" / "qBittorrent" / "config" / "qBittorrent.ini"
        ini.parent.mkdir(parents=True)
        ini.write_text(
            "[Preferences]\n"
            "WebUI\\Address=127.0.0.1\n"
            "WebUI\\AuthSubnetWhitelist=127.0.0.1/32\n"
        )
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        qbt._preseed_windows_profile(18574)
        content = ini.read_text()
        assert "WebUI\\Address=0.0.0.0" in content
        assert "WebUI\\Port=18574" in content


class TestExtractPortableWindows:
    def _fake_isfile(self, results):
        iterator = iter(results)

        def _isfile(path):
            try:
                return next(iterator)
            except StopIteration:
                return True

        return _isfile

    def test_launcher_already_present(self, tmp_path, monkeypatch):
        launcher = tmp_path / "qbittorrent-portable.exe"
        launcher.write_bytes(b"x")
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        assert qbt._extract_portable_windows() == str(launcher)

    def test_missing_7z_returns_none(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path / "none"))
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(tmp_path / "missing.7z"))
        assert qbt._extract_portable_windows() is None

    def test_successful_extraction(self, tmp_path, monkeypatch):
        launcher = tmp_path / "qbittorrent-portable.exe"
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(tmp_path / "x.7z"))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(SEVEN_Z_EXE="7z"))
        monkeypatch.setattr(subprocess, "run",
                            lambda *a, **k: types.SimpleNamespace(returncode=0, stdout=""))
        monkeypatch.setattr(os.path, "isfile",
                            self._fake_isfile([False, True, True]))
        assert qbt._extract_portable_windows() == str(launcher)

    def test_failed_extraction_returns_none(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(tmp_path / "x.7z"))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(SEVEN_Z_EXE="7z"))
        monkeypatch.setattr(subprocess, "run",
                            lambda *a, **k: types.SimpleNamespace(returncode=1, stdout="boom"))
        monkeypatch.setattr(os.path, "isfile", self._fake_isfile([False, True, False]))
        assert qbt._extract_portable_windows() is None

    def test_extraction_exception_returns_none(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_PORTABLE_7Z", str(tmp_path / "x.7z"))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(SEVEN_Z_EXE="7z"))

        def boom(*a, **k):
            raise OSError("7z introuvable")

        monkeypatch.setattr(subprocess, "run", boom)
        monkeypatch.setattr(os.path, "isfile", self._fake_isfile([False, True]))
        assert qbt._extract_portable_windows() is None


class TestFindExecutable:
    def test_linux_bundled_nox_chmod(self, tmp_path, monkeypatch):
        nox = tmp_path / "qbittorrent-nox_linux"
        nox.write_bytes(b"x")
        nox.chmod(0o644)
        monkeypatch.setattr(qbt, "_NOX_LINUX", str(nox))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Android"))
        assert qbt._find_qbittorrent_executable() == str(nox)
        assert os.stat(nox).st_mode & 0o111

    def test_linux_which_fallback(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_NOX_LINUX", str(tmp_path / "missing"))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Android"))
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: types.SimpleNamespace(stdout=""))

        import shutil
        monkeypatch.setattr(shutil, "which",
                            lambda name: "/usr/bin/qbittorrent-nox" if name == "qbittorrent-nox" else None)
        assert qbt._find_qbittorrent_executable() == "/usr/bin/qbittorrent-nox"

    def test_linux_nothing_found(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_NOX_LINUX", str(tmp_path / "missing"))
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Android"))

        import shutil
        monkeypatch.setattr(shutil, "which", lambda name: None)
        assert qbt._find_qbittorrent_executable() is None

    def test_windows_registry_fallback(self, monkeypatch):
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Windows"))
        monkeypatch.setattr(qbt, "_extract_portable_windows", lambda: None)

        import shutil
        monkeypatch.setattr(shutil, "which", lambda name: None)
        monkeypatch.setattr(subprocess, "run",
                            lambda *a, **k: types.SimpleNamespace(stdout='  InstallDir    REG_SZ   C:\\Program Files\\qBittorrent\n'))
        target = os.path.join("C:\\Program Files\\qBittorrent", "qbittorrent.exe")
        monkeypatch.setattr(os.path, "isfile", lambda p: p == target)
        assert qbt._find_qbittorrent_executable() == target

    def test_windows_registry_read_failure(self, monkeypatch):
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Windows"))
        monkeypatch.setattr(qbt, "_extract_portable_windows", lambda: None)

        import shutil
        monkeypatch.setattr(shutil, "which", lambda name: None)

        def boom(*a, **k):
            raise OSError("reg yok")

        monkeypatch.setattr(subprocess, "run", boom)
        monkeypatch.setattr(os.path, "isfile", lambda p: False)
        assert qbt._find_qbittorrent_executable() is None

    def test_is_available(self, monkeypatch):
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: "/usr/bin/qbittorrent-nox")
        assert qbt.is_available() is True
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: None)
        assert qbt.is_available() is False


class TestSocketHelpers:
    def test_is_port_open_closed(self):
        s = _SocketBindHelper()
        port = s.bind_ephemeral()
        s.close()
        assert qbt._is_port_open("127.0.0.1", port) is False

    def test_is_port_open_listening(self):
        s = _SocketBindHelper()
        port = s.listen_ephemeral()
        try:
            assert qbt._is_port_open("127.0.0.1", port) is True
        finally:
            s.close()


class _SocketBindHelper:
    def __init__(self):
        import socket
        self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    def bind_ephemeral(self):
        self._sock.bind(("127.0.0.1", 0))
        return self._sock.getsockname()[1]

    def listen_ephemeral(self):
        self._sock.setsockopt(__import__("socket").SOL_SOCKET, __import__("socket").SO_REUSEADDR, 1)
        self._sock.bind(("127.0.0.1", 0))
        self._sock.listen(1)
        return self._sock.getsockname()[1]

    def close(self):
        self._sock.close()


class TestWaitForWebui:
    def test_accepts_200(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(200))])
        assert qbt._wait_for_webui(session, "http://x", timeout=5) is True

    def test_accepts_403(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(403))])
        assert qbt._wait_for_webui(session, "http://x", timeout=5) is True

    def test_rejects_500_then_succeeds(self):
        session = FakeHttpSession([
            ("get", "*", FakeHttpResponse(500)),
            ("get", "*", FakeHttpResponse(200)),
        ])

        now = [0.0]

        def _time():
            return now[0]

        def _sleep(_s):
            now[0] += 1.0

        monkeypatch_time = _MonkeypatchTime(_time, _sleep)
        monkeypatch_time.apply()
        try:
            assert qbt._wait_for_webui(session, "http://x", timeout=5) is True
        finally:
            monkeypatch_time.restore()

    def test_timeout_on_exception(self):
        class BoomSession:
            def get(self, url, **kw):
                raise requests.exceptions.ConnectionError("refused")

        values = iter([0.0, 0.1, 0.9])

        def _time():
            return next(values)

        monkeypatch_time = _MonkeypatchTime(_time, lambda s: None)
        monkeypatch_time.apply()
        try:
            assert qbt._wait_for_webui(BoomSession(), "http://x", timeout=0.3) is False
        finally:
            monkeypatch_time.restore()


class _MonkeypatchTime:
    """qbt.time'i kisa omurlu olarak degistirir (sinirli kullanim icin)."""

    def __init__(self, time_fn, sleep_fn):
        self._orig = qbt.time
        self._time_fn = time_fn
        self._sleep_fn = sleep_fn

    def apply(self):
        qbt.time = types.SimpleNamespace(time=self._time_fn, sleep=self._sleep_fn)

    def restore(self):
        qbt.time = self._orig


class TestLogin:
    def test_success_with_configured_password(self, monkeypatch):
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200, "Ok."))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is True

    def test_success_with_temp_password_priority(self, monkeypatch):
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200, "Ok."))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "")
        assert qbt._login(session, "http://127.0.0.1:18572",
                          ["temporary password: TempPass123"]) is True

    def test_banned_returns_false(self, monkeypatch):
        session = FakeHttpSession([("post", "*", FakeHttpResponse(403, "ip has been banned"))])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is False

    def test_localhost_bypass_success(self, monkeypatch):
        session = FakeHttpSession([
            ("post", "*", FakeHttpResponse(403, "Fails.")),
            ("get", "*/api/v2/app/preferences", FakeHttpResponse(200, "")),
        ])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        assert qbt._login(session, "http://127.0.0.1:18572", []) is True

    def test_retry_loop_retries_temp_password(self, monkeypatch):
        session = FakeHttpSession([
            ("post", "*", FakeHttpResponse(403, "Fails.")),
            ("post", "*", FakeHttpResponse(403, "Fails.")),
            ("get", "*/api/v2/app/preferences", FakeHttpResponse(500, "err")),
            ("post", "*", FakeHttpResponse(204, "")),
        ])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        monkeypatch_time = _MonkeypatchTime(lambda: 0.0, lambda s: None)
        monkeypatch_time.apply()
        try:
            assert qbt._login(session, "http://127.0.0.1:18572",
                              ["temporary password: TempPass123"], timeout=2) is True
        finally:
            monkeypatch_time.restore()

    def test_total_failure(self, monkeypatch):
        session = FakeHttpSession([
            ("post", "*", FakeHttpResponse(403, "Fails.")),
            ("post", "*", FakeHttpResponse(403, "Fails.")),
            ("get", "*/api/v2/app/preferences", FakeHttpResponse(500, "err")),
        ])
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")

        now = [0.0]

        def _time():
            return now[0]

        def _sleep(_s):
            now[0] += 0.5

        monkeypatch_time = _MonkeypatchTime(_time, _sleep)
        monkeypatch_time.apply()
        try:
            assert qbt._login(session, "http://127.0.0.1:18572", [], timeout=0.01) is False
        finally:
            monkeypatch_time.restore()

    def test_post_exception_returns_false(self, monkeypatch):
        class BoomSession:
            cookies = _FakeCookies()

            def post(self, url, **kw):
                raise requests.exceptions.ConnectionError("refused")

            def get(self, url, **kw):
                raise requests.exceptions.ConnectionError("refused")

        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp")
        now = [0.0]

        def _time():
            return now[0]

        def _sleep(_s):
            now[0] += 0.5

        monkeypatch_time = _MonkeypatchTime(_time, _sleep)
        monkeypatch_time.apply()
        try:
            assert qbt._login(BoomSession(), "http://127.0.0.1:18572", [], timeout=0.01) is False
        finally:
            monkeypatch_time.restore()


class TestEnsureRunning:
    def test_reuses_live_process(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_recovers_after_retry(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_qbt_process", proc)

        waits = iter([False, True])
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: next(waits))
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        monkeypatch_time = _MonkeypatchTime(lambda: 0.0, lambda s: None)
        monkeypatch_time.apply()
        try:
            session = qbt._ensure_qbittorrent_running()
        finally:
            monkeypatch_time.restore()
        assert session is not None
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_binary_missing_returns_none(self, monkeypatch):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: None)
        assert qbt._ensure_qbittorrent_running() is None
        assert qbt.get_backend_state() == qbt.STATE_STOPPED

    def test_no_free_port_returns_none(self, monkeypatch):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable", lambda: "/bin/fake")
        monkeypatch.setattr(qbt, "_profile_dir", "/tmp/fake-profile")
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 0)
        assert qbt._ensure_qbittorrent_running() is None
        assert qbt.get_backend_state() == qbt.STATE_STOPPED

    def test_full_linux_start(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch, base_url="http://127.0.0.1:18572")
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable",
                            lambda: str(tmp_path / "qbittorrent-nox"))
        monkeypatch.setattr(qbt, "_preseed_linux_profile", lambda port: None)
        monkeypatch.setattr(qbt, "_preseed_windows_profile", lambda port: None)
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 19000)
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: False)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        monkeypatch.setattr(subprocess, "Popen", lambda cmd, **kw: proc)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert qbt._base_url == "http://127.0.0.1:19000"
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_port_occupied_between_probe_and_launch(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch, base_url="http://127.0.0.1:18572")
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable",
                            lambda: str(tmp_path / "qbittorrent-nox"))
        monkeypatch.setattr(qbt, "_preseed_linux_profile", lambda port: None)
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 18575)
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: True)
        monkeypatch.setattr(qbt, "_terminate_existing_qbittorrent_processes", lambda: None)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        monkeypatch.setattr(subprocess, "Popen", lambda cmd, **kw: proc)
        session = qbt._ensure_qbittorrent_running()
        assert session is not None
        assert qbt._base_url == "http://127.0.0.1:18575"

    def test_reuses_probed_existing_instance(self, monkeypatch):
        _reset_globals(monkeypatch, base_url="http://127.0.0.1:18572")
        existing = FakeHttpSession()
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([18572]))
        monkeypatch.setattr(qbt, "_probe_existing_webui_session", lambda port: existing)
        monkeypatch.setattr(qbt, "maybe_migrate_qbittorrent_password", lambda s, l: None)
        session = qbt._ensure_qbittorrent_running()
        assert session is existing
        assert qbt.get_backend_state() == qbt.STATE_RUNNING

    def test_popen_failure_returns_none(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_webui_port_candidates", lambda: iter([]))
        monkeypatch.setattr(qbt, "_find_qbittorrent_executable",
                            lambda: str(tmp_path / "qbittorrent-nox"))
        monkeypatch.setattr(qbt, "_preseed_linux_profile", lambda port: None)
        monkeypatch.setattr(qbt, "_profile_dir", str(tmp_path))
        monkeypatch.setattr(qbt, "_find_free_webui_port", lambda: 19001)
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: False)

        def boom(*a, **kw):
            raise OSError("Popen basarisiz")

        monkeypatch.setattr(subprocess, "Popen", boom)
        assert qbt._ensure_qbittorrent_running() is None


class TestProbeExisting:
    def test_success_path(self, monkeypatch):
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: True)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        session = qbt._probe_existing_webui_session(18572)
        assert session is not None

    def test_login_failure_returns_none(self, monkeypatch):
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: True)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: False)
        assert qbt._probe_existing_webui_session(18572) is None

    def test_webui_not_ready_returns_none(self, monkeypatch):
        monkeypatch.setattr(qbt, "_is_port_open", lambda h, p: True)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        assert qbt._probe_existing_webui_session(18572) is None


class TestTerminateManagedProcess:
    def test_no_process_noop(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._terminate_managed_process()
        assert qbt._qbt_process is None

    def test_terminate_running(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt._terminate_managed_process()
        assert proc._terminated is True
        assert qbt._qbt_process is None

    def test_terminate_timeout_kills(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None, wait_exc=subprocess.TimeoutExpired("cmd", 5))
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt._terminate_managed_process()
        assert proc._terminated is True
        assert proc._killed is True
        assert qbt._qbt_process is None


class TestShutdown:
    def test_shutdown_terminates_process(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt.shutdown()
        assert proc._terminated is True
        assert qbt._qbt_process is None

    def test_shutdown_no_process(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt.shutdown()
        assert qbt._qbt_process is None

    def test_shutdown_kill_on_exception(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None, wait_exc=RuntimeError("boom"))
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        qbt.shutdown()
        assert proc._killed is True


class TestTerminateExistingProcesses:
    def test_linux_kills_qbittorrent_pids(self, monkeypatch):
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Android"))
        result = types.SimpleNamespace(
            stdout="  1234 qbittorrent-nox\n  5678 /usr/bin/qbittorrent\n  9999 python qbt.py\n"
        )
        monkeypatch.setattr(subprocess, "run", lambda *a, **k: result)
        kills = []

        def fake_kill(pid, sig):
            kills.append((pid, sig))
            if sig == signal.SIGTERM and pid == 5678:
                raise PermissionError()
            if sig == 0 and pid == 5678:
                raise ProcessLookupError()

        monkeypatch.setattr(os, "kill", fake_kill)
        monkeypatch_time = _MonkeypatchTime(lambda: 0.0, lambda s: None)
        monkeypatch_time.apply()
        try:
            qbt._terminate_existing_qbittorrent_processes()
        finally:
            monkeypatch_time.restore()
        killed_pids = [pid for pid, _ in kills]
        assert 1234 in killed_pids
        assert 5678 in killed_pids
        assert 9999 not in killed_pids
        assert any(sig == signal.SIGKILL for _, sig in kills)

    def test_windows_taskkill(self, monkeypatch):
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Windows"))
        called = []
        monkeypatch.setattr(subprocess, "run",
                            lambda *a, **k: called.append(a) or types.SimpleNamespace(stdout=""))
        qbt._terminate_existing_qbittorrent_processes()
        assert called and "taskkill" in called[0][0]

    def test_ps_failure_ignored(self, monkeypatch):
        monkeypatch.setattr(qbt, "config", types.SimpleNamespace(OPERATING_SYSTEM="Android"))

        def boom(*a, **k):
            raise OSError("ps yok")

        monkeypatch.setattr(subprocess, "run", boom)
        qbt._terminate_existing_qbittorrent_processes()


class TestTorrentInfoHelpers:
    def test_info_by_tag(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(200, json_data=[{"hash": "h1"}]))])
        assert qbt._torrent_info_by_tag(session, "tag1") == {"hash": "h1"}

    def test_info_by_tag_empty(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(200, json_data=[]))])
        assert qbt._torrent_info_by_tag(session, "tag1") is None

    def test_info_by_tag_error(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(500))])
        assert qbt._torrent_info_by_tag(session, "tag1") is None

    def test_info_by_hash(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(200, json_data=[{"hash": "h1"}]))])
        assert qbt._torrent_info_by_hash(session, "h1") == {"hash": "h1"}

    def test_find_existing_by_save_path(self):
        session = FakeHttpSession([
            ("get", "*", FakeHttpResponse(200, json_data=[
                {"save_path": "/dl/a"}, {"save_path": "/dl/b"},
            ])),
        ])
        found = qbt._find_existing_torrent_by_save_path(session, "/dl/b")
        assert found is not None and found["save_path"] == "/dl/b"

    def test_find_existing_not_matching(self):
        session = FakeHttpSession([
            ("get", "*", FakeHttpResponse(200, json_data=[{"save_path": "/dl/a"}])),
        ])
        assert qbt._find_existing_torrent_by_save_path(session, "/dl/z") is None

    def test_find_existing_error(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(500))])
        assert qbt._find_existing_torrent_by_save_path(session, "/dl/a") is None

    def test_torrent_files(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(200, json_data=[{"index": 0}]))])
        assert qbt._torrent_files(session, "h1") == [{"index": 0}]

    def test_torrent_files_error(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(500))])
        assert qbt._torrent_files(session, "h1") == []

    def test_count_active_peers(self):
        session = FakeHttpSession([
            ("get", "*", FakeHttpResponse(200, json_data={
                "peers": {"a": {"dl_speed": 5}, "b": {"dl_speed": 0}},
            })),
        ])
        assert qbt._count_active_peers(session, "h1", "dl_speed") == 1

    def test_count_active_peers_error(self):
        session = FakeHttpSession([("get", "*", FakeHttpResponse(500))])
        assert qbt._count_active_peers(session, "h1", "dl_speed") == 0


class TestResolveTargetFileIndex:
    def test_match_by_name(self):
        files = [{"index": 1, "name": "a/b/c.iso"}, {"index": 2, "name": "c.iso"}]
        assert qbt._resolve_target_file_index(files, "c.iso") == 1

    def test_match_by_exact_name_when_first(self):
        files = [{"index": 2, "name": "c.iso"}, {"index": 1, "name": "a/b/c.iso"}]
        assert qbt._resolve_target_file_index(files, "c.iso") == 2

    def test_match_by_full_path(self):
        files = [{"index": 1, "name": "a/b/c.iso"}, {"index": 2, "name": "d.iso"}]
        assert qbt._resolve_target_file_index(files, "a/b/c.iso") == 1

    def test_no_match(self):
        files = [{"index": 1, "name": "a.iso"}]
        assert qbt._resolve_target_file_index(files, "missing.iso") is None


class TestApplyFileSelection:
    def _setup(self, monkeypatch, files):
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200)),
                                   ("post", "*", FakeHttpResponse(200))])
        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: files)
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        return session

    def test_no_files(self, monkeypatch):
        session = self._setup(monkeypatch, [])
        assert qbt._apply_file_selection(session, "h1", "x.iso") == (None, 0)

    def test_single_file(self, monkeypatch):
        session = self._setup(monkeypatch, [{"index": 0, "size": 123}])
        assert qbt._apply_file_selection(session, "h1", "x.iso") == (0, 123)

    def test_multi_file_target_found(self, monkeypatch):
        session = self._setup(monkeypatch, [
            {"index": 1, "name": "a.iso", "size": 100},
            {"index": 2, "name": "b.iso", "size": 200},
        ])
        idx, size = qbt._apply_file_selection(session, "h1", "b.iso")
        assert idx == 2
        assert size == 200
        assert len(session.posts()) == 2

    def test_target_unresolved(self, monkeypatch):
        session = self._setup(monkeypatch, [
            {"index": 1, "name": "a.iso", "size": 100},
            {"index": 2, "name": "b.iso", "size": 200},
        ])
        assert qbt._apply_file_selection(session, "h1", "nope.iso") == (None, 0)

    def test_single_file_short_circuits_any_name(self, monkeypatch):
        session = self._setup(monkeypatch, [
            {"index": 1, "name": "a.iso", "size": 100},
        ])
        assert qbt._apply_file_selection(session, "h1", "nope.iso") == (0, 100)

    def test_file_prio_request_exception(self, monkeypatch):
        class BoomPost:
            def post(self, url, **kw):
                raise requests.exceptions.ConnectionError("x")

        monkeypatch.setattr(qbt, "_torrent_files", lambda s, h: [
            {"index": 1, "name": "a.iso", "size": 100},
            {"index": 2, "name": "b.iso", "size": 200},
        ])
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        idx, size = qbt._apply_file_selection(BoomPost(), "h1", "b.iso")
        assert idx == 2
        assert size == 200


class TestGetTargetFileProgress:
    def test_none_index_fallback(self):
        assert qbt._get_target_file_progress([], None, 500) == (0, 500, False)

    def test_matched_file_partial(self):
        files = [{"index": 2, "size": 1000, "progress": 0.5}]
        assert qbt._get_target_file_progress(files, 2, 500) == (500, 1000, False)

    def test_matched_file_completed(self):
        files = [{"index": 2, "size": 1000, "progress": 1.0}]
        assert qbt._get_target_file_progress(files, 2, 500) == (1000, 1000, True)

    def test_index_not_found(self):
        files = [{"index": 1, "size": 1000}]
        assert qbt._get_target_file_progress(files, 9, 500) == (0, 500, False)

    def test_malformed_values(self):
        files = [{"index": 2, "size": "not-a-number", "progress": "x"}]
        result = qbt._get_target_file_progress(files, 2, 500)
        assert result == (0, 500, False)


class TestActiveReferences:
    def test_full_reference_lifecycle(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._register_active_download("t1", "hash1", "url1", 3)
        assert qbt._has_other_hash_references("hash1") is True
        assert qbt._has_other_hash_references("hash9") is False

        qbt._promote_active_download_to_seed("t1", {"hash": "hash1", "tag": "rgsx_t1",
                                                    "original_history_url": "url1"})
        assert qbt._active_qbt_downloads.get("t1") is None
        assert qbt.has_active_seed("t1") is True
        assert qbt.has_active_seed(original_history_url="url1") is True

        task, entry = qbt._pop_active_reference(task_id="t1")
        assert task == "t1"
        assert qbt.has_active_seed("t1") is False

        qbt._register_active_download("t2", "hash2", "url2", None)
        task, entry = qbt._pop_active_reference(original_history_url="url2")
        assert task == "t2"

        assert qbt._pop_active_reference(task_id="missing") == (None, None)

    def test_shared_hash_reference_check(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._register_active_download("t1", "shared", "url1", None)
        qbt._promote_active_download_to_seed("t2", {"hash": "shared", "tag": "rgsx_t2"})
        assert qbt._has_other_hash_references("shared", exclude_task_id="t2") is True
        assert qbt._has_other_hash_references("shared", exclude_task_id="t1") is True

    def test_cleanup_hash_state(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._selected_file_indexes_by_hash["orphan"] = {2}
        qbt._cleanup_hash_state_if_unused("orphan")
        assert "orphan" not in qbt._selected_file_indexes_by_hash

    def test_cleanup_kept_when_referenced(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._register_active_download("t1", "kept", "url1", None)
        qbt._selected_file_indexes_by_hash["kept"] = {2}
        qbt._cleanup_hash_state_if_unused("kept")
        assert "kept" in qbt._selected_file_indexes_by_hash


class TestPrewarm:
    def test_success(self, monkeypatch):
        _reset_globals(monkeypatch)
        session = FakeHttpSession()
        closed = []
        session.close = lambda: closed.append(True)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        assert qbt.prewarm_startup() is True
        assert closed == [True]

    def test_unavailable(self, monkeypatch):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        assert qbt.prewarm_startup() is False

    def test_exception(self, monkeypatch):
        _reset_globals(monkeypatch)

        def boom():
            raise RuntimeError("x")

        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", boom)
        assert qbt.prewarm_startup() is False

    def test_async_single_thread(self, monkeypatch):
        _reset_globals(monkeypatch)
        started = []
        release = threading.Event()

        def fake_prewarm():
            started.append(True)
            release.wait(2)

        monkeypatch.setattr(qbt, "prewarm_startup", fake_prewarm)
        qbt.prewarm_startup_async()
        qbt.prewarm_startup_async()
        assert len(started) == 1
        release.set()
        thread = qbt._prewarm_thread
        if thread is not None:
            thread.join(timeout=3)
        assert qbt._prewarm_thread is None

    def test_async_skips_when_process_running(self, monkeypatch):
        _reset_globals(monkeypatch)
        proc = FakeProcess(poll=None)
        monkeypatch.setattr(qbt, "_qbt_process", proc)
        called = []
        monkeypatch.setattr(qbt, "prewarm_startup", lambda: called.append(True))
        qbt.prewarm_startup_async()
        assert called == []

    def test_async_skips_when_thread_alive(self, monkeypatch):
        _reset_globals(monkeypatch)
        blocked = threading.Event()

        def fake_prewarm():
            blocked.wait(2)

        monkeypatch.setattr(qbt, "prewarm_startup", fake_prewarm)
        qbt.prewarm_startup_async()
        try:
            qbt.prewarm_startup_async()
            assert qbt._prewarm_thread is not None
        finally:
            blocked.set()
            thread = qbt._prewarm_thread
            if thread is not None:
                thread.join(timeout=3)


class TestEnsureRunningApi:
    def test_success(self, monkeypatch):
        _reset_globals(monkeypatch)
        session = FakeHttpSession()
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        assert qbt.ensure_running(timeout=5) is True

    def test_unavailable(self, monkeypatch):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        assert qbt.ensure_running(timeout=5) is False

    def test_webui_never_ready(self, monkeypatch):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: False)
        monkeypatch.setattr(_time, "sleep", lambda s: None)
        assert qbt.ensure_running(timeout=0.01) is False


class TestHasActiveSeed:
    def test_seed_lookups(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {"original_history_url": "http://x/1"}
        assert qbt.has_active_seed(task_id="t1") is True
        assert qbt.has_active_seed(original_history_url="http://x/1") is True
        assert qbt.has_active_seed(task_id="t2") is False
        assert qbt.has_active_seed() is False


class TestSeedStatusWorker:
    def test_stops_when_info_gone(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {
            "hash": "h1", "tag": "rgsx_t1", "original_history_url": "http://x/1",
        }
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: None)
        monkeypatch_time = _MonkeypatchTime(lambda: 0.0, lambda s: None)
        monkeypatch_time.apply()
        try:
            qbt._seed_status_worker("t1", None)
        finally:
            monkeypatch_time.restore()
        assert "t1" not in qbt._active_qbt_seeds


class TestStopSeed:
    def test_no_active_and_not_running(self, monkeypatch):
        _reset_globals(monkeypatch)
        assert qbt.stop_seed(task_id="t1") is False

    def test_shared_hash_detaches(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {
            "hash": "h1", "tag": "rgsx_t1", "original_history_url": "http://x/1",
        }
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: True)
        monkeypatch.setitem(sys.modules, "network",
                            types.SimpleNamespace(_stop_seeding_status=lambda *a, **k: None))
        assert qbt.stop_seed(task_id="t1") is True
        assert "t1" not in qbt._active_qbt_seeds

    def test_deletes_torrent(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {
            "hash": "h1", "tag": "rgsx_t1", "original_history_url": "http://x/1",
        }
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200))])
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: session)
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_torrent_info_by_tag",
                            lambda s, tag: {"hash": "h1"})
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        monkeypatch.setitem(sys.modules, "network",
                            types.SimpleNamespace(_stop_seeding_status=lambda *a, **k: None))
        assert qbt.stop_seed(task_id="t1") is True
        assert any("delete" in url for _, url in session.calls)

    def test_torrent_already_gone(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {
            "hash": "h1", "tag": "rgsx_t1", "original_history_url": "http://x/1",
        }
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: True)
        monkeypatch.setattr(qbt, "_login", lambda s, u, p: True)
        monkeypatch.setattr(qbt, "_torrent_info_by_tag", lambda s, tag: None)
        assert qbt.stop_seed(task_id="t1") is True

    def test_login_failure(self, monkeypatch):
        _reset_globals(monkeypatch)
        qbt._active_qbt_seeds["t1"] = {
            "hash": "h1", "tag": "rgsx_t1", "original_history_url": "http://x/1",
        }
        monkeypatch.setattr(qbt, "is_process_running", lambda: True)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(requests, "Session", lambda: FakeHttpSession())
        monkeypatch.setattr(qbt, "_wait_for_webui", lambda s, u, timeout: False)
        assert qbt.stop_seed(task_id="t1") is False


class TestResolveDownloadedFile:
    def test_content_path_is_file(self, tmp_path):
        f = tmp_path / "x.iso"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file(str(f), str(tmp_path), "a/b/x.iso", "x.iso") == str(f)

    def test_expected_path(self, tmp_path):
        (tmp_path / "a" / "b").mkdir(parents=True)
        f = tmp_path / "a" / "b" / "x.iso"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file("", str(tmp_path), "a/b/x.iso", "x.iso") == str(f)

    def test_walk_fallback(self, tmp_path):
        sub = tmp_path / "sub"
        sub.mkdir()
        f = sub / "game.bin"
        f.write_bytes(b"x")
        assert qbt._resolve_downloaded_file("", str(tmp_path), "", "game.bin") == str(f)

    def test_not_found(self, tmp_path):
        assert qbt._resolve_downloaded_file("", str(tmp_path), "", "missing.iso") is None


class TestMaybeMigratePassword:
    def _migrate_fixture(self, monkeypatch, settings):
        import rgsx_settings
        applied = []
        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", lambda: settings)
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_migration_done", lambda v: None)
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_mode", lambda m: None)
        monkeypatch.setattr(qbt, "_extract_temp_password", lambda lines: None)
        monkeypatch.setattr(qbt, "generate_random_password", lambda: "rtG4_9Q2xLmZ7vPw")
        monkeypatch.setattr(qbt, "_apply_webui_password",
                            lambda s, pw: applied.append(pw))
        monkeypatch.setattr(qbt, "_notify_password_migrated", lambda: None)
        return applied

    def test_settings_unreadable_failed(self, monkeypatch):
        import rgsx_settings

        def boom():
            raise RuntimeError("settings yok")

        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings", boom)
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "failed"

    def test_already_done(self, monkeypatch):
        import rgsx_settings
        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings",
                            lambda: {"migration_v1_done": True})
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "already_done"

    def test_no_stored_password_migrates(self, monkeypatch):
        applied = self._migrate_fixture(monkeypatch, {})
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "migrated"
        assert applied == ["rtG4_9Q2xLmZ7vPw"]

    def test_default_password_rotated(self, monkeypatch):
        applied = self._migrate_fixture(monkeypatch, {"qbittorrent_webui_password": "admin"})
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "migrated"
        assert applied == ["rtG4_9Q2xLmZ7vPw"]

    def test_user_defined_password_noop(self, monkeypatch):
        import rgsx_settings
        done = []
        monkeypatch.setattr(rgsx_settings, "load_rgsx_settings",
                            lambda: {"qbittorrent_webui_password": "monMdp-2026"})
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_password_migration_done",
                            lambda v: done.append(v))
        assert qbt.maybe_migrate_qbittorrent_password(None, []) == "noop"
        assert done == [True]


class TestNotifyPasswordMigrated:
    def test_broadcast_and_toast(self, monkeypatch):
        broadcast = []
        monkeypatch.setitem(sys.modules, "rgsx_manager",
                            types.SimpleNamespace(SUBSCRIBERS=[1],
                                                  _broadcast=lambda t, p: broadcast.append((t, p))))
        toasts = []
        monkeypatch.setitem(sys.modules, "display",
                            types.SimpleNamespace(show_toast=lambda msg, duration=0: toasts.append(msg)))
        monkeypatch.setitem(sys.modules, "rgsx_web",
                            types.SimpleNamespace(get_translation=lambda k: "msg translated"))
        qbt._notify_password_migrated()
        assert broadcast == [("toast", {"message": "msg translated"})]
        assert toasts == ["msg translated"]

    def test_fallback_message(self, monkeypatch):
        broadcast = []
        monkeypatch.setitem(sys.modules, "rgsx_manager",
                            types.SimpleNamespace(SUBSCRIBERS=[1],
                                                  _broadcast=lambda t, p: broadcast.append((t, p))))
        toasts = []
        monkeypatch.setitem(sys.modules, "display",
                            types.SimpleNamespace(show_toast=lambda msg, duration=0: toasts.append(msg)))
        monkeypatch.setitem(sys.modules, "rgsx_web",
                            types.SimpleNamespace(get_translation=lambda k: "qbt_password_migrated"))
        qbt._notify_password_migrated()
        message = broadcast[0][1]["message"]
        assert message == "qBittorrent WebUI password was automatically rotated for security."
        assert toasts[0] == message


class TestApplyWebuiPassword:
    def test_with_session_posts_and_saves(self, monkeypatch):
        import rgsx_settings
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200))])
        wrote = []
        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password",
                            lambda pw: wrote.append(pw))
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18572")
        qbt._apply_webui_password(session, "Nouveau")
        assert wrote == ["Nouveau"]
        assert any("setPreferences" in url for _, url in session.calls)

    def test_post_exception_still_saves(self, monkeypatch):
        import rgsx_settings
        wrote = []

        class BoomPost:
            def post(self, url, **kw):
                raise requests.exceptions.ConnectionError("x")

        monkeypatch.setattr(rgsx_settings, "set_qbittorrent_webui_password",
                            lambda pw: wrote.append(pw))
        qbt._apply_webui_password(BoomPost(), "Nouveau")
        assert wrote == ["Nouveau"]


class TestDownloadTorrent:
    def _download_mocks(self, monkeypatch, tmp_path, info_state="downloading",
                        add_status=200, file_progress=(500, 500, True)):
        dest_dir = tmp_path / "dl"
        dest_dir.mkdir()
        dest_path = dest_dir / "game.iso"
        downloaded_file = tmp_path / "artifact" / "game.iso"
        downloaded_file.parent.mkdir(parents=True)
        downloaded_file.write_bytes(b"x" * 10)

        session = FakeHttpSession([
            ("post", "*", FakeHttpResponse(add_status)),
            ("post", "*", FakeHttpResponse(200)),
        ])
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(requests, "get",
                            lambda url, timeout=None, headers=None: types.SimpleNamespace(content=b"torrent"))
        monkeypatch.setattr(qbt, "_find_existing_torrent_by_save_path",
                            lambda s, p: {"hash": "h1", "save_path": p})
        monkeypatch.setattr(qbt, "_apply_file_selection", lambda s, h, rp: (2, 500))
        monkeypatch.setattr(qbt, "_torrent_info_by_hash", lambda s, h: {
            "state": info_state, "dlspeed": 2048 * 1024, "num_leechs": 2, "num_seeds": 1,
            "size": 500, "downloaded": 100, "progress": 1.0,
            "content_path": str(downloaded_file),
        })
        monkeypatch.setattr(qbt, "_torrent_files",
                            lambda s, h: [{"index": 2, "size": 500, "progress": 1.0}])
        monkeypatch.setattr(qbt, "_get_target_file_progress", lambda f, i, fb: file_progress)
        monkeypatch.setattr(qbt, "_count_active_peers", lambda s, h, f: 2)
        monkeypatch.setattr(qbt, "_resolve_downloaded_file",
                            lambda cp, td, rp, fb: str(downloaded_file))
        monkeypatch.setattr(qbt, "_seed_status_worker", lambda *a, **k: None)
        monkeypatch.setattr(qbt, "_has_other_hash_references", lambda *a, **k: False)
        monkeypatch.setattr(qbt, "_cleanup_hash_state_if_unused", lambda h: None)
        return session, dest_dir, dest_path

    def test_backend_unavailable(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        with pytest.raises(qbt.BackendUnavailableError):
            qbt.download_torrent_via_qbittorrent(
                {"source_url": "http://x/a.torrent"}, str(tmp_path), str(tmp_path / "g.iso"),
                "t0", StubEvent([]), Recorder())

    def test_success(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(monkeypatch, tmp_path)
        progress = Recorder()
        ok, msg = qbt.download_torrent_via_qbittorrent(
            {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso",
             "size_bytes": 1000},
            str(dest_dir), str(dest_path), "t-dl1", StubEvent([]), progress,
            original_history_url="",
        )
        assert ok is True
        assert dest_path.exists()
        assert "game.iso" in msg
        assert len(progress.items) >= 2
        # seed kaydina alindi; temizle
        qbt._pop_active_reference(task_id="t-dl1")

    def test_cancel_raises(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(monkeypatch, tmp_path)
        with pytest.raises(RuntimeError):
            qbt.download_torrent_via_qbittorrent(
                {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
                str(dest_dir), str(dest_path), "t-cancel", StubEvent([True]), Recorder())

    def test_error_state_raises(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(
            monkeypatch, tmp_path, info_state="error")
        with pytest.raises(RuntimeError, match="état d'erreur"):
            qbt.download_torrent_via_qbittorrent(
                {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
                str(dest_dir), str(dest_path), "t-err", StubEvent([]), Recorder())

    def test_409_reuse_then_cancel(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(
            monkeypatch, tmp_path, add_status=409)
        with pytest.raises(RuntimeError):
            qbt.download_torrent_via_qbittorrent(
                {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
                str(dest_dir), str(dest_path), "t-409", StubEvent([True]), Recorder())

    def test_pause_resume_cycle(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(monkeypatch, tmp_path)
        progress = Recorder()
        ok, msg = qbt.download_torrent_via_qbittorrent(
            {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
            str(dest_dir), str(dest_path), "t-pause",
            StubEvent([]), progress,
            pause_ev=StubEvent([True, False]),
        )
        assert ok is True
        qbt._pop_active_reference(task_id="t-pause")

    def test_add_not_confirmed_raises(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        dest_dir = tmp_path / "dl2"
        dest_dir.mkdir()
        session = FakeHttpSession([("post", "*", FakeHttpResponse(200))])
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: session)
        monkeypatch.setattr(requests, "get",
                            lambda url, timeout=None, headers=None: types.SimpleNamespace(content=b"torrent"))
        monkeypatch.setattr(qbt, "_find_existing_torrent_by_save_path", lambda s, p: None)
        clock = [0.0]

        def _tick():
            clock[0] += 0.5
            return clock[0]

        monkeypatch_time = _MonkeypatchTime(_tick, lambda s: None)
        monkeypatch_time.apply()
        try:
            with pytest.raises(RuntimeError, match="confirmé"):
                qbt.download_torrent_via_qbittorrent(
                    {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
                    str(dest_dir), str(dest_dir / "g.iso"), "t-nohash", StubEvent([]), Recorder())
        finally:
            monkeypatch_time.restore()

    def test_missing_file_raises(self, monkeypatch, tmp_path):
        _reset_globals(monkeypatch)
        session, dest_dir, dest_path = self._download_mocks(monkeypatch, tmp_path)
        monkeypatch.setattr(qbt, "_resolve_downloaded_file", lambda *a, **k: None)
        with pytest.raises(FileNotFoundError):
            qbt.download_torrent_via_qbittorrent(
                {"source_url": "http://ex.com/a.torrent", "relative_path": "Game/game.iso"},
                str(dest_dir), str(dest_path), "t-nofile", StubEvent([]), Recorder())
