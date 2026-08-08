"""
Theme colors and background theme presets for RGSX display.
"""

from typing import Tuple

# Modern color palette
THEME_COLORS = {
    # Selected line background
    "fond_lignes": (0, 255, 0),  # green
    # Default grid system image background
    "fond_image": (50, 50, 70),  # Dark metallic blue
    # Neon grid accent
    "neon": (0, 134, 179),  # blue
    # Dark gradient background
    "background_top": (20, 25, 35),
    "background_bottom": (45, 55, 75),  # Black to dark blue
    # Frame backgrounds
    "button_idle": (45, 50, 65, 180),  # Dark metallic blue with opacity
    # Selected button background
    "button_selected": (70, 80, 110, 220),  # Lighter blue
    # Hover button in popups/menu
    "button_hover": (255, 0, 255, 240),  # Bright pink
    # Generic
    "text": (255, 255, 255),  # white
    # Selected text (alias for compatibility)
    "text_selected": (0, 255, 0),  # same green as fond_lignes
    # Error
    "error_text": (255, 60, 60),  # bright red
    # Success
    "success_text": (0, 255, 150),  # cyan green
    # Warning
    "warning_text": (255, 150, 0),  # bright orange
    # Titles
    "title_text": (220, 220, 230),  # very light gray
    # Borders
    "border": (100, 120, 150),  # Bluish borders
    "border_selected": (0, 255, 150),  # Cyan green border for selection
    # Filter colors
    "green": (0, 255, 0),  # green
    "red": (255, 0, 0),  # red
    # Modern effects
    "shadow": (0, 0, 0, 100),  # Drop shadow
    "glow": (100, 180, 255, 40),  # Soft blue glow
    "highlight": (255, 255, 255, 20),  # Subtle highlight
    "accent_gradient_start": (80, 120, 200),  # Accent gradient start
    "accent_gradient_end": (120, 80, 200),  # Accent gradient end
}

# Background theme presets
BACKGROUND_THEME_PRESETS = {
    "default": {
        "label_key": "background_theme_default",
        "top": THEME_COLORS["background_top"],
        "bottom": THEME_COLORS["background_bottom"],
    },
    "sunset": {
        "label_key": "background_theme_sunset",
        "top": (52, 24, 44),
        "bottom": (173, 82, 56),
    },
    "forest": {
        "label_key": "background_theme_forest",
        "top": (18, 36, 32),
        "bottom": (50, 88, 72),
    },
    "midnight": {
        "label_key": "background_theme_midnight",
        "top": (8, 13, 26),
        "bottom": (27, 43, 79),
    },
}


def get_background_theme_colors(theme_key: str | None = None) -> Tuple[Tuple[int, int, int], Tuple[int, int, int]]:
    """Get gradient colors for the selected background theme."""
    from rgsx_settings import get_display_background_theme

    selected_theme = (theme_key or get_display_background_theme() or "default").lower()
    preset = BACKGROUND_THEME_PRESETS.get(selected_theme, BACKGROUND_THEME_PRESETS["default"])
    return preset["top"], preset["bottom"]


def get_background_theme_label(theme_key: str | None = None) -> str:
    """Get translated label for the selected background theme."""
    from language import _
    from rgsx_settings import get_display_background_theme

    selected_theme = (theme_key or get_display_background_theme() or "default").lower()
    preset = BACKGROUND_THEME_PRESETS.get(selected_theme, BACKGROUND_THEME_PRESETS["default"])
    label_key = preset.get("label_key")
    if not label_key:
        return selected_theme
    translated = _(label_key) if _ else label_key
    return translated if translated != label_key else selected_theme