
"""game_list module."""

from pathlib import Path
import os
import pygame  # type: ignore
import urllib.request

from typing import Any, Dict

import config

from history import is_game_downloaded

from language import _

from utils import (truncate_text_middle, wrap_text, truncate_text_end, sort_games_list)

from .colors import THEME_COLORS
from .components import draw_header_badge, measure_header_badge
from .grid import (
    draw_platform_header_info,
    get_default_disk_space_line,
    get_platform_header_badge_layout,
    get_platform_header_info_lines,
)

from . import core
import logging
logger = logging.getLogger(__name__)

FBNEO_GAME_LIST = "fbneo_gamelist.txt"
def download_fbneo_list(path_to_save: str) -> None:
    url = "https://raw.githubusercontent.com/libretro/FBNeo/master/gamelist.txt"
    path = Path(path_to_save)

    if not path.exists():
        logger.debug("Downloading fbneo gamelist.txt from github ...")
        urllib.request.urlretrieve(url, path)
        logger.debug("Download finished: %s", path)
    ...

def parse_fbneo_list(path: str) -> Dict[str, Any]:
    games : Dict[str, Any] = {}
    headers = None

    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.rstrip()

            if line.startswith("+"):
                continue

            if "|" not in line:
                continue

            parts = [p.strip() for p in line.split("|")[1:-1]]

            if headers is None:
                headers = parts
                continue

            row = dict(zip(headers, parts))

            name = row["name"]
            games[name] = row

    return games

def draw_game_list(screen):
    """Affiche la liste des jeux avec un style moderne."""
    #logger.debug(f"[DRAW_GAME_LIST] Called - platform={config.current_platform}, search_mode={config.search_mode}, filter_active={config.filter_active}")
    platform = config.platforms[config.current_platform]
    platform_name = config.platform_names.get(platform, platform)

    fbneo_selected = platform_name == 'Final Burn Neo'
    if fbneo_selected:
        fbneo_game_list_path = os.path.join(config.SAVE_FOLDER, FBNEO_GAME_LIST)
        if not config.fbneo_games:
            download_fbneo_list(fbneo_game_list_path) # download the fbneo game list if necessary - 10 MB file
            config.fbneo_games = parse_fbneo_list(fbneo_game_list_path)
        for game in config.games:
            clean_name = game.display_name
            if clean_name in config.fbneo_games:
                fbneo_game = config.fbneo_games[clean_name]
                full_name = fbneo_game["full name"]
                if game.display_name != full_name:
                    game.display_name = full_name
                    game.regions = None
                    game.is_non_release = None
                    game.base_name = None
        ...

    if config.game_filter_obj and config.game_filter_obj.is_active() and not config.search_query:
        config.filtered_games = sort_games_list(
            config.game_filter_obj.apply_filters(config.games, platform_name),
            getattr(config, 'global_sort_option', 'name_asc'),
        )

    games = config.filtered_games if config.filter_active or config.search_mode else config.games
    game_count = len(games)
    #logger.debug(f"[DRAW_GAME_LIST] Games count={game_count}, current_game={config.current_game}, filtered_games={len(config.filtered_games) if config.filtered_games else 0}, config.games={len(config.games) if config.games else 0}")

    if not games:
        logger.debug("Aucune liste de jeux disponible")
        message = _("game_no_games")
        lines = wrap_text(message, config.font, config.screen_width - 80)
        line_height = config.font.get_height() + 5
        text_height = len(lines) * line_height
        margin_top_bottom = 20
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

    line_height = config.small_font.get_height() + 10
    header_height = line_height  # hauteur de l'en-tête identique à une ligne
    margin_top_bottom = 20
    extra_margin_top = 20
    extra_margin_bottom = 60
    title_height = config.title_font.get_height() + 20

    # Réserver de l'espace pour l'en-tête (header_height)
    available_height = config.screen_height - title_height - extra_margin_top - extra_margin_bottom - 2 * margin_top_bottom - header_height
    if download_all_row_visible():
        # Faz 9: ilk satır "Tümünü İndir" için bir satır yüksekliği ayır
        available_height -= line_height
    items_per_page = max(1, available_height // line_height)

    rect_height = header_height + items_per_page * line_height + 2 * margin_top_bottom
    rect_width = int(0.95 * config.screen_width)
    rect_x = (config.screen_width - rect_width) // 2
    rect_y = title_height + extra_margin_top + (config.screen_height - title_height - extra_margin_top - extra_margin_bottom - rect_height) // 2

    config.scroll_offset = max(0, min(config.scroll_offset, max(0, len(games) - items_per_page)))
    if config.current_game < config.scroll_offset:
        config.scroll_offset = config.current_game
    elif config.current_game >= config.scroll_offset + items_per_page:
        config.scroll_offset = config.current_game - items_per_page + 1

    screen.blit(core.OVERLAY, (0, 0))

    header_margin_x = 14
    header_y = 10
    left_badge_lines = []
    left_badge_width = 0
    right_badge_lines = get_platform_header_info_lines(None, include_details=False)

    disk_space_line = get_default_disk_space_line()
    if disk_space_line:
        left_badge_candidate_lines = [disk_space_line]
    else:
        left_badge_candidate_lines = []

    header_layout = get_platform_header_badge_layout(
        config.screen_width,
        left_lines=left_badge_candidate_lines,
        right_lines=right_badge_lines,
        center_min_width=max(180, int(config.screen_width * 0.18)),
        header_margin_x=header_margin_x,
    )
    header_gap = header_layout["header_gap"]
    left_badge_max_width = header_layout["left_max_width"]
    right_badge_max_width = header_layout["right_max_width"]

    if left_badge_candidate_lines:
        left_badge_width, left_badge_height, left_badge_lines = measure_header_badge(
            left_badge_candidate_lines,
            font=config.tiny_font,
            max_badge_width=left_badge_max_width,
        )

    right_badge_lines = get_platform_header_info_lines(right_badge_max_width, include_details=False)
    right_badge_width, right_badge_height, right_badge_lines = measure_header_badge(
        right_badge_lines,
        font=config.tiny_font,
        max_badge_width=right_badge_max_width,
    )

    title_left = header_margin_x + (left_badge_width + header_gap if left_badge_lines else 0)
    title_right = config.screen_width - header_margin_x - (right_badge_width + header_gap if right_badge_lines else 0)
    title_badge_max_width = max(180, title_right - title_left)

    def _build_game_header_title(title_text_value, font_candidates, text_color, border_color=None):
        padding_x = 18
        padding_y = 10
        selected_font = font_candidates[-1]
        selected_text = title_text_value
        for candidate_font in font_candidates:
            raw_width = candidate_font.size(title_text_value)[0] + padding_x * 2
            if raw_width <= title_badge_max_width:
                selected_font = candidate_font
                selected_text = title_text_value
                break
        else:
            selected_text = truncate_text_end(title_text_value, selected_font, max(80, title_badge_max_width - padding_x * 2))

        title_surface_local = selected_font.render(selected_text, True, text_color)
        title_rect_local = title_surface_local.get_rect()
        title_rect_inflated_local = title_rect_local.inflate(padding_x * 2, padding_y * 2)
        title_rect_inflated_local.x = title_left + max(0, (title_badge_max_width - title_rect_inflated_local.width) // 2)
        title_rect_inflated_local.y = header_y
        title_rect_local.center = title_rect_inflated_local.center
        return title_surface_local, title_rect_local, title_rect_inflated_local, border_color or THEME_COLORS["border"]

    if config.search_mode:
        search_text = _("game_search").format(config.search_query + "_")
        title_surface, title_rect, title_rect_inflated, title_border_color = _build_game_header_title(
            search_text,
            [config.search_font, config.font, config.small_font],
            THEME_COLORS["text"],
        )
        
        # Ombre pour le titre de recherche
        shadow = pygame.Surface((title_rect_inflated.width + 10, title_rect_inflated.height + 10), pygame.SRCALPHA)
        pygame.draw.rect(shadow, (0, 0, 0, 120), (5, 5, title_rect_inflated.width, title_rect_inflated.height), border_radius=14)
        screen.blit(shadow, (title_rect_inflated.left - 5, title_rect_inflated.top - 5))
        
        # Glow pour recherche active
        glow = pygame.Surface((title_rect_inflated.width + 20, title_rect_inflated.height + 20), pygame.SRCALPHA)
        pygame.draw.rect(glow, (*THEME_COLORS["glow"][:3], 60), glow.get_rect(), border_radius=16)
        screen.blit(glow, (title_rect_inflated.left - 10, title_rect_inflated.top - 10))
        
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)
        pygame.draw.rect(screen, title_border_color, title_rect_inflated, 2, border_radius=12)
        screen.blit(title_surface, title_rect)
    elif config.filter_active:
        # Afficher le nom de la plateforme avec indicateur de filtre actif
        filter_indicator = " (Active Filter)"
        if config.search_query:
            # Si recherche par nom active, afficher aussi la recherche
            filter_indicator = f" - {_('game_filter').format(config.search_query)}"
        
        title_text = _("game_count").format(platform_name, game_count) + filter_indicator
        title_surface, title_rect, title_rect_inflated, title_border_color = _build_game_header_title(
            title_text,
            [config.title_font, config.search_font, config.font, config.small_font],
            THEME_COLORS["green"],
            border_color=THEME_COLORS["border_selected"],
        )
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)
        pygame.draw.rect(screen, title_border_color, title_rect_inflated, 3, border_radius=12)
        screen.blit(title_surface, title_rect)
    else:
        # Ajouter indicateur de filtre actif si filtres avancés sont actifs
        filter_indicator = ""
        if hasattr(config, 'game_filter_obj') and config.game_filter_obj and config.game_filter_obj.is_active():
            filter_indicator = " (Active Filter)"
        
        title_text = _("game_count").format(platform_name, game_count) + filter_indicator
        title_surface, title_rect, title_rect_inflated, title_border_color = _build_game_header_title(
            title_text,
            [config.title_font, config.search_font, config.font, config.small_font],
            THEME_COLORS["text"],
        )
        
        # Ombre et glow pour titre normal
        shadow = pygame.Surface((title_rect_inflated.width + 10, title_rect_inflated.height + 10), pygame.SRCALPHA)
        pygame.draw.rect(shadow, (0, 0, 0, 120), (5, 5, title_rect_inflated.width, title_rect_inflated.height), border_radius=14)
        screen.blit(shadow, (title_rect_inflated.left - 5, title_rect_inflated.top - 5))
        
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=12)
        pygame.draw.rect(screen, title_border_color, title_rect_inflated, 2, border_radius=12)
        screen.blit(title_surface, title_rect)

    if left_badge_lines:
        draw_header_badge(
            screen,
            left_badge_lines,
            header_margin_x,
            header_y,
            False,
            font=config.tiny_font,
            max_badge_width=left_badge_max_width,
        )

    if right_badge_lines:
        right_badge_x = config.screen_width - right_badge_width - header_margin_x
        draw_platform_header_info(
            screen,
            False,
            badge_x=right_badge_x,
            max_badge_width=right_badge_max_width,
            include_details=False,
        )

    # Ombre portée pour le cadre principal
    shadow_rect = pygame.Rect(rect_x + 6, rect_y + 6, rect_width, rect_height)
    shadow_surf = pygame.Surface((rect_width + 8, rect_height + 8), pygame.SRCALPHA)
    pygame.draw.rect(shadow_surf, (0, 0, 0, 100), (4, 4, rect_width, rect_height), border_radius=14)
    screen.blit(shadow_surf, (rect_x - 4, rect_y - 4))
    
    # Fond du cadre avec légère transparence glassmorphism
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, rect_width, rect_height), border_radius=12)
    
    # Reflet en haut du cadre
    highlight = pygame.Surface((rect_width - 8, 40), pygame.SRCALPHA)
    highlight.fill((255, 255, 255, 15))
    screen.blit(highlight, (rect_x + 4, rect_y + 4))
    
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, rect_width, rect_height), 2, border_radius=12)

    # Largeur colonnes nom / ext / taille
    ext_col_width = max(90, int(rect_width * 0.08))
    size_col_width = max(120, int(rect_width * 0.15))
    name_col_width = rect_width - 40 - ext_col_width - size_col_width

    # ---- En-tête ----
    header_name = _("game_header_name")
    header_ext = _("game_header_ext")
    header_size = _("game_header_size")
    header_y_center = rect_y + margin_top_bottom + header_height // 2
    # Nom aligné gauche
    header_name_surface = config.small_font.render(header_name, True, THEME_COLORS["text"])
    header_name_rect = header_name_surface.get_rect()
    header_name_rect.midleft = (rect_x + 20, header_y_center)
    # Extension centree
    header_ext_surface = config.small_font.render(header_ext, True, THEME_COLORS["text"])
    header_ext_rect = header_ext_surface.get_rect()
    header_ext_rect.center = (rect_x + rect_width - 20 - size_col_width - ext_col_width // 2, header_y_center)
    # Taille alignée droite
    header_size_surface = config.small_font.render(header_size, True, THEME_COLORS["text"])
    header_size_rect = header_size_surface.get_rect()
    header_size_rect.midright = (rect_x + rect_width - 20, header_y_center)
    screen.blit(header_name_surface, header_name_rect)
    screen.blit(header_ext_surface, header_ext_rect)
    screen.blit(header_size_surface, header_size_rect)
    # Ligne de séparation sous l'en-tête
    separator_y = rect_y + margin_top_bottom + header_height
    pygame.draw.line(screen, THEME_COLORS["border"], (rect_x + 20, separator_y), (rect_x + rect_width - 20, separator_y), 2)

    # Position de départ des lignes après l'en-tête
    list_start_y = rect_y + margin_top_bottom + header_height

    # Faz 9 — Filtre aktifken ilk satır: "Tümünü İndir" (oyun listesinden bağımsız sabit satır)
    extra_row = 0
    if download_all_row_visible():
        extra_row = line_height
        dl_all_focus = bool(getattr(config, 'download_all_focus', False))
        dl_all_text = download_all_row_text(download_all_row_games(), platform_name)
        dl_all_truncated = truncate_text_middle(dl_all_text, config.small_font, name_col_width, is_filename=False)
        dl_all_surface = config.small_font.render(
            dl_all_truncated, True,
            THEME_COLORS["green"] if dl_all_focus else THEME_COLORS["text"],
        )
        dl_all_rect = dl_all_surface.get_rect()
        dl_all_rect.midleft = (rect_x + 20, list_start_y + line_height // 2)
        if dl_all_focus:
            glow_width = rect_width - 40
            glow_height = dl_all_rect.height + 12
            selection_bg = pygame.Surface((glow_width, glow_height), pygame.SRCALPHA)
            selection_bg.fill((*THEME_COLORS["fond_lignes"][:3], 90))
            screen.blit(selection_bg, (rect_x + 20, list_start_y + line_height // 2 - glow_height // 2))
            border_rect = pygame.Rect(rect_x + 20, list_start_y + line_height // 2 - glow_height // 2, glow_width, glow_height)
            pygame.draw.rect(screen, THEME_COLORS["border_selected"], border_rect, 2, border_radius=8)
        screen.blit(dl_all_surface, dl_all_rect)

    for i in range(config.scroll_offset, min(config.scroll_offset + items_per_page, len(games))):
        item = games[i]
        game_name = item.display_name
        size_val = item.size
      
        # Vérifier si le jeu est déjà téléchargé en comparant le nom réel sans extension
        is_downloaded = is_game_downloaded(platform_name, item.name)
        
        # Vérifier si le jeu est en cours de téléchargement
        is_downloading = False
        download_percent = 0
        # 1) Tâches locales (mode --ui-only / sans manager)
        for tid, (task, dl_url, dl_name, dl_platform) in getattr(config, 'download_tasks', {}).items():
            dl_name_stem = os.path.splitext(dl_name)[0] if dl_name else ""
            if dl_name_stem and dl_name_stem.lower() == game_name.lower():
                is_downloading = True
                dl_progress = getattr(config, 'download_progress', {}).get(dl_url, {})
                download_percent = int(dl_progress.get("progress_percent", 0))
                if download_percent == 0:
                    for prog_url, prog_data in getattr(config, 'download_progress', {}).items():
                        prog_name_stem = os.path.splitext(prog_data.get("game_name", ""))[0]
                        if prog_name_stem.lower() == game_name.lower():
                            download_percent = int(prog_data.get("progress_percent", 0))
                            break
                break
        # 2) Téléchargements du manager (état reflété via SSE)
        if not is_downloading:
            for prog_url, prog_data in getattr(config, 'download_progress', {}).items():
                prog_name_stem = os.path.splitext(prog_data.get("game_name", "") or "")[0]
                if prog_name_stem and prog_name_stem.lower() == game_name.lower():
                    prog_status = str(prog_data.get("status", ""))
                    if (prog_status in ("Downloading", "Connecting", "Extracting")
                            or prog_status.startswith("Try ") or "Downloading" in prog_status):
                        is_downloading = True
                        download_percent = int(prog_data.get("progress_percent", 0))
                    break
        
        # Vérifier si le jeu a échoué (dernière tentative dans l'historique)
        is_failed = False
        if not is_downloaded and not is_downloading:
            for entry in reversed(getattr(config, 'history', [])):
                entry_name = os.path.splitext(entry.get("game_name", ""))[0]
                if entry_name.lower() == game_name.lower() and entry.get("platform", "").lower() == platform_name.lower():
                    if entry.get("status") in ("Erreur", "Error"):
                        is_failed = True
                    break
        
        ext_text = get_display_extension(item.name)
        size_text = size_val if (isinstance(size_val, str) and size_val.strip()) else "N/A"
        color = THEME_COLORS["fond_lignes"] if i == config.current_game else THEME_COLORS["text"]
        
        # Couleur et marqueur selon l'état: téléchargé > en cours > échoué > normal
        if is_downloaded:
            prefix = "[>] "
            name_color = (100, 255, 100)  # Vert clair
        elif is_downloading:
            prefix = f"[~] {download_percent}% "
            name_color = (255, 200, 0)  # Jaune
        elif is_failed:
            prefix = "[X] "
            name_color = (255, 80, 80)  # Rouge
        else:
            prefix = ""
            name_color = color
        
        truncated_name = truncate_text_middle(prefix + game_name, config.small_font, name_col_width, is_filename=False)
        name_surface = config.small_font.render(truncated_name, True, name_color)
        ext_surface = config.small_font.render(ext_text, True, THEME_COLORS["text"])
        size_surface = config.small_font.render(size_text, True, THEME_COLORS["text"])
        row_center_y = list_start_y + extra_row + (i - config.scroll_offset) * line_height + line_height // 2
        # Position nom (aligné à gauche dans la boite)
        name_rect = name_surface.get_rect()
        name_rect.midleft = (rect_x + 20, row_center_y)
        ext_rect = ext_surface.get_rect()
        ext_rect.center = (rect_x + rect_width - 20 - size_col_width - ext_col_width // 2, row_center_y)
        size_rect = size_surface.get_rect()
        size_rect.midright = (rect_x + rect_width - 20, row_center_y)
        if i == config.current_game:
            glow_width = rect_width - 40
            glow_height = name_rect.height + 12
            
            # Effet de glow plus doux pour la sélection
            glow_surface = pygame.Surface((glow_width + 6, glow_height + 6), pygame.SRCALPHA)
            alpha = 50
            pygame.draw.rect(glow_surface, (*THEME_COLORS["fond_lignes"][:3], alpha), 
                           (3, 3, glow_width, glow_height), 
                           border_radius=8)
            screen.blit(glow_surface, (rect_x + 17, row_center_y - glow_height // 2 - 3))
            
            # Fond principal de la sélection avec dégradé subtil
            selection_bg = pygame.Surface((glow_width, glow_height), pygame.SRCALPHA)
            for j in range(glow_height):
                ratio = j / glow_height
                alpha = int(60 + 20 * ratio)
                pygame.draw.line(selection_bg, (*THEME_COLORS["fond_lignes"][:3], alpha), 
                               (0, j), (glow_width, j))
            screen.blit(selection_bg, (rect_x + 20, row_center_y - glow_height // 2))
            
            # Bordure lumineuse plus subtile
            border_rect = pygame.Rect(rect_x + 20, row_center_y - glow_height // 2, glow_width, glow_height)
            pygame.draw.rect(screen, (*THEME_COLORS["fond_lignes"][:3], 120), border_rect, width=1, border_radius=8)
        
        screen.blit(name_surface, name_rect)
        screen.blit(ext_surface, ext_rect)
        screen.blit(size_surface, size_rect)

    if len(games) > items_per_page:
        try:
            draw_game_scrollbar(
                screen,
                config.scroll_offset,
                len(games),
                items_per_page,
                rect_x + rect_width - 10,
                rect_y,
                rect_height
            )
        except NameError as e:
            logger.error(f"Erreur : draw_game_scrollbar non défini: {str(e)}")

def draw_game_scrollbar(screen, scroll_offset, total_items, visible_items, x, y, height):
    """Affiche la barre de défilement pour la liste des jeux."""
    if total_items <= visible_items:
        return
    game_area_height = height
    scrollbar_height = game_area_height * (visible_items / total_items)
    scrollbar_y = y + (game_area_height - scrollbar_height) * (scroll_offset / max(1, total_items - visible_items))
    pygame.draw.rect(screen, THEME_COLORS["fond_lignes"], (x, scrollbar_y, 15, scrollbar_height), border_radius=4)

def get_display_extension(file_name):
    """Retourne l'extension finale d'un nom de fichier pour affichage."""
    if not isinstance(file_name, str) or not file_name.strip():
        return "-"
    suffix = Path(file_name).suffix.strip()
    if not suffix:
        return "-"
    return suffix.lower()


def download_all_row_visible() -> bool:
    """Faz 9 — Filtrelenmiş set (+) görüntülenirken ilk satır 'Tümünü İndir'."""
    return bool(getattr(config, 'filter_active', False) and not getattr(config, 'search_mode', False))


def download_all_row_games():
    """Faz 9 — 'Tümünü İndir'in etki ettiği o an görünen set."""
    if getattr(config, 'filter_active', False):
        return config.filtered_games or []
    return config.games or []


def download_all_row_text(games, platform_name) -> str:
    """Faz 9 — İlk satır metni: '⬇ Tümünü İndir (N oyun · M zaten indirilmiş)'."""
    try:
        already = count_downloaded_in(games, platform_name)
    except Exception:
        already = 0
    try:
        label = _("game_download_all_label")
        if isinstance(label, str) and label and not label.startswith("game_download_all_label"):
            return label.format(len(games), already)
    except Exception:
        pass
    return f"⬇ Download all ({len(games)} games, {already} already downloaded)"


def count_downloaded_in(games, platform_name) -> int:
    try:
        return sum(1 for g in games if is_game_downloaded(platform_name, g.name))
    except Exception:
        return 0
