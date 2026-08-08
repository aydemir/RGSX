"""
UI components: buttons, shadows, glows, header badges.
"""

import math
import pygame  # type: ignore

import config
from .colors import THEME_COLORS
from .fonts import get_badge_font
from utils import wrap_text, truncate_text_end
from rgsx_settings import get_light_mode


def draw_shadow(surface: pygame.Surface, rect: pygame.Rect, offset: int = 6, alpha: int = 120, light_mode: bool | None = None) -> pygame.Surface | None:
    """Draw drop shadow for a rectangle. Disabled in light mode."""
    if light_mode is None:
        light_mode = get_light_mode()
    if light_mode:
        return None
    shadow = pygame.Surface((rect.width + offset, rect.height + offset), pygame.SRCALPHA)
    pygame.draw.rect(shadow, (0, 0, 0, alpha), (0, 0, rect.width + offset, rect.height + offset), border_radius=15)
    return shadow


def draw_glow_effect(screen: pygame.Surface, rect: pygame.Rect, color: tuple, intensity: int = 80, size: int = 10, light_mode: bool | None = None) -> None:
    """Draw glow effect around a rectangle. Disabled in light mode."""
    if light_mode is None:
        light_mode = get_light_mode()
    if light_mode:
        return
    glow = pygame.Surface((rect.width + size * 2, rect.height + size * 2), pygame.SRCALPHA)
    for i in range(size):
        alpha = int(intensity * (1 - i / size))
        pygame.draw.rect(glow, (*color[:3], alpha),
                        (i, i, rect.width + (size - i) * 2, rect.height + (size - i) * 2),
                        border_radius=15)
    screen.blit(glow, (rect.x - size, rect.y - size))


def draw_stylized_button(
    screen: pygame.Surface,
    text: str,
    x: int,
    y: int,
    width: int,
    height: int,
    selected: bool = False,
    light_mode: bool | None = None,
) -> None:
    """Draw modern button with hover effect, shadow, and rounded corners."""
    if light_mode is None:
        light_mode = get_light_mode()

    button_color = THEME_COLORS["button_hover"] if selected else THEME_COLORS["button_idle"]

    if light_mode:
        pygame.draw.rect(screen, button_color[:3], (x, y, width, height), border_radius=8)
        if selected:
            pygame.draw.rect(screen, THEME_COLORS["neon"], (x, y, width, height), width=2, border_radius=8)
    else:
        shadow_surf = pygame.Surface((width + 6, height + 6), pygame.SRCALPHA)
        pygame.draw.rect(shadow_surf, THEME_COLORS["shadow"], (3, 3, width, height), border_radius=12)
        screen.blit(shadow_surf, (x - 3, y - 3))

        button_surface = pygame.Surface((width, height), pygame.SRCALPHA)

        if selected:
            for i in range(height):
                ratio = i / height
                brightness = 1 + 0.2 * ratio
                r = min(255, int(button_color[0] * brightness))
                g = min(255, int(button_color[1] * brightness))
                b = min(255, int(button_color[2] * brightness))
                alpha = button_color[3] if len(button_color) > 3 else 255
                pygame.draw.rect(button_surface, (r, g, b, alpha), (0, i, width, 1))

            mask_surface = pygame.Surface((width, height), pygame.SRCALPHA)
            pygame.draw.rect(mask_surface, (255, 255, 255, 255), (0, 0, width, height), border_radius=12)
            button_surface.blit(mask_surface, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
        else:
            pygame.draw.rect(button_surface, button_color, (0, 0, width, height), border_radius=12)

        highlight = pygame.Surface((width - 4, height // 3), pygame.SRCALPHA)
        highlight.fill(THEME_COLORS["highlight"])
        button_surface.blit(highlight, (2, 2))

        pygame.draw.rect(button_surface, THEME_COLORS["border"], (0, 0, width, height), 2, border_radius=12)

        if selected:
            glow_surface = pygame.Surface((width + 16, height + 16), pygame.SRCALPHA)
            for i in range(6):
                alpha = int(40 * (1 - i / 6))
                pygame.draw.rect(glow_surface, (*THEME_COLORS["glow"][:3], alpha),
                               (i, i, width + 16 - i * 2, height + 16 - i * 2), border_radius=15)
            screen.blit(glow_surface, (x - 8, y - 8))

        screen.blit(button_surface, (x, y))

    text_surface = config.font.render(text, True, THEME_COLORS["text"])
    available_width = width - 20

    if text_surface.get_width() > available_width:
        truncated_text = text
        while text_surface.get_width() > available_width and len(truncated_text) > 0:
            truncated_text = truncated_text[:-1]
            text_surface = config.font.render(truncated_text + "...", True, THEME_COLORS["text"])

    text_rect = text_surface.get_rect(center=(x + width // 2, y + height // 2))
    screen.blit(text_surface, text_rect)


def get_adaptive_badge_layout(lines: list[str], base_font: pygame.font.Font, max_badge_width: int | None = None, padding_x: int = 12, min_font_size: int = 10) -> tuple[pygame.font.Font, list[str]]:
    """Find the largest font size that fits all lines within max_badge_width."""
    clean_lines = [line for line in lines if isinstance(line, str) and line]
    if not clean_lines:
        return base_font, []
    if not max_badge_width:
        return base_font, clean_lines

    max_text_width = max(40, max_badge_width - padding_x * 2)
    footer_font_scale = config.accessibility_settings.get("footer_font_scale", 1.0)
    nominal_size = max(min_font_size, int(20 * footer_font_scale))
    candidate_sizes = []
    for size in range(nominal_size, min_font_size - 1, -2):
        if size not in candidate_sizes:
            candidate_sizes.append(size)
    if min_font_size not in candidate_sizes:
        candidate_sizes.append(min_font_size)

    for size in candidate_sizes:
        candidate_font = get_badge_font(size)
        if all(candidate_font.size(line)[0] <= max_text_width for line in clean_lines):
            return candidate_font, clean_lines

    fallback_font = get_badge_font(candidate_sizes[-1])
    fitted_lines = [truncate_text_end(line, fallback_font, max_text_width) for line in clean_lines]
    return fallback_font, fitted_lines


def fit_badge_lines(lines: list[str], font: pygame.font.Font, max_badge_width: int | None = None, padding_x: int = 12) -> list[str]:
    """Return lines fitted to badge width."""
    _, fitted_lines = get_adaptive_badge_layout(lines, font, max_badge_width=max_badge_width, padding_x=padding_x)
    return fitted_lines


def measure_header_badge(
    lines: list[str],
    font: pygame.font.Font | None = None,
    max_badge_width: int | None = None,
    padding_x: int = 12,
    padding_y: int = 8,
    line_gap: int = 4,
) -> tuple[int, int, list[str]]:
    """Calculate badge dimensions for given lines."""
    header_font = font or config.tiny_font
    header_font, fitted_lines = get_adaptive_badge_layout(lines, header_font, max_badge_width=max_badge_width, padding_x=padding_x)
    if not fitted_lines:
        return 0, 0, []

    text_surfaces = [header_font.render(line, True, THEME_COLORS["text"]) for line in fitted_lines]
    content_width = max((surface.get_width() for surface in text_surfaces), default=0)
    content_height = sum(surface.get_height() for surface in text_surfaces) + max(0, len(text_surfaces) - 1) * line_gap
    badge_width = content_width + padding_x * 2
    badge_height = content_height + padding_y * 2
    return badge_width, badge_height, fitted_lines


def draw_header_badge(
    screen: pygame.Surface,
    lines: list[str],
    badge_x: int,
    badge_y: int,
    light_mode: bool = False,
    font: pygame.font.Font | None = None,
    max_badge_width: int | None = None,
    padding_x: int = 12,
    padding_y: int = 8,
    line_gap: int = 4,
) -> None:
    """Draw a compact text badge in header area."""
    header_font = font or config.tiny_font
    header_font, _ = get_adaptive_badge_layout(lines, header_font, max_badge_width=max_badge_width, padding_x=padding_x)
    badge_width, badge_height, fitted_lines = measure_header_badge(
        lines,
        font=header_font,
        max_badge_width=max_badge_width,
        padding_x=padding_x,
        padding_y=padding_y,
        line_gap=line_gap,
    )
    if not fitted_lines:
        return

    text_surfaces = [header_font.render(line, True, THEME_COLORS["text"]) for line in fitted_lines]

    if light_mode:
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (badge_x, badge_y, badge_width, badge_height), border_radius=12)
    else:
        shadow = pygame.Surface((badge_width + 8, badge_height + 8), pygame.SRCALPHA)
        pygame.draw.rect(shadow, (0, 0, 0, 110), (4, 4, badge_width, badge_height), border_radius=12)
        screen.blit(shadow, (badge_x - 4, badge_y - 4))

        badge_surface = pygame.Surface((badge_width, badge_height), pygame.SRCALPHA)
        pygame.draw.rect(badge_surface, THEME_COLORS["button_idle"], (0, 0, badge_width, badge_height), border_radius=12)
        highlight = pygame.Surface((badge_width - 6, max(10, badge_height // 3)), pygame.SRCALPHA)
        highlight.fill((255, 255, 255, 18))
        badge_surface.blit(highlight, (3, 3))
        screen.blit(badge_surface, (badge_x, badge_y))

    pygame.draw.rect(screen, THEME_COLORS["border"], (badge_x, badge_y, badge_width, badge_height), 2, border_radius=12)

    current_y = badge_y + padding_y
    for surface in text_surfaces:
        line_x = badge_x + (badge_width - surface.get_width()) // 2
        screen.blit(surface, (line_x, current_y))
        current_y += surface.get_height() + line_gap