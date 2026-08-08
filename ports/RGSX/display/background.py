"""
Background rendering with gradient and grain texture.
"""

import random
import pygame  # type: ignore

from .colors import THEME_COLORS
from rgsx_settings import get_light_mode


_gradient_cache: dict = {"surface": None, "top": None, "bottom": None, "size": None}
_grain_cache: dict = {"surface": None, "size": None}


def _build_grain_surface(width: int, height: int) -> pygame.Surface:
    """Build a static grain texture (fixed seed=42) for gradient background."""
    surface = pygame.Surface((width, height), pygame.SRCALPHA)
    random.seed(42)
    for _ in range(width * height // 200):
        x = random.randint(0, width - 1)
        y = random.randint(0, height - 1)
        alpha = random.randint(5, 20)
        surface.set_at((x, y), (255, 255, 255, alpha))
    return surface


def draw_gradient(screen: pygame.Surface, top_color: tuple, bottom_color: tuple, light_mode: bool | None = None) -> None:
    """Draw vertical gradient background with grain texture. Light mode uses solid color."""
    if light_mode is None:
        light_mode = get_light_mode()

    height = screen.get_height()
    width = screen.get_width()

    if light_mode:
        avg_color = (
            (top_color[0] + bottom_color[0]) // 2,
            (top_color[1] + bottom_color[1]) // 2,
            (top_color[2] + bottom_color[2]) // 2,
        )
        screen.fill(avg_color)
        return

    current_size = (width, height)
    if (_gradient_cache["surface"] is not None and
        _gradient_cache["top"] == top_color and
        _gradient_cache["bottom"] == bottom_color and
        _gradient_cache["size"] == current_size):
        screen.blit(_gradient_cache["surface"], (0, 0))
        return

    top_c = pygame.Color(*top_color)
    bottom_c = pygame.Color(*bottom_color)

    gradient = pygame.Surface((width, height))
    for y in range(height):
        ratio = y / height
        color = top_c.lerp(bottom_c, ratio)
        pygame.draw.line(gradient, color, (0, y), (width, y))

    grain = _build_grain_surface(width, height)
    gradient.blit(grain, (0, 0))

    _gradient_cache["surface"] = gradient
    _gradient_cache["top"] = top_color
    _gradient_cache["bottom"] = bottom_color
    _gradient_cache["size"] = current_size

    screen.blit(gradient, (0, 0))


def draw_app_background(screen: pygame.Surface, light_mode: bool | None = None) -> None:
    """Draw app background using current theme."""
    from .colors import get_background_theme_colors
    top_color, bottom_color = get_background_theme_colors()
    draw_gradient(screen, top_color, bottom_color, light_mode=light_mode)