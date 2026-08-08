"""Smoke tests: the display package imports cleanly and exports everything in __all__."""

import pygame

import display


def test_package_imports():
    assert display.__doc__ and "display" in display.__doc__.lower()


def test_all_exports_exist():
    for name in display.__all__:
        assert hasattr(display, name), f"missing export: {name}"


def test_export_count():
    assert len(display.__all__) >= 80


def test_public_api_conversions_present():
    for name in [
        "get_badge_font",
        "get_adaptive_badge_layout",
        "fit_badge_lines",
        "format_disk_size_gb",
        "render_combined_footer_controls",
        "get_overlay",
    ]:
        assert name in display.__all__
        assert callable(getattr(display, name))


def test_core_overlay_accessor():
    from display import core, get_overlay

    core.OVERLAY = None
    assert get_overlay() is None
    surf = pygame.Surface((10, 10))
    core.OVERLAY = surf
    assert get_overlay() is surf
