"""Tests for pure/stateless display helper functions (no pygame surface needed)."""

import os

import pygame
import pytest

import config
from display.grid import (
    format_disk_size_gb,
    get_display_resolution_line,
    get_default_disk_space_line,
    get_platform_header_info_lines,
)
from display.game_list import get_display_extension
from display.components import fit_badge_lines, get_adaptive_badge_layout
from display.fonts import get_badge_font


@pytest.fixture
def real_font_env():
    """Set up real fonts + minimal config so font-measuring helpers run."""
    config.tiny_font = pygame.font.Font(None, 18)
    config.font = pygame.font.Font(None, 20)
    config.title_font = pygame.font.Font(None, 28)
    config.small_font = pygame.font.Font(None, 16)
    config.accessibility_settings = {"font_scale": 1.0, "footer_font_scale": 1.0}
    config.app_version = "2.6.5.6"
    config.screen_width = 1280
    config.screen_height = 720
    yield config


class TestFormatDiskSize:
    def test_zero(self):
        assert format_disk_size_gb(0) == "0.00 GB"

    def test_small_values_two_decimals(self):
        assert format_disk_size_gb(1) == "0.00 GB"
        assert format_disk_size_gb(int(1.5 * 1024 ** 3)) == "1.50 GB"

    def test_ten_plus_one_decimal(self):
        assert format_disk_size_gb(int(10 * 1024 ** 3)) == "10.0 GB"
        assert format_disk_size_gb(int(12.34 * 1024 ** 3)) == "12.3 GB"

    def test_hundred_plus_integer(self):
        assert format_disk_size_gb(int(100 * 1024 ** 3)) == "100 GB"
        assert format_disk_size_gb(int(123.7 * 1024 ** 3)) == "124 GB"


class TestGetDisplayExtension:
    def test_normal(self):
        assert get_display_extension("game.zip") == ".zip"

    def test_uppercase_lowercased(self):
        assert get_display_extension("GAME.7Z") == ".7z"

    def test_double_extension(self):
        assert get_display_extension("archive.tar.gz") == ".gz"

    def test_no_extension(self):
        assert get_display_extension("gamename") == "-"

    def test_empty_and_non_string(self):
        assert get_display_extension("") == "-"
        assert get_display_extension("   ") == "-"
        assert get_display_extension(None) == "-"


class TestAdaptiveBadgeLayout:
    def test_empty_lines_returns_base_font(self, real_font_env):
        base = real_font_env.tiny_font
        font, lines = get_adaptive_badge_layout([], base)
        assert font is base
        assert lines == []

    def test_no_max_width_returns_lines(self, real_font_env):
        base = real_font_env.tiny_font
        font, lines = get_adaptive_badge_layout(["v1.0", "Gamepad"], base)
        assert lines == ["v1.0", "Gamepad"]

    def test_fits_within_width(self, real_font_env):
        base = real_font_env.tiny_font
        font, lines = get_adaptive_badge_layout(["short"], base, max_badge_width=300)
        assert lines == ["short"]
        assert font is not base

    def test_very_narrow_truncates(self, real_font_env):
        base = real_font_env.tiny_font
        font, lines = get_adaptive_badge_layout(
            ["A rather long platform header line"], base, max_badge_width=60
        )
        assert lines
        assert len(lines[0]) < len("A rather long platform header line")

    def test_fit_badge_lines_delegates(self, real_font_env):
        base = real_font_env.tiny_font
        assert fit_badge_lines([], base) == []
        fitted = fit_badge_lines(["v2.0", ""], base)
        assert fitted == ["v2.0"]


class TestGetBadgeFont:
    def test_returns_cached_font(self):
        f1 = get_badge_font(14)
        f2 = get_badge_font(14)
        assert f1 is f2
        assert f1.size("x")[0] > 0

    def test_clamps_small_size(self):
        font = get_badge_font(1)
        assert font.size("x")[0] > 0

    def test_dejavu_family_uses_sysfont(self, monkeypatch):
        import config
        monkeypatch.setattr(config, "current_font_family_index", 2)
        font = get_badge_font(16)
        assert font.size("x")[0] > 0

    def test_missing_font_file_falls_back_to_tiny(self, monkeypatch):
        import config
        monkeypatch.setattr(config, "APP_FOLDER", "C:/nonexistent-rgsx-dir")
        real_tiny = pygame.font.Font(None, 12)
        monkeypatch.setattr(config, "tiny_font", real_tiny)
        font = get_badge_font(15)
        assert font is real_tiny


class TestHeaderInfoLines:
    def test_minimal(self, real_font_env):
        lines = get_platform_header_info_lines(include_details=False)
        assert lines == [f"v{real_font_env.app_version}"]

    def test_with_device_and_ip(self, real_font_env):
        real_font_env.controller_device_name = "8BitDo Controller"
        real_font_env.SYSTEM_INFO = {"network_ip": "192.168.1.5"}
        real_font_env.manager_port = 5000
        lines = get_platform_header_info_lines(include_details=True)
        assert any("8BitDo" in line for line in lines)
        assert any("192.168.1.5:5000" in line for line in lines)

    def test_no_details_skip_controller(self, real_font_env):
        real_font_env.controller_device_name = "8BitDo Controller"
        lines = get_platform_header_info_lines(include_details=False)
        assert not any("8BitDo" in line for line in lines)


class TestDiskSpaceLine:
    def test_missing_roms_folder(self, real_font_env, monkeypatch):
        monkeypatch.delattr(config, "ROMS_FOLDER", raising=False)
        assert get_default_disk_space_line() == ""

    def test_empty_roms_folder(self, real_font_env):
        real_font_env.ROMS_FOLDER = ""
        assert get_default_disk_space_line() == ""

    def test_nonexistent_path_walks_to_ancestor(self, real_font_env, tmp_path):
        real_font_env.ROMS_FOLDER = str(tmp_path / "does-not-exist")
        result = get_default_disk_space_line()
        assert result.startswith("[HDD]")

    def test_real_path_returns_hdd_line(self, real_font_env, tmp_path):
        real_font_env.ROMS_FOLDER = str(tmp_path)
        result = get_default_disk_space_line()
        assert result.startswith("[HDD]")


class TestResolutionLine:
    def test_from_system_info(self, real_font_env):
        real_font_env.SYSTEM_INFO = {"display_resolution": "1920x1080"}
        assert get_display_resolution_line() == "Res : 1920x1080"

    def test_fallback_to_screen(self, real_font_env):
        real_font_env.SYSTEM_INFO = {}
        assert get_display_resolution_line() == "Res : 1280x720"

    def test_no_info(self, real_font_env, monkeypatch):
        monkeypatch.delattr(config, "SYSTEM_INFO", raising=False)
        assert get_display_resolution_line() == "Res : 1280x720"
