"""Faz 5 — qBittorrent şifre migration v1 testleri.

DRIFT düzeltmesi: rastgele şifre üretimi kodda yoktu, kurulumlar öntanımlı
`RGSXqbt`'de duruyordu. Migration yalnızca öntanımlı/eksik şifreye dokunur,
kullanıcı tanımlı şifreye asla; `migration_v1_done` flag'i bir kereliğini garanti
eder. Gerçek qBittorrent süreci başlatılmaz — saf karar/uygulama mantığı test edilir.
"""

import json

import pytest

import qbittorrent_backend as qbt
import rgsx_settings


@pytest.fixture
def migration_env(tmp_path, monkeypatch):
    import config

    settings_path = tmp_path / "rgsx_settings.json"
    settings_path.write_text(json.dumps({"language": "en"}), encoding="utf-8")

    monkeypatch.setattr(config, "SAVE_FOLDER", str(tmp_path))
    monkeypatch.setattr(config, "RGSX_SETTINGS_PATH", str(settings_path))
    # Bildirim best effort'tur; testlerde TVUI/manager bağımlılığı istenmez.
    monkeypatch.setattr(qbt, "_notify_password_migrated", lambda: None)
    return tmp_path, settings_path


def _read_settings(settings_path):
    return json.loads(settings_path.read_text(encoding="utf-8"))


def _write_settings(settings_path, data):
    settings_path.write_text(json.dumps(data), encoding="utf-8")


class TestGenerateRandomPassword:
    def test_returns_long_cryptographic_string(self):
        pw = qbt.generate_random_password()
        assert isinstance(pw, str)
        assert len(pw) >= 6
        assert pw not in qbt.KNOWN_DEFAULT_PASSWORDS

    def test_calls_are_unique(self):
        passwords = {qbt.generate_random_password() for _ in range(50)}
        assert len(passwords) == 50


class TestKnownDefaultPasswords:
    def test_includes_legacy_rgsx_constant(self):
        assert "RGSXqbt" in qbt.KNOWN_DEFAULT_PASSWORDS

    def test_known_qbittorrent_defaults_included(self):
        assert "admin" in qbt.KNOWN_DEFAULT_PASSWORDS
        assert "adminadmin" in qbt.KNOWN_DEFAULT_PASSWORDS

    def test_temp_password_patterns_excluded(self):
        # roadmap guard 1: _TEMP_PASSWORD_PATTERNS listeye DAHİL DEĞİL
        # (geçici şifreler zaten rastgele üretilir).
        for pattern in qbt._TEMP_PASSWORD_PATTERNS:
            assert pattern not in qbt.KNOWN_DEFAULT_PASSWORDS


class TestExtractTempPassword:
    def test_extracts_english_temp_password(self):
        lines = ["qBittorrent 4.6.2 ready", "The WebUI temporary password is: r4nd0m_T0k3n", "Done."]
        assert qbt._extract_temp_password(lines) == "r4nd0m_T0k3n"

    def test_extracts_french_temp_password(self):
        lines = ["Le mot de passe temporaire est: r4nd0m_Fr", "Prêt"]
        assert qbt._extract_temp_password(lines) == "r4nd0m_Fr"

    def test_no_match_returns_none(self):
        assert qbt._extract_temp_password(["no password here", "starting..."]) is None

    def test_empty_lines_returns_none(self):
        assert qbt._extract_temp_password([]) is None


class TestMigrationPersister:
    def test_flag_default_false(self, migration_env):
        assert rgsx_settings.get_qbittorrent_password_migration_done() is False

    def test_set_flag_persists(self, migration_env):
        tmp_path, settings_path = migration_env
        assert rgsx_settings.set_qbittorrent_password_migration_done(True) is True
        assert rgsx_settings.get_qbittorrent_password_migration_done() is True
        assert _read_settings(settings_path)["migration_v1_done"] is True


class TestMaybeMigratePassword:
    def test_missing_field_generates_random(self, migration_env):
        tmp_path, settings_path = migration_env
        result = qbt.maybe_migrate_qbittorrent_password(None, [])
        assert result == "migrated"
        data = _read_settings(settings_path)
        assert isinstance(data["qbittorrent_webui_password"], str)
        assert data["qbittorrent_webui_password"]
        assert data["qbittorrent_webui_password"] not in qbt.KNOWN_DEFAULT_PASSWORDS
        assert data["migration_v1_done"] is True

    def test_missing_field_prefers_temp_password(self, migration_env):
        tmp_path, settings_path = migration_env
        lines = ["The WebUI temporary password is: tmpS3cret", "Done"]
        result = qbt.maybe_migrate_qbittorrent_password(None, lines)
        assert result == "migrated"
        data = _read_settings(settings_path)
        assert data["qbittorrent_webui_password"] == "tmpS3cret"
        assert data["migration_v1_done"] is True

    def test_default_password_migrated(self, migration_env):
        tmp_path, settings_path = migration_env
        _write_settings(settings_path, {"language": "en", "qbittorrent_webui_password": "RGSXqbt"})
        result = qbt.maybe_migrate_qbittorrent_password(None, [])
        assert result == "migrated"
        data = _read_settings(settings_path)
        assert data["qbittorrent_webui_password"] not in qbt.KNOWN_DEFAULT_PASSWORDS
        assert data["migration_v1_done"] is True

    def test_any_known_default_migrated(self, migration_env):
        tmp_path, settings_path = migration_env
        for default in qbt.KNOWN_DEFAULT_PASSWORDS:
            _write_settings(settings_path, {"language": "en", "qbittorrent_webui_password": default})
            result = qbt.maybe_migrate_qbittorrent_password(None, [])
            assert result == "migrated", f"default {default!r} migrate edilmedi"
            data = _read_settings(settings_path)
            assert data["qbittorrent_webui_password"] not in qbt.KNOWN_DEFAULT_PASSWORDS

    def test_user_password_untouched(self, migration_env):
        tmp_path, settings_path = migration_env
        _write_settings(settings_path, {"language": "en", "qbittorrent_webui_password": "myS3cret!2024"})
        result = qbt.maybe_migrate_qbittorrent_password(None, [])
        assert result == "noop"
        data = _read_settings(settings_path)
        assert data["qbittorrent_webui_password"] == "myS3cret!2024"
        assert data["migration_v1_done"] is True

    def test_flag_prevents_second_migration(self, migration_env):
        tmp_path, settings_path = migration_env
        _write_settings(settings_path, {
            "language": "en",
            "qbittorrent_webui_password": "RGSXqbt",
            "migration_v1_done": True,
        })
        result = qbt.maybe_migrate_qbittorrent_password(None, [])
        assert result == "already_done"
        data = _read_settings(settings_path)
        assert data["qbittorrent_webui_password"] == "RGSXqbt"


class TestApplyWebuiPassword:
    def test_posts_setpreferences_and_persists(self, migration_env):
        tmp_path, settings_path = migration_env

        class FakeSession:
            def __init__(self):
                self.posts = []

            def post(self, url, **kwargs):
                self.posts.append((url, kwargs))
                return None

        session = FakeSession()
        qbt._apply_webui_password(session, "rand0mNewPw")
        assert len(session.posts) == 1
        url, kwargs = session.posts[0]
        assert url.endswith("/api/v2/app/setPreferences")
        body = json.loads(kwargs["data"]["json"])
        assert body["web_ui_password"] == "rand0mNewPw"
        assert _read_settings(settings_path)["qbittorrent_webui_password"] == "rand0mNewPw"

    def test_no_session_still_persists(self, migration_env):
        tmp_path, settings_path = migration_env
        qbt._apply_webui_password(None, "rand0mNoSession")
        assert _read_settings(settings_path)["qbittorrent_webui_password"] == "rand0mNoSession"


class TestChangeWebuiPassword:
    def test_short_password_rejected(self):
        ok, message = qbt.change_webui_password("123")
        assert ok is False
        assert message == "password_too_short"

    def test_no_backend_still_saves(self, migration_env, monkeypatch):
        tmp_path, settings_path = migration_env
        monkeypatch.setattr(qbt, "_ensure_qbittorrent_running", lambda: None)
        ok, message = qbt.change_webui_password("newPassw0rd")
        assert ok is True
        assert _read_settings(settings_path)["qbittorrent_webui_password"] == "newPassw0rd"
