"""
Screen rendering: loading, error, popup, toast.
"""

import pygame  # type: ignore

import config
from language import _
from utils import wrap_text, truncate_text_middle
from .colors import THEME_COLORS
from .background import draw_app_background
from .components import draw_stylized_button
from . import core


def draw_loading_screen(screen: pygame.Surface) -> None:
    """Draw loading screen with disclaimer and progress."""
    disclaimer_lines = [
        _("welcome_message"),
        _("disclaimer_line1"),
        _("disclaimer_line2"),
        _("disclaimer_line3"),
        _("disclaimer_line4"),
        _("disclaimer_line5"),
    ]

    margin_horizontal = int(config.screen_width * 0.025)
    padding_vertical = int(config.screen_height * 0.0185)
    padding_between = int(config.screen_height * 0.0074)
    border_radius = 16
    border_width = 3
    shadow_offset = 6

    line_height = config.small_font.get_height() + padding_between
    total_height = line_height * len(disclaimer_lines) - padding_between
    rect_width = config.screen_width - 2 * margin_horizontal
    rect_height = total_height + 2 * padding_vertical
    rect_x = margin_horizontal
    rect_y = int(config.screen_height * 0.0185)

    shadow_rect = pygame.Rect(rect_x + shadow_offset, rect_y + shadow_offset, rect_width, rect_height)
    shadow_surface = pygame.Surface((rect_width, rect_height), pygame.SRCALPHA)
    pygame.draw.rect(shadow_surface, (0, 0, 0, 100), shadow_surface.get_rect(), border_radius=border_radius)
    screen.blit(shadow_surface, shadow_rect.topleft)

    disclaimer_rect = pygame.Rect(rect_x, rect_y, rect_width, rect_height)
    disclaimer_surface = pygame.Surface((rect_width, rect_height), pygame.SRCALPHA)
    pygame.draw.rect(disclaimer_surface, THEME_COLORS["button_idle"], disclaimer_surface.get_rect(), border_radius=border_radius)
    screen.blit(disclaimer_surface, disclaimer_rect.topleft)

    pygame.draw.rect(screen, THEME_COLORS["border"], disclaimer_rect, border_width, border_radius=border_radius)

    max_text_width = rect_width - 2 * padding_vertical
    for i, line in enumerate(disclaimer_lines):
        wrapped_lines = wrap_text(line, config.small_font, max_text_width)
        for j, wrapped_line in enumerate(wrapped_lines):
            text_surface = config.small_font.render(wrapped_line, True, THEME_COLORS["title_text"])
            text_rect = text_surface.get_rect(center=(
                config.screen_width // 2,
                rect_y + padding_vertical + (i * len(wrapped_lines) + j + 0.5) * line_height - padding_between // 2
            ))
            screen.blit(text_surface, text_rect)

    loading_y = rect_y + rect_height + int(config.screen_height * 0.0926)
    text = config.small_font.render(
        truncate_text_middle(f"{config.current_loading_system}", config.small_font, config.screen_width - 2 * margin_horizontal, is_filename=False),
        True,
        THEME_COLORS["text"]
    )
    text_rect = text.get_rect(center=(config.screen_width // 2, loading_y))
    screen.blit(text, text_rect)

    progress_text = config.small_font.render(_("loading_progress").format(int(config.loading_progress)), True, THEME_COLORS["text"])
    progress_rect = progress_text.get_rect(center=(config.screen_width // 2, loading_y + int(config.screen_height * 0.0463)))
    screen.blit(progress_text, progress_rect)

    bar_width = int(config.screen_width * 0.2083)
    bar_height = int(config.screen_height * 0.037)
    bar_y = loading_y + int(config.screen_height * 0.0926)
    progress_width = (bar_width * config.loading_progress) / 100
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (config.screen_width // 2 - bar_width // 2, bar_y, bar_width, bar_height), border_radius=8)
    pygame.draw.rect(screen, THEME_COLORS["fond_lignes"], (config.screen_width // 2 - bar_width // 2, bar_y, progress_width, bar_height), border_radius=8)

    detail_lines = getattr(config, 'loading_detail_lines', []) or []
    detail_y = bar_y + bar_height + 14
    max_detail_width = config.screen_width - 2 * margin_horizontal
    rendered_lines = []
    for detail_line in detail_lines:
        if not detail_line:
            continue
        rendered_lines.append(truncate_text_middle(str(detail_line), config.small_font, max_detail_width, is_filename=False))

    for index, detail_line in enumerate(rendered_lines[:3]):
        detail_surface = config.small_font.render(detail_line, True, THEME_COLORS["title_text"])
        detail_rect = detail_surface.get_rect(center=(config.screen_width // 2, detail_y + index * (config.small_font.get_height() + 4)))
        screen.blit(detail_surface, detail_rect)


def draw_error_screen(screen: pygame.Surface) -> None:
    """Draw error screen with message and OK button."""
    wrapped_message = wrap_text(config.error_message, config.small_font, config.screen_width - 80)
    line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 80
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    screen.blit(core.OVERLAY, (0, 0))
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_message):
        text = config.small_font.render(line, True, THEME_COLORS["error_text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    draw_stylized_button(screen, _("button_OK"), rect_x + rect_width // 2 - 80, rect_y + text_height + margin_top_bottom, 160, button_height, selected=True)


def draw_popup(screen: pygame.Surface) -> None:
    """Draw popup with message and countdown timer."""
    screen.blit(core.OVERLAY, (0, 0))

    popup_width = int(config.screen_width * 0.8)
    max_inner_width = popup_width - 60
    line_height = config.small_font.get_height() + 8
    margin_top_bottom = 24

    raw_segments = config.popup_message.split('\n') if config.popup_message else []
    wrapped_lines = []
    for seg in raw_segments:
        if seg.strip() == "":
            wrapped_lines.append("")
        else:
            wrapped_lines.extend(wrap_text(seg, config.small_font, max_inner_width))
    if not wrapped_lines:
        wrapped_lines = [""]

    text_height = len(wrapped_lines) * line_height
    popup_height = text_height + 2 * margin_top_bottom + line_height
    popup_x = (config.screen_width - popup_width) // 2
    popup_y = (config.screen_height - popup_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (popup_x, popup_y, popup_width, popup_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (popup_x, popup_y, popup_width, popup_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_lines):
        text_surface = config.small_font.render(line, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=(config.screen_width // 2, popup_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text_surface, text_rect)

    remaining_time = max(0, config.popup_timer // 1000)
    countdown_text = _("popup_countdown").format(remaining_time, 's' if remaining_time != 1 else '')
    countdown_surface = config.small_font.render(countdown_text, True, THEME_COLORS["text"])
    countdown_rect = countdown_surface.get_rect(center=(config.screen_width // 2, popup_y + margin_top_bottom + len(wrapped_lines) * line_height + line_height // 2))
    screen.blit(countdown_surface, countdown_rect)


def draw_toast(screen: pygame.Surface) -> None:
    """Draw toast notification in top-right corner."""
    if not hasattr(config, 'toast_message') or not config.toast_message:
        return

    if not hasattr(config, 'toast_start_time'):
        config.toast_start_time = pygame.time.get_ticks()

    current_time = pygame.time.get_ticks()
    elapsed = current_time - config.toast_start_time

    toast_duration = getattr(config, 'toast_duration', 2000)

    if elapsed > toast_duration:
        config.toast_message = ""
        config.toast_start_time = 0
        return

    opacity = 255
    fade_start = max(0, toast_duration - 300)
    if elapsed > fade_start:
        opacity = int(255 * (1 - (elapsed - fade_start) / 300))

    toast_padding = 15
    line_height = config.small_font.get_height() + 6

    text_lines = config.toast_message.split('\n')
    wrapped_lines = []
    max_width = int(config.screen_width * 0.3)

    for line in text_lines:
        if line.strip() == "":
            wrapped_lines.append("")
        else:
            wrapped_lines.extend(wrap_text(line, config.small_font, max_width - 2 * toast_padding))

    toast_width = max_width
    toast_height = len(wrapped_lines) * line_height + 2 * toast_padding

    margin = 20
    toast_x = config.screen_width - toast_width - margin
    toast_y = margin

    toast_surface = pygame.Surface((toast_width, toast_height), pygame.SRCALPHA)

    toast_bg_color = (*THEME_COLORS["fond_lignes"], int(opacity * 0.4))
    toast_border_color = (*THEME_COLORS["fond_lignes"], int(opacity))

    pygame.draw.rect(toast_surface, toast_bg_color, (0, 0, toast_width, toast_height), border_radius=8)
    pygame.draw.rect(toast_surface, toast_border_color, (0, 0, toast_width, toast_height), 2, border_radius=8)

    for i, line in enumerate(wrapped_lines):
        text_render = config.small_font.render(line, True, THEME_COLORS["text"])
        toast_surface.blit(text_render, (toast_padding, toast_padding + i * line_height))

    screen.blit(toast_surface, (toast_x, toast_y))


def show_toast(message: str, duration: int = 2000) -> None:
    """Helper to show a toast notification."""
    config.toast_message = message
    config.toast_duration = duration
    config.toast_start_time = pygame.time.get_ticks()