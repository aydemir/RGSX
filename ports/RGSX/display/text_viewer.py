
"""text_viewer module."""

import pygame  # type: ignore

import config

from utils import (wrap_text)

from .colors import THEME_COLORS

from . import core
def draw_text_file_viewer(screen):
    """Affiche le contenu d'un fichier texte avec défilement."""
    screen.blit(core.OVERLAY, (0, 0))
    
    # Récupérer les données du fichier texte
    content = getattr(config, 'text_file_content', '')
    filename = getattr(config, 'text_file_name', 'Unknown')
    scroll_offset = getattr(config, 'text_file_scroll_offset', 0)
    viewer_mode = getattr(config, 'text_file_mode', '')
    
    # Dimensions
    margin = 40
    header_height = 60
    controls_y = config.screen_height - int(config.screen_height * 0.037)
    bottom_margin = 10
    
    rect_width = config.screen_width - 2 * margin
    rect_height = controls_y - 2 * margin - bottom_margin
    rect_x = margin
    rect_y = margin
    
    content_area_y = rect_y + header_height
    content_area_height = rect_height - header_height - 20
    
    # Fond principal
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    # Titre/nom du fichier
    title_text = f"{filename}"
    title_surface = config.font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + 30))
    screen.blit(title_surface, title_rect)
    
    # Séparateur
    pygame.draw.line(screen, THEME_COLORS["border"], (rect_x + 20, content_area_y - 10), (rect_x + rect_width - 20, content_area_y - 10), 2)
    
    # Contenu du fichier
    if content:
        # Diviser le contenu en lignes et appliquer le word wrap
        original_lines = content.split('\n')
        wrapped_lines = []
        max_width = rect_width - 60
        
        # Appliquer wrap_text à chaque ligne originale
        for original_line in original_lines:
            if original_line.strip():  # Si la ligne n'est pas vide
                wrapped = wrap_text(original_line, config.small_font, max_width)
                wrapped_lines.extend(wrapped)
            else:  # Ligne vide
                wrapped_lines.append('')
        
        line_height = config.small_font.get_height() + 2
        
        # Calculer le nombre de lignes visibles
        visible_lines = int(content_area_height / line_height)
        
        # Appliquer le scroll
        start_line = scroll_offset
        end_line = min(start_line + visible_lines, len(wrapped_lines))
        
        # Afficher les lignes visibles
        for i, line_index in enumerate(range(start_line, end_line)):
            if line_index < len(wrapped_lines):
                line = wrapped_lines[line_index]
                line_surface = config.small_font.render(line, True, THEME_COLORS["text"])
                line_rect = line_surface.get_rect(left=rect_x + 30, top=content_area_y + i * line_height)
                screen.blit(line_surface, line_rect)
        
        # Scrollbar si nécessaire
        if len(wrapped_lines) > visible_lines:
            scrollbar_height = int((visible_lines / len(wrapped_lines)) * content_area_height)
            scrollbar_y = content_area_y + int((scroll_offset / len(wrapped_lines)) * content_area_height)
            scrollbar_x = rect_x + rect_width - 15
            
            # Fond de la scrollbar
            pygame.draw.rect(screen, THEME_COLORS["border"], (scrollbar_x, content_area_y, 8, content_area_height), border_radius=4)
            # Barre de défilement
            pygame.draw.rect(screen, THEME_COLORS["button_hover"], (scrollbar_x, scrollbar_y, 8, scrollbar_height), border_radius=4)
        
        # Indicateur de position
        position_text = f"{scroll_offset + 1}-{end_line}/{len(wrapped_lines)}"
        position_surface = config.small_font.render(position_text, True, THEME_COLORS["title_text"])
        position_rect = position_surface.get_rect(right=rect_x + rect_width - 30, bottom=rect_y + rect_height - 10)
        screen.blit(position_surface, position_rect)

        if viewer_mode == 'ota_update':
            hint_surface = config.small_font.render("Confirm: Update", True, THEME_COLORS["text_selected"])
            hint_rect = hint_surface.get_rect(left=rect_x + 30, bottom=rect_y + rect_height - 10)
            screen.blit(hint_surface, hint_rect)
    else:
        # Aucun contenu
        no_content_text = "Empty file"
        no_content_surface = config.font.render(no_content_text, True, THEME_COLORS["title_text"])
        no_content_rect = no_content_surface.get_rect(center=(config.screen_width // 2, content_area_y + content_area_height // 2))
        screen.blit(no_content_surface, no_content_rect)
