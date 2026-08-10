# -*- coding: utf-8 -*-
"""Faz 7 - Characterization tests: qbittorrent_backend contract.

Migration mantigi test_password_migration.py'de kapsanmis. Bu dosya manager
API'sinin guvendigi yuzeyi (get_password_status / _get_configured_password /
change_webui_password) sabitler. Gercek qBittorrent sureci baslatilmaz.
"""

import pytest

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
