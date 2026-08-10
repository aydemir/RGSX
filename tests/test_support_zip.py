"""Faz 1 — support ZIP secret redaksiyonu testleri.

P0: generate_support_zip() ve /api/support, rgsx_settings.json'ı redakte etmeden
paketliyordu; qBittorrent WebUI şifresi ZIP'e sızıyordu. Testler redaksiyonun hem
saf fonksiyon hem de ZIP entegrasyonu seviyesinde çalıştığını doğrular.
"""

import json
import zipfile

import pytest

from utils import generate_support_zip, redact_sensitive_settings


class TestRedactSensitiveSettings:
    def test_redacts_password_field(self):
        data = {"qbittorrent_webui_password": "s3cret", "language": "tr"}
        out = redact_sensitive_settings(data)
        assert out["qbittorrent_webui_password"] == "<redacted>"
        assert out["language"] == "tr"

    def test_redacts_nested_sensitive(self):
        data = {"sources": {"mode": "rgsx", "custom_url": "https://x", "api_key": "k123"}}
        out = redact_sensitive_settings(data)
        assert out["sources"]["api_key"] == "<redacted>"
        assert out["sources"]["mode"] == "rgsx"
        assert out["sources"]["custom_url"] == "https://x"

    def test_redacts_secret_token_credential(self):
        data = {"webhook_secret": "a", "refresh_token": "b", "proxy_credentials": {"user": "u", "passwd": "p"}}
        out = redact_sensitive_settings(data)
        assert out["webhook_secret"] == "<redacted>"
        assert out["refresh_token"] == "<redacted>"
        assert out["proxy_credentials"] == "<redacted>"

    def test_redacts_items_in_lists(self):
        data = {"servers": [{"name": "x", "apikey": "abc"}, {"name": "y"}]}
        out = redact_sensitive_settings(data)
        assert out["servers"][0]["apikey"] == "<redacted>"
        assert out["servers"][0]["name"] == "x"
        assert out["servers"][1]["name"] == "y"

    def test_non_sensitive_keys_untouched(self):
        data = {
            "region_priority": ["USA"],
            "hide_downloaded": False,
            "manager_port": 5000,
            "platform_custom_paths": {"ps2": "/roms/ps2"},
            "keyboard_layout": "tr",
        }
        assert redact_sensitive_settings(data) == data

    def test_does_not_mutate_original(self):
        data = {"qbittorrent_webui_password": "x", "nested": {"token": "y"}}
        redact_sensitive_settings(data)
        assert data["qbittorrent_webui_password"] == "x"
        assert data["nested"]["token"] == "y"


@pytest.fixture
def support_env(tmp_path, monkeypatch):
    import config

    settings_path = tmp_path / "rgsx_settings.json"
    settings_path.write_text(json.dumps({
        "language": "en",
        "qbittorrent_webui_password": "s3cret!",
        "sources": {"mode": "rgsx", "custom_url": "https://example.com/dir"},
    }), encoding="utf-8")

    monkeypatch.setattr(config, "SAVE_FOLDER", str(tmp_path))
    monkeypatch.setattr(config, "RGSX_SETTINGS_PATH", str(settings_path))
    return tmp_path, settings_path


class TestGenerateSupportZip:
    def test_settings_redacted_and_disk_untouched(self, support_env):
        tmp_path, settings_path = support_env
        ok, message, zip_path = generate_support_zip()
        assert ok, message
        with zipfile.ZipFile(zip_path) as zf:
            assert "rgsx_settings.json" in zf.namelist()
            content = json.loads(zf.read("rgsx_settings.json"))
        assert content["qbittorrent_webui_password"] == "<redacted>"
        assert content["sources"]["custom_url"] == "https://example.com/dir"
        on_disk = json.loads(settings_path.read_text(encoding="utf-8"))
        assert on_disk["qbittorrent_webui_password"] == "s3cret!"

    def test_zip_contains_readme(self, support_env):
        tmp_path, _ = support_env
        ok, message, zip_path = generate_support_zip()
        assert ok, message
        with zipfile.ZipFile(zip_path) as zf:
            assert "README.txt" in zf.namelist()
