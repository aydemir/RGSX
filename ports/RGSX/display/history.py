
"""history module."""

import os
import pygame  # type: ignore

import config

from history import load_history, _strip_history_error_noise

from language import _, get_size_units

from utils import (truncate_text_middle, wrap_text, truncate_text_end, _get_dest_folder_name, find_file_with_or_without_extension, find_matching_files, get_clean_display_name, get_existing_history_matches, remember_history_local_match)

from .colors import THEME_COLORS
from .components import draw_stylized_button
from .game_list import get_display_extension

from . import core
import logging
logger = logging.getLogger(__name__)


def draw_history_list(screen):
    # logger.debug(f"Dessin historique, history={config.history}, needs_redraw={config.needs_redraw}")
    history = config.history if hasattr(config, 'history') else load_history()
    history_count = len(history)
    
    # Inverser l'historique pour afficher les plus récents en premier
    # Convertir l'index sélectionné de l'original au tableau inversé
    original_index = config.current_history_item
    history = list(reversed(history))
    
    # Calcul de l'index dans la liste inversée
    # Si original_index=0 (premier), devient len-1 (dernier dans la liste inversée)
    # Si original_index=len-1 (dernier), devient 0 (premier dans la liste inversée)
    if history_count > 0 and original_index >= 0 and original_index < history_count:
        current_history_item_inverted = history_count - 1 - original_index
    else:
        current_history_item_inverted = 0

    active_statuses = {"Téléchargement", "Downloading", "Extracting", "Converting", "Connecting", "Queued", "Paused"}
    completed_statuses = {"Download_OK", "Completed"}
    error_statuses = {"Erreur", "Error"}
    canceled_statuses = {"Canceled", "Cancelled", "Annulé", "Annule"}

    selected_entry = history[current_history_item_inverted] if history and 0 <= current_history_item_inverted < len(history) else None
    selected_status = str((selected_entry or {}).get("status") or "")

    active_download_entry = None
    for entry in history:
        entry_status = str(entry.get("status") or "")
        if entry_status in active_statuses:
            active_download_entry = entry
            break

    # La barre de titre doit refléter l'élément actuellement sélectionné dans la liste
    # d'historique (navigation utilisateur), pas systématiquement le téléchargement actif
    # en arrière-plan si l'utilisateur regarde une autre entrée. On ne se rabat sur le
    # téléchargement actif que si rien n'est sélectionné (ex: historique vide).
    display_entry = selected_entry if selected_entry is not None else active_download_entry
    display_status = str((display_entry or {}).get("status") or "")

    if display_entry and display_status in active_statuses:
        downloaded_size = int(display_entry.get("downloaded_size", 0) or 0)
        total_size_val = int(display_entry.get("total_size", 0) or 0)
        size_text = f"{format_size(downloaded_size)} / {format_size(total_size_val)}" if total_size_val > 0 else format_size(downloaded_size)
        try:
            selected_speed = float(display_entry.get("speed", 0.0) or 0.0)
        except Exception:
            selected_speed = 0.0
        speed_text = format_speed_adaptive(selected_speed)
        title_text = _("history_title_downloading_active").format(size_text, speed_text)
        # SD/CN (seeds/connexions) n'a de sens que pour les téléchargements torrent.
        is_torrent_entry = str(display_entry.get("url") or "").startswith("rgsx+torrent://")
        if is_torrent_entry:
            # Afficher SD/CN dans le titre
            progress_entry = None
            entry_url = str(display_entry.get("url") or "")
            if entry_url and entry_url in config.download_progress:
                progress_entry = config.download_progress[entry_url]
            if progress_entry is not None:
                _sd = int(progress_entry.get("seeds", display_entry.get("seeds", 0) or 0) or 0)
                _cn = int(progress_entry.get("connections", display_entry.get("connections", 0) or 0) or 0)
                downloaded_size = int(progress_entry.get("downloaded_size", display_entry.get("downloaded_size", 0) or 0) or 0)
                total_size_val = int(progress_entry.get("total_size", display_entry.get("total_size", 0) or 0) or 0)
                size_text = f"{format_size(downloaded_size)} / {format_size(total_size_val)}" if total_size_val > 0 else format_size(downloaded_size)
                title_text = _("history_title_downloading_active").format(size_text, speed_text)
            else:
                _sd = int(display_entry.get("seeds", 0) or 0)
                _cn = int(display_entry.get("connections", 0) or 0)
            title_text = f"{title_text}  [{_sd}SD/{_cn}CN]"
        # Afficher l'étape torrent courante dans le titre (connecting / verifying / waiting).
        # On ne montre rien quand on télécharge activement (speed > 0) car l'info de vitesse suffit.
        _aria2_phase = str(display_entry.get("aria2_phase") or "")
        _phase_labels = {
            "connecting": _("aria2_phase_connecting"),
            "verifying":  _("aria2_phase_verifying"),
            "waiting":    _("aria2_phase_waiting"),
            "paused":     _("aria2_phase_paused"),
        }
        _phase_label = _phase_labels.get(_aria2_phase, "")
        if _phase_label:
            title_text = f"{title_text}  [{_phase_label}]"
    elif display_entry and display_status == "Seeding":
        _cn = int(display_entry.get("seeds", 0) or 0)
        _ul = float(display_entry.get("ul_speed", 0.0) or 0.0)
        _ul_text = format_speed_adaptive(_ul)
        title_text = f"Seeding - {_ul_text} - [{_cn}p]"
    elif display_entry and display_status in completed_statuses:
        completed_count = sum(1 for item in history if str(item.get("status") or "") in completed_statuses)
        title_text = _("history_title_completed_count").format(completed_count)
    elif selected_entry and selected_status in error_statuses:
        error_count = sum(1 for item in history if str(item.get("status") or "") in error_statuses)
        title_text = _("history_title_error_count").format(error_count)
    elif selected_entry and selected_status in canceled_statuses:
        canceled_count = sum(1 for item in history if str(item.get("status") or "") in canceled_statuses)
        title_text = _("history_title_canceled_count").format(canceled_count)
    else:
        title_text = _("history_title").format(history_count)

    screen.blit(core.OVERLAY, (0, 0))
    title_surface = config.title_font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, title_surface.get_height() // 2 + 20))
    title_rect_inflated = title_rect.inflate(60, 30)
    title_rect_inflated.topleft = ((config.screen_width - title_rect_inflated.width) // 2, 10)
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)  # fond opaque
    pygame.draw.rect(screen, THEME_COLORS["border"], title_rect_inflated, 2, border_radius=12)
    screen.blit(title_surface, title_rect)

    # Prioritize the game title by shrinking size/status columns.
    column_width_percentages = {
        "platform": 0.13,
        "game_name": 0.40,
        "ext": 0.07,
        "folder": 0.16,
        "size": 0.06,
        "status": 0.18
    }
    available_width = int(0.95 * config.screen_width - 60)  # Total available width for columns
    col_platform_width = int(available_width * column_width_percentages["platform"])
    col_game_width = int(available_width * column_width_percentages["game_name"])
    col_ext_width = int(available_width * column_width_percentages["ext"])
    col_folder_width = int(available_width * column_width_percentages["folder"])
    col_size_width = int(available_width * column_width_percentages["size"])
    col_status_width = int(available_width * column_width_percentages["status"])
    rect_width = int(0.95 * config.screen_width)

    line_height = config.small_font.get_height() + 10
    header_height = line_height
    margin_top_bottom = 20
    extra_margin_top = 40
    extra_margin_bottom = 80
    title_height = config.title_font.get_height() + 20

    # Sécuriser current_history_item_inverted pour éviter IndexError
    if history:
        if current_history_item_inverted < 0 or current_history_item_inverted >= len(history):
            current_history_item_inverted = max(0, min(len(history) - 1, current_history_item_inverted))
    else:
        current_history_item_inverted = 0

    if not history:
        logger.debug("Aucun historique disponible")
        message = _("history_empty")
        lines = wrap_text(message, config.font, config.screen_width - 80)
        line_height = config.font.get_height() + 5
        text_height = len(lines) * line_height
        rect_height = text_height + 2 * margin_top_bottom
        max_text_width = max([config.font.size(line)[0] for line in lines], default=300)
        rect_width = max_text_width + 80
        rect_x = (config.screen_width - rect_width) // 2
        rect_y = (config.screen_height - rect_height) // 2

        screen.blit(core.OVERLAY, (0, 0))
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
        pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

        for i, line in enumerate(lines):
            text_surface = config.font.render(line, True, THEME_COLORS["text"])
            text_rect = text_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
            screen.blit(text_surface, text_rect)
        return

    # Espace visible garanti entre le titre et la liste, et au-dessus du footer
    top_gap = 20
    bottom_reserved = 70  # réserve pour le footer (barre des contrôles) + marge visuelle (réduit)

    # Positionner la liste juste après le titre, avec un espace dédié
    # Utiliser le rectangle du titre déjà dessiné pour une meilleure précision
    title_bottom = title_rect_inflated.bottom
    rect_y = title_bottom + top_gap

    # Calculer l'espace disponible en bas en réservant une zone pour le footer
    available_height = max(0, config.screen_height - rect_y - bottom_reserved)
    # Déterminer le nombre d'éléments par page en tenant compte de l'en-tête et des marges internes
    items_per_page = max(1, (available_height - header_height - 2 * margin_top_bottom) // line_height)

    rect_height = header_height + items_per_page * line_height + 2 * margin_top_bottom
    rect_x = (config.screen_width - rect_width) // 2

    config.history_scroll_offset = max(0, min(config.history_scroll_offset, max(0, len(history) - items_per_page)))
    if current_history_item_inverted < config.history_scroll_offset:
        config.history_scroll_offset = current_history_item_inverted
    elif current_history_item_inverted >= config.history_scroll_offset + items_per_page:
        config.history_scroll_offset = current_history_item_inverted - items_per_page + 1


    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    headers = [_("history_column_system"), _("history_column_game"), _("game_header_ext"), _("history_column_folder"), _("history_column_size"), _("history_column_status")]
    header_y = rect_y + margin_top_bottom + header_height // 2
    header_x_positions = [
        rect_x + 20 + col_platform_width // 2,
        rect_x + 20 + col_platform_width + col_game_width // 2,
        rect_x + 20 + col_platform_width + col_game_width + col_ext_width // 2,
        rect_x + 20 + col_platform_width + col_game_width + col_ext_width + col_folder_width // 2,
        rect_x + 20 + col_platform_width + col_game_width + col_ext_width + col_folder_width + col_size_width // 2,
        rect_x + 20 + col_platform_width + col_game_width + col_ext_width + col_folder_width + col_size_width + col_status_width // 2
    ]
    for header, x_pos in zip(headers, header_x_positions):
        text_surface = config.small_font.render(header, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=(x_pos, header_y))
        screen.blit(text_surface, text_rect)

    separator_y = rect_y + margin_top_bottom + header_height
    pygame.draw.line(screen, THEME_COLORS["border"], (rect_x + 20, separator_y), (rect_x + rect_width - 20, separator_y), 2)

    for idx, i in enumerate(range(config.history_scroll_offset, min(config.history_scroll_offset + items_per_page, len(history)))):
        entry = history[i]
        platform = entry.get("platform", "Inconnu")
        raw_game_name = entry.get("game_name", "Inconnu")
        game_name = entry.get("display_name") or get_clean_display_name(raw_game_name, platform)
        ext_text = get_display_extension(raw_game_name)
        folder_text = _get_dest_folder_name(platform)
        
        # Correction du calcul de la taille
        status = entry.get("status", "Inconnu")
        progress = entry.get("progress", 0)
        progress = max(0, min(100, progress))  # Clamp progress between 0 and 100

        size = entry.get("total_size", 0)
        if (not size or int(size or 0) <= 0) and status in ["Téléchargement", "Downloading"]:
            size = entry.get("downloaded_size", 0)
        color = THEME_COLORS["fond_lignes"] if i == current_history_item_inverted else THEME_COLORS["text"]
        size_text = format_size(size)

        # Precompute provider prefix once
        provider_prefix = entry.get("provider_prefix") or (entry.get("provider") + ":" if entry.get("provider") else "")
        
        # Compute status text (optimized version without redundant prefix for errors)
        if status in ["Téléchargement", "Downloading"]:
            # Vérifier si un message personnalisé existe (ex: mode gratuit avec attente)
            custom_message = entry.get('message', '')
            total_size_value = int(entry.get("total_size", 0) or 0)
            downloaded_size_value = int(entry.get("downloaded_size", 0) or 0)
            seeds_value = int(entry.get("seeds", 0) or 0)
            connections_value = int(entry.get("connections", 0) or 0)
            # Détecter les messages du mode gratuit (commencent par '[' dans toutes les langues)
            if custom_message and custom_message.strip().startswith('['):
                # Utiliser le message personnalisé pour le mode gratuit
                status_text = custom_message
            elif total_size_value <= 0 and downloaded_size_value > 0:
                status_text = str(status)
            else:
                # Comportement normal: afficher le pourcentage
                display_progress = "<1" if (progress <= 0 and total_size_value > 0 and downloaded_size_value > 0) else progress
                status_text = _("history_status_downloading").format(display_progress)
                # SD/CN sont maintenant affichés dans le titre, pas ici
                # Coerce to string and prefix provider when relevant
                status_text = str(status_text or "")
                if provider_prefix and not status_text.startswith(provider_prefix):
                    status_text = f"{provider_prefix} {status_text}"
        elif status == "Extracting":
            status_text = _("history_status_extracting").format(progress)
            status_text = str(status_text or "")
            if provider_prefix and not status_text.startswith(provider_prefix):
                status_text = f"{provider_prefix} {status_text}"
        elif status == "Download_OK":
            # Completed: no provider prefix (per requirement)
            status_text = _("history_status_completed")
            status_text = str(status_text or "")
        elif status == "Seeding":
            _cn = int(entry.get("seeds", 0) or 0)
            status_text = _("history_status_seeding").format(_cn)
            status_text = str(status_text or "")
        elif status == "Erreur":
            # Prefer friendly mapped message now stored in 'message'
            status_text = entry.get('message')
            if not status_text:
                # Some legacy entries might have only raw in result[1] or auxiliary field
                status_text = entry.get('raw_error_realdebrid') or entry.get('error') or 'Échec'
            # Coerce to string early for safe operations
            status_text = str(status_text or "")
            # Strip redundant prefixes if any
            for prefix in ["Erreur :", "Erreur:", "Error:", "Error :"]:
                if status_text.startswith(prefix):
                    status_text = status_text[len(prefix):].strip()
                    break
            # Durum sütunu için kısa tut: "Download error {game}:" önekini çıkar
            # (oyun adı zaten ayrı sütunda görünür) ve uzun dosya listesi bloklarını at.
            status_text = _strip_history_error_noise(status_text)
            if provider_prefix and not status_text.startswith(provider_prefix):
                status_text = f"{provider_prefix} {status_text}"
        elif status == "Canceled":
            status_text = _("history_status_canceled")
            status_text = str(status_text or "")
        else:
            status_text = str(status or "")

        # Determine color dedicated to status (independent from selection for better readability)
        if status == "Erreur" or status == "Error":
            status_color = THEME_COLORS.get("error_text", (255, 0, 0))
        elif status == "Canceled":
            status_color = THEME_COLORS.get("warning_text", (255, 100, 0))
        elif status == "Download_OK" or status == "Completed":
            # Use green OK color
            status_color = THEME_COLORS.get("success_text", (0, 255, 0))
        elif status == "Seeding":
            # Seeding : couleur verte légèrement différente
            status_color = THEME_COLORS.get("success_text", (0, 220, 120))
        elif status in ("Downloading", "Téléchargement", "downloading", "Extracting", "Converting", "Queued", "Connecting"):
            # En cours - couleur bleue/cyan pour différencier des autres
            status_color = THEME_COLORS.get("text_selected", (100, 180, 255))
        else:
            status_color = THEME_COLORS.get("text", (255, 255, 255))

        platform_text = truncate_text_end(platform, config.small_font, col_platform_width - 10)
        game_text = truncate_text_middle(str(game_name), config.small_font, col_game_width - 10, is_filename=False)
        ext_text = truncate_text_end(ext_text, config.small_font, col_ext_width - 10)
        folder_text = truncate_text_end(folder_text, config.small_font, col_folder_width - 10)
        size_text = truncate_text_end(size_text, config.small_font, col_size_width - 10)
        status_text = truncate_text_middle(str(status_text or ""), config.small_font, col_status_width - 10, is_filename=False)

        y_pos = rect_y + margin_top_bottom + header_height + idx * line_height + line_height // 2
        platform_surface = config.small_font.render(platform_text, True, color)
        game_surface = config.small_font.render(game_text, True, color)
        ext_surface = config.small_font.render(ext_text, True, color)
        folder_surface = config.small_font.render(folder_text, True, color)
        size_surface = config.small_font.render(size_text, True, color)  # Correction ici
        status_surface = config.small_font.render(status_text, True, status_color)

        platform_rect = platform_surface.get_rect(center=(header_x_positions[0], y_pos))
        game_rect = game_surface.get_rect(center=(header_x_positions[1], y_pos))
        ext_rect = ext_surface.get_rect(center=(header_x_positions[2], y_pos))
        folder_rect = folder_surface.get_rect(center=(header_x_positions[3], y_pos))
        size_rect = size_surface.get_rect(center=(header_x_positions[4], y_pos))
        status_rect = status_surface.get_rect(center=(header_x_positions[5], y_pos))

        if i == current_history_item_inverted:
            glow_surface = pygame.Surface((rect_width - 40, line_height), pygame.SRCALPHA)
            pygame.draw.rect(glow_surface, THEME_COLORS["fond_lignes"] + (50,), (0, 0, rect_width - 40, line_height), border_radius=8)
            screen.blit(glow_surface, (rect_x + 20, y_pos - line_height // 2))

        screen.blit(platform_surface, platform_rect)
        screen.blit(game_surface, game_rect)
        screen.blit(ext_surface, ext_rect)
        screen.blit(folder_surface, folder_rect)
        screen.blit(size_surface, size_rect)
        screen.blit(status_surface, status_rect)

    if len(history) > items_per_page:
        try:
            draw_history_scrollbar(
                screen,
                config.history_scroll_offset,
                len(history),
                items_per_page,
                rect_x + rect_width - 10,
                rect_y,
                rect_height
            )
        except NameError as e:
            logger.error(f"Erreur : draw_history_scrollbar non défini: {str(e)}")

def draw_history_scrollbar(screen, scroll_offset, total_items, visible_items, x, y, height):
    """Affiche la barre de défilement avec un style moderne."""
    if total_items <= visible_items:
        return
    game_area_height = height
    scrollbar_height = game_area_height * (visible_items / total_items) - 10
    scrollbar_y = y + (game_area_height - scrollbar_height) * (scroll_offset / max(1, total_items - visible_items)) + 10
    pygame.draw.rect(screen, THEME_COLORS["fond_lignes"], (x, scrollbar_y, 5, scrollbar_height), border_radius=4)

def draw_clear_history_dialog(screen):
    """Affiche la boîte de dialogue de confirmation pour vider l'historique."""
    screen.blit(core.OVERLAY, (0, 0))

    message = _("confirm_clear_history")
    wrapped_message = wrap_text(message, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 150
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_message):
        text = config.font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    button_width = min(160, (rect_width - 60) // 2)
    draw_stylized_button(screen, _("button_yes"), rect_x + rect_width // 2 - button_width - 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=config.confirm_clear_selection == 1)
    draw_stylized_button(screen, _("button_no"), rect_x + rect_width // 2 + 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=config.confirm_clear_selection == 0)

def draw_cancel_download_dialog(screen):
    """Affiche la boîte de dialogue de confirmation pour annuler un téléchargement."""
    screen.blit(core.OVERLAY, (0, 0))

    message = _("confirm_cancel_download")
    wrapped_message = wrap_text(message, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 150
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    for i, line in enumerate(wrapped_message):
        text = config.font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)

    button_width = min(160, (rect_width - 60) // 2)
    draw_stylized_button(screen, _("button_yes"), rect_x + rect_width // 2 - button_width - 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=config.confirm_cancel_selection == 1)
    draw_stylized_button(screen, _("button_no"), rect_x + rect_width // 2 + 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=config.confirm_cancel_selection == 0)

def draw_history_game_options(screen):
    """Affiche le menu d'options pour un jeu de l'historique."""
    
    screen.blit(core.OVERLAY, (0, 0))
    
    if not config.history or config.current_history_item >= len(config.history):
        return
    
    entry = config.history[config.current_history_item]
    status = entry.get("status", "")
    game_name = entry.get("game_name", "Unknown")
    platform = entry.get("platform", "Unknown")
    
    # Vérifier l'existence du fichier (avec ou sans extension)
    dest_folder = _get_dest_folder_name(platform)
    base_path = os.path.join(config.ROMS_FOLDER, dest_folder)
    file_exists, actual_filename, actual_path = find_file_with_or_without_extension(base_path, game_name)
    actual_matches = find_matching_files(base_path, game_name)
    local_path = entry.get("local_path")
    local_filename = entry.get("local_filename")
    if not file_exists and local_path and os.path.isfile(local_path):
        actual_filename = os.path.basename(local_path)
        actual_path = local_path
        file_exists = True
        actual_matches = [(actual_filename, actual_path)]
        logger.debug("[HISTORY_OPTIONS_RENDER] direct local_path match used: %s", actual_path)
    elif not file_exists and local_filename:
        local_filename_path = os.path.join(base_path, str(local_filename))
        if os.path.isfile(local_filename_path):
            actual_filename = os.path.basename(local_filename_path)
            actual_path = local_filename_path
            file_exists = True
            actual_matches = [(actual_filename, actual_path)]
            logger.debug("[HISTORY_OPTIONS_RENDER] direct local_filename match used: %s", actual_path)
    if not actual_matches:
        actual_matches = get_existing_history_matches(entry)
        if actual_matches:
            actual_filename, actual_path = actual_matches[0]
            file_exists = True
    if file_exists and actual_path:
        remember_history_local_match(entry, actual_filename, actual_path)
    
    # Déterminer les options disponibles selon le statut
    options = []
    option_labels = []
    
    # Options communes

    options.append("scraper")
    option_labels.append(_("history_option_scraper"))
 
    # Options selon statut
    if status == "Queued":
        # En attente dans la queue
        options.append("force_download")
        option_labels.append(_("history_option_force_download"))
        options.append("remove_from_queue")
        option_labels.append(_("history_option_remove_from_queue"))
    elif status in ["Downloading", "Téléchargement", "Extracting", "Paused"]:
        # Téléchargement en cours ou en pause
        options.append("pause_resume_download")
        # Afficher le bon label selon l'état actuel
        if status == "Paused":
            option_labels.append(_("history_option_resume_download"))
        else:
            option_labels.append(_("history_option_pause_download"))
        options.append("cancel_download")
        option_labels.append(_("history_option_cancel_download"))
    elif status == "Seeding":
        options.append("cancel_download")
        option_labels.append(_("history_option_stop_seeding"))
        # Vérifier si c'est une archive ET si le fichier existe
        if actual_filename and file_exists:
            ext = os.path.splitext(actual_filename)[1].lower()
            if ext in ['.zip', '.rar', '.7z']:
                options.append("extract_archive")
                option_labels.append(_("history_option_extract_archive"))
            elif ext == '.txt':
                options.append("open_file")
                option_labels.append(_("history_option_open_file"))
    elif status == "Download_OK" or status == "Completed":
        # Vérifier si c'est une archive ET si le fichier existe
        if actual_filename and file_exists:
            ext = os.path.splitext(actual_filename)[1].lower()
            if ext in ['.zip', '.rar', '.7z']:
                options.append("extract_archive")
                option_labels.append(_("history_option_extract_archive"))
            elif ext == '.txt':
                options.append("open_file")
                option_labels.append(_("history_option_open_file"))
    elif status in ["Erreur", "Error", "Canceled"]:
        options.append("error_info")
        option_labels.append(_("history_option_error_info"))
        options.append("retry")
        option_labels.append(_("history_option_retry"))

    # Options communes
    if file_exists:
        options.append("download_folder")
        option_labels.append(_("history_option_download_folder"))
        options.append("delete_game")
        option_labels.append(_("history_option_delete_game"))
    options.append("back")
    option_labels.append(_("history_option_back"))

    diagnostics_signature = (
        entry.get("url", ""),
        status,
        file_exists,
        actual_filename or "",
        actual_path or "",
        tuple(options),
    )
    if getattr(config, 'history_options_render_signature', None) != diagnostics_signature:
        config.history_options_render_signature = diagnostics_signature
        logger.debug(
            "[HISTORY_OPTIONS_RENDER] platform=%s game=%s status=%s dest_folder=%s base_path=%s file_exists=%s actual_filename=%s actual_path=%s local_path=%s moved_paths=%s options=%s",
            platform,
            game_name,
            status,
            dest_folder,
            base_path,
            file_exists,
            actual_filename,
            actual_path,
            entry.get("local_path"),
            entry.get("moved_paths"),
            options,
        )
    
    # Calculer dimensions
    title = _("history_game_options_title")
    line_height = config.font.get_height() + 10
    margin_top_bottom = 30
    margin_sides = 40
    
    # Hauteur pour titre + options
    total_height = margin_top_bottom * 2 + line_height + len(option_labels) * line_height
    max_width = max(
        config.font.size(title)[0],
        max([config.font.size(label)[0] for label in option_labels], default=300)
    ) + margin_sides * 2
    
    rect_width = min(max_width + 100, config.screen_width - 100)
    rect_height = total_height
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    # Fond
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    # Titre
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom))
    screen.blit(title_surface, title_rect)
    
    # Options
    sel = getattr(config, 'history_game_option_selection', 0)
    for i, label in enumerate(option_labels):
        y_pos = rect_y + margin_top_bottom + line_height + i * line_height
        
        if i == sel:
            # Option sélectionnée
            highlight_rect = pygame.Rect(rect_x + 20, y_pos - 5, rect_width - 40, line_height)
            pygame.draw.rect(screen, THEME_COLORS["button_hover"], highlight_rect, border_radius=8)
            text_color = THEME_COLORS["text_selected"]
        else:
            text_color = THEME_COLORS["text"]
        
        text_surface = config.font.render(label, True, text_color)
        text_rect = text_surface.get_rect(left=rect_x + margin_sides, centery=y_pos + line_height // 2 - 5)
        screen.blit(text_surface, text_rect)

def draw_history_show_folder(screen):
    """Affiche le chemin complet du fichier téléchargé."""
    
    screen.blit(core.OVERLAY, (0, 0))
    
    if not config.history or config.current_history_item >= len(config.history):
        return
    
    entry = config.history[config.current_history_item]
    game_name = entry.get("game_name", "Unknown")
    platform = entry.get("platform", "Unknown")
    
    # Utiliser le chemin réel trouvé (avec ou sans extension)
    actual_path = getattr(config, 'history_actual_path', None)
    actual_filename = getattr(config, 'history_actual_filename', None)
    actual_matches = getattr(config, 'history_actual_matches', None) or []
    
    if not actual_path or not actual_filename:
        # Fallback si pas trouvé
        dest_folder = _get_dest_folder_name(platform)
        actual_path = os.path.join(config.ROMS_FOLDER, dest_folder, game_name)
        actual_filename = game_name
    
    # Vérifier si le fichier existe
    file_exists = bool(actual_matches) or os.path.exists(actual_path)
    
    # Message
    title = _("history_folder_path_label") if _ else "Destination path:"
    
    # Calculer dimensions d'abord pour avoir la largeur correcte
    line_height = config.font.get_height() + 10
    small_line_height = config.small_font.get_height() + 5
    margin_top_bottom = 30
    rect_width = min(config.screen_width - 100, 800)
    
    # Wrapper les chemins avec la bonne largeur (largeur de la boîte - marges)
    if actual_matches:
        path_wrapped = []
        for index, (match_filename, match_path) in enumerate(actual_matches, start=1):
            wrapped_match = wrap_text(match_path, config.small_font, rect_width - 80)
            if wrapped_match:
                path_wrapped.append(f"{index}. {wrapped_match[0]}")
                path_wrapped.extend(wrapped_match[1:])
            else:
                path_wrapped.append(f"{index}. {match_path}")
    else:
        path_wrapped = wrap_text(actual_path, config.small_font, rect_width - 80)
    
    # Ajouter un message si le fichier n'existe pas
    warning_lines = []
    if not file_exists:
        warning_text = "⚠️ " + (_("history_file_not_found") if _ else "File not found")
        warning_lines = wrap_text(warning_text, config.small_font, rect_width - 80)
    
    total_height = margin_top_bottom * 2 + line_height + len(path_wrapped) * small_line_height + len(warning_lines) * small_line_height + 60
    rect_height = total_height
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    # Fond
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    # Titre
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom))
    screen.blit(title_surface, title_rect)
    
    # Chemin
    current_y = rect_y + margin_top_bottom + line_height + 10
    for i, line in enumerate(path_wrapped):
        color = THEME_COLORS["text_selected"] if file_exists else THEME_COLORS["error_text"]
        path_surface = config.small_font.render(line, True, color)
        path_rect = path_surface.get_rect(left=rect_x + 40, top=current_y + i * small_line_height)
        screen.blit(path_surface, path_rect)
    
    # Avertissement si fichier non trouvé
    if warning_lines:
        current_y += len(path_wrapped) * small_line_height + 10
        for i, line in enumerate(warning_lines):
            warning_surface = config.small_font.render(line, True, THEME_COLORS["error_text"])
            warning_rect = warning_surface.get_rect(left=rect_x + 40, top=current_y + i * small_line_height)
            screen.blit(warning_surface, warning_rect)
    
    # Bouton OK
    button_height = int(config.screen_height * 0.0463)
    button_width = 120
    draw_stylized_button(screen, _("button_OK"), rect_x + (rect_width - button_width) // 2, rect_y + rect_height - button_height - 20, button_width, button_height, selected=True)

def draw_history_scraper_info(screen):
    """Affiche l'information que le scraper n'est pas implémenté."""
    screen.blit(core.OVERLAY, (0, 0))
    
    message = _("history_scraper_not_implemented")
    wrapped_message = wrap_text(message, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 150
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    for i, line in enumerate(wrapped_message):
        text = config.font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)
    
    button_width = 120
    draw_stylized_button(screen, _("button_OK"), rect_x + (rect_width - button_width) // 2, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=True)

def draw_history_error_details(screen):
    """Affiche les détails de l'erreur du téléchargement."""
    screen.blit(core.OVERLAY, (0, 0))
    
    if not config.history or config.current_history_item >= len(config.history):
        return
    
    entry = config.history[config.current_history_item]
    error_message = entry.get("message", _("history_no_error_message"))
    
    title = _("history_error_details_title")
    wrapped_error = wrap_text(error_message, config.small_font, config.screen_width - 120)
    
    line_height = config.font.get_height() + 10
    small_line_height = config.small_font.get_height() + 5
    text_height = len(wrapped_error) * small_line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 30
    rect_height = text_height + button_height + line_height + 3 * margin_top_bottom
    max_text_width = max([config.small_font.size(line)[0] for line in wrapped_error], default=300)
    rect_width = min(max_text_width + 150, config.screen_width - 100)
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    # Titre
    title_surface = config.font.render(title, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom))
    screen.blit(title_surface, title_rect)
    
    # Message d'erreur
    for i, line in enumerate(wrapped_error):
        text = config.small_font.render(line, True, THEME_COLORS["text_selected"])
        text_rect = text.get_rect(left=rect_x + 40, top=rect_y + margin_top_bottom + line_height + 10 + i * small_line_height)
        screen.blit(text, text_rect)
    
    button_width = 120
    draw_stylized_button(screen, _("button_OK"), rect_x + (rect_width - button_width) // 2, rect_y + rect_height - button_height - 20, button_width, button_height, selected=True)

def draw_history_confirm_delete(screen):
    """Affiche la confirmation de suppression d'un jeu."""
    screen.blit(core.OVERLAY, (0, 0))
    
    message = _("history_confirm_delete")
    wrapped_message = wrap_text(message, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 150
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    for i, line in enumerate(wrapped_message):
        text = config.font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)
    
    button_width = min(160, (rect_width - 60) // 2)
    sel = getattr(config, 'history_delete_confirm_selection', 0)
    draw_stylized_button(screen, _("button_yes"), rect_x + rect_width // 2 - button_width - 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=sel == 1)
    draw_stylized_button(screen, _("button_no"), rect_x + rect_width // 2 + 10, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=sel == 0)

def draw_history_extract_archive(screen):
    """Affiche la confirmation d'extraction forcée d'archive."""
    screen.blit(core.OVERLAY, (0, 0))
    
    if not config.history or config.current_history_item >= len(config.history):
        return
    
    entry = config.history[config.current_history_item]
    game_name = entry.get("game_name", "Unknown")
    
    prompt = _("history_extract_archive_confirm") if _ else "Force extract archive"
    message = f"{prompt}: {game_name}?"
    wrapped_message = wrap_text(message, config.font, config.screen_width - 80)
    line_height = config.font.get_height() + 5
    text_height = len(wrapped_message) * line_height
    button_height = int(config.screen_height * 0.0463)
    margin_top_bottom = 20
    rect_height = text_height + button_height + 2 * margin_top_bottom
    max_text_width = max([config.font.size(line)[0] for line in wrapped_message], default=300)
    rect_width = max_text_width + 150
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = (config.screen_height - rect_height) // 2
    
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)
    
    for i, line in enumerate(wrapped_message):
        text = config.font.render(line, True, THEME_COLORS["text"])
        text_rect = text.get_rect(center=(config.screen_width // 2, rect_y + margin_top_bottom + i * line_height + line_height // 2))
        screen.blit(text, text_rect)
    
    button_width = 120
    draw_stylized_button(screen, _("button_OK"), rect_x + (rect_width - button_width) // 2, rect_y + text_height + margin_top_bottom, button_width, button_height, selected=True)

def format_size(size):
    """Convertit une taille en octets en format lisible avec unités adaptées à la langue."""
    if not isinstance(size, (int, float)) or size == 0:
        return "N/A"
    
    units = get_size_units()
    for unit in units[:-1]:  # Tous sauf le dernier (Po/PB)
        if size < 1024.0:
            return f"{size:.1f} {unit}"
        size /= 1024.0
    return f"{size:.1f} {units[-1]}"

def format_speed_adaptive(speed_mib_s):
    """Formate une vitesse stockée en MiB/s avec une unité lisible selon son ordre de grandeur."""
    try:
        speed_mib_s = float(speed_mib_s or 0.0)
    except Exception:
        speed_mib_s = 0.0

    if speed_mib_s <= 0:
        units = get_size_units()
        base = units[0] if units else "B"
        return f"0 {base}/s"

    bytes_per_second = speed_mib_s * 1024.0 * 1024.0
    units = get_size_units()
    if not units or len(units) < 4:
        units = ["B", "KB", "MB", "GB"]

    if bytes_per_second < 1024.0:
        return f"{bytes_per_second:.0f} {units[0]}/s"
    if bytes_per_second < (1024.0 ** 2):
        return f"{bytes_per_second / 1024.0:.1f} {units[1]}/s"
    if bytes_per_second < (1024.0 ** 3):
        return f"{bytes_per_second / (1024.0 ** 2):.2f} {units[2]}/s"
    return f"{bytes_per_second / (1024.0 ** 3):.2f} {units[3]}/s"
