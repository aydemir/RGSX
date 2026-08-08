"""Tests for display/core.py: OVERLAY lifecycle and metric sync."""

import pygame

from display import core


def test_overlay_none_before_sync():
    core.OVERLAY = None
    assert core.get_overlay() is None


def test_sync_display_metrics_with_surface():
    core.OVERLAY = None
    screen = pygame.display.set_mode((800, 600))
    result = core.sync_display_metrics(screen)
    assert result is screen
    assert core.OVERLAY is not None
    assert core.OVERLAY.get_size() == (800, 600)
    assert core.get_overlay() is core.OVERLAY
    assert core.get_overlay() is not None


def test_sync_display_metrics_sets_config(monkeypatch):
    import config
    core.OVERLAY = None
    screen = pygame.display.set_mode((640, 480))
    core.sync_display_metrics(screen)
    assert config.screen_width == 640
    assert config.screen_height == 480


def test_sync_display_metrics_none_returns_none(monkeypatch):
    monkeypatch.setattr(pygame.display, "get_surface", lambda: None)
    core.OVERLAY = None
    assert core.sync_display_metrics(None) is None


def test_init_display_returns_surface(monkeypatch):
    import config
    monkeypatch.setattr(config, "screen_width", 0)
    monkeypatch.setattr(config, "screen_height", 0)
    screen = core.init_display()
    assert screen is not None
    assert core.OVERLAY is not None


def test_init_display_windowed_branch(monkeypatch):
    import config

    monkeypatch.setattr(core, "get_display_fullscreen", lambda settings: False)
    monkeypatch.setattr(core, "load_rgsx_settings", lambda: {"display": {"monitor": 0}})

    class FakeInfo:
        current_w = 1920
        current_h = 1080

    monkeypatch.setattr(pygame.display, "Info", lambda: FakeInfo())
    monkeypatch.setattr(pygame.display, "get_desktop_sizes", lambda: [(1920, 1080)])
    monkeypatch.setattr(config, "screen_width", 0)
    monkeypatch.setattr(config, "screen_height", 0)

    screen = core.init_display()
    assert screen is not None
    assert config.screen_width > 0
    assert config.screen_height > 0


def test_init_display_env_monitor_override(monkeypatch):
    import config

    monkeypatch.setattr(core, "get_display_fullscreen", lambda settings: False)
    monkeypatch.setattr(core, "load_rgsx_settings", lambda: {"display": {"monitor": 0}})
    monkeypatch.setenv("RGSX_DISPLAY", "1")  # 1 >= num_displays(1) -> falls back to 0
    monkeypatch.setattr(config, "screen_width", 0)
    monkeypatch.setattr(config, "screen_height", 0)

    screen = core.init_display()
    assert screen is not None
    assert config.current_monitor == 0


def test_init_display_env_invalid_value(monkeypatch):
    import config

    monkeypatch.setattr(core, "get_display_fullscreen", lambda settings: False)
    monkeypatch.setattr(core, "load_rgsx_settings", lambda: {"display": {"monitor": 0}})
    monkeypatch.setenv("RGSX_DISPLAY", "abc")  # ValueError -> override ignored
    monkeypatch.setattr(config, "screen_width", 0)
    monkeypatch.setattr(config, "screen_height", 0)

    screen = core.init_display()
    assert screen is not None
