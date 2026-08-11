"""rgsx_settings.py davranış testleri (TASK-001 — Faz 7 kapsam artırma).

Hedef: modül kapsamını %26 -> %60'a taşımak. Saf persister/getter mantığı
tmp_path üzerinde sabitlenir; ağ çağrıları FakeSession ile mock'lanır; pygame
çağrıları stub display nesnesi ile sürülür. Gerçek qBittorrent süreci veya
gerçek ekran başlatılmaz.
"""

import json
from datetime import datetime, timezone

import pytest

import pygame

import rgsx_settings
from rgsx_settings import (
    delete_old_files,
    format_gamelist_update_display,
    get_all_platform_custom_paths,
    get_allow_unknown_extensions,
    get_auto_extract,
    get_autostart_on_boot,
    get_available_monitors,
    get_custom_sources_url,
    get_display_background_theme,
    get_display_fullscreen,
    get_display_grid,
    get_display_monitor,
    get_font_family,
    get_global_sort_option,
    get_hide_premium_systems,
    get_last_gamelist_prompt_remote_update,
    get_last_gamelist_update,
    get_light_mode,
    get_manager_host,
    get_manager_port,
    get_max_simultaneous_downloads,
    get_nintendo_layout,
    get_platform_custom_path,
    get_qbittorrent_password_mode,
    get_qbittorrent_webui_password,
    get_remote_gamelist_timestamp,
    get_roms_folder,
    get_show_unsupported_platforms,
    get_sources_mode,
    get_sources_zip_url,
    get_symlink_option,
    load_game_filters,
    load_rgsx_settings,
    load_symlink_settings,
    parse_gamelist_update_timestamp,
    save_game_filters,
    save_rgsx_settings,
    save_symlink_settings,
    set_allow_unknown_extensions,
    set_auto_extract,
    set_autostart_on_boot,
    set_display_background_theme,
    set_display_fullscreen,
    set_display_grid,
    set_display_monitor,
    set_font_family,
    set_global_sort_option,
    set_hide_premium_systems,
    set_last_gamelist_prompt_remote_update,
    set_last_gamelist_update,
    set_light_mode,
    set_manager_host,
    set_manager_port,
    set_max_simultaneous_downloads,
    set_nintendo_layout,
    set_platform_custom_path,
    set_qbittorrent_password_mode,
    set_qbittorrent_webui_password,
    set_roms_folder,
    set_show_unsupported_platforms,
    set_sources_mode,
    set_symlink_option,
    apply_symlink_path,
    find_local_custom_sources_zip,
)


@pytest.fixture
def settings_env(tmp_path, monkeypatch):
    import config

    settings_path = tmp_path / "rgsx_settings.json"
    monkeypatch.setattr(config, "SAVE_FOLDER", str(tmp_path))
    monkeypatch.setattr(config, "APP_FOLDER", str(tmp_path / "app"))
    monkeypatch.setattr(config, "RGSX_SETTINGS_PATH", str(settings_path))
    return tmp_path, settings_path


def _read(settings_path):
    return json.loads(settings_path.read_text(encoding="utf-8"))


def _write(settings_path, data):
    settings_path.write_text(json.dumps(data), encoding="utf-8")


class TestLoadSave:
    def test_missing_file_returns_defaults(self, settings_env):
        settings = load_rgsx_settings()
        assert settings["language"] == "en"
        assert settings["max_simultaneous_downloads"] == 5

    def test_existing_file_merged_with_defaults(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"language": "tr"})
        settings = load_rgsx_settings()
        assert settings["language"] == "tr"
        assert settings["music_enabled"] is True

    def test_broken_json_returns_defaults(self, settings_env):
        tmp_path, settings_path = settings_env
        settings_path.write_text("{not json", encoding="utf-8")
        settings = load_rgsx_settings()
        assert settings["language"] == "en"

    def test_save_writes_file_and_creates_folder(self, settings_env, tmp_path):
        settings = {"language": "de", "music_enabled": False}
        save_rgsx_settings(settings)
        saved = _read(tmp_path / "rgsx_settings.json")
        assert saved == settings

    def test_save_error_is_swallowed(self, settings_env, monkeypatch):
        import builtins

        def _boom(*a, **k):
            raise OSError("disk full")

        monkeypatch.setattr(builtins, "open", _boom)
        save_rgsx_settings({"language": "en"})


class TestDeleteOldFiles:
    def test_removes_legacy_files(self, settings_env):
        tmp_path, settings_path = settings_env
        app = tmp_path / "app"
        app.mkdir()
        for name in ("accessibility.json", "language.json", "sources.json"):
            (tmp_path / name).write_text("{}", encoding="utf-8")
        for name in ("rom_extensions.json", "es_input_parser.py"):
            (app / name).write_text("{}", encoding="utf-8")
        delete_old_files()
        assert not (tmp_path / "accessibility.json").exists()
        assert not (tmp_path / "language.json").exists()
        assert not (app / "es_input_parser.py").exists()

    def test_missing_files_are_ignored(self, settings_env):
        delete_old_files()

    def test_remove_error_is_swallowed(self, settings_env, monkeypatch):
        (settings_env[0] / "accessibility.json").write_text("{}", encoding="utf-8")
        monkeypatch.setattr(
            rgsx_settings.os, "remove",
            lambda p: (_ for _ in ()).throw(OSError("locked")),
        )
        delete_old_files()


class TestGamelistUpdateTimestamp:
    def test_parse_none(self):
        assert parse_gamelist_update_timestamp(None) is None

    def test_parse_empty_string(self):
        assert parse_gamelist_update_timestamp("   ") is None

    def test_parse_aware_datetime(self):
        dt = datetime(2024, 1, 1, 10, 0, tzinfo=timezone.utc)
        assert parse_gamelist_update_timestamp(dt) == dt

    def test_parse_naive_datetime_gets_utc(self):
        dt = datetime(2024, 1, 1, 10, 0)
        parsed = parse_gamelist_update_timestamp(dt)
        assert parsed.tzinfo == timezone.utc

    def test_parse_iso_zulu(self):
        parsed = parse_gamelist_update_timestamp("2024-01-01T10:00:00Z")
        assert parsed == datetime(2024, 1, 1, 10, 0, tzinfo=timezone.utc)

    def test_parse_iso_naive(self):
        parsed = parse_gamelist_update_timestamp("2024-01-01T10:00:00")
        assert parsed == datetime(2024, 1, 1, 10, 0, tzinfo=timezone.utc)

    def test_parse_date_only(self):
        parsed = parse_gamelist_update_timestamp("2024-01-01")
        assert parsed == datetime(2024, 1, 1, tzinfo=timezone.utc)

    def test_parse_datetime_with_space(self):
        parsed = parse_gamelist_update_timestamp("2024-01-01 10:00:00")
        assert parsed == datetime(2024, 1, 1, 10, 0, tzinfo=timezone.utc)

    def test_parse_http_date(self):
        parsed = parse_gamelist_update_timestamp("Tue, 02 Jan 2024 10:00:00 GMT")
        assert parsed == datetime(2024, 1, 2, 10, 0, tzinfo=timezone.utc)

    def test_parse_invalid_returns_none(self):
        assert parse_gamelist_update_timestamp("not-a-date") is None

    def test_format_valid(self):
        assert format_gamelist_update_display("2024-01-01T10:00:00Z") == "2024-01-01"

    def test_format_invalid_returns_original(self):
        assert format_gamelist_update_display("garbage") == "garbage"

    def test_format_none_returns_empty(self):
        assert format_gamelist_update_display(None) == ""

    def test_set_last_gamelist_update_explicit(self, settings_env):
        tmp_path, settings_path = settings_env
        result = set_last_gamelist_update("2024-01-15")
        assert result == "2024-01-15T00:00:00Z"
        assert _read(settings_path)["last_gamelist_update"] == result
        assert get_last_gamelist_update() == result

    def test_set_last_gamelist_update_now(self, settings_env):
        tmp_path, settings_path = settings_env
        result = set_last_gamelist_update()
        assert result.endswith("Z")
        assert _read(settings_path)["last_gamelist_update"] == result

    def test_set_last_gamelist_update_invalid_uses_now(self, settings_env):
        tmp_path, settings_path = settings_env
        result = set_last_gamelist_update("nope")
        assert result.endswith("Z")

    def test_get_last_gamelist_update_via_settings_arg(self):
        assert get_last_gamelist_update({"last_gamelist_update": "2024-01-01"}) == "2024-01-01"

    def test_set_prompt_remote_update_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        result = set_last_gamelist_prompt_remote_update("2024-01-15")
        assert result == "2024-01-15T00:00:00Z"
        assert _read(settings_path)["last_gamelist_prompt_remote_update"] == result
        assert get_last_gamelist_prompt_remote_update() == result

    def test_set_prompt_remote_update_none_clears(self, settings_env):
        tmp_path, settings_path = settings_env
        set_last_gamelist_prompt_remote_update("2024-01-15")
        result = set_last_gamelist_prompt_remote_update(None)
        assert result is None
        assert _read(settings_path)["last_gamelist_prompt_remote_update"] is None
        assert get_last_gamelist_prompt_remote_update() is None


class _FakeResponse:
    def __init__(self, status_code=200, headers=None):
        self.status_code = status_code
        self.headers = headers or {}
        self.closed = False

    def close(self):
        self.closed = True


class _FakeSession:
    def __init__(self, head_response, get_response):
        self.head_response = head_response
        self.get_response = get_response
        self.entered = False
        self.exited = False

    def __enter__(self):
        self.entered = True
        return self

    def __exit__(self, *exc):
        self.exited = True

    def head(self, url, **kwargs):
        return self.head_response

    def get(self, url, **kwargs):
        return self.get_response


class TestRemoteGamelistTimestamp:
    def test_empty_url_returns_none(self, settings_env):
        assert get_remote_gamelist_timestamp("") is None

    def test_head_ok_with_last_modified(self, settings_env, monkeypatch):
        head = _FakeResponse(200, {"Last-Modified": "Tue, 02 Jan 2024 10:00:00 GMT"})
        monkeypatch.setattr(rgsx_settings.requests, "Session", lambda: _FakeSession(head, None))
        result = get_remote_gamelist_timestamp("https://example.com/games.zip")
        assert result == datetime(2024, 1, 2, 10, 0, tzinfo=timezone.utc)

    def test_head_error_falls_back_to_get(self, settings_env, monkeypatch):
        head = _FakeResponse(404, {})
        get_resp = _FakeResponse(200, {"Last-Modified": "Wed, 03 Jan 2024 10:00:00 GMT"})
        monkeypatch.setattr(rgsx_settings.requests, "Session", lambda: _FakeSession(head, get_resp))
        result = get_remote_gamelist_timestamp("https://example.com/games.zip")
        assert result == datetime(2024, 1, 3, 10, 0, tzinfo=timezone.utc)

    def test_no_last_modified_returns_none(self, settings_env, monkeypatch):
        head = _FakeResponse(200, {})
        get_resp = _FakeResponse(200, {})
        monkeypatch.setattr(rgsx_settings.requests, "Session", lambda: _FakeSession(head, get_resp))
        assert get_remote_gamelist_timestamp("https://example.com/games.zip") is None

    def test_invalid_header_returns_none(self, settings_env, monkeypatch):
        head = _FakeResponse(200, {"Last-Modified": "garbage"})
        monkeypatch.setattr(rgsx_settings.requests, "Session", lambda: _FakeSession(head, None))
        assert get_remote_gamelist_timestamp("https://example.com/games.zip") is None

    def test_network_error_returns_none(self, settings_env, monkeypatch):
        class BoomSession(_FakeSession):
            def head(self, url, **kwargs):
                raise ConnectionError("timeout")

        monkeypatch.setattr(rgsx_settings.requests, "Session", lambda: BoomSession(None, None))
        assert get_remote_gamelist_timestamp("https://example.com/games.zip") is None


class TestSymlink:
    def test_load_default_disabled(self, settings_env):
        assert load_symlink_settings() == {"use_symlink_path": False}

    def test_load_enabled(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"symlink": {"enabled": True, "target_directory": "/x"}})
        assert load_symlink_settings() == {"use_symlink_path": True}

    def test_load_non_dict_resets(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"symlink": "broken"})
        assert load_symlink_settings() == {"use_symlink_path": False}

    def test_load_legacy_use_symlink_path(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"symlink": {"use_symlink_path": True}})
        assert load_symlink_settings() == {"use_symlink_path": True}

    def test_load_error_returns_disabled(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert load_symlink_settings() == {"use_symlink_path": False}

    def test_save_writes_new_format(self, settings_env):
        tmp_path, settings_path = settings_env
        assert save_symlink_settings({"use_symlink_path": True, "target_directory": "/dl"}) is True
        assert _read(settings_path)["symlink"] == {"enabled": True, "target_directory": "/dl"}

    def test_save_error_returns_false(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "save_rgsx_settings",
            lambda s: (_ for _ in ()).throw(OSError("nope")),
        )
        assert save_symlink_settings({"use_symlink_path": True}) is False

    def test_set_symlink_option_enable(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_symlink_option(True) == (True, "symlink_settings_saved_successfully")
        assert get_symlink_option() is True

    def test_set_symlink_option_error(self, settings_env, monkeypatch):
        monkeypatch.setattr(rgsx_settings, "save_symlink_settings", lambda s: False)
        ok, msg = set_symlink_option(True)
        assert ok is False
        assert msg == "symlink_settings_save_error"

    def test_apply_symlink_path_enabled(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"symlink": {"enabled": True}})
        assert apply_symlink_path("/roms", "nes") == ("/roms/nes/nes")

    def test_apply_symlink_path_disabled(self, settings_env):
        assert apply_symlink_path("/roms", "nes") == ("/roms/nes")


class TestSources:
    def test_get_sources_mode_default(self, settings_env):
        assert get_sources_mode() == "rgsx"

    def test_get_sources_mode_via_settings(self):
        assert get_sources_mode({"sources": {"mode": "custom"}}) == "custom"

    def test_set_sources_mode_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_sources_mode("custom") == "custom"
        assert _read(settings_path)["sources"]["mode"] == "custom"

    def test_set_sources_mode_invalid_clamps(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_sources_mode("bogus") == "rgsx"
        assert _read(settings_path)["sources"]["mode"] == "rgsx"

    def test_get_custom_sources_url(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"sources": {"custom_url": "  https://x.z/a.zip  "}})
        assert get_custom_sources_url() == "https://x.z/a.zip"

    def test_get_sources_zip_url_rgsx_mode(self, settings_env):
        assert get_sources_zip_url("https://rgsx.zip") == "https://rgsx.zip"

    def test_get_sources_zip_url_custom_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"sources": {"mode": "custom", "custom_url": "https://my.zip"}})
        assert get_sources_zip_url("https://rgsx.zip") == "https://my.zip"

    def test_get_sources_zip_url_custom_invalid_returns_none(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"sources": {"mode": "custom", "custom_url": "not-a-url"}})
        assert get_sources_zip_url("https://rgsx.zip") is None

    def test_find_local_zip_missing_folder(self, settings_env):
        assert find_local_custom_sources_zip() is None

    def test_find_local_zip_missing_file(self, settings_env):
        assert find_local_custom_sources_zip() is None

    def test_find_local_zip_present(self, settings_env):
        tmp_path, settings_path = settings_env
        (tmp_path / "games.zip").write_bytes(b"zip")
        assert find_local_custom_sources_zip() == str(tmp_path / "games.zip")


class TestToggles:
    def test_show_unsupported_platforms_roundtrip(self, settings_env):
        tmp_path, settings_path = settings_env
        assert get_show_unsupported_platforms() is False
        assert set_show_unsupported_platforms(True) is True
        assert _read(settings_path)["show_unsupported_platforms"] is True

    def test_allow_unknown_extensions_roundtrip(self, settings_env):
        tmp_path, settings_path = settings_env
        assert get_allow_unknown_extensions() is False
        assert set_allow_unknown_extensions(True) is True
        assert _read(settings_path)["allow_unknown_extensions"] is True

    def test_nintendo_layout_roundtrip(self, settings_env):
        tmp_path, settings_path = settings_env
        assert get_nintendo_layout() is False
        assert set_nintendo_layout(True) is True
        assert _read(settings_path)["nintendo_layout"] is True

    def test_hide_premium_systems_roundtrip(self, settings_env):
        tmp_path, settings_path = settings_env
        assert get_hide_premium_systems() is False
        assert set_hide_premium_systems(True) is True
        assert _read(settings_path)["hide_premium_systems"] is True

    def test_toggles_read_via_settings_arg(self):
        assert get_show_unsupported_platforms({"show_unsupported_platforms": True}) is True
        assert get_allow_unknown_extensions({"allow_unknown_extensions": True}) is True
        assert get_nintendo_layout({"nintendo_layout": True}) is True
        assert get_hide_premium_systems({"hide_premium_systems": True}) is True


class TestDisplay:
    def test_grid_default(self, settings_env):
        assert get_display_grid() == (3, 4)

    def test_grid_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"display": {"grid": "4x3"}})
        assert get_display_grid() == (4, 3)

    def test_grid_invalid_falls_back(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"display": {"grid": "junk"}})
        assert get_display_grid() == (3, 4)

    def test_set_grid_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_grid(4, 3) == (4, 3)
        assert _read(settings_path)["display"]["grid"] == "4x3"

    def test_set_grid_invalid_clamps(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_grid(9, 9) == (3, 4)

    def test_monitor_default(self, settings_env):
        assert get_display_monitor() == 0

    def test_set_monitor_clamps_negative(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_monitor(-3) == 0
        assert set_display_monitor(2) == 2

    def test_fullscreen_default_true(self, settings_env):
        assert get_display_fullscreen() is True

    def test_set_fullscreen(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_fullscreen(False) is False
        assert _read(settings_path)["display"]["fullscreen"] is False

    def test_light_mode_default_false(self, settings_env):
        assert get_light_mode() is False

    def test_set_light_mode(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_light_mode(True) is True

    def test_background_theme_default(self, settings_env):
        assert get_display_background_theme() == "default"

    def test_background_theme_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"display": {"background_theme": "Sunset"}})
        assert get_display_background_theme() == "sunset"

    def test_background_theme_invalid_clamps(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"display": {"background_theme": "neon"}})
        assert get_display_background_theme() == "default"

    def test_set_background_theme_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_background_theme("forest") == "forest"
        assert _read(settings_path)["display"]["background_theme"] == "forest"

    def test_set_background_theme_invalid_clamps(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_display_background_theme("neon") == "default"


class _FakeDisplay:
    def __init__(self, num_displays, sizes=None, sizes_raise_index=None, num_raise=False,
                 has_sizes=True):
        self._num = num_displays
        self._sizes = sizes
        self._sizes_raise = sizes_raise_index
        self._num_raise = num_raise
        self._has_sizes = has_sizes

    def __getattribute__(self, name):
        if name == "get_desktop_sizes" and not object.__getattribute__(self, "_has_sizes"):
            raise AttributeError(name)
        return object.__getattribute__(self, name)

    def get_init(self):
        return True

    def init(self):
        return None

    def get_num_displays(self):
        if self._num_raise:
            raise RuntimeError("no display")
        return self._num

    def get_desktop_sizes(self):
        if self._sizes_raise is not None and len(self._sizes) > self._sizes_raise:
            raise IndexError("out of range")
        return self._sizes

    class _Info:
        current_w = 640
        current_h = 480

    def Info(self):
        return self._Info()


class TestGetAvailableMonitors:
    def test_pygame_error_falls_back(self, settings_env, monkeypatch):
        monkeypatch.setattr(pygame, "display", _FakeDisplay(0, num_raise=True))
        assert get_available_monitors() == [{
            "index": 0, "name": "Monitor 1 (Default)", "resolution": "Auto",
        }]

    def test_returns_monitors_with_sizes(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            pygame, "display",
            _FakeDisplay(2, sizes=[(1920, 1080), (800, 600)]),
        )
        assert get_available_monitors() == [
            {"index": 0, "name": "Monitor 1", "resolution": "1920x1080"},
            {"index": 1, "name": "Monitor 2", "resolution": "800x600"},
        ]

    def test_no_sizes_falls_back_to_info(self, settings_env, monkeypatch):
        fake = _FakeDisplay(1, sizes=None, has_sizes=False)
        monkeypatch.setattr(pygame, "display", fake)
        assert get_available_monitors() == [
            {"index": 0, "name": "Monitor 1", "resolution": "640x480"},
        ]

    def test_per_display_error_marks_unknown(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            pygame, "display",
            _FakeDisplay(2, sizes=[(1920, 1080)], sizes_raise_index=1),
        )
        assert get_available_monitors() == [
            {"index": 0, "name": "Monitor 1", "resolution": "1920x1080"},
            {"index": 1, "name": "Monitor 2", "resolution": "Unknown"},
        ]


class TestFontRomsLanguage:
    def test_font_family_default(self, settings_env):
        assert get_font_family() == "pixel"

    def test_set_font_family(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_font_family("sans") == "sans"
        assert _read(settings_path)["display"]["font_family"] == "sans"

    def test_roms_folder_default_empty(self, settings_env):
        assert get_roms_folder() == ""

    def test_set_roms_folder_strips(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_roms_folder("  /my/roms  ") == "/my/roms"
        assert _read(settings_path)["roms_folder"] == "/my/roms"

    def test_get_language_default_en(self, settings_env):
        assert rgsx_settings.get_language() == "en"

    def test_get_language_stored(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"language": "fr"})
        assert rgsx_settings.get_language() == "fr"

    def test_get_language_via_settings_arg(self):
        assert rgsx_settings.get_language({"language": "de"}) == "de"


class TestGameFilters:
    def test_load_empty(self, settings_env):
        assert load_game_filters() == {}

    def test_load_stored(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"game_filters": {"region": ["us"]}})
        assert load_game_filters() == {"region": ["us"]}

    def test_load_error_returns_empty(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert load_game_filters() == {}

    def test_save_persists(self, settings_env):
        tmp_path, settings_path = settings_env
        assert save_game_filters({"region": ["eu"]}) is True
        assert _read(settings_path)["game_filters"] == {"region": ["eu"]}

    def test_save_error_returns_false(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "save_rgsx_settings",
            lambda s: (_ for _ in ()).throw(OSError("boom")),
        )
        assert save_game_filters({}) is False


class TestGlobalSort:
    def test_default(self, settings_env):
        assert get_global_sort_option() == "name_asc"

    def test_stored_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"global_sort_option": "size_desc"})
        assert get_global_sort_option() == "size_desc"

    def test_invalid_falls_back(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"global_sort_option": "bogus"})
        assert get_global_sort_option() == "name_asc"

    def test_set_valid(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_global_sort_option("name_desc") == "name_desc"
        assert _read(settings_path)["global_sort_option"] == "name_desc"

    def test_set_invalid_clamps(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_global_sort_option("bogus") == "name_asc"
        assert set_global_sort_option(None) == "name_asc"


class TestPlatformCustomPaths:
    def test_get_empty(self, settings_env):
        assert get_platform_custom_path("nes") == ""

    def test_get_stored(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"platform_custom_paths": {"nes": "/roms/nes"}})
        assert get_platform_custom_path("nes") == "/roms/nes"

    def test_set_and_get_all(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_platform_custom_path("nes", "/roms/nes") is True
        assert get_all_platform_custom_paths() == {"nes": "/roms/nes"}

    def test_set_empty_removes_entry(self, settings_env):
        tmp_path, settings_path = settings_env
        set_platform_custom_path("nes", "/roms/nes")
        assert set_platform_custom_path("nes", "") is True
        assert get_platform_custom_path("nes") == ""

    def test_get_error_returns_empty(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert get_platform_custom_path("nes") == ""

    def test_get_all_error_returns_empty(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert get_all_platform_custom_paths() == {}

    def test_set_error_returns_false(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert set_platform_custom_path("nes", "/x") is False


class TestAutoExtract:
    def test_default_true(self, settings_env):
        assert get_auto_extract() is True

    def test_set_false(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_auto_extract(False) is True
        assert get_auto_extract() is False

    def test_error_returns_default(self, settings_env, monkeypatch):
        monkeypatch.setattr(
            rgsx_settings, "load_rgsx_settings",
            lambda: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert get_auto_extract() is True


class TestMaxSimultaneousDownloads:
    def test_default(self, settings_env):
        assert get_max_simultaneous_downloads() == 5

    def test_stored_value(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"max_simultaneous_downloads": 3})
        assert get_max_simultaneous_downloads() == 3

    def test_clamped_low(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"max_simultaneous_downloads": 0})
        assert get_max_simultaneous_downloads() == 1

    def test_clamped_high(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"max_simultaneous_downloads": 99})
        assert get_max_simultaneous_downloads() == 10

    def test_invalid_falls_back(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"max_simultaneous_downloads": "abc"})
        assert get_max_simultaneous_downloads() == 5

    def test_set_updates_config(self, settings_env, monkeypatch):
        import config

        monkeypatch.setattr(config, "max_simultaneous_downloads", 5)
        assert set_max_simultaneous_downloads(8) == 8
        assert config.max_simultaneous_downloads == 8

    def test_set_clamps(self, settings_env):
        assert set_max_simultaneous_downloads(0) == 1
        assert set_max_simultaneous_downloads(99) == 10


class TestAutostart:
    def test_default_true(self, settings_env):
        assert get_autostart_on_boot() is True

    def test_set_false(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_autostart_on_boot(False) is False
        assert _read(settings_path)["autostart_on_boot"] is False
        assert get_autostart_on_boot() is False

    def test_via_settings_arg(self):
        assert get_autostart_on_boot({"autostart_on_boot": False}) is False


class TestQbittorrentWebuiPassword:
    def test_default_fallback(self, settings_env):
        assert get_qbittorrent_webui_password() == "RGSXqbt"

    def test_config_fallback(self, settings_env, monkeypatch):
        import config

        monkeypatch.setattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", "CfgPw")
        assert get_qbittorrent_webui_password() == "CfgPw"

    def test_stored_wins(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"qbittorrent_webui_password": "storedPw"})
        assert get_qbittorrent_webui_password() == "storedPw"

    def test_empty_stored_uses_fallback(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"qbittorrent_webui_password": ""})
        assert get_qbittorrent_webui_password() == "RGSXqbt"

    def test_set_persists(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_qbittorrent_webui_password("newPw") == "newPw"
        assert _read(settings_path)["qbittorrent_webui_password"] == "newPw"


class TestQbittorrentPasswordMode:
    def test_default(self, settings_env):
        assert get_qbittorrent_password_mode() == "default"

    def test_random(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"qbittorrent_password_mode": "random"})
        assert get_qbittorrent_password_mode() == "random"

    def test_custom_flag(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"qbittorrent_password_mode": "custom"})
        assert get_qbittorrent_password_mode() == "custom"

    def test_stored_password_implies_custom(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"qbittorrent_webui_password": "userPw"})
        assert get_qbittorrent_password_mode() == "custom"

    def test_set_random(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_qbittorrent_password_mode("random") == "random"
        assert _read(settings_path)["qbittorrent_password_mode"] == "random"

    def test_set_invalid_clamps_custom(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_qbittorrent_password_mode("bogus") == "custom"
        assert _read(settings_path)["qbittorrent_password_mode"] == "custom"


class TestManagerPortHost:
    def test_port_default(self, settings_env):
        assert get_manager_port() == 5000

    def test_port_stored(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"manager_port": 8080})
        assert get_manager_port() == 8080

    def test_port_invalid_falls_back(self, settings_env):
        tmp_path, settings_path = settings_env
        _write(settings_path, {"manager_port": "abc"})
        assert get_manager_port() == 5000

    def test_set_port(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_manager_port(8080) == 8080
        assert _read(settings_path)["manager_port"] == 8080

    def test_host_default(self, settings_env):
        assert get_manager_host() == "0.0.0.0"

    def test_set_host(self, settings_env):
        tmp_path, settings_path = settings_env
        assert set_manager_host("127.0.0.1") == "127.0.0.1"
        assert _read(settings_path)["manager_host"] == "127.0.0.1"
