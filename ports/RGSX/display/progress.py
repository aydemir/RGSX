
"""progress module."""

import pygame  # type: ignore

import config

from language import _

from utils import (truncate_text_middle, wrap_text)

from .colors import THEME_COLORS
from .components import draw_stylized_button

from . import core
import logging
logger = logging.getLogger(__name__)
def draw_progress_screen(screen):
    """Affiche l'écran de progression des téléchargements avec un style moderne."""
    if not config.download_tasks:
        logger.debug("Aucune tâche de téléchargement active")
        return

    task = list(config.download_tasks.keys())[0]
    game_name = config.download_tasks[task][2]
    url = config.download_tasks[task][1]
    progress = config.download_progress.get(url, {"downloaded_size": 0, "total_size": 0, "status": "Téléchargement", "progress_percent": 0})
    status = progress.get("status", "Téléchargement")
    downloaded_size = progress["downloaded_size"]
    total_size = progress["total_size"]
    progress_percent = progress["progress_percent"]
    # S'assurer que le pourcentage est entre 0 et 100
    progress_percent = max(0, min(100, progress_percent))

    screen.blit(core.OVERLAY, (0, 0))

    title_text = _("download_status").format(status, truncate_text_middle(game_name, config.font, config.screen_width - 200))
    title_lines = wrap_text(title_text, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(title_lines) * line_height
    margin_top_bottom = 20
    bar_height = int(config.screen_height * 0.0278)
    percent_height = config.progress_font.get_height() + 5
    rect_height = text_height + bar_height + percent_height + 3 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in title_lines], default=300)
    bar_width = max_text_width
    rect_width = max_text_width + 80
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(title_lines):
        title_render = config.font.render(line, True, THEME_COLORS["text"])
        title_rect = title_render.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(title_render, title_rect)

    bar_y = rect_y + text_height + margin_top_bottom
    progress_width = 0
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x + 20, bar_y, bar_width, bar_height), border_radius=8)
    if total_size > 0:
        # Limiter le pourcentage entre 0 et 100 pour l'affichage de la barre
        progress_width = int(bar_width * (min(100, max(0, progress_percent)) / 100))

def draw_extension_warning(screen):
    """Affiche un avertissement pour une extension non reconnue ou un fichier ZIP."""
    if not config.pending_download:
        logger.error("config.pending_download est None ou vide dans extension_warning, retour anticipé")
        return
    
    url, platform, game_name, is_zip_non_supported = config.pending_download
    # Log réduit: pas de détail verbeux ici
    is_zip = is_zip_non_supported
    if not game_name:
        game_name = "Inconnu"
        logger.warning("game_name vide, utilisation de 'Inconnu'")

    if is_zip:
        core = _("extension_warning_zip").format(game_name)
        hint = ""
    else:
        # Ajout d'un indice pour activer le téléchargement des extensions inconnues
        try:
            hint = _("extension_warning_enable_unknown_hint")
        except Exception:
            hint = ""
        core = _("extension_warning_unsupported").format(game_name)

    # Nettoyer et préparer les lignes
    max_width = config.screen_width - 80
    core_lines = wrap_text(core, config.font, max_width)
    hint_text = (hint or "").replace("\n", " ").strip()
    hint_lines = wrap_text(hint_text, config.small_font, max_width) if hint_text else []

    try:
        line_height_core = config.font.get_height() + 5
        line_height_hint = config.small_font.get_height() + 4
        spacing_between = 6 if hint_lines else 0
        text_height = len(core_lines) * line_height_core + (spacing_between) + len(hint_lines) * line_height_hint
        button_height = int(config.screen_height * 0.0463)
        margin_top_bottom = 20
        rect_height = text_height + button_height + 2 * margin_top_bottom
        max_text_width = max(
            [config.font.size(l)[0] for l in core_lines] + ([config.small_font.size(l)[0] for l in hint_lines] if hint_lines else []),
            default=300,
        )
        rect_width = max_text_width + 80
        rect_x = (config.screen_width - rect_width) // 2
        rect_y = (config.screen_height - rect_height) // 2

        screen.blit(core.OVERLAY, (0, 0))
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
        pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

        # Lignes du cœur du message (orange)
        for i, line in enumerate(core_lines):
            text_surface = config.font.render(line, True, THEME_COLORS["warning_text"])
            text_rect = text_surface.get_rect(center=(
                config.screen_width // 2,
                rect_y + margin_top_bottom + i * line_height_core + line_height_core // 2,
            ))
            screen.blit(text_surface, text_rect)

        # Lignes d'indice (blanc/gris) si présentes
        if hint_lines:
            hint_start_y = rect_y + margin_top_bottom + len(core_lines) * line_height_core + spacing_between
            for j, hline in enumerate(hint_lines):
                hsurf = config.small_font.render(hline, True, THEME_COLORS["text"])
                hrect = hsurf.get_rect(center=(
                    config.screen_width // 2,
                    hint_start_y + j * line_height_hint + line_height_hint // 2,
                ))
                screen.blit(hsurf, hrect)

        draw_stylized_button(screen, _("button_yes"), rect_x + rect_width // 2 - 180, rect_y + text_height + margin_top_bottom, 160, button_height, selected=config.extension_confirm_selection == 0)
        draw_stylized_button(screen, _("button_no"), rect_x + rect_width // 2 + 20, rect_y + text_height + margin_top_bottom, 160, button_height, selected=config.extension_confirm_selection == 1)

    except Exception as e:
        logger.error(f"Erreur lors du rendu de extension_warning : {str(e)}")
        error_message = "Erreur d'affichage de l'avertissement."
        wrapped_error = wrap_text(error_message, config.font, config.screen_width - 80)
        line_height = config.font.get_height() + 5
        rect_height = len(wrapped_error) * line_height + 2 * 20
        max_text_width = max([config.font.size(line)[0] for line in wrapped_error], default=300)
        rect_width = max_text_width + 80
        rect_x = (config.screen_width - rect_width) // 2
        rect_y = (config.screen_height - rect_height) // 2

        screen.blit(core.OVERLAY, (0, 0))
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
        pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

        for i, line in enumerate(wrapped_error):
            error_surface = config.font.render(line, True, THEME_COLORS["error_text"])
            error_rect = error_surface.get_rect(center=(config.screen_width // 2, rect_y + 20 + i * line_height + line_height // 2))
            screen.blit(error_surface, error_rect)
