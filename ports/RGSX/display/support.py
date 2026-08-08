
"""support module."""

import pygame  # type: ignore

import config

from language import _

from utils import (wrap_text)

from .colors import THEME_COLORS
from .controls import get_control_display

from . import core
import logging
logger = logging.getLogger(__name__)
def draw_support_dialog(screen):
    """Affiche la boîte de dialogue du fichier de support généré."""

    if core.OVERLAY is None or core.OVERLAY.get_size() != (config.screen_width, config.screen_height):
        core.OVERLAY = pygame.Surface((config.screen_width, config.screen_height), pygame.SRCALPHA)
        core.OVERLAY.fill((0, 0, 0, 150))
        logger.debug("core.OVERLAY recréé dans draw_support_dialog")

    screen.blit(core.OVERLAY, (0, 0))
    
    # Cet écran se ferme via l'action Start dans la navigation actuelle.
    return_key = get_control_display("start", "Start")
    
    # Déterminer le message à afficher (succès ou erreur)
    if hasattr(config, 'support_zip_error') and config.support_zip_error:
        title = _("support_dialog_title")
        message = _("support_dialog_error").format(config.support_zip_error, return_key)
    else:
        title = _("support_dialog_title")
        zip_path = getattr(config, 'support_zip_path', 'rgsx_support.zip')
        message = _("support_dialog_message").format(zip_path, return_key)
    
    # Diviser le message par les retours à la ligne puis wrapper chaque segment
    raw_segments = message.split('\n') if message else []
    wrapped_message = []
    for seg in raw_segments:
        if seg.strip() == "":
            wrapped_message.append("")  # Ligne vide pour espacement
        else:
            wrapped_message.extend(wrap_text(seg, config.small_font, config.screen_width - 100))
    
    line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    
    # Calculer la hauteur du titre
    title_height = config.font.get_height() + 10
    
    # Calculer les dimensions de la boîte
    margin_top_bottom = 20
    rect_height = title_height + text_height + 2 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_message if line], default=300)
    title_width = config.font.size(title)[0]
    rect_width = max(max_text_width, title_width) + 100
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    # Dessiner la boîte
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    # Afficher le titre
    title_surf = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surf.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + title_height // 2))
    screen.blit(title_surf, title_rect)

    # Afficher le message
    for i, line in enumerate(wrapped_message):
        if line:  # Ne pas rendre les lignes vides
            text = config.small_font.render(line, True, THEME_COLORS["text"])
            text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + title_height + i * line_height + line_height // 2))
            screen.blit(text, text_rect)
