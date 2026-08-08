
"""folder_browser module."""

import os
import pygame  # type: ignore

import config

from language import _

from .colors import THEME_COLORS

from . import core
def draw_folder_browser(screen):
    """Affiche le navigateur de dossiers intégré."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 180))

    screen.blit(core.OVERLAY, (0, 0))
    
    browser_mode = getattr(config, 'folder_browser_mode', 'platform')
    platform_name = getattr(config, 'platform_config_name', '')
    current_path = config.folder_browser_path
    items = config.folder_browser_items
    selection = config.folder_browser_selection
    scroll_offset = config.folder_browser_scroll_offset
    visible_items = config.folder_browser_visible_items
    
    # Dimensions du panneau
    panel_width = int(config.screen_width * 0.8)
    panel_height = int(config.screen_height * 0.85)
    panel_x = (config.screen_width - panel_width) // 2
    panel_y = (config.screen_height - panel_height) // 2
    
    # Fond du panneau
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (panel_x, panel_y, panel_width, panel_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (panel_x, panel_y, panel_width, panel_height), 2, border_radius=12)
    
    # Titre selon le mode
    if browser_mode == "roms_root":
        title = _("folder_browser_title_roms_root") if _ else "Select default ROMs folder"
    elif browser_mode == "history_move":
        title = _("folder_browser_title_history_move") if _ else "Select destination folder"
    else:
        title = _("folder_browser_title").format(platform_name) if _ else f"Select folder for {platform_name}"
    title_text = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_text.get_rect(center=(config.screen_width // 2, panel_y + 30))
    screen.blit(title_text, title_rect)
    
    # Chemin actuel (tronqué si trop long)
    path_max_width = panel_width - 40
    path_display = current_path
    if not path_display and os.name == 'nt':
        path_display = "Available drives"
    while config.small_font.size(path_display)[0] > path_max_width and len(path_display) > 10:
        path_display = "..." + path_display[4:]
    path_text = config.small_font.render(path_display, True, THEME_COLORS["highlight"])
    path_rect = path_text.get_rect(center=(config.screen_width // 2, panel_y + 70))
    screen.blit(path_text, path_rect)
    
    # Zone de liste des dossiers
    list_y = panel_y + 100
    list_height = panel_height - 180
    item_height = max(35, config.small_font.get_height() + 10)
    visible_items = max(1, list_height // item_height)
    config.folder_browser_visible_items = visible_items

    max_scroll_offset = max(0, len(items) - visible_items)
    if scroll_offset > max_scroll_offset:
        scroll_offset = max_scroll_offset
        config.folder_browser_scroll_offset = scroll_offset

    if selection >= len(items) and items:
        selection = len(items) - 1
        config.folder_browser_selection = selection
    
    # Afficher les éléments visibles
    for i in range(visible_items):
        item_index = scroll_offset + i
        if item_index >= len(items):
            break
        
        item = items[item_index]
        item_y = list_y + i * item_height
        is_selected = item_index == selection
        
        # Fond de l'élément sélectionné
        if is_selected:
            sel_rect = (panel_x + 20, item_y, panel_width - 40, item_height)
            pygame.draw.rect(screen, THEME_COLORS["button_hover"], sel_rect, border_radius=6)
            pygame.draw.rect(screen, THEME_COLORS["highlight"], sel_rect, 2, border_radius=6)
        
        # Icône dossier (texte simple au lieu d'emoji)
        is_drive = isinstance(item, str) and len(item) >= 2 and item[1] == ':'
        folder_icon = "[..]" if item == ".." else ("[DRV]" if is_drive else "[D]")
        icon_text = config.small_font.render(folder_icon, True, THEME_COLORS["highlight"] if item == ".." else THEME_COLORS["text"])
        icon_x = panel_x + 30
        icon_y = item_y + (item_height - icon_text.get_height()) // 2
        screen.blit(icon_text, (icon_x, icon_y))
        
        # Nom du dossier
        display_name = _("folder_browser_parent") if item == ".." and _ else (".." if item == ".." else item)
        text_color = THEME_COLORS["highlight"] if is_selected else THEME_COLORS["text"]
        item_text = config.small_font.render(display_name, True, text_color)
        text_x = icon_x + icon_text.get_width() + 12
        screen.blit(item_text, (text_x, item_y + (item_height - item_text.get_height()) // 2))
    
    # Indicateur de scroll si nécessaire
    if len(items) > visible_items:
        scrollbar_x = panel_x + panel_width - 25
        scrollbar_y = list_y
        scrollbar_height = list_height
        scrollbar_width = 8
        
        # Fond de la scrollbar
        pygame.draw.rect(screen, THEME_COLORS["border"], (scrollbar_x, scrollbar_y, scrollbar_width, scrollbar_height), border_radius=4)
        
        # Curseur de la scrollbar
        cursor_height = max(20, scrollbar_height * visible_items // len(items))
        cursor_y = scrollbar_y + (scrollbar_height - cursor_height) * scroll_offset // max(1, len(items) - visible_items)
        pygame.draw.rect(screen, THEME_COLORS["highlight"], (scrollbar_x, cursor_y, scrollbar_width, cursor_height), border_radius=4)

def draw_folder_browser_new_folder(screen):
    """Affiche l'écran de création d'un nouveau dossier avec clavier virtuel."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 200))

    screen.blit(core.OVERLAY, (0, 0))
    
    # Dimensions du panneau
    panel_width = int(config.screen_width * 0.7)
    panel_height = int(config.screen_height * 0.6)
    panel_x = (config.screen_width - panel_width) // 2
    panel_y = (config.screen_height - panel_height) // 2
    
    # Fond du panneau
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (panel_x, panel_y, panel_width, panel_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (panel_x, panel_y, panel_width, panel_height), 2, border_radius=12)
    
    # Titre
    title = _("folder_new_title") if _ else "Create New Folder"
    title_text = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_text.get_rect(center=(config.screen_width // 2, panel_y + 30))
    screen.blit(title_text, title_rect)
    
    # Champ de saisie avec le nom actuel
    folder_name = getattr(config, 'new_folder_name', '')
    input_y = panel_y + 70
    input_width = panel_width - 60
    input_height = 40
    input_x = panel_x + 30
    
    # Fond du champ de saisie
    pygame.draw.rect(screen, THEME_COLORS["button_selected"], (input_x, input_y, input_width, input_height), border_radius=6)
    pygame.draw.rect(screen, THEME_COLORS["border_selected"], (input_x, input_y, input_width, input_height), 2, border_radius=6)
    
    # Texte du champ de saisie avec curseur
    display_text = folder_name + "_"
    input_text = config.font.render(display_text, True, THEME_COLORS["text"])
    input_rect = input_text.get_rect(midleft=(input_x + 10, input_y + input_height // 2))
    screen.blit(input_text, input_rect)
    
    # Clavier virtuel
    keyboard_layout = [
        ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
        ['A', 'Z', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
        ['Q', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M'],
        ['W', 'X', 'C', 'V', 'B', 'N', '-', '_', '.']
    ]
    
    selected_row, selected_col = getattr(config, 'new_folder_selected_key', (0, 0))
    
    keyboard_y = input_y + input_height + 30
    key_size = min(40, (panel_width - 60) // 10)
    key_gap = 5
    
    for row_idx, row in enumerate(keyboard_layout):
        row_width = len(row) * (key_size + key_gap) - key_gap
        row_x = (config.screen_width - row_width) // 2
        
        for col_idx, key in enumerate(row):
            key_x = row_x + col_idx * (key_size + key_gap)
            key_y = keyboard_y + row_idx * (key_size + key_gap)
            
            is_selected = (row_idx == selected_row and col_idx == selected_col)
            
            # Fond de la touche
            if is_selected:
                pygame.draw.rect(screen, THEME_COLORS["button_hover"], (key_x, key_y, key_size, key_size), border_radius=4)
                pygame.draw.rect(screen, THEME_COLORS["border_selected"], (key_x, key_y, key_size, key_size), 2, border_radius=4)
            else:
                pygame.draw.rect(screen, THEME_COLORS["button_idle"], (key_x, key_y, key_size, key_size), border_radius=4)
                pygame.draw.rect(screen, THEME_COLORS["border"], (key_x, key_y, key_size, key_size), 1, border_radius=4)
            
            # Lettre
            key_text = config.small_font.render(key, True, THEME_COLORS["text_selected"] if is_selected else THEME_COLORS["text"])
            key_rect = key_text.get_rect(center=(key_x + key_size // 2, key_y + key_size // 2))
            screen.blit(key_text, key_rect)
