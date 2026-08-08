
"""global_search module."""

import pygame  # type: ignore

import config

from language import _

from utils import (truncate_text_middle, wrap_text, truncate_text_end)

from .colors import THEME_COLORS
from .game_list import draw_game_scrollbar, get_display_extension

from . import core
def draw_global_search_list(screen):
    """Affiche la vue globale unifiée (recherche, filtre, tri)."""
    query = getattr(config, 'global_search_query', '') or ''
    results = getattr(config, 'global_search_results', []) or []
    keyboard_active = bool(getattr(config, 'joystick', False) and getattr(config, 'global_search_editing', False))
    allow_empty = bool(getattr(config, 'global_search_allow_empty', False))
    custom_title = (getattr(config, 'global_search_title_override', '') or '').strip()

    screen.blit(core.OVERLAY, (0, 0))

    title_query = query + "_" if (getattr(config, 'joystick', False) and getattr(config, 'global_search_editing', False)) or (not getattr(config, 'joystick', False)) else query
    if custom_title:
        title_text = custom_title if not title_query else f"{custom_title} : {title_query}"
    else:
        title_text = _("global_search_title").format(title_query)
    if results:
        title_text += f" ({len(results)})"

    title_surface = config.search_font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, title_surface.get_height() // 2 + 20))
    title_rect_inflated = title_rect.inflate(60, 30)
    title_rect_inflated.topleft = ((config.screen_width - title_rect_inflated.width) // 2, 10)

    shadow = pygame.Surface((title_rect_inflated.width + 10, title_rect_inflated.height + 10), pygame.SRCALPHA)
    pygame.draw.rect(shadow, (0, 0, 0, 120), (5, 5, title_rect_inflated.width, title_rect_inflated.height), border_radius=14)
    screen.blit(shadow, (title_rect_inflated.left - 5, title_rect_inflated.top - 5))

    glow = pygame.Surface((title_rect_inflated.width + 20, title_rect_inflated.height + 20), pygame.SRCALPHA)
    pygame.draw.rect(glow, (*THEME_COLORS["glow"][:3], 60), glow.get_rect(), border_radius=16)
    screen.blit(glow, (title_rect_inflated.left - 10, title_rect_inflated.top - 10))

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], title_rect_inflated, 2, border_radius=12)
    screen.blit(title_surface, title_rect)

    reserved_bottom = config.screen_height - 40
    if keyboard_active:
        key_width = int(config.screen_width * 0.03125)
        key_height = int(config.screen_height * 0.0556)
        key_spacing = int(config.screen_width * 0.0052)
        keyboard_layout = [10, 10, 10, 6]
        keyboard_width = keyboard_layout[0] * (key_width + key_spacing) - key_spacing
        keyboard_height = len(keyboard_layout) * (key_height + key_spacing) - key_spacing
        start_x = (config.screen_width - keyboard_width) // 2
        search_bottom_y = int(config.screen_height * 0.111) + (config.search_font.get_height() + 40) // 2
        controls_y = config.screen_height - int(config.screen_height * 0.037)
        available_height = controls_y - search_bottom_y
        start_y = search_bottom_y + (available_height - keyboard_height - 40) // 2
        reserved_bottom = start_y - 24

    message_zone_top = title_rect_inflated.bottom + 24
    message_zone_bottom = max(message_zone_top + 80, reserved_bottom)

    if not query.strip() and not allow_empty:
        message = _("global_search_empty_query")
        lines = wrap_text(message, config.font, config.screen_width - 80)
        line_height = config.font.get_height() + 5
        text_height = len(lines) * line_height
        margin_top_bottom = 20
        rect_height = text_height + 2 * margin_top_bottom
        max_text_width = max([config.font.size(line)[0] for line in lines], default=300)
        rect_width = max_text_width + 80
        rect_x = (config.screen_width - rect_width) // 2
        available_message_height = max(rect_height, message_zone_bottom - message_zone_top)
        rect_y = message_zone_top + max(0, (available_message_height - rect_height) // 2)

        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
        pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

        for i, line in enumerate(lines):
            text_surface = config.font.render(line, True, THEME_COLORS["text"])
            text_rect = text_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
            screen.blit(text_surface, text_rect)
        return

    if not results:
        message = _("global_search_no_results").format(query)
        lines = wrap_text(message, config.font, config.screen_width - 80)
        line_height = config.font.get_height() + 5
        text_height = len(lines) * line_height
        margin_top_bottom = 20
        rect_height = text_height + 2 * margin_top_bottom
        max_text_width = max([config.font.size(line)[0] for line in lines], default=300)
        rect_width = max_text_width + 80
        rect_x = (config.screen_width - rect_width) // 2
        available_message_height = max(rect_height, message_zone_bottom - message_zone_top)
        rect_y = message_zone_top + max(0, (available_message_height - rect_height) // 2)

        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
        pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

        for i, line in enumerate(lines):
            text_surface = config.font.render(line, True, THEME_COLORS["text"])
            text_rect = text_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
            screen.blit(text_surface, text_rect)
        return

    line_height = config.small_font.get_height() + 10
    header_height = line_height
    margin_top_bottom = 20
    extra_margin_top = 20
    extra_margin_bottom = 60
    title_height = config.title_font.get_height() + 20
    available_height = config.screen_height - title_height - extra_margin_top - extra_margin_bottom - 2 * margin_top_bottom - header_height
    items_per_page = max(1, available_height // line_height)

    rect_height = header_height + items_per_page * line_height + 2 * margin_top_bottom
    rect_width = int(0.95 * config.screen_width)
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = title_height + extra_margin_top + (config.screen_height - title_height - extra_margin_top - extra_margin_bottom - rect_height) // 2

    config.global_search_scroll_offset = max(0, min(config.global_search_scroll_offset, max(0, len(results) - items_per_page)))
    if config.global_search_selected < config.global_search_scroll_offset:
        config.global_search_scroll_offset = config.global_search_selected
    elif config.global_search_selected >= config.global_search_scroll_offset + items_per_page:
        config.global_search_scroll_offset = config.global_search_selected - items_per_page + 1

    shadow_rect = pygame.Rect(rect_x + 6, rect_y + 6, rect_width, rect_height)
    shadow_surf = pygame.Surface((rect_width + 8, rect_height + 8), pygame.SRCALPHA)
    pygame.draw.rect(shadow_surf, (0, 0, 0, 100), (4, 4, rect_width, rect_height), border_radius=14)
    screen.blit(shadow_surf, (rect_x - 4, rect_y - 4))

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    highlight = pygame.Surface((rect_width - 8, 40), pygame.SRCALPHA)
    highlight.fill((255, 255, 255, 15))
    screen.blit(highlight, (rect_x + 4, rect_y + 4))
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    ext_col_width = max(90, int(rect_width * 0.08))
    size_col_width = max(120, int(rect_width * 0.15))
    platform_col_width = max(220, int(rect_width * 0.22))
    name_col_width = rect_width - 40 - platform_col_width - ext_col_width - size_col_width
    header_y_center = rect_y + margin_top_bottom + header_height // 2

    header_platform_surface = config.small_font.render(_("history_column_system"), True, THEME_COLORS["text"])
    header_platform_rect = header_platform_surface.get_rect()
    header_platform_rect.midleft = (rect_x + 20, header_y_center)
    header_name_surface = config.small_font.render(_("game_header_name"), True, THEME_COLORS["text"])
    header_name_rect = header_name_surface.get_rect()
    header_name_rect.midleft = (rect_x + 20 + platform_col_width, header_y_center)
    header_ext_surface = config.small_font.render(_("game_header_ext"), True, THEME_COLORS["text"])
    header_ext_rect = header_ext_surface.get_rect()
    header_ext_rect.center = (rect_x + rect_width - 20 - size_col_width - ext_col_width // 2, header_y_center)
    header_size_surface = config.small_font.render(_("game_header_size"), True, THEME_COLORS["text"])
    header_size_rect = header_size_surface.get_rect()
    header_size_rect.midright = (rect_x + rect_width - 20, header_y_center)
    screen.blit(header_platform_surface, header_platform_rect)
    screen.blit(header_name_surface, header_name_rect)
    screen.blit(header_ext_surface, header_ext_rect)
    screen.blit(header_size_surface, header_size_rect)

    separator_y = rect_y + margin_top_bottom + header_height
    pygame.draw.line(screen, THEME_COLORS["border"], (rect_x + 20, separator_y), (rect_x + rect_width - 20, separator_y), 2)
    list_start_y = rect_y + margin_top_bottom + header_height

    for i in range(config.global_search_scroll_offset, min(config.global_search_scroll_offset + items_per_page, len(results))):
        item = results[i]
        row_center_y = list_start_y + (i - config.global_search_scroll_offset) * line_height + line_height // 2
        is_selected = i == config.global_search_selected
        row_color = THEME_COLORS["fond_lignes"] if is_selected else THEME_COLORS["text"]

        platform_text = truncate_text_end(item["platform_label"], config.small_font, platform_col_width - 10)
        game_text = truncate_text_middle(item["display_name"], config.small_font, name_col_width - 10, is_filename=False)
        ext_text = get_display_extension(item.get("game_name"))
        size_value = item.get("size")
        size_text = size_value if (isinstance(size_value, str) and size_value.strip()) else "N/A"

        platform_surface = config.small_font.render(platform_text, True, row_color)
        game_surface = config.small_font.render(game_text, True, row_color)
        ext_surface = config.small_font.render(ext_text, True, THEME_COLORS["text"])
        size_surface = config.small_font.render(size_text, True, THEME_COLORS["text"])

        platform_rect = platform_surface.get_rect()
        platform_rect.midleft = (rect_x + 20, row_center_y)
        game_rect = game_surface.get_rect()
        game_rect.midleft = (rect_x + 20 + platform_col_width, row_center_y)
        ext_rect = ext_surface.get_rect()
        ext_rect.center = (rect_x + rect_width - 20 - size_col_width - ext_col_width // 2, row_center_y)
        size_rect = size_surface.get_rect()
        size_rect.midright = (rect_x + rect_width - 20, row_center_y)

        if is_selected:
            glow_width = rect_width - 40
            glow_height = game_rect.height + 12
            glow_surface = pygame.Surface((glow_width + 6, glow_height + 6), pygame.SRCALPHA)
            pygame.draw.rect(glow_surface, (*THEME_COLORS["fond_lignes"][:3], 50), (3, 3, glow_width, glow_height), border_radius=8)
            screen.blit(glow_surface, (rect_x + 17, row_center_y - glow_height // 2 - 3))

            selection_bg = pygame.Surface((glow_width, glow_height), pygame.SRCALPHA)
            for j in range(glow_height):
                ratio = j / glow_height
                alpha = int(60 + 20 * ratio)
                pygame.draw.line(selection_bg, (*THEME_COLORS["fond_lignes"][:3], alpha), (0, j), (glow_width, j))
            screen.blit(selection_bg, (rect_x + 20, row_center_y - glow_height // 2))

            border_rect = pygame.Rect(rect_x + 20, row_center_y - glow_height // 2, glow_width, glow_height)
            pygame.draw.rect(screen, (*THEME_COLORS["fond_lignes"][:3], 120), border_rect, width=1, border_radius=8)

        screen.blit(platform_surface, platform_rect)
        screen.blit(game_surface, game_rect)
        screen.blit(ext_surface, ext_rect)
        screen.blit(size_surface, size_rect)

    if len(results) > items_per_page:
        draw_game_scrollbar(
            screen,
            config.global_search_scroll_offset,
            len(results),
            items_per_page,
            rect_x + rect_width - 10,
            rect_y,
            rect_height
        )
