"""
Font caching for badge rendering.
"""

import os
import pygame  # type: ignore

import config

_BADGE_FONT_CACHE: dict = {}


def get_badge_font(size: int) -> pygame.font.Font:
    """Get cached font for badge rendering with specified size and font family."""
    size = max(10, int(size))
    family_id = config.FONT_FAMILIES[config.current_font_family_index] if 0 <= config.current_font_family_index < len(config.FONT_FAMILIES) else "pixel"
    cache_key = (family_id, size)
    if cache_key in _BADGE_FONT_CACHE:
        return _BADGE_FONT_CACHE[cache_key]

    try:
        if family_id == "pixel":
            path = os.path.join(config.APP_FOLDER, "assets", "fonts", "Pixel-UniCode.ttf")
            font = pygame.font.Font(path, size)
        else:
            try:
                font = pygame.font.SysFont("dejavusans", size)
            except Exception:
                font = pygame.font.SysFont("dejavu sans", size)
    except Exception:
        font = config.tiny_font

    _BADGE_FONT_CACHE[cache_key] = font
    return font