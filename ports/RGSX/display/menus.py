
"""menus module."""

from datetime import datetime
import pygame  # type: ignore
import re

import config

from language import _, get_available_languages, get_language_name

from rgsx_settings import (load_rgsx_settings, get_light_mode, get_show_unsupported_platforms, get_allow_unknown_extensions, get_display_monitor, get_display_fullscreen, get_available_monitors, get_font_family, get_symlink_option)

from utils import (wrap_text, truncate_text_end, check_web_service_status, check_custom_dns_status, load_api_keys, get_connection_status_targets, get_connection_status_snapshot)

from .colors import THEME_COLORS, get_background_theme_label
from .icons import get_help_icon_surface, render_icons_line
from .components import draw_stylized_button

from . import core
import logging
logger = logging.getLogger(__name__)
def draw_language_menu(screen):
    """Dessine le menu de sélection de langue avec un style moderne.

    Améliorations:
    - Hauteur des boutons réduite et responsive selon la taille d'écran.
    - Bloc (titre + liste de langues) centré verticalement.
    - Gestion d'overflow: réduit légèrement la hauteur/espacement si nécessaire.
    """
    
    screen.blit(core.OVERLAY, (0, 0))
    
    # Obtenir les langues disponibles
    available_languages = get_available_languages()
    
    if not available_languages:
        logger.error("Aucune langue disponible")
        return
    
    # Instruction en haut - calculer d'abord pour connaître l'espace disponible
    instruction_text = _("language_select_instruction")
    instruction_height = get_top_instruction_height(instruction_text)
    footer_height = 70
    
    # Espace disponible pour le contenu (entre instruction et footer)
    available_h = config.screen_height - instruction_height - footer_height - 20
    
    # Titre (mesuré d'abord pour connaître la hauteur réelle du fond)
    title_text = _("language_select_title")
    title_surface = config.font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect()
    # Padding responsive plus léger
    hpad = max(20, min(30, int(config.screen_width * 0.03)))
    vpad = max(6, min(10, int(title_surface.get_height() * 0.3)))
    title_bg_rect = title_rect.inflate(hpad, vpad)

    # Calculer hauteur dynamique basée sur la taille de police
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    
    # Calculer largeur maximale nécessaire pour les noms de langues
    max_text_width = 0
    for lang_code in available_languages:
        lang_name = get_language_name(lang_code)
        text_surface = config.font.render(lang_name, True, THEME_COLORS["text"])
        if text_surface.get_width() > max_text_width:
            max_text_width = text_surface.get_width()
    
    # Largeur bornée entre valeur calculée et limites raisonnables
    button_width = max(200, min(400, max_text_width + 40))
    
    # Nombre de langues
    n = len(available_languages)
    
    # Calculer la hauteur de bouton idéale en fonction de l'espace disponible
    # Espace pour les boutons = available_h - titre - espacement titre
    title_total_height = title_bg_rect.height + 8  # titre + petit espace
    space_for_buttons = available_h - title_total_height
    
    # Calculer hauteur et espacement optimaux
    # On veut : n * button_height + (n-1) * spacing <= space_for_buttons
    # Avec spacing = 0.2 * button_height environ
    # Donc : n * h + (n-1) * 0.2 * h = h * (n + 0.2*(n-1)) <= space_for_buttons
    # h <= space_for_buttons / (n + 0.2*(n-1))
    
    max_button_height = space_for_buttons / (n + 0.15 * max(0, n - 1))
    
    # Borner la hauteur des boutons
    button_height = int(min(50, max(24, min(max_button_height, font_height + 12))))
    button_spacing = max(4, min(8, int(button_height * 0.15)))
    
    # Recalculer la hauteur totale
    total_buttons_height = n * button_height + (n - 1) * button_spacing
    content_height = title_bg_rect.height + 8 + total_buttons_height
    
    # Réduction supplémentaire si nécessaire
    safety_counter = 0
    while content_height > available_h and safety_counter < 30:
        if button_height > 24:
            button_height -= 1
        elif button_spacing > 2:
            button_spacing -= 1
        else:
            break
        total_buttons_height = n * button_height + (n - 1) * button_spacing
        content_height = title_bg_rect.height + 8 + total_buttons_height
        safety_counter += 1
    
    # Positionner le bloc au centre verticalement
    content_top = instruction_height + max(5, (available_h - content_height) // 2)
    
    # Positionner le titre
    title_bg_rect.centerx = config.screen_width // 2
    title_bg_rect.y = content_top
    title_rect.center = (title_bg_rect.centerx, title_bg_rect.y + title_bg_rect.height // 2)

    # Dessiner le titre
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_bg_rect, border_radius=8)
    pygame.draw.rect(screen, THEME_COLORS["border"], title_bg_rect, 2, border_radius=8)
    screen.blit(title_surface, title_rect)

    # Démarrer la liste juste sous le titre
    start_y = title_bg_rect.bottom + 8
    
    for i, lang_code in enumerate(available_languages):
        # Obtenir le nom de la langue
        lang_name = get_language_name(lang_code)

        # Position du bouton
        button_x = (config.screen_width - button_width) // 2
        button_y = start_y + i * (button_height + button_spacing)

        # Dessiner le bouton
        button_color = THEME_COLORS["button_hover"] if i == config.selected_language_index else THEME_COLORS["button_idle"]
        pygame.draw.rect(screen, button_color, (button_x, button_y, button_width, button_height), border_radius=8)
        pygame.draw.rect(screen, THEME_COLORS["border"], (button_x, button_y, button_width, button_height), 2, border_radius=8)

        # Texte avec gestion du dépassement
        text_surface = config.font.render(lang_name, True, THEME_COLORS["text"])
        available_width = button_width - 16  # Marge de 8px de chaque côté
        
        if text_surface.get_width() > available_width:
            # Tronquer le texte avec "..."
            truncated_text = lang_name
            while text_surface.get_width() > available_width and len(truncated_text) > 0:
                truncated_text = truncated_text[:-1]
                text_surface = config.font.render(truncated_text + "...", True, THEME_COLORS["text"])
        
        text_rect = text_surface.get_rect(center=(button_x + button_width // 2, button_y + button_height // 2))
        screen.blit(text_surface, text_rect)
    
    # Dessiner l'instruction en haut
    draw_menu_instruction(screen, instruction_text)

def get_top_instruction_height(instruction_text):
    """Calcule la hauteur totale occupée par l'instruction en haut (cadre + marge).
    
    Retourne 0 si pas d'instruction.
    """
    if not instruction_text:
        return 0
    try:
        margin_top = 3
        margin_bottom = 6  # Espace entre l'instruction et le menu
        padding_y = 4
        text_surface = config.small_font.render(instruction_text, True, THEME_COLORS["text"])
        frame_height = text_surface.get_height() + (padding_y * 2)
        return margin_top + frame_height + margin_bottom
    except Exception:
        return 0

def draw_top_instruction(screen, instruction_text):
    """Dessine une instruction en haut de l'écran dans un cadre élégant sur une ligne.
    
    - Largeur maximale de l'écran avec marges
    - Centré horizontalement
    - Fond semi-transparent avec bordure
    
    Retourne la hauteur totale occupée (pour le positionnement des menus).
    """
    if not instruction_text:
        return 0
    try:
        # Marges réduites pour coller au haut
        margin_x = 20
        margin_top = 3
        margin_bottom = 6  # Espace entre l'instruction et le menu
        padding_x = 15
        padding_y = 4
        
        # Rendre le texte
        text_surface = config.small_font.render(instruction_text, True, THEME_COLORS["text"])
        
        # Calculer les dimensions du cadre
        max_width = config.screen_width - (margin_x * 2)
        frame_width = min(text_surface.get_width() + (padding_x * 2), max_width)
        frame_height = text_surface.get_height() + (padding_y * 2)
        
        # Position du cadre (centré en haut)
        frame_x = (config.screen_width - frame_width) // 2
        frame_y = margin_top
        
        # Créer surface avec transparence pour le fond
        frame_surface = pygame.Surface((frame_width, frame_height), pygame.SRCALPHA)
        
        # Dessiner le fond semi-transparent avec coins arrondis
        pygame.draw.rect(frame_surface, THEME_COLORS["button_idle"], 
                        (0, 0, frame_width, frame_height), border_radius=10)
        
        # Dessiner la bordure
        pygame.draw.rect(frame_surface, THEME_COLORS["border"], 
                        (0, 0, frame_width, frame_height), 2, border_radius=10)
        
        # Blitter le cadre sur l'écran
        screen.blit(frame_surface, (frame_x, frame_y))
        
        # Calculer la position du texte (centré dans le cadre)
        text_x = frame_x + (frame_width - text_surface.get_width()) // 2
        text_y = frame_y + padding_y
        
        # Dessiner le texte
        screen.blit(text_surface, (text_x, text_y))
        
        return margin_top + frame_height + margin_bottom
        
    except Exception as e:
        logger.error(f"Erreur draw_top_instruction: {e}")
        return 0

def draw_menu_instruction(screen, instruction_text, last_button_bottom=None):
    """Dessine une ligne d'instruction centrée en haut de l'écran dans un cadre.

    Utilise draw_top_instruction pour un affichage cohérent.
    Le paramètre last_button_bottom est conservé pour compatibilité mais n'est plus utilisé.
    Retourne la hauteur totale occupée.
    """
    return draw_top_instruction(screen, instruction_text)

def draw_display_menu(screen):
    """Affiche le sous-menu Affichage (layout, taille de police, systèmes non supportés, moniteur)."""
    screen.blit(core.OVERLAY, (0, 0))

    # États actuels
    layout_str = f"{getattr(config, 'GRID_COLS', 3)}x{getattr(config, 'GRID_ROWS', 4)}"
    font_scale = config.accessibility_settings.get("font_scale", 1.0)
    show_unsupported = get_show_unsupported_platforms()
    allow_unknown = get_allow_unknown_extensions()
    
    # Monitor info
    current_monitor = get_display_monitor()
    is_fullscreen = get_display_fullscreen()
    monitors = get_available_monitors()
    num_monitors = len(monitors)
    
    # Construire le label du moniteur
    if num_monitors > 1:
        monitor_info = monitors[current_monitor] if current_monitor < num_monitors else monitors[0]
        monitor_label = f"{_('display_monitor')}: {monitor_info['name']} ({monitor_info['resolution']})"
    else:
        monitor_label = f"{_('display_monitor')}: {_('display_monitor_single')}"
    
    # Label mode écran
    fullscreen_label = f"{_('display_mode')}: {_('display_fullscreen') if is_fullscreen else _('display_windowed')}"

    # Compter les systèmes non supportés actuellement masqués
    unsupported_list = getattr(config, "unsupported_platforms", []) or []
    try:
        hidden_count = 0 if show_unsupported else len(list(unsupported_list))
    except Exception:
        hidden_count = 0
    if hidden_count > 0:
        unsupported_label = _("menu_show_unsupported_and_hidden").format(hidden_count)
    else:
        unsupported_label = _("menu_show_unsupported_all_displayed")

    # Libellés - ajout des options moniteur et mode écran
    options = [
        f"{_('display_layout')}: {layout_str}",
        _("accessibility_font_size").format(f"{font_scale:.1f}"),
        monitor_label,
        fullscreen_label,
        unsupported_label,
        _("menu_allow_unknown_ext_on") if allow_unknown else _("menu_allow_unknown_ext_off"),
        _("menu_filter_platforms"),
    ]

    selected = getattr(config, 'display_menu_selection', 0)
    
    # Instruction à afficher en haut
    instruction_text = _("language_select_instruction")
    instruction_height = get_top_instruction_height(instruction_text)

    # Dimensions du cadre (cohérent avec le menu pause)
    title_text = _("menu_display")
    title_surface = config.title_font.render(title_text, True, THEME_COLORS["text"])
    title_height = title_surface.get_height() + 10
    menu_width = int(config.screen_width * 0.7)
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    vertical_spacing = 10
    footer_height = 70
    menu_height = title_height + len(options) * (button_height + vertical_spacing) + 2 * margin_top_bottom
    menu_x = (config.screen_width - menu_width) // 2
    
    # Calculer menu_y en tenant compte de l'instruction et du footer
    available_height = config.screen_height - instruction_height - footer_height
    menu_y = instruction_height + (available_height - menu_height) // 2

    # Cadre
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (menu_x, menu_y, menu_width, menu_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (menu_x, menu_y, menu_width, menu_height), 2, border_radius=12)

    # Titre centré dans le cadre
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, menu_y + margin_top_bottom + title_surface.get_height() // 2))
    screen.blit(title_surface, title_rect)

    # Boutons des options
    for i, option_text in enumerate(options):
        y = menu_y + margin_top_bottom + title_height + i * (button_height + vertical_spacing)
        draw_stylized_button(
            screen,
            option_text,
            menu_x + 20,
            y,
            menu_width - 40,
            button_height,
            selected=(i == selected)
        )

    # Dessiner l'instruction en haut
    draw_menu_instruction(screen, instruction_text)

def draw_pause_menu(screen, selected_option):
    """Dessine le menu pause racine (catégories)."""
    screen.blit(core.OVERLAY, (0, 0))
    # Nouvel ordre: Games / Language / Controls / Display / Settings / Support / Reset / Quit
    reset_label = _("menu_reset_default_settings") if _ else "Reset default settings"
    if not reset_label or reset_label == "menu_reset_default_settings":
        reset_label = "Reset default settings"

    options = [
        _("menu_games") if _ else "Games",                  # 0 -> sous-menu games (history + sources + update)
        _("menu_language") if _ else "Language",            # 1 -> sélecteur de langue direct
        _("menu_controls"),                                 # 2 -> sous-menu controls
        _("menu_display"),                                  # 3 -> sous-menu display
        _("menu_settings_category") if _ else "Settings",   # 4 -> sous-menu settings
        _("menu_support"),                                  # 5 -> support
        reset_label,                                          # 6 -> reset settings (delete + restart)
        _("menu_quit")                                      # 7 -> sous-menu quit (quit + restart)
    ]
    
    # Instruction contextuelle pour l'option sélectionnée
    instruction_keys = [
        "instruction_pause_games",
        "instruction_pause_language",
        "instruction_pause_controls",
        "instruction_pause_display",
        "instruction_pause_settings",
        "instruction_pause_support",
        "instruction_pause_reset_settings",
        "instruction_pause_quit",
    ]
    try:
        key = instruction_keys[selected_option]
        instruction_text = _(key)
        if instruction_text == key:
            instruction_text = ""
    except Exception:
        instruction_text = ""
    
    # Calculer la hauteur de l'instruction AVANT de dessiner le menu
    instruction_height = get_top_instruction_height(instruction_text) if instruction_text else 0
    
    # Calculer hauteur dynamique basée sur la taille de police
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(int(config.screen_height * 0.048), font_height + 20)
    
    # Calculer largeur maximale nécessaire pour le texte
    max_text_width = 0
    for option in options:
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        if text_surface.get_width() > max_text_width:
            max_text_width = text_surface.get_width()
    
    # Largeur du menu basée sur le texte le plus long + marges
    menu_width = min(int(config.screen_width * 0.8), max(int(config.screen_width * 0.5), max_text_width + 80))
    margin_top_bottom = 24
    menu_height = len(options) * (button_height + 12) + 2 * margin_top_bottom
    menu_x = (config.screen_width - menu_width) // 2
    
    # Calculer menu_y en tenant compte de l'instruction en haut
    # Zone disponible = écran - instruction_height - footer (70px)
    footer_height = 70
    available_height = config.screen_height - instruction_height - footer_height
    menu_y = instruction_height + (available_height - menu_height) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (menu_x, menu_y, menu_width, menu_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (menu_x, menu_y, menu_width, menu_height), 2, border_radius=12)
    for i, option in enumerate(options):
        draw_stylized_button(
            screen,
            option,
            menu_x + 20,
            menu_y + margin_top_bottom + i * (button_height + 12),
            menu_width - 40,
            button_height,
            selected=i == selected_option
        )
    config.pause_menu_total_options = len(options)

    # Dessiner l'instruction en haut
    if instruction_text:
        draw_menu_instruction(screen, instruction_text)

def _calc_submenu_dimensions(num_options, instruction_height=0):
    """Calcule les dimensions adaptatives pour un sous-menu.
    
    Args:
        num_options: Nombre d'options dans le menu
        instruction_height: Hauteur de l'instruction en haut (0 si pas d'instruction)
    """
    sample_text = config.font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    title_height = font_height + 10
    margin_top_bottom = 20
    footer_height = 70
    
    max_menu_height = int(config.screen_height * 0.85)
    available_height_for_buttons = max_menu_height - title_height - 2 * margin_top_bottom
    
    ideal_button_height = max(int(config.screen_height * 0.040), font_height + 12)
    ideal_spacing = 6
    total_ideal_height = num_options * ideal_button_height + (num_options - 1) * ideal_spacing
    
    if total_ideal_height <= available_height_for_buttons:
        button_height = ideal_button_height
        button_spacing = ideal_spacing
    else:
        min_spacing = 3
        min_button_height = font_height + 6
        available_for_buttons = available_height_for_buttons - (num_options - 1) * min_spacing
        button_height = max(min_button_height, available_for_buttons // num_options)
        button_spacing = min_spacing
        total_height = num_options * button_height + (num_options - 1) * button_spacing
        if total_height > available_height_for_buttons:
            button_height = min_button_height
            button_spacing = max(1, (available_height_for_buttons - num_options * button_height) // max(1, num_options - 1))
    
    menu_height = title_height + num_options * button_height + (num_options - 1) * button_spacing + 2 * margin_top_bottom
    
    # Calculer menu_y en tenant compte de l'instruction en haut et du footer
    available_height = config.screen_height - instruction_height - footer_height
    menu_y = instruction_height + (available_height - menu_height) // 2
    
    start_y = menu_y + margin_top_bottom + title_height
    last_button_bottom = start_y + (num_options - 1) * (button_height + button_spacing) + button_height
    
    return {
        'button_height': button_height,
        'button_spacing': button_spacing,
        'menu_height': menu_height,
        'menu_y': menu_y,
        'start_y': start_y,
        'last_button_bottom': last_button_bottom,
        'margin_top_bottom': margin_top_bottom
    }

def _draw_submenu_generic(screen, title, options, selected_index, instruction_text=None):
    """Helper générique pour dessiner un sous-menu hiérarchique.
    
    Args:
        screen: Surface pygame
        title: Titre du menu
        options: Liste des options
        selected_index: Index de l'option sélectionnée
        instruction_text: Texte d'instruction optionnel à afficher en haut
    """
    screen.blit(core.OVERLAY, (0, 0))
    
    # Calculer la hauteur de l'instruction si présente
    instruction_height = get_top_instruction_height(instruction_text) if instruction_text else 0
    
    # Calculer les dimensions adaptatives en tenant compte de l'instruction
    dims = _calc_submenu_dimensions(len(options), instruction_height)
    button_height = dims['button_height']
    button_spacing = dims['button_spacing']
    menu_height = dims['menu_height']
    menu_y = dims['menu_y']
    margin_top_bottom = dims['margin_top_bottom']
    
    # Calculer largeur maximale nécessaire pour le texte (titre + options)
    max_text_width = 0
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    max_text_width = title_surface.get_width()
    for option in options:
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        if text_surface.get_width() > max_text_width:
            max_text_width = text_surface.get_width()
    
    # Largeur du menu basée sur le texte le plus long + marges
    menu_width = min(int(config.screen_width * 0.85), max(int(config.screen_width * 0.55), max_text_width + 80))
    menu_x = (config.screen_width - menu_width) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (menu_x, menu_y, menu_width, menu_height), border_radius=14)
    pygame.draw.rect(screen, THEME_COLORS["border"], (menu_x, menu_y, menu_width, menu_height), 2, border_radius=14)
    # Title
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width//2, menu_y + margin_top_bottom//2 + title_surface.get_height()//2))
    screen.blit(title_surface, title_rect)
    # Options
    start_y = title_rect.bottom + 10
    for i, opt in enumerate(options):
        draw_stylized_button(
            screen,
            opt,
            menu_x + 20,
            start_y + i * (button_height + button_spacing),
            menu_width - 40,
            button_height,
            selected=(i == selected_index)
        )
    
    # Dessiner l'instruction en haut si présente
    if instruction_text:
        draw_menu_instruction(screen, instruction_text)

def draw_pause_controls_menu(screen, selected_index):
    # Synchronisé avec controls.py : help, remap, back
    options = [
        _( "controls_help_title"),
        _( "menu_remap_controls"),
        _( "menu_back") if _ else "Back"
    ]
    instruction_keys = [
        "instruction_controls_help",
        "instruction_controls_remap",
        "instruction_generic_back",
    ]
    key = instruction_keys[selected_index] if 0 <= selected_index < len(instruction_keys) else None
    instruction_text = _(key) if key else None
    _draw_submenu_generic(screen, _( "menu_controls") if _ else "Controls", options, selected_index, instruction_text)

def draw_pause_display_menu(screen, selected_index):
    # Layout label - now opens a submenu
    layout_txt = f"{_('submenu_display_layout') if _ else 'Layout'} >"
    # Font size
    opts = getattr(config, 'font_scale_options', [0.75, 1.0, 1.25, 1.5, 1.75])
    cur_idx = getattr(config, 'current_font_scale_index', 1)
    font_value = f"{opts[cur_idx]}x"
    font_txt = f"{_('submenu_display_font_size') if _ else 'Font Size'}: < {font_value} >"
    # Footer font size
    footer_opts = getattr(config, 'footer_font_scale_options', [0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0])
    footer_cur_idx = getattr(config, 'current_footer_font_scale_index', 3)
    footer_font_value = f"{footer_opts[footer_cur_idx]}x"
    footer_font_txt = f"{_('accessibility_footer_font_size').split(':')[0] if _ else 'Footer Font Size'}: < {footer_font_value} >"
    # Font family
    current_family = get_font_family()
    # Nom user-friendly
    family_map = {
        "pixel": "Pixel",
        "bell_centennial": "Bell Centennial",
        "dejavu": "DejaVu Sans"
    }
    fam_label = family_map.get(current_family, current_family)
    font_family_txt = f"{_('submenu_display_font_family') if _ else 'Font'}: < {fam_label} >"

    # Monitor selection - only show if multiple monitors
    current_monitor = get_display_monitor()
    monitors = get_available_monitors()
    num_monitors = len(monitors)
    show_monitor_option = num_monitors > 1
    
    if show_monitor_option:
        monitor_info = monitors[current_monitor] if current_monitor < num_monitors else monitors[0]
        monitor_value = f"{monitor_info['name']} ({monitor_info['resolution']})"
        monitor_txt = f"{_('display_monitor') if _ else 'Monitor'}: < {monitor_value} >"

    # Display mode - Windows only
    show_display_mode_option = getattr(config, 'OPERATING_SYSTEM', '') == "Windows"
    if show_display_mode_option:
        is_fullscreen = get_display_fullscreen()
        display_mode_value = _("display_fullscreen") if is_fullscreen else _("display_windowed")
        display_mode_txt = f"{_('display_mode') if _ else 'Screen mode'}: < {display_mode_value} >"
    
    # Allow unknown extensions
    allow_unknown = get_allow_unknown_extensions()
    status_unknown = _('status_on') if allow_unknown else _('status_off')
    raw_unknown_label = _('submenu_display_allow_unknown_ext') if _ else 'Hide unknown ext warn: {status}'
    if '{status}' in raw_unknown_label:
        raw_unknown_label = raw_unknown_label.split('{status}')[0].rstrip(' :')
    unknown_txt = f"{raw_unknown_label}: < {status_unknown} >"

    # Light mode (performance)
    light_mode = get_light_mode()
    light_status = _('status_on') if light_mode else _('status_off')
    light_txt = f"{_('display_light_mode') if _ else 'Light mode'}: < {light_status} >"

    # Background gradient theme
    background_theme_label = get_background_theme_label()
    background_txt = f"{_('display_background') if _ else 'Background'}: < {background_theme_label} >"

    back_txt = _("menu_back") if _ else "Back"
    
    # Build options list - conditional monitor and display mode options
    font_submenu_txt = f"{_('submenu_display_font_size') if _ else 'Font Size'} >"
    options = [layout_txt, font_submenu_txt, font_family_txt]
    instructions = [
        _("instruction_display_layout"),
        _("instruction_display_font_size"),
        _("instruction_display_font_family"),
    ]
    
    if show_monitor_option:
        options.append(monitor_txt)
        instructions.append(_("instruction_display_monitor"))

    if show_display_mode_option:
        options.append(display_mode_txt)
        instructions.append(_("instruction_display_mode"))
    
    bg_instruction = _("instruction_display_background_theme") if _ else ""
    if not bg_instruction or bg_instruction == "instruction_display_background_theme":
        bg_instruction = "Left/Right: change background theme"

    options.extend([background_txt, light_txt, unknown_txt, back_txt])
    instructions.extend([
        bg_instruction,
        _("instruction_display_light_mode"),
        _("instruction_display_unknown_ext"),
        _("instruction_generic_back"),
    ])

    instruction_text = instructions[selected_index] if 0 <= selected_index < len(instructions) else None
    
    _draw_submenu_generic(screen, _("menu_display"), options, selected_index, instruction_text)

def draw_pause_display_layout_menu(screen, selected_index):
    """Sous-menu pour la disposition avec visualisation schématique des grilles."""
    layouts = [(3,3),(3,4),(4,3),(4,4)]
    layout_labels = ["3x3", "3x4", "4x3", "4x4"]
    
    # Trouver le layout actuel
    try:
        current_idx = layouts.index((config.GRID_COLS, config.GRID_ROWS))
    except ValueError:
        current_idx = 0
    
    # Créer les options avec indicateur du layout actuel
    options = []
    for i, label in enumerate(layout_labels):
        if i == current_idx:
            options.append(f"{label} [CURRENT]" if not _ else f"{label} [{_('status_current') if _ else 'ACTUEL'}]")
        else:
            options.append(label)
    options.append(_("menu_back") if _ else "Back")
    
    # Déterminer l'instruction
    if selected_index < len(layouts):
        instruction = _("instruction_display_layout") if _ else "Left/Right: Navigate • Confirm: Select"
    else:
        instruction = _("instruction_generic_back") if _ else "Confirm: Go back"
    
    # Calculer la hauteur de l'instruction
    instruction_height = get_top_instruction_height(instruction)
    
    # Dessiner le menu de base
    title = _("submenu_display_layout") if _ else "Layout"
    
    # Calculer les dimensions
    button_height = int(config.screen_height * 0.045)
    menu_width = int(config.screen_width * 0.72)
    margin_top_bottom = 26
    footer_height = 70
    
    # Calculer la hauteur nécessaire pour les boutons
    menu_height = (len(options)+1) * (button_height + 10) + 2 * margin_top_bottom
    menu_x = (config.screen_width - menu_width) // 2
    
    # Calculer menu_y en tenant compte de l'instruction et du footer
    available_height = config.screen_height - instruction_height - footer_height
    menu_y = instruction_height + (available_height - menu_height) // 2
    
    # Fond du menu
    menu_rect = pygame.Rect(menu_x, menu_y, menu_width, menu_height)
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], menu_rect, border_radius=14)
    pygame.draw.rect(screen, THEME_COLORS["border"], menu_rect, 3, border_radius=14)
    
    # Titre
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, menu_y + margin_top_bottom//2 + title_surface.get_height()//2))
    screen.blit(title_surface, title_rect)
    
    # Position de départ pour le contenu
    content_start_y = title_rect.bottom + 20
    
    # Division en deux colonnes : gauche pour la grille, droite pour les options
    left_column_x = menu_x + 20
    left_column_width = int(menu_width * 0.4)
    right_column_x = left_column_x + left_column_width + 20
    right_column_width = menu_width - left_column_width - 60
    
    # COLONNE GAUCHE : Dessiner uniquement la grille sélectionnée
    if selected_index < len(layouts):
        cols, rows = layouts[selected_index]
        
        # Calculer la taille des cellules pour le schéma
        cell_size = min(60, (left_column_width - 20) // max(cols, rows))
        grid_width = cols * cell_size
        grid_height = rows * cell_size
        
        # Centrer la grille verticalement dans l'espace disponible
        available_height = (len(options) * (button_height + 10)) - 10
        grid_x = left_column_x + (left_column_width - grid_width) // 2
        grid_y = content_start_y + (available_height - grid_height) // 2
        
        # Dessiner le schéma de la grille sélectionnée
        for row in range(rows):
            for col in range(cols):
                cell_rect = pygame.Rect(
                    grid_x + col * cell_size,
                    grid_y + row * cell_size,
                    cell_size - 3,
                    cell_size - 3
                )
                # Couleur selon si c'est aussi le layout actuel
                if selected_index == current_idx:
                    # Sélectionné ET actuel : vert brillant
                    pygame.draw.rect(screen, THEME_COLORS["fond_lignes"], cell_rect)
                    pygame.draw.rect(screen, THEME_COLORS["text"], cell_rect, 2)
                else:
                    # Seulement sélectionné : bleu clair
                    pygame.draw.rect(screen, THEME_COLORS["button_selected"], cell_rect)
                    pygame.draw.rect(screen, THEME_COLORS["text"], cell_rect, 2)
    
    # COLONNE DROITE : Dessiner les boutons d'options
    for i, option in enumerate(options):
        button_x = right_column_x
        button_y = content_start_y + i * (button_height + 10)
        button_width = right_column_width
        
        button_rect = pygame.Rect(button_x, button_y, button_width, button_height)
        
        if i == selected_index:
            pygame.draw.rect(screen, THEME_COLORS["button_selected"], button_rect, border_radius=8)
        else:
            pygame.draw.rect(screen, THEME_COLORS["button_idle"], button_rect, border_radius=8)
        
        pygame.draw.rect(screen, THEME_COLORS["border"], button_rect, 2, border_radius=8)
        
        text_surface = config.font.render(option, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=button_rect.center)
        screen.blit(text_surface, text_rect)
    
    # Dessiner l'instruction en haut
    draw_menu_instruction(screen, instruction)

def draw_pause_display_font_menu(screen, selected_index):
    """Sous-menu pour les tailles de police."""
    # Font size
    opts = getattr(config, 'font_scale_options', [0.75, 1.0, 1.25, 1.5, 1.75])
    cur_idx = getattr(config, 'current_font_scale_index', 1)
    font_value = f"{opts[cur_idx]}x"
    font_txt = f"{_('submenu_display_font_size') if _ else 'Font Size'}: < {font_value} >"
    
    # Footer font size
    footer_opts = getattr(config, 'footer_font_scale_options', [0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0])
    footer_cur_idx = getattr(config, 'current_footer_font_scale_index', 3)
    footer_font_value = f"{footer_opts[footer_cur_idx]}x"
    footer_font_txt = f"{_('accessibility_footer_font_size').split(':')[0] if _ else 'Footer Font Size'}: < {footer_font_value} >"
    
    back_txt = _("menu_back") if _ else "Back"
    
    options = [font_txt, footer_font_txt, back_txt]
    instruction_keys = [
        "instruction_display_font_size",
        "instruction_display_footer_font_size",
        "instruction_generic_back",
    ]
    
    key = instruction_keys[selected_index] if 0 <= selected_index < len(instruction_keys) else None
    instruction_text = _(key) if key else None
    _draw_submenu_generic(screen, _("submenu_display_font_size") if _ else "Font Size", options, selected_index, instruction_text)

def draw_pause_games_menu(screen, selected_index):
    update_txt = _("menu_redownload_cache")
    scan_txt = _("menu_scan_owned_roms") if _ else "Scan owned ROMs"
    history_txt = _("menu_history") if _ else "History"
    
    # Show unsupported systems
    unsupported = get_show_unsupported_platforms()
    status_unsupported = _('status_on') if unsupported else _('status_off')
    raw_unsupported_label = _('submenu_display_show_unsupported') if _ else 'Show unsupported systems: {status}'
    if '{status}' in raw_unsupported_label:
        raw_unsupported_label = raw_unsupported_label.split('{status}')[0].rstrip(' :')
    unsupported_txt = f"{raw_unsupported_label}: < {status_unsupported} >"
    
    # Filter platforms
    filter_txt = _("submenu_display_filter_platforms") if _ else "Show/Hide Platforms"
    
    back_txt = _("menu_back") if _ else "Back"
    options = [update_txt, scan_txt, history_txt, unsupported_txt, filter_txt, back_txt]
    instruction_keys = [
        "instruction_games_update_cache",
        "instruction_games_scan_owned",
        "instruction_games_history",
        "instruction_display_show_unsupported",
        "instruction_display_filter_platforms",
        "instruction_generic_back",
    ]
    key = instruction_keys[selected_index] if 0 <= selected_index < len(instruction_keys) else None
    instruction_text = None
    if key:
        instruction_text = _(key)
    
    _draw_submenu_generic(screen, _("menu_games") if _ else "Games", options, selected_index, instruction_text)

def draw_pause_settings_menu(screen, selected_index):
    from rgsx_settings import get_auto_extract, get_roms_folder, get_max_simultaneous_downloads
    # Music
    if config.music_enabled:
        music_name = config.current_music_name or ""
        music_option = _("menu_music_enabled").format(music_name)
    else:
        music_option = _("menu_music_disabled")
    # Uniformiser en < value > pour les réglages basculables
    if ' : ' in music_option:
        base, val = music_option.split(' : ',1)
        music_option = f"{base} : < {val.strip()} >"
    symlink_option = _("symlink_option_enabled") if get_symlink_option() else _("symlink_option_disabled")
    if ' ' in symlink_option:
        parts = symlink_option.split(' ',1)
        # On garde phrase intacte si elle n'a pas de forme label: valeur ; sinon transformer
    if ' : ' in symlink_option:
        base, val = symlink_option.split(' : ',1)
        symlink_option = f"{base} : < {val.strip()} >"
    
    # Auto Extract option
    auto_extract_enabled = get_auto_extract()
    auto_extract_status = _("settings_auto_extract_enabled") if auto_extract_enabled else _("settings_auto_extract_disabled")
    auto_extract_txt = f"{_('settings_auto_extract')} : < {auto_extract_status} >"
    
    # ROMs folder option
    roms_folder_custom = get_roms_folder()
    if roms_folder_custom:
        # Tronquer si trop long pour affichage
        max_display = 25
        display_path = roms_folder_custom if len(roms_folder_custom) <= max_display else "..." + roms_folder_custom[-(max_display-3):]
        roms_folder_txt = f"{_('settings_roms_folder')} : {display_path}"
    else:
        roms_folder_txt = f"{_('settings_roms_folder')} : < {_('settings_roms_folder_default')} >"

    # Max simultaneous downloads option
    max_dl = get_max_simultaneous_downloads()
    max_dl_txt = f"{_('settings_max_simultaneous_dl')} : < {max_dl} >"
    
    # Web Service at boot (only on Linux/Batocera)
    web_service_txt = ""
    custom_dns_txt = ""
    if config.OPERATING_SYSTEM == "Linux":
        web_service_enabled = check_web_service_status()
        web_service_status = _("settings_web_service_enabled") if web_service_enabled else _("settings_web_service_disabled")
        web_service_txt = f"{_('settings_web_service')} : < {web_service_status} >"
        
        # Custom DNS at boot
        custom_dns_enabled = check_custom_dns_status()
        custom_dns_status = _("settings_custom_dns_enabled") if custom_dns_enabled else _("settings_custom_dns_disabled")
        custom_dns_txt = f"{_('settings_custom_dns')} : < {custom_dns_status} >"
    
    api_keys_txt = _("menu_api_keys_status") if _ else "API Keys"
    connection_status_txt = _("menu_connection_status") if _ else "Connection status"
    back_txt = _("menu_back") if _ else "Back"

    # qBittorrent WebUI şifre durumu
    from rgsx_settings import get_qbittorrent_webui_password
    current_qbt_password = get_qbittorrent_webui_password()
    default_qbt_password = str(getattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", "") or "RGSXqbt")
    qbt_using_default = bool(current_qbt_password) and current_qbt_password == default_qbt_password
    qbt_status_txt = _("qbt_password_default") if _ else "Default"
    if not qbt_using_default:
        qbt_status_txt = _("qbt_password_custom") if _ else "Custom"
    qbt_password_txt = f"{_('qbt_password_menu') if _ else 'qBittorrent WebUI Password'} : < {qbt_status_txt} >"

    # Construction de la liste des options
    options = [music_option, symlink_option, auto_extract_txt, roms_folder_txt, max_dl_txt]
    if web_service_txt:  # Ajouter seulement si Linux/Batocera
        options.append(web_service_txt)
    if custom_dns_txt:  # Ajouter seulement si Linux/Batocera
        options.append(custom_dns_txt)
    options.extend([api_keys_txt, qbt_password_txt, connection_status_txt, back_txt])

    # Index de l'option Dossier ROMs
    roms_folder_index = 3

    # Instructions textuelles pour chaque option
    instruction_keys = [
        "instruction_settings_music",
        "instruction_settings_symlink",
        "instruction_settings_auto_extract",
        "instruction_settings_roms_folder",
        "instruction_settings_max_simultaneous_dl",
    ]
    if web_service_txt:
        instruction_keys.append("instruction_settings_web_service")
    if custom_dns_txt:
        instruction_keys.append("instruction_settings_custom_dns")
    instruction_keys.extend([
        "instruction_settings_api_keys",
        "instruction_settings_qbt_password",
        "instruction_settings_connection_status",
        "instruction_generic_back",
    ])
    key = instruction_keys[selected_index] if 0 <= selected_index < len(instruction_keys) else None
    instruction_text = _(key) if key else None
    
    _draw_submenu_generic(screen, _("menu_settings_category") if _ else "Settings", options, selected_index, instruction_text)

def draw_pause_api_keys_status(screen):
    screen.blit(core.OVERLAY, (0,0))
    keys = load_api_keys()
    title = _("api_keys_status_title") if _ else "API Keys Status"
    # Préparer données avec masquage partiel des clés (afficher 4 premiers et 2 derniers caractères si longueur > 10)
    def mask_key(value: str|None):
        if not value:
            return ""  # rien si absent
        v = value.strip()
        if len(v) <= 10:
            return v  # courte, afficher entière
        return f"{v[:4]}…{v[-2:]}"  # masque au milieu

    providers = [
        ("1fichier", keys.get('1fichier')),
        ("AllDebrid", keys.get('alldebrid')),
        ("Debrid-Link", keys.get('debridlink')),
        ("RealDebrid", keys.get('realdebrid')),
        ("TorBox", keys.get('torbox'))
    ]
    # Dimensions dynamiques en fonction du contenu
    row_height = config.small_font.get_height() + 14
    header_height = 60
    inner_rows = len(providers)
    menu_width = int(config.screen_width * 0.60)
    menu_height = header_height + inner_rows * row_height + 80
    menu_x = (config.screen_width - menu_width)//2
    menu_y = (config.screen_height - menu_height)//2
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (menu_x, menu_y, menu_width, menu_height), border_radius=22)
    pygame.draw.rect(screen, THEME_COLORS["border"], (menu_x, menu_y, menu_width, menu_height), 2, border_radius=22)

    # Titre
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width//2, menu_y + 36))
    screen.blit(title_surface, title_rect)

    status_present_txt = _("status_present") if _ else "Present"
    status_missing_txt = _("status_missing") if _ else "Missing"
    # Plus de légende textuelle Présent / Missing (demandé) – seules les pastilles couleur serviront.
    legend_rect = pygame.Rect(0,0,0,0)

    # Colonnes: Provider | Status badge | (key masked)
    col_provider_x = menu_x + 40
    col_status_x = menu_x + int(menu_width * 0.40)
    col_key_x = menu_x + int(menu_width * 0.58)

    # Démarrage des lignes sous le titre avec un padding
    y = title_rect.bottom + 24
    badge_font = config.tiny_font if hasattr(config, 'tiny_font') else config.small_font
    for provider, value in providers:
        present = bool(value)
        # Provider name
        prov_surf = config.small_font.render(provider, True, THEME_COLORS["text"])
        screen.blit(prov_surf, (col_provider_x, y))

        # Pastille circulaire simple (couleur = statut)
        circle_color = (60, 170, 60) if present else (180, 55, 55)
        circle_bg = (30, 70, 30) if present else (70, 25, 25)
        radius = 14
        center_x = col_status_x + radius
        center_y = y + badge_font.get_height()//2
        pygame.draw.circle(screen, circle_bg, (center_x, center_y), radius)
        pygame.draw.circle(screen, circle_color, (center_x, center_y), radius, 2)

        # Masked key (dim color) or hint
        if present:
            masked = mask_key(value)
            key_color = THEME_COLORS.get("text_dim", (180,180,180))
            key_label = masked
        else:
            key_color = THEME_COLORS.get("text_dim", (150,150,150))
            # Afficher nom de fichier + 'empty'
            filename_display = {
                '1fichier': '1FichierAPI.txt',
                'AllDebrid': 'AllDebridAPI.txt',
                'Debrid-Link': 'DebridLinkAPI.txt',
                'RealDebrid': 'RealDebridAPI.txt',
                'TorBox' : 'TorBoxAPI.txt'
            }.get(provider, 'key.txt')
            empty_suffix = _("api_key_empty_suffix") if _ and _("api_key_empty_suffix") != "api_key_empty_suffix" else "empty"
            key_label = f"{filename_display} {empty_suffix}"
        key_surf = config.tiny_font.render(key_label, True, key_color) if hasattr(config, 'tiny_font') else config.small_font.render(key_label, True, key_color)
        screen.blit(key_surf, (col_key_x, y))

        # Ligne séparatrice (optionnelle)
        sep_y = y + row_height - 8
        if provider != providers[-1][0]:
            pygame.draw.line(screen, THEME_COLORS["border"], (menu_x + 25, sep_y), (menu_x + menu_width - 25, sep_y), 1)
        y += row_height

    # Indication basique: utiliser config.SAVE_FOLDER (chemin dynamique)
    save_folder_path = config.SAVE_FOLDER
    # Utiliser placeholder {path} si traduction fournie
    if _ and _("api_keys_hint_manage") != "api_keys_hint_manage":
        try:
            hint_txt = _("api_keys_hint_manage").format(path=save_folder_path)
        except Exception:
            hint_txt = f"Put your keys in {save_folder_path}"
    else:
        hint_txt = f"Put your keys in {save_folder_path}"
    hint_font = config.tiny_font if hasattr(config, 'tiny_font') else config.small_font
    hint_surf = hint_font.render(hint_txt, True, THEME_COLORS.get("text_dim", THEME_COLORS["text"]))
    # Positionné un peu plus haut pour aérer
    hint_rect = hint_surf.get_rect(center=(config.screen_width//2, menu_y + menu_height - 30))
    screen.blit(hint_surf, hint_rect)

def draw_pause_qbt_password(screen):
    """qBittorrent WebUI şifresi değiştirme ekranı: durum + sanal klavye girişi."""
    screen.blit(core.OVERLAY, (0, 0))

    panel_width = int(config.screen_width * 0.75)
    panel_height = int(config.screen_height * 0.68)
    panel_x = (config.screen_width - panel_width) // 2
    panel_y = (config.screen_height - panel_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (panel_x, panel_y, panel_width, panel_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (panel_x, panel_y, panel_width, panel_height), 2, border_radius=12)

    # Başlık
    title = _("qbt_password_title") if _ else "qBittorrent WebUI Password"
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, panel_y + 34))
    screen.blit(title_surface, title_rect)

    # Durum satırı
    from rgsx_settings import get_qbittorrent_webui_password
    current_qbt_password = get_qbittorrent_webui_password()
    default_qbt_password = str(getattr(config, "TORRENT_QBITTORRENT_WEBUI_PASSWORD", "") or "RGSXqbt")
    qbt_using_default = bool(current_qbt_password) and current_qbt_password == default_qbt_password
    status_txt = _("qbt_password_default") if _ else "Default"
    if not qbt_using_default:
        status_txt = _("qbt_password_custom") if _ else "Custom"
    status_label = f"{_('qbt_password_current') if _ else 'Current'}: {status_txt}"
    status_surface = config.small_font.render(status_label, True, THEME_COLORS.get("text_dim", THEME_COLORS["text"]))
    status_rect = status_surface.get_rect(center=(config.screen_width // 2, panel_y + 70))
    screen.blit(status_surface, status_rect)

    # Girdi alanı
    input_y = panel_y + 100
    input_width = panel_width - 60
    input_height = 40
    input_x = panel_x + 30

    pygame.draw.rect(screen, THEME_COLORS["button_selected"], (input_x, input_y, input_width, input_height), border_radius=6)
    pygame.draw.rect(screen, THEME_COLORS["border_selected"], (input_x, input_y, input_width, input_height), 2, border_radius=6)

    password_text = getattr(config, 'qbt_password_text', '')
    # Mask: şifreyi nokta ile göster
    masked_display = "•" * len(password_text) + "_"
    input_surface = config.font.render(masked_display, True, THEME_COLORS["text"])
    input_rect = input_surface.get_rect(midleft=(input_x + 10, input_y + input_height // 2))
    screen.blit(input_surface, input_rect)

    # Sanal klavye
    keyboard_layout = [
        ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
        ['A', 'Z', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
        ['Q', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M'],
        ['W', 'X', 'C', 'V', 'B', 'N', '-', '_', '.']
    ]
    selected_row, selected_col = getattr(config, 'qbt_password_selected_key', (0, 0))

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
            if is_selected:
                pygame.draw.rect(screen, THEME_COLORS["button_hover"], (key_x, key_y, key_size, key_size), border_radius=4)
                pygame.draw.rect(screen, THEME_COLORS["border_selected"], (key_x, key_y, key_size, key_size), 2, border_radius=4)
            else:
                pygame.draw.rect(screen, THEME_COLORS["button_idle"], (key_x, key_y, key_size, key_size), border_radius=4)
                pygame.draw.rect(screen, THEME_COLORS["border"], (key_x, key_y, key_size, key_size), 1, border_radius=4)
            key_surface = config.small_font.render(key, True, THEME_COLORS["text_selected"] if is_selected else THEME_COLORS["text"])
            key_rect = key_surface.get_rect(center=(key_x + key_size // 2, key_y + key_size // 2))
            screen.blit(key_surface, key_rect)

    # Alt ipuçları
    hint_y = keyboard_y + len(keyboard_layout) * (key_size + key_gap) + 18
    hint_parts = [
        f"{_('instruction_generic_select') if _ else 'Select'}: {_('qbt_password_input_hint') if _ else 'Enter character'}",
        f"{_('instruction_generic_confirm') if _ else 'OK'}: {_('qbt_password_save_hint') if _ else 'Save'}",
        f"{_('instruction_generic_cancel') if _ else 'Back'}: {_('instruction_generic_back') if _ else 'Back'}",
    ]
    hint_txt = "    ".join(part for part in hint_parts)
    hint_surface = config.tiny_font.render(hint_txt, True, THEME_COLORS.get("text_dim", THEME_COLORS["text"]))
    hint_rect = hint_surface.get_rect(center=(config.screen_width // 2, hint_y))
    screen.blit(hint_surface, hint_rect)

def draw_pause_connection_status(screen):
    screen.blit(core.OVERLAY, (0, 0))
    status_map, last_ts, in_progress, progress = get_connection_status_snapshot()
    targets = get_connection_status_targets()

    title = _("connection_status_title") if _ else "Connection status"
    cat_updates = _("connection_status_category_updates") if _ else "Updates"
    cat_sources = _("connection_status_category_sources") if _ else "Sources"

    # Group rows by category
    category_labels_map = {
        "updates": cat_updates,
        "sources": cat_sources,
    }

    categories_order = []
    for target in targets:
        cat = str(target.get("category", "sources")).strip().lower() or "sources"
        if cat not in categories_order:
            categories_order.append(cat)

    def _category_label(cat_key: str) -> str:
        if cat_key in category_labels_map:
            return category_labels_map[cat_key]
        cleaned = cat_key.replace("_", " ").strip()
        return cleaned.title() if cleaned else cat_sources

    rows = []  # list of (type, data)
    for cat in categories_order:
        cat_items = [t for t in targets if str(t.get("category", "sources")).strip().lower() == cat]
        if not cat_items:
            continue
        rows.append(("header", _category_label(cat)))
        for item in cat_items:
            rows.append(("item", item))

    # Title surface (used for sizing)
    title_surface = config.font.render(title, True, THEME_COLORS["text"])

    # Dimensions
    row_height = config.small_font.get_height() + 14
    header_row_height = config.small_font.get_height() + 10
    title_height = 60
    footer_height = 55
    content_height = 0
    for row_type, row_data in rows:
        content_height += header_row_height if row_type == "header" else row_height

    # Measure max text width to size the menu
    max_text_width = title_surface.get_width()
    for row_type, row_data in rows:
        if row_type == "header":
            w = config.small_font.size(str(row_data))[0]
        else:
            label = row_data.get("label") or row_data.get("key", "")
            w = config.small_font.size(str(label))[0]
        if w > max_text_width:
            max_text_width = w

    circle_area_width = 46  # status circle + gap
    inner_padding = 70
    menu_width = min(int(config.screen_width * 0.70), max(360, max_text_width + circle_area_width + inner_padding))
    menu_height = title_height + content_height + footer_height
    menu_x = (config.screen_width - menu_width) // 2
    menu_y = (config.screen_height - menu_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (menu_x, menu_y, menu_width, menu_height), border_radius=22)
    pygame.draw.rect(screen, THEME_COLORS["border"], (menu_x, menu_y, menu_width, menu_height), 2, border_radius=22)

    # Title
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, menu_y + 34))
    screen.blit(title_surface, title_rect)

    # Columns
    col_site_x = menu_x + 40
    col_status_x = menu_x + int(menu_width * 0.70)

    y = menu_y + title_height - 5
    for row_type, data in rows:
        if row_type == "header":
            header_text = data
            header_surf = config.small_font.render(header_text, True, THEME_COLORS.get("text_dim", THEME_COLORS["text"]))
            screen.blit(header_surf, (col_site_x, y))
            # separator line
            sep_y = y + header_row_height - 6
            pygame.draw.line(screen, THEME_COLORS["border"], (menu_x + 25, sep_y), (menu_x + menu_width - 25, sep_y), 1)
            y += header_row_height
            continue

        item = data
        key = item.get("key")
        label = item.get("label") or item.get("key", "")

        status_val = status_map.get(key)
        if status_val is True:
            circle_color = (60, 170, 60)
            circle_bg = (30, 70, 30)
        elif status_val is False:
            circle_color = (180, 55, 55)
            circle_bg = (70, 25, 25)
        else:
            circle_color = (140, 140, 140)
            circle_bg = (60, 60, 60)

        # Site label (indent to distinguish from category title)
        label_surf = config.small_font.render(label, True, THEME_COLORS["text"])
        screen.blit(label_surf, (col_site_x + 18, y))

        # Status circle
        radius = 14
        center_x = col_status_x + radius
        center_y = y + config.small_font.get_height() // 2
        pygame.draw.circle(screen, circle_bg, (center_x, center_y), radius)
        pygame.draw.circle(screen, circle_color, (center_x, center_y), radius, 2)

        # Separator
        sep_y = y + row_height - 8
        pygame.draw.line(screen, THEME_COLORS["border"], (menu_x + 25, sep_y), (menu_x + menu_width - 25, sep_y), 1)
        y += row_height

    # Footer hint
    hint_font = config.tiny_font if hasattr(config, "tiny_font") else config.small_font
    if in_progress:
        done = int(progress.get("done", 0)) if isinstance(progress, dict) else 0
        total = int(progress.get("total", 0)) if isinstance(progress, dict) else 0
        if _ and _("connection_status_progress") != "connection_status_progress":
            try:
                hint_txt = _("connection_status_progress").format(done=done, total=total)
            except Exception:
                hint_txt = _("connection_status_checking") if _ else "Checking..."
        else:
            hint_txt = f"Checking... {done}/{total}" if total else ("Checking..." if not _ else _("connection_status_checking"))
    elif last_ts:
        try:
            time_str = datetime.fromtimestamp(last_ts).strftime("%H:%M:%S")
        except Exception:
            time_str = ""
        if _ and _("connection_status_last_check") != "connection_status_last_check":
            try:
                hint_txt = _("connection_status_last_check").format(time=time_str)
            except Exception:
                hint_txt = f"Last check: {time_str}" if time_str else ""
        else:
            hint_txt = f"Last check: {time_str}" if time_str else ""
    else:
        hint_txt = ""

    if hint_txt:
        hint_surf = hint_font.render(hint_txt, True, THEME_COLORS.get("text_dim", THEME_COLORS["text"]))
        hint_rect = hint_surf.get_rect(center=(config.screen_width // 2, menu_y + menu_height - 26))
        screen.blit(hint_surf, hint_rect)

def draw_filter_platforms_menu(screen):
    """Affiche le menu de filtrage des plateformes (sources + plateformes collapsibles)."""
    screen.blit(core.OVERLAY, (0, 0))
    settings = load_rgsx_settings()
    hidden = set(settings.get("hidden_platforms", [])) if isinstance(settings, dict) else set()

    def _extract_source(platform_name: str) -> str:
        match = re.search(r'\(([^()]+)\)\s*$', str(platform_name).strip())
        if match:
            return match.group(1).strip()
        fallback = _("games_source_rgsx") if _ else "RGSX"
        return fallback if fallback != "games_source_rgsx" else "RGSX"

    def _strip_source_suffix(platform_name: str) -> str:
        return re.sub(r'\s*\([^()]+\)\s*$', '', str(platform_name)).strip()

    # Construire mapping source -> plateformes (trié, sans doublons)
    source_to_platforms = {}
    for entry in config.platform_dicts:
        platform_name = entry.get("platform_name", "") if isinstance(entry, dict) else ""
        platform_name = str(platform_name).strip()
        if not platform_name:
            continue
        source_name = _extract_source(platform_name)
        source_to_platforms.setdefault(source_name, []).append(platform_name)

    for source_name in list(source_to_platforms.keys()):
        source_to_platforms[source_name] = sorted(set(source_to_platforms[source_name]), key=lambda s: str(s).lower())
    source_to_platforms = dict(sorted(source_to_platforms.items(), key=lambda kv: str(kv[0]).lower()))
    config.filter_platforms_source_map = source_to_platforms

    all_platform_names = []
    for source_name in source_to_platforms:
        all_platform_names.extend(source_to_platforms[source_name])

    # Initialiser/synchroniser la copie de travail par plateforme
    current_map = {}
    if isinstance(config.filter_platforms_selection, list):
        for item in config.filter_platforms_selection:
            if isinstance(item, (list, tuple)) and len(item) == 2:
                name = str(item[0]).strip()
                if name:
                    current_map[name] = bool(item[1])

    expected_set = set(all_platform_names)
    if set(current_map.keys()) != expected_set:
        config.filter_platforms_selection = [(name, name in hidden) for name in all_platform_names]
        config.selected_filter_index = 0
        config.filter_platforms_scroll_offset = 0
        config.filter_platforms_dirty = False
    else:
        config.filter_platforms_selection = [(name, current_map.get(name, False)) for name in all_platform_names]

    hidden_map = {name: bool(is_hidden) for name, is_hidden in config.filter_platforms_selection}

    expanded_raw = getattr(config, 'filter_platforms_expanded_sources', [])
    expanded_sources = set(expanded_raw if isinstance(expanded_raw, list) else [])
    expanded_sources = {source_name for source_name in expanded_sources if source_name in source_to_platforms}
    config.filter_platforms_expanded_sources = sorted(expanded_sources, key=lambda s: str(s).lower())

    rows = []
    for source_name, platforms in source_to_platforms.items():
        total = len(platforms)
        hidden_count = sum(1 for platform_name in platforms if hidden_map.get(platform_name, False))
        rows.append({
            "type": "source",
            "source": source_name,
            "platforms": platforms,
            "total": total,
            "hidden_count": hidden_count,
            "expanded": source_name in expanded_sources,
        })
        if source_name in expanded_sources:
            for platform_name in platforms:
                rows.append({
                    "type": "platform",
                    "source": source_name,
                    "platform": platform_name,
                    "hidden": bool(hidden_map.get(platform_name, False)),
                })

    if rows:
        config.selected_filter_index = max(0, min(config.selected_filter_index, len(rows) - 1))
    else:
        config.selected_filter_index = 0

    title_text = _("filter_platforms_title")
    title_surface = config.title_font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, title_surface.get_height() // 2 + 14))
    hpad = max(36, min(64, int(config.screen_width * 0.06)))
    vpad = max(10, min(20, int(title_surface.get_height() * 0.45)))
    title_rect_inflated = title_rect.inflate(hpad, vpad)
    title_rect_inflated.topleft = ((config.screen_width - title_rect_inflated.width) // 2, 10)
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], title_rect_inflated, 2, border_radius=12)
    screen.blit(title_surface, title_rect)

    # Zone liste: laisser de la place au footer de controls + infos
    footer_reserved = max(95, int(config.screen_height * 0.15))
    list_width = int(config.screen_width * 0.78)
    list_x = (config.screen_width - list_width) // 2
    list_y = title_rect_inflated.bottom + 16
    list_bottom_limit = config.screen_height - footer_reserved - 38
    list_height = max(140, list_bottom_limit - list_y)

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (list_x, list_y, list_width, list_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (list_x, list_y, list_width, list_height), 2, border_radius=12)

    line_height = config.small_font.get_height() + 8
    visible_items = max(4, (list_height - 20) // line_height)
    total_items = len(rows)

    if config.selected_filter_index < config.filter_platforms_scroll_offset:
        config.filter_platforms_scroll_offset = config.selected_filter_index
    elif config.selected_filter_index >= config.filter_platforms_scroll_offset + visible_items:
        config.filter_platforms_scroll_offset = config.selected_filter_index - visible_items + 1
    config.filter_platforms_scroll_offset = max(0, min(config.filter_platforms_scroll_offset, max(0, total_items - visible_items)))

    # Dessin des lignes source + plateformes
    start = config.filter_platforms_scroll_offset
    end = min(start + visible_items, total_items)
    for i in range(start, end):
        row = rows[i]
        idx_on_screen = i - start
        y_center = list_y + 10 + idx_on_screen * line_height + line_height // 2
        selected = (config.selected_filter_index == i)

        if selected:
            glow_surface = pygame.Surface((list_width - 32, line_height), pygame.SRCALPHA)
            pygame.draw.rect(glow_surface, THEME_COLORS["fond_lignes"] + (50,), (0, 0, list_width - 32, line_height), border_radius=8)
            screen.blit(glow_surface, (list_x + 16, y_center - line_height // 2))

        if row.get("type") == "source":
            total = max(1, int(row.get("total", 0)))
            hidden_count = int(row.get("hidden_count", 0))
            visible_count = max(0, total - hidden_count)
            if hidden_count == 0:
                checkbox = "[X]"
            elif hidden_count >= total:
                checkbox = "[ ]"
            else:
                checkbox = "[-]"
            collapse = "v" if row.get("expanded") else ">"
            display_text = f"{checkbox} {collapse} {row.get('source', '')} ({visible_count}/{total})"
            text_x = list_x + 20
        else:
            platform_name = row.get("platform", "")
            checkbox = "[X]" if not row.get("hidden") else "[ ]"
            clean_name = _strip_source_suffix(platform_name) or platform_name
            display_text = f"{checkbox}   {clean_name}"
            text_x = list_x + 44

        max_text_w = max(60, list_width - (text_x - list_x) - 38)
        fitted_text = truncate_text_end(display_text, config.small_font, max_text_w)
        color = THEME_COLORS["fond_lignes"] if selected else THEME_COLORS["text"]
        text_surface = config.small_font.render(fitted_text, True, color)
        text_rect = text_surface.get_rect(midleft=(text_x, y_center))
        screen.blit(text_surface, text_rect)

    # Scrollbar
    if total_items > visible_items:
        scroll_track_height = list_height - 20
        scroll_height = int((visible_items / total_items) * scroll_track_height)
        scroll_height = max(20, scroll_height)
        scroll_range = max(1, total_items - visible_items)
        scroll_y = int((config.filter_platforms_scroll_offset / scroll_range) * (scroll_track_height - scroll_height))
        pygame.draw.rect(screen, THEME_COLORS["fond_lignes"], (list_x + list_width - 22, list_y + 10 + scroll_y, 9, scroll_height), border_radius=4)

    # Infos bas
    total_platforms = len(all_platform_names)
    hidden_count = sum(1 for _, is_hidden in config.filter_platforms_selection if is_hidden)
    visible_count = total_platforms - hidden_count
    info_text = _("filter_platforms_info").format(visible_count, hidden_count, total_platforms)
    info_surface = config.small_font.render(info_text, True, THEME_COLORS["text"])
    info_rect = info_surface.get_rect(center=(config.screen_width // 2, list_y + list_height + 18))
    screen.blit(info_surface, info_rect)

    if config.filter_platforms_dirty:
        dirty_text = _("filter_unsaved_warning")
        dirty_surface = config.small_font.render(dirty_text, True, THEME_COLORS["warning_text"])
        dirty_rect = dirty_surface.get_rect(center=(config.screen_width // 2, info_rect.bottom + 22))
        screen.blit(dirty_surface, dirty_rect)

def draw_controls_help(screen, previous_state):
    """Affiche la liste des contrôles (aide) avec mise en page adaptative."""
    # Contenu des catégories (avec icônes si disponibles)
    control_categories = {
        _("controls_category_navigation"): [
            ("icons", ["up", "down", "left", "right"], _('controls_navigation')),
            ("icons", ["page_up", "page_down"], _('controls_pages')),
        ],
        _("controls_category_main_actions"): [
            ("icons", ["confirm"], _('controls_confirm_select')),
            ("icons", ["cancel"], _('controls_cancel_back')),
            ("icons", ["start"], _('controls_action_start')),
        ],
        _("controls_category_downloads"): [
            ("icons", ["history"], _('controls_action_history')),
            ("icons", ["clear_history"], _('controls_action_clear_history')),
        ],
        _("controls_category_search"): [
            ("icons", ["filter"], _('controls_filter_search')),
            ("icons", ["delete"], _('controls_action_delete')),
            ("icons", ["space"], _('controls_action_space')),
        ],
    }

    # États autorisés (même logique qu'avant)
    allowed_states = {
        # États classiques où l'aide était accessible
        "error", "platform", "game", "confirm_exit",
        "extension_warning", "history", "clear_history",
        # Nouveaux états hiérarchiques pause
        "pause_controls_menu", "pause_menu"
    }
    if previous_state not in allowed_states:
        return

    screen.blit(core.OVERLAY, (0, 0))

    # Paramètres d'affichage
    font = config.small_font
    title_font = config.title_font
    section_font = config.font
    line_spacing = max(4, font.get_height() // 6)
    section_spacing = font.get_height() // 2
    title_spacing = font.get_height()
    padding = 24
    inter_col_spacing = 48
    max_panel_width = int(config.screen_width * 0.9)
    max_panel_height = int(config.screen_height * 0.9)

    # Découpage en 2 colonnes (équilibré)
    categories_list = list(control_categories.items())
    mid = len(categories_list) // 2
    col1_categories = categories_list[:mid]
    col2_categories = categories_list[mid:]

    # Largeur cible par colonne (avant wrapping)
    target_col_width = (max_panel_width - 2 * padding - inter_col_spacing) // 2

    def wrap_lines_for_column(cat_pairs):
        wrapped = []  # liste de (is_section_title, surface)
        max_width = 0
        total_height = 0
        for section_title, lines in cat_pairs:
            # Titre section
            sec_surf = section_font.render(section_title, True, THEME_COLORS["fond_lignes"])
            wrapped.append((True, sec_surf))
            total_height += sec_surf.get_height() + line_spacing

            for raw_line in lines:
                # Deux formats possibles:
                # - tuple ("icons", [actions], text)
                # - chaîne texte simple
                line_surface = None
                if isinstance(raw_line, tuple) and len(raw_line) >= 3 and raw_line[0] == "icons":
                    _, actions, text = raw_line
                    try:
                        line_surface = render_icons_line(actions, text, target_col_width, font, THEME_COLORS["text"])
                    except Exception:
                        line_surface = None
                if line_surface is None:
                    # Fallback: traitement texte comme avant
                    words = str(raw_line).split()
                    cur = ""
                    for word in words:
                        test = (cur + " " + word).strip()
                        if font.size(test)[0] <= target_col_width:
                            cur = test
                        else:
                            if cur:
                                line_surf = font.render(cur, True, THEME_COLORS["text"])
                                wrapped.append((False, line_surf))
                                total_height += line_surf.get_height() + line_spacing
                                max_width = max(max_width, line_surf.get_width())
                            cur = word
                    if cur:
                        line_surf = font.render(cur, True, THEME_COLORS["text"])
                        wrapped.append((False, line_surf))
                        total_height += line_surf.get_height() + line_spacing
                        max_width = max(max_width, line_surf.get_width())
                else:
                    wrapped.append((False, line_surface))
                    total_height += line_surface.get_height() + line_spacing
                    max_width = max(max_width, line_surface.get_width())

            total_height += section_spacing  # espace après section
            max_width = max(max_width, sec_surf.get_width())

        if wrapped and not wrapped[-1][0]:
            total_height -= section_spacing  # retirer excédent final
        return wrapped, max_width, total_height

    col1_wrapped, col1_w, col1_h = wrap_lines_for_column(col1_categories)
    col2_wrapped, col2_w, col2_h = wrap_lines_for_column(col2_categories)

    col_widths_sum = col1_w + col2_w + inter_col_spacing
    content_width = min(max_panel_width - 2 * padding, max(col_widths_sum, col1_w + col2_w + inter_col_spacing))
    panel_width = content_width + 2 * padding

    title_surf = title_font.render(_("controls_help_title"), True, THEME_COLORS["text"])
    title_height = title_surf.get_height()

    content_height = max(col1_h, col2_h)
    # Réserver un espace supplémentaire en bas pour éviter que le cadre ne coupe les icônes/boutons
    extra_bottom_space = max(20, int(font.get_height() * 1.5))
    panel_height = title_height + title_spacing + content_height + 2 * padding + extra_bottom_space
    if panel_height > max_panel_height:
        panel_height = max_panel_height
        enable_clip = True
    else:
        enable_clip = False

    panel_x = (config.screen_width - panel_width) // 2
    panel_y = (config.screen_height - panel_height) // 2

    # Fond panel
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (panel_x, panel_y, panel_width, panel_height), border_radius=16)
    pygame.draw.rect(screen, THEME_COLORS["border"], (panel_x, panel_y, panel_width, panel_height), 2, border_radius=16)

    # Titre
    title_rect = title_surf.get_rect(center=(panel_x + panel_width // 2, panel_y + padding + title_height // 2))
    screen.blit(title_surf, title_rect)

    # Zones de colonnes
    col_top = panel_y + padding + title_height + title_spacing
    col1_x = panel_x + padding
    col2_x = panel_x + panel_width - padding - col2_w

    # Clip si nécessaire
    prev_clip = None
    if enable_clip:
        prev_clip = screen.get_clip()
        clip_rect = pygame.Rect(panel_x + padding, col_top, panel_width - 2 * padding, panel_height - (col_top - panel_y) - padding)
        screen.set_clip(clip_rect)

    # Dessin colonne 1
    y1 = col_top
    last_section = False
    for is_section, surf in col1_wrapped:
        if is_section:
            y1 += 0
        if y1 + surf.get_height() > panel_y + panel_height - padding:
            break
        screen.blit(surf, (col1_x, y1))
        y1 += surf.get_height() + (section_spacing if is_section else line_spacing)

    # Dessin colonne 2
    y2 = col_top
    for is_section, surf in col2_wrapped:
        if y2 + surf.get_height() > panel_y + panel_height - padding:
            break
        screen.blit(surf, (col2_x, y2))
        y2 += surf.get_height() + (section_spacing if is_section else line_spacing)

    if enable_clip and prev_clip is not None:
        screen.set_clip(prev_clip)

    # Footer: controller style selector display
    try:
        style_is_inverted = getattr(config, 'nintendo_layout', False)
        style_label = _('controller_style_label') if _ else 'Controller Style :'
        # When inverted flag is True we show Nintendo style (A/B swapped vs Xbox)
        style_name = _('controller_style_nintendo') if style_is_inverted else _('controller_style_xbox')
        # Render footer with left/right helper icons and the current controller style label
        style_label = style_label
        style_name = style_name
        icon_size = max(18, font.get_height())
        left_icon = get_help_icon_surface('left', icon_size)
        right_icon = get_help_icon_surface('right', icon_size)
        label_surf = font.render(f"{style_label} {style_name}", True, THEME_COLORS['text'])

        # Compose horizontal footer surface: [left_icon]  label  [right_icon]
        parts_width = 0
        parts_height = 0
        if left_icon:
            parts_width += left_icon.get_width() + 8
            parts_height = max(parts_height, left_icon.get_height())
        parts_width += label_surf.get_width()
        parts_height = max(parts_height, label_surf.get_height())
        if right_icon:
            parts_width += 8 + right_icon.get_width()
            parts_height = max(parts_height, right_icon.get_height())

        footer_surf = pygame.Surface((max(1, parts_width), max(1, parts_height)), pygame.SRCALPHA)
        x = 0
        if left_icon:
            footer_surf.blit(left_icon, (x, (parts_height - left_icon.get_height()) // 2))
            x += left_icon.get_width() + 8
        footer_surf.blit(label_surf, (x, (parts_height - label_surf.get_height()) // 2))
        x += label_surf.get_width()
        if right_icon:
            x += 8
            footer_surf.blit(right_icon, (x, (parts_height - right_icon.get_height()) // 2))

        # Place footer inside the panel, just above the bottom padding so it stays visible
        try:
            footer_y = panel_y + panel_height - padding - (footer_surf.get_height() // 2) - 4
        except Exception:
            footer_y = panel_y + panel_height - padding - 8
        footer_rect = footer_surf.get_rect(center=(config.screen_width // 2, int(footer_y)))
        screen.blit(footer_surf, footer_rect)
    except Exception:
        pass

def draw_confirm_dialog(screen):
    """Affiche le sous-menu Quit avec les options Quit et Restart."""
    options = [
        _("menu_quit_app") if _ else "Quit RGSX",
        _("menu_restart") if _ else "Restart RGSX",
        _("menu_back") if _ else "Back"
    ]
    instruction_keys = [
        "instruction_quit_app",
        "instruction_quit_restart",
        "instruction_generic_back",
    ]
    key = instruction_keys[config.confirm_selection] if 0 <= config.confirm_selection < len(instruction_keys) else None
    instruction_text = _(key) if key else None
    _draw_submenu_generic(screen, _("menu_quit") if _ else "Quit", options, config.confirm_selection, instruction_text)

def draw_reload_games_data_dialog(screen):
    """Affiche la boîte de dialogue de confirmation pour retélécharger le cache des jeux."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 150))

    screen.blit(core.OVERLAY, (0, 0))
    message = _("confirm_redownload_cache")
    wrapped_message = wrap_text(message, config.small_font, config.screen_width - 80)
    line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    # Adapter hauteur bouton en fonction de la taille de police
    sample_text = config.small_font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(int(config.screen_height * 0.0463), font_height + 15)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 80
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_message):
        text = config.small_font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    # Calcule une largeur de bouton cohérente avec la boîte et centre les deux boutons
    button_width = min(160, (rect_width - 60) // 2)
    yes_x = rect_x + rect_width // 2 - button_width - 10
    no_x = rect_x + rect_width // 2 + 10
    buttons_y = rect_y + text_height + margin_top_bottom
    draw_stylized_button(screen, _("button_yes"), yes_x, buttons_y, button_width, button_height, selected=config.redownload_confirm_selection == 1)
    draw_stylized_button(screen, _("button_no"), no_x, buttons_y, button_width, button_height, selected=config.redownload_confirm_selection == 0)

def draw_reset_settings_confirm_dialog(screen):
    """Affiche un avertissement avant reset des paramètres (oui/non)."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 150))

    screen.blit(core.OVERLAY, (0, 0))

    title = _("menu_reset_default_settings") if _ else "Reset default settings"
    if not title or title == "menu_reset_default_settings":
        title = "Reset default settings"

    message = _("confirm_reset_settings_warning") if _ else (
        "Warning: no file, history or game will be deleted.\n"
        "Only settings will be reset (platform filtering, sort order, custom ROM paths).\n"
        "Continue?"
    )
    if not message or message == "confirm_reset_settings_warning":
        message = (
            "Warning: no file, history or game will be deleted.\n"
            "Only settings will be reset (platform filtering, sort order, custom ROM paths).\n"
            "Continue?"
        )

    wrapped_message = []
    for paragraph in str(message).split("\n"):
        lines = wrap_text(paragraph, config.small_font, config.screen_width - 120) if paragraph else [""]
        wrapped_message.extend(lines)

    line_height = config.small_font.get_height() + 5
    title_height = config.font.get_height() + 10
    text_height = len(wrapped_message) * line_height
    sample_text = config.small_font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(int(config.screen_height * 0.0463), font_height + 15)
    margin_top_bottom = 20
    rect_height = title_height + text_height + button_height + 2 * margin_top_bottom + 8
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message], default=420)
    title_width = config.font.size(title)[0]
    rect_width = max(max_text_width + 80, title_width + 80)
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + title_height // 2))
    screen.blit(title_surface, title_rect)

    text_top = rect_y + margin_top_bottom + title_height
    for i, line in enumerate(wrapped_message):
        text = config.small_font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, text_top + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    button_width = min(170, (rect_width - 60) // 2)
    yes_x = rect_x + rect_width // 2 - button_width - 10
    no_x = rect_x + rect_width // 2 + 10
    buttons_y = rect_y + margin_top_bottom + title_height + text_height + 8
    sel = int(getattr(config, 'reset_settings_confirm_selection', 0))
    draw_stylized_button(screen, _("button_yes"), yes_x, buttons_y, button_width, button_height, selected=sel == 1)
    draw_stylized_button(screen, _("button_no"), no_x, buttons_y, button_width, button_height, selected=sel == 0)

def draw_gamelist_update_prompt(screen):
    """Affiche la boîte de dialogue pour proposer la mise à jour de la liste des jeux."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 150))

    screen.blit(core.OVERLAY, (0, 0))
    
    from rgsx_settings import get_last_gamelist_update, format_gamelist_update_display
    
    last_update = get_last_gamelist_update()
    remote_update = getattr(config, 'gamelist_remote_update_display', '') or ''
    local_update = getattr(config, 'gamelist_local_update_display', '') or format_gamelist_update_display(last_update)
    if last_update and remote_update:
        message = _("gamelist_update_prompt_remote_newer").format(local_update, remote_update) if _ else f"A newer online game list is available (local: {local_update}, online: {remote_update}). Download the latest version?"
    elif last_update:
        message = _("gamelist_update_prompt_with_date").format(local_update) if _ else f"Local game list last update: {local_update}. Download the latest version?"
    else:
        message = _("gamelist_update_prompt_first_time") if _ else "Would you like to download the latest game list?"
    
    wrapped_message = wrap_text(message, config.small_font, config.screen_width - 80)
    line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    
    sample_text = config.small_font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(int(config.screen_height * 0.0463), font_height + 15)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 80
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_message):
        text = config.small_font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    button_width = min(160, (rect_width - 60) // 2)
    yes_x = rect_x + rect_width // 2 - button_width - 10
    no_x = rect_x + rect_width // 2 + 10
    buttons_y = rect_y + text_height + margin_top_bottom
    draw_stylized_button(screen, _("button_yes"), yes_x, buttons_y, button_width, button_height, selected=config.gamelist_update_selection == 1)
    draw_stylized_button(screen, _("button_no"), no_x, buttons_y, button_width, button_height, selected=config.gamelist_update_selection == 0)

def draw_platform_folder_config_dialog(screen):
    """Affiche le dialogue de configuration du dossier personnalisé pour une plateforme."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 150))

    screen.blit(core.OVERLAY, (0, 0))
    
    from rgsx_settings import get_platform_custom_path
    platform_name = getattr(config, 'platform_config_name', '')
    current_path = get_platform_custom_path(platform_name)
    
    # Message d'information
    if current_path:
        message = _("platform_folder_config_current").format(platform_name, current_path) if _ else f"Configure download folder for {platform_name}\nCurrent: {current_path}"
    else:
        message = _("platform_folder_config_default").format(platform_name) if _ else f"Configure download folder for {platform_name}\nUsing default location"
    
    # Traiter les sauts de ligne explicites, puis wrapper chaque partie
    wrapped_message = []
    for part in message.split('\n'):
        wrapped_message.extend(wrap_text(part, config.small_font, config.screen_width - 100))
    
    line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    
    # Options
    options = [
        _("platform_folder_show_current") if _ else "Show current path",
        _("platform_folder_browse") if _ else "Browse",
        _("platform_folder_reset") if _ else "Reset to default",
        _("web_cancel") if _ else "Cancel"
    ]
    
    sample_text = config.small_font.render("Sample", True, THEME_COLORS["text"])
    font_height = sample_text.get_height()
    button_height = max(int(config.screen_height * 0.0463), font_height + 15)
    margin_top_bottom = 20
    buttons_spacing = 10
    
    rect_height = text_height + len(options) * (button_height + buttons_spacing) + 2 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message], default=400)
    max_button_width = max([config.small_font.size(opt)[0] for opt in options], default=200) + 60  # Plus de marge pour les boutons
    rect_width = max(max_text_width + 80, max_button_width + 40, 550)  # Largeur minimale augmentée
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    # Afficher le message
    for i, line in enumerate(wrapped_message):
        text = config.small_font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    # Afficher les boutons
    button_width = min(max_button_width, rect_width - 60)
    buttons_start_y = rect_y + text_height + margin_top_bottom
    
    for i, option in enumerate(options):
        button_x = rect_x + (rect_width - button_width) // 2
        button_y = buttons_start_y + i * (button_height + buttons_spacing)
        selected = config.platform_folder_selection == i
        draw_stylized_button(screen, option, button_x, button_y, button_width, button_height, selected=selected)
