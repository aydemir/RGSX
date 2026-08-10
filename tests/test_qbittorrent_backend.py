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
        monkeypatch.setattr(qbt, "is_available", lambda: True)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "RGSXqbt")
        status = qbt.get_password_status()
        assert set(status.keys()) == {"available", "using_default", "webui_url"}
        assert status["available"] is True
        assert status["using_default"] is True
        assert status["webui_url"] == "http://localhost:18572/"

    def test_custom_password_not_default(self, monkeypatch):
        monkeypatch.setattr(qbt, "is_available", lambda: True)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "monMdp-2026")
        status = qbt.get_password_status()
        assert status["using_default"] is False

    def test_settings_failure_falls_back_to_default(self, monkeypatch):
        monkeypatch.setattr(qbt, "is_available", lambda: False)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")

        def boom():
            raise RuntimeError("settings erisilemez")

        monkeypatch.setattr(qbt, "_get_configured_password", boom)
        status = qbt.get_password_status()
        assert status["available"] is False
        assert status["using_default"] is True

    def test_config_constant_absence_defaults(self, monkeypatch):
        monkeypatch.delattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", raising=False)
        monkeypatch.setattr(qbt, "is_available", lambda: False)
        monkeypatch.setattr(qbt, "get_webui_url", lambda: "http://localhost:18572/")
        monkeypatch.setattr(qbt, "_get_configured_password", lambda: "RGSXqbt")
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
