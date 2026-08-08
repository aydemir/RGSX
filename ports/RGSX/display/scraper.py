
"""scraper module."""

import pygame  # type: ignore

import config

from utils import (truncate_text_middle, wrap_text)

from .colors import THEME_COLORS

from . import core
def draw_scraper_screen(screen):
    screen.blit(core.OVERLAY, (0, 0))
    
    # Dimensions de l'écran avec marge pour les contrôles en bas
    margin = 40
    # Calcul exact de la position des contrôles (même formule que draw_controls)
    controls_y = config.screen_height - int(config.screen_height * 0.037)
    bottom_margin = 10
    
    rect_width = config.screen_width - 2 * margin
    rect_height = controls_y - 2 * margin - bottom_margin
    rect_x = margin
    rect_y = margin
    
    # Fond principal
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    # Titre
    title_text = f"Scraper: {config.scraper_game_name}"
    title_surface = config.title_font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + 40))
    screen.blit(title_surface, title_rect)
    
    # Sous-titre avec plateforme
    subtitle_text = f"Platform: {config.scraper_platform_name}"
    subtitle_surface = config.font.render(subtitle_text, True, THEME_COLORS["title_text"])
    subtitle_rect = subtitle_surface.get_rect(center=(config.screen_width // 2, rect_y + 80))
    screen.blit(subtitle_surface, subtitle_rect)
    
    # Zone de contenu (après titre et sous-titre)
    content_y = rect_y + 120
    content_height = rect_height - 140  # Ajusté pour ne pas inclure les marges du bas
    
    # Si chargement en cours
    if config.scraper_loading:
        loading_text = "Searching for metadata..."
        loading_surface = config.font.render(loading_text, True, THEME_COLORS["text"])
        loading_rect = loading_surface.get_rect(center=(config.screen_width // 2, config.screen_height // 2))
        screen.blit(loading_surface, loading_rect)
    
    # Si erreur
    elif config.scraper_error_message:
        error_lines = wrap_text(config.scraper_error_message, config.font, rect_width - 80)
        line_height = config.font.get_height() + 10
        start_y = config.screen_height // 2 - (len(error_lines) * line_height) // 2
        
        for i, line in enumerate(error_lines):
            error_surface = config.font.render(line, True, THEME_COLORS["error_text"])
            error_rect = error_surface.get_rect(center=(config.screen_width // 2, start_y + i * line_height))
            screen.blit(error_surface, error_rect)
    
    # Si données disponibles
    else:
        # Division en deux colonnes: image à gauche, métadonnées à droite
        left_width = int(rect_width * 0.4)
        right_width = rect_width - left_width - 20
        left_x = rect_x + 20
        right_x = left_x + left_width + 20
        
        # === COLONNE GAUCHE: IMAGE ===
        if config.scraper_image_surface:
            # Calculer la taille max pour l'image
            max_image_width = left_width - 20
            max_image_height = content_height - 20
            
            # Redimensionner l'image en conservant le ratio
            image = config.scraper_image_surface
            img_width, img_height = image.get_size() if image else (0, 0)
            
            # Calculer le ratio de redimensionnement
            width_ratio = max_image_width / img_width
            height_ratio = max_image_height / img_height
            scale_ratio = min(width_ratio, height_ratio, 1.0)
            
            new_width = int(img_width * scale_ratio)
            new_height = int(img_height * scale_ratio)
            
            # Redimensionner l'image
            scaled_image = pygame.transform.smoothscale(image, (new_width, new_height))
            
            # Centrer l'image dans la colonne gauche
            image_x = left_x + (left_width - new_width) // 2
            image_y = content_y + (content_height - new_height) // 2
            
            # Fond derrière l'image
            padding = 10
            bg_rect = pygame.Rect(image_x - padding, image_y - padding, new_width + 2 * padding, new_height + 2 * padding)
            pygame.draw.rect(screen, THEME_COLORS["fond_image"], bg_rect, border_radius=8)
            pygame.draw.rect(screen, THEME_COLORS["neon"], bg_rect, 2, border_radius=8)
            
            # Afficher l'image
            screen.blit(scaled_image, (image_x, image_y))
        else:
            # Pas d'image disponible
            no_image_text = "No image available"
            no_image_surface = config.font.render(no_image_text, True, THEME_COLORS["title_text"])
            no_image_rect = no_image_surface.get_rect(center=(left_x + left_width // 2, content_y + content_height // 2))
            screen.blit(no_image_surface, no_image_rect)
        
        # === COLONNE DROITE: METADONNEES (centrées verticalement) ===
        line_height = config.font.get_height() + 8
        small_line_height = config.small_font.get_height() + 5
        
        # Calculer la hauteur totale des métadonnées pour centrer verticalement
        total_metadata_height = 0
        
        # Compter les lignes de genre
        if config.scraper_genre:
            total_metadata_height += line_height * 2 + 10  # Label + valeur + espace
        
        # Compter les lignes de date
        if config.scraper_release_date:
            total_metadata_height += line_height * 2 + 10  # Label + valeur + espace
        
        # Compter les lignes de description
        if config.scraper_description:
            desc_lines = wrap_text(config.scraper_description, config.small_font, right_width - 100)
            max_desc_lines = min(len(desc_lines), int((content_height - total_metadata_height - 100) / small_line_height))
            total_metadata_height += line_height + 5  # Label + espace
            total_metadata_height += max_desc_lines * small_line_height
        
        # Calculer le Y de départ pour centrer verticalement
        metadata_y = content_y + (content_height - total_metadata_height) // 2
        
        # Genre
        if config.scraper_genre:
            genre_label = config.font.render("Genre:", True, THEME_COLORS["neon"])
            screen.blit(genre_label, (right_x, metadata_y))
            metadata_y += line_height
            
            genre_value = config.font.render(config.scraper_genre, True, THEME_COLORS["text"])
            screen.blit(genre_value, (right_x + 10, metadata_y))
            metadata_y += line_height + 10
        
        # Date de sortie
        if config.scraper_release_date:
            date_label = config.font.render("Release Date:", True, THEME_COLORS["neon"])
            screen.blit(date_label, (right_x, metadata_y))
            metadata_y += line_height
            
            date_value = config.font.render(config.scraper_release_date, True, THEME_COLORS["text"])
            screen.blit(date_value, (right_x + 10, metadata_y))
            metadata_y += line_height + 10
        
        # Description
        if config.scraper_description:
            desc_label = config.font.render("Description:", True, THEME_COLORS["neon"])
            screen.blit(desc_label, (right_x, metadata_y))
            metadata_y += line_height + 5
            
            # Wrapper la description avec plus de padding à droite
            desc_lines = wrap_text(config.scraper_description, config.small_font, right_width - 40)
            max_desc_lines = min(len(desc_lines), int((content_height - (metadata_y - content_y)) / small_line_height))
            
            for i, line in enumerate(desc_lines[:max_desc_lines]):
                desc_surface = config.small_font.render(line, True, THEME_COLORS["text"])
                screen.blit(desc_surface, (right_x + 10, metadata_y))
                metadata_y += small_line_height
            
            # Si trop de lignes, afficher "..."
            if len(desc_lines) > max_desc_lines:
                more_text = config.small_font.render("...", True, THEME_COLORS["title_text"])
                screen.blit(more_text, (right_x + 10, metadata_y))
        
        # URL de la source en bas (si disponible)
        if config.scraper_game_page_url:
            url_text = truncate_text_middle(config.scraper_game_page_url, config.small_font, rect_width - 80, is_filename=False)
            url_surface = config.small_font.render(url_text, True, THEME_COLORS["title_text"])
            url_rect = url_surface.get_rect(center=(config.screen_width // 2, rect_y + rect_height - 20))
            screen.blit(url_surface, url_rect)
