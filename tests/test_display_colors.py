"""Tests for display/colors.py theme presets and helpers."""

from display.colors import (
    THEME_COLORS,
    BACKGROUND_THEME_PRESETS,
    get_background_theme_colors,
    get_background_theme_label,
)


class TestThemeColors:
    def test_required_keys_present(self):
        for key in ["fond_lignes", "text", "border", "button_selected", "title_text", "green", "red"]:
            assert key in THEME_COLORS

    def test_presets_have_expected_keys(self):
        assert set(BACKGROUND_THEME_PRESETS) == {"default", "sunset", "forest", "midnight"}

    def test_preset_shape(self):
        for preset in BACKGROUND_THEME_PRESETS.values():
            assert "label_key" in preset
            assert "top" in preset
            assert "bottom" in preset


class TestBackgroundThemeColors:
    def test_default_explicit(self):
        assert get_background_theme_colors("default") == (
            THEME_COLORS["background_top"],
            THEME_COLORS["background_bottom"],
        )

    def test_sunset(self):
        top, bottom = get_background_theme_colors("sunset")
        assert top == (52, 24, 44)
        assert bottom == (173, 82, 56)

    def test_unknown_falls_back_to_default(self):
        assert get_background_theme_colors("nope") == get_background_theme_colors("default")

    def test_case_insensitive(self):
        assert get_background_theme_colors("SUNSET") == get_background_theme_colors("sunset")


class TestBackgroundThemeLabel:
    def test_known_theme(self, monkeypatch):
        import sys
        import types

        fake_language = types.SimpleNamespace(_=lambda key: key)
        fake_settings = types.SimpleNamespace(get_display_background_theme=lambda: "default")
        monkeypatch.setitem(sys.modules, "language", fake_language)
        monkeypatch.setitem(sys.modules, "rgsx_settings", fake_settings)
        assert get_background_theme_label(None) == "default"

    def test_unknown_falls_back_to_key(self, monkeypatch):
        import sys
        import types

        fake_language = types.SimpleNamespace(_=lambda key: key)
        fake_settings = types.SimpleNamespace(get_display_background_theme=lambda: "default")
        monkeypatch.setitem(sys.modules, "language", fake_language)
        monkeypatch.setitem(sys.modules, "rgsx_settings", fake_settings)
        assert get_background_theme_label("nope") == "nope"

    def test_label_key_with_translation(self, monkeypatch):
        import sys
        import types

        fake_language = types.SimpleNamespace(_=lambda key: f"LBL:{key}")
        fake_settings = types.SimpleNamespace(get_display_background_theme=lambda: "sunset")
        monkeypatch.setitem(sys.modules, "language", fake_language)
        monkeypatch.setitem(sys.modules, "rgsx_settings", fake_settings)
        assert get_background_theme_label(None) == "LBL:background_theme_sunset"
