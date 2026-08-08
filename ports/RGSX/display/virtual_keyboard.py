
"""virtual_keyboard module."""

import pygame  # type: ignore

import config

from .colors import THEME_COLORS
def draw_virtual_keyboard(screen):
    """Affiche un clavier virtuel avec un style moderne."""
    keyboard_layout = [
        ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
        ['A', 'Z', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
        ['Q', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M'],
        ['W', 'X', 'C', 'V', 'B', 'N']
    ]
    key_width = int(config.screen_width * 0.03125)
    key_height = int(config.screen_height * 0.0556)
    key_spacing = int(config.screen_width * 0.0052)
    keyboard_width = len(keyboard_layout[0]) * (key_width + key_spacing) - key_spacing
    keyboard_height = len(keyboard_layout) * (key_height + key_spacing) - key_spacing
    start_x = (config.screen_width - keyboard_width) // 2
    search_bottom_y = int(config.screen_height * 0.111) + (config.search_font.get_height() + 40) // 2
    controls_y = config.screen_height - int(config.screen_height * 0.037)
    available_height = controls_y - search_bottom_y
    start_y = search_bottom_y + (available_height - keyboard_height - 40) // 2

    keyboard_rect = pygame.Rect(start_x - 20, start_y - 20, keyboard_width + 40, keyboard_height + 40)
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], keyboard_rect, border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], keyboard_rect, 2, border_radius=12)

    for row_idx, row in enumerate(keyboard_layout):
        for col_idx, key in enumerate(row):
            x = start_x + col_idx * (key_width + key_spacing)
            y = start_y + row_idx * (key_height + key_spacing)
            key_rect = pygame.Rect(x, y, key_width, key_height)
            if (row_idx, col_idx) == config.selected_key:
                pygame.draw.rect(screen, THEME_COLORS["fond_lignes"] + (150,), key_rect, border_radius=8)
            else:
                pygame.draw.rect(screen, THEME_COLORS["button_idle"], key_rect, border_radius=8)
            pygame.draw.rect(screen, THEME_COLORS["border"], key_rect, 1, border_radius=8)
            text = config.font.render(key, True, THEME_COLORS["text"])
            text_rect = text.get_rect(center=key_rect.center)
            screen.blit(text, text_rect)
