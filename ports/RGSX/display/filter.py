
"""filter module."""

import pygame  # type: ignore

import config

from game_filters import GameFilters

from language import _

from .colors import THEME_COLORS

from . import core
def draw_filter_menu_choice(screen):
    """Affiche le menu filtre unifie."""
    screen.blit(core.OVERLAY, (0, 0))
    
    # Titre
    title = _("filter_menu_title")
    title_surface = config.title_font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, 60))
    screen.blit(title_surface, title_rect)
    
    # Options
    entries = getattr(config, 'filter_menu_entries', []) or []
    options = [entry.get('label', '') for entry in entries]
    
    # Calculer hauteur dynamique basée sur la taille de police
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(60, font_height + 30)
    
    # Calculer largeur maximale nécessaire pour le texte
    max_text_width = 0
    for option in options:
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        if text_surface.get_width() > max_text_width:
            max_text_width = text_surface.get_width()
    
    # Largeur du bouton basée sur le texte le plus long + marges
    button_width = max(400, max_text_width + 80)
    
    # Calculer positions
    menu_y = 150
    button_spacing = 20
    
    for i, option in enumerate(options):
        y = menu_y + i * (button_height + button_spacing)
        x = (config.screen_width - button_width) // 2
        
        # Couleur selon sélection
        if i == config.selected_filter_choice:
            color = THEME_COLORS["button_selected"]
            border_color = THEME_COLORS["border_selected"]
        else:
            color = THEME_COLORS["button_idle"]
            border_color = THEME_COLORS["border"]
        
        # Dessiner bouton
        pygame.draw.rect(screen, color, (x, y, button_width, button_height), border_radius=12)
        pygame.draw.rect(screen, border_color, (x, y, button_width, button_height), 3, border_radius=12)
        
        # Texte avec gestion du dépassement
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        available_width = button_width - 40  # Marge de 20px de chaque côté
        
        if text_surface.get_width() > available_width:
            # Tronquer le texte avec "..."
            truncated_text = option
            while text_surface.get_width() > available_width and len(truncated_text) > 0:
                truncated_text = truncated_text[:-1]
                text_surface = config.font.render(truncated_text + "...", True, THEME_COLORS["text"])
        
        text_rect = text_surface.get_rect(center=(config.screen_width // 2, y + button_height // 2))
        screen.blit(text_surface, text_rect)

def draw_global_sort_menu(screen):
    screen.blit(core.OVERLAY, (0, 0))

    title = _("web_sort") if _ else "Trier"
    title_surface = config.title_font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, 60))
    screen.blit(title_surface, title_rect)

    options = [
        _("web_sort_name_asc") if _ else "A-Z (Nom)",
        _("web_sort_name_desc") if _ else "Z-A (Nom)",
        _("web_sort_size_asc") if _ else "Taille -+ (Petit d'abord)",
        _("web_sort_size_desc") if _ else "Taille +- (Grand d'abord)",
        _("menu_back") if _ else "Retour",
    ]

    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(60, font_height + 30)
    max_text_width = 0
    for option in options:
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        max_text_width = max(max_text_width, text_surface.get_width())
    button_width = max(460, max_text_width + 80)
    menu_y = 150
    button_spacing = 20

    for i, option in enumerate(options):
        y = menu_y + i * (button_height + button_spacing)
        x = (config.screen_width - button_width) // 2
        if i == getattr(config, 'global_sort_selected', 0):
            color = THEME_COLORS["button_selected"]
            border_color = THEME_COLORS["border_selected"]
        else:
            color = THEME_COLORS["button_idle"]
            border_color = THEME_COLORS["border"]
        pygame.draw.rect(screen, color, (x, y, button_width, button_height), border_radius=12)
        pygame.draw.rect(screen, border_color, (x, y, button_width, button_height), 3, border_radius=12)
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=(config.screen_width // 2, y + button_height // 2))
        screen.blit(text_surface, text_rect)

def draw_filter_advanced(screen):
    """Affiche l'écran de filtrage avancé"""
    
    screen.blit(core.OVERLAY, (0, 0))
    
    # Initialiser le filtre si nécessaire
    if not hasattr(config, 'game_filter_obj'):
        config.game_filter_obj = GameFilters()
        # Charger depuis settings
        from rgsx_settings import load_game_filters
        filter_dict = load_game_filters()
        if filter_dict:
            config.game_filter_obj.load_from_dict(filter_dict)
    
    # Liste des options (sans les régions pour l'instant)
    options = []
    
    # Section Régions (titre seulement)
    region_title = _("filter_region_title")
    options.append(('header', region_title))
    
    # On va afficher les régions en grille 3x3, donc on ajoute des placeholders
    regions_list = []
    for region in GameFilters.REGIONS:
        region_key = f"filter_region_{region.lower()}"
        region_label = _(region_key)
        filter_state = config.game_filter_obj.region_filters.get(region, 'include')  # Par défaut: include
        
        if filter_state == 'exclude':
            status = f"[X] {_('filter_region_exclude')}"
            color = THEME_COLORS["red"]
        else:  # 'include'
            status = f"[V] {_('filter_region_include')}"
            color = THEME_COLORS["green"]
        
        regions_list.append(('region', region, f"{region_label}: {status}", color))
    
    # Ajouter les régions comme une seule entrée "grid" dans options
    options.append(('region_grid', regions_list))
    
    # Section Autres options
    options.append(('separator', ''))
    options.append(('header', _("filter_other_options")))
    
    hide_text = _("filter_hide_non_release")
    hide_status = "[X]" if config.game_filter_obj.hide_non_release else "[ ]"
    options.append(('toggle', 'hide_non_release', f"{hide_text}: {hide_status}"))
    
    one_rom_text = _("filter_one_rom_per_game")
    one_rom_status = "[X]" if config.game_filter_obj.one_rom_per_game else "[ ]"
    # Afficher les 3 premières régions de priorité
    priority_preview = " → ".join(config.game_filter_obj.region_priority[:3]) + "..."
    options.append(('toggle', 'one_rom_per_game', f"{one_rom_text}: {one_rom_status}"))

    hide_downloaded_text = _("filter_hide_downloaded")
    hide_downloaded_status = "[X]" if config.game_filter_obj.hide_downloaded else "[ ]"
    options.append(('toggle', 'hide_downloaded', f"{hide_downloaded_text}: {hide_downloaded_status}"))

    options.append(('button_inline', 'priority_config', f"{_('filter_priority_order')}: {priority_preview}"))
    
    # Boutons d'action (seront affichés séparément en bas)
    buttons = [
        ('apply', _("filter_apply_filters")),
        ('reset', _("filter_reset_filters")),
        ('back', _("filter_back"))
    ]
    
    # Afficher les options (sans les boutons)
    if not hasattr(config, 'selected_filter_option'):
        config.selected_filter_option = 0
    
    # Calculer le nombre total d'items sélectionnables (régions individuelles + autres options + boutons)
    total_items = len(regions_list) + len([opt for opt in options if opt[0] in ['toggle', 'button_inline']]) + len(buttons)
    if config.selected_filter_option >= total_items:
        config.selected_filter_option = total_items - 1
    
    # Calculer d'abord la hauteur totale nécessaire
    # Adapter la hauteur en fonction de la taille de police
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    line_height = max(50, font_height + 30)
    item_height = max(45, font_height + 20)
    item_spacing_y = 10
    items_per_row = 3
    
    # Titre
    title_height = 60
    
    # Hauteur du header régions
    header_height = line_height
    
    # Hauteur de la grille de régions
    num_rows = (len(regions_list) + items_per_row - 1) // items_per_row
    grid_height = num_rows * (item_height + item_spacing_y)
    
    # Hauteur du séparateur
    separator_height = 10
    
    # Hauteur du header autres options
    header2_height = line_height
    
    # Hauteur des autres options (3 options)
    num_other_options = len([opt for opt in options if opt[0] in ['toggle', 'button_inline']])
    other_options_height = num_other_options * (item_height + 10)
    
    # Hauteur des boutons
    # Adapter en fonction de la taille de police
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(50, font_height + 20)
    buttons_top_margin = 30
    
    # Hauteur totale du contenu
    total_content_height = (title_height + header_height + grid_height + separator_height + 
                           header2_height + other_options_height + buttons_top_margin + button_height)
    
    # Calculer position de départ pour centrer verticalement
    control_bar_estimated_height = 80
    available_height = config.screen_height - control_bar_estimated_height
    start_y = (available_height - total_content_height) // 2
    if start_y < 20:
        start_y = 20  # Marge minimale du haut
    
    current_y = start_y
    
    # Titre
    title = _("filter_advanced_title")
    title_surface = config.title_font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, current_y + 20))
    screen.blit(title_surface, title_rect)
    current_y += title_height
    
    region_index_start = 0  # Les régions commencent à l'index 0
    
    for option in options:
        option_type = option[0]
        
        if option_type == 'header':
            # En-tête de section
            text_surface = config.font.render(option[1], True, THEME_COLORS["title_text"])
            text_rect = text_surface.get_rect(center=(config.screen_width // 2, current_y + 20))
            screen.blit(text_surface, text_rect)
            current_y += line_height
        
        elif option_type == 'separator':
            current_y += separator_height
        
        elif option_type == 'region_grid':
            # Afficher les régions en grille 3 par ligne
            regions_data = option[1]
            item_spacing_x = 20
            
            # Calculer la largeur maximale nécessaire pour les boutons de régions
            max_region_width = 0
            for region_data in regions_data:
                text = region_data[2]
                text_surface = config.font.render(text, True, THEME_COLORS["text"])
                text_width = text_surface.get_width() + 30  # Padding de 30px
                if text_width > max_region_width:
                    max_region_width = text_width
            
            # Largeur minimale de 200px
            item_width = max(max_region_width, 200)
            
            # Calculer la largeur totale de la grille
            total_grid_width = items_per_row * item_width + (items_per_row - 1) * item_spacing_x
            grid_start_x = (config.screen_width - total_grid_width) // 2
            
            for idx, region_data in enumerate(regions_data):
                row = idx // items_per_row
                col = idx % items_per_row
                
                x = grid_start_x + col * (item_width + item_spacing_x)
                y = current_y + row * (item_height + item_spacing_y)
                
                # Index global de cette région
                global_idx = region_index_start + idx
                
                # Couleur selon sélection
                if global_idx == config.selected_filter_option:
                    bg_color = THEME_COLORS["button_selected"]
                    border_color = THEME_COLORS["border_selected"]
                else:
                    bg_color = THEME_COLORS["button_idle"]
                    border_color = THEME_COLORS["border"]
                
                # Dessiner fond
                pygame.draw.rect(screen, bg_color, (x, y, item_width, item_height), border_radius=8)
                pygame.draw.rect(screen, border_color, (x, y, item_width, item_height), 2, border_radius=8)
                
                # Texte centré
                text = region_data[2]
                text_color = region_data[3]
                
                text_surface = config.font.render(text, True, text_color)
                text_rect = text_surface.get_rect(center=(x + item_width // 2, y + item_height // 2))
                screen.blit(text_surface, text_rect)
            
            # Calculer la hauteur occupée par la grille
            current_y += num_rows * (item_height + item_spacing_y) + 10
        
        elif option_type in ['toggle', 'button_inline']:
            # Option sélectionnable - largeur adaptée au texte
            text = option[2]
            text_surface = config.font.render(text, True, THEME_COLORS["text"])
            text_width = text_surface.get_width()
            
            # Largeur avec padding
            width = text_width + 40
            x = (config.screen_width - width) // 2  # Centrer
            height = item_height
            
            # Index global de cette option (après les régions)
            global_idx = len(regions_list) + len([opt for opt in options[:options.index(option)] if opt[0] in ['toggle', 'button_inline']])
            
            # Couleur selon sélection
            if global_idx == config.selected_filter_option:
                bg_color = THEME_COLORS["button_selected"]
                border_color = THEME_COLORS["border_selected"]
            else:
                bg_color = THEME_COLORS["button_idle"]
                border_color = THEME_COLORS["border"]
            
            # Dessiner fond
            pygame.draw.rect(screen, bg_color, (x, current_y, width, height), border_radius=8)
            pygame.draw.rect(screen, border_color, (x, current_y, width, height), 2, border_radius=8)
            
            # Texte centré
            text_color = THEME_COLORS["text"]
            text_rect = text_surface.get_rect(center=(x + width // 2, current_y + height // 2))
            screen.blit(text_surface, text_rect)
            
            current_y += height + 10
    
    # Afficher les 3 boutons côte à côte en bas
    current_y += buttons_top_margin
    button_y = current_y
    button_spacing = 20
    
    # Calculer la largeur de chaque bouton en fonction du texte
    button_widths = []
    for button_id, button_text in buttons:
        text_surface = config.font.render(button_text, True, THEME_COLORS["text"])
        button_widths.append(text_surface.get_width() + 40)  # Padding de 40px
    
    # Largeur totale des boutons
    total_buttons_width = sum(button_widths) + button_spacing * (len(buttons) - 1)
    button_start_x = (config.screen_width - total_buttons_width) // 2
    
    # Calculer l'index de début des boutons (après toutes les régions et autres options)
    button_index_start = len(regions_list) + num_other_options
    
    current_button_x = button_start_x
    for i, (button_id, button_text) in enumerate(buttons):
        button_index = button_index_start + i
        button_width = button_widths[i]
        
        # Couleur selon sélection
        if button_index == config.selected_filter_option:
            bg_color = THEME_COLORS["button_selected"]
            border_color = THEME_COLORS["border_selected"]
        else:
            bg_color = THEME_COLORS["button_idle"]
            border_color = THEME_COLORS["border"]
        
        # Dessiner bouton
        pygame.draw.rect(screen, bg_color, (current_button_x, button_y, button_width, button_height), border_radius=8)
        pygame.draw.rect(screen, border_color, (current_button_x, button_y, button_width, button_height), 2, border_radius=8)
        
        # Texte centré
        text_surface = config.font.render(button_text, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=(current_button_x + button_width // 2, button_y + button_height // 2))
        screen.blit(text_surface, text_rect)
        
        current_button_x += button_width + button_spacing
    
    # Info filtre actif (au-dessus des boutons)
    if config.game_filter_obj.is_active():
        info_text = _("filter_active")
        info_surface = config.small_font.render(info_text, True, THEME_COLORS["green"])
        info_rect = info_surface.get_rect(center=(config.screen_width // 2, button_y - 20))
        screen.blit(info_surface, info_rect)

def draw_filter_priority_config(screen):
    """Affiche l'écran de configuration de la priorité des régions pour One ROM per game"""
    
    screen.blit(core.OVERLAY, (0, 0))
    
    # Titre
    title = _("filter_priority_title")
    title_surface = config.title_font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, 40))
    screen.blit(title_surface, title_rect)
    
    # Description
    desc = _("filter_priority_desc")
    desc_surface = config.small_font.render(desc, True, THEME_COLORS["title_text"])
    desc_rect = desc_surface.get_rect(center=(config.screen_width // 2, 85))
    screen.blit(desc_surface, desc_rect)
    
    # Initialiser le filtre si nécessaire
    if not hasattr(config, 'game_filter_obj'):
        from rgsx_settings import load_game_filters
        config.game_filter_obj = GameFilters()
        filter_dict = load_game_filters()
        if filter_dict:
            config.game_filter_obj.load_from_dict(filter_dict)
    
    # Liste des régions avec leur priorité
    start_y = 130
    line_height = 60
    
    if not hasattr(config, 'selected_priority_index'):
        config.selected_priority_index = 0
    
    priority_list = config.game_filter_obj.region_priority.copy()
    
    # Afficher chaque région avec sa position
    for i, region in enumerate(priority_list):
        y = start_y + i * line_height
        x = 120
        width = config.screen_width - 240
        height = 50
        
        # Couleur selon sélection
        if i == config.selected_priority_index:
            bg_color = THEME_COLORS["button_selected"]
            border_color = THEME_COLORS["border_selected"]
        else:
            bg_color = THEME_COLORS["button_idle"]
            border_color = THEME_COLORS["border"]
        
        # Dessiner fond
        pygame.draw.rect(screen, bg_color, (x, y, width, height), border_radius=8)
        pygame.draw.rect(screen, border_color, (x, y, width, height), 2, border_radius=8)
        
        # Numéro de priorité
        priority_text = f"#{i+1}"
        priority_surface = config.font.render(priority_text, True, THEME_COLORS["text"])
        screen.blit(priority_surface, (x + 15, y + (height - priority_surface.get_height()) // 2))
        
        # Nom de la région (traduit si possible)
        region_key = f"filter_region_{region.lower()}"
        region_label = _(region_key)
        region_surface = config.font.render(region_label, True, THEME_COLORS["text"])
        screen.blit(region_surface, (x + 80, y + (height - region_surface.get_height()) // 2))
        
        # Flèches pour réorganiser (si sélectionné)
        if i == config.selected_priority_index:
            arrows_text = "← →"
            arrows_surface = config.font.render(arrows_text, True, THEME_COLORS["green"])
            screen.blit(arrows_surface, (x + width - 50, y + (height - arrows_surface.get_height()) // 2))
    
    # Boutons en bas
    control_bar_estimated_height = 80
    button_width = 300
    button_height = 50
    button_x = (config.screen_width - button_width) // 2
    button_y = config.screen_height - control_bar_estimated_height - button_height - 20
    
    # Bouton Back
    is_button_selected = config.selected_priority_index >= len(priority_list)
    bg_color = THEME_COLORS["button_selected"] if is_button_selected else THEME_COLORS["button_idle"]
    border_color = THEME_COLORS["border_selected"] if is_button_selected else THEME_COLORS["border"]
    
    pygame.draw.rect(screen, bg_color, (button_x, button_y, button_width, button_height), border_radius=8)
    pygame.draw.rect(screen, border_color, (button_x, button_y, button_width, button_height), 2, border_radius=8)
    
    back_text = _("filter_back")
    text_surface = config.font.render(back_text, True, THEME_COLORS["text"])
    text_rect = text_surface.get_rect(center=(button_x + button_width // 2, button_y + button_height // 2))
    screen.blit(text_surface, text_rect)
