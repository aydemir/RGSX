import logging
import os

import config
from config import Game
from language import _
from utils import sort_games_list

logger = logging.getLogger("controls")

GLOBAL_SORT_OPTIONS = [
    ("name_asc", lambda: _("web_sort_name_asc") or "A-Z (Name)"),
    ("name_desc", lambda: _("web_sort_name_desc") or "Z-A (Name)"),
    ("size_asc", lambda: _("web_sort_size_asc") or "Size -+ (Small first)"),
    ("size_desc", lambda: _("web_sort_size_desc") or "Size +- (Large first)"),
]

def _wrap_index(current_index: int, delta: int, item_count: int) -> int:
    if item_count <= 0:
        return 0
    return (current_index + delta) % item_count

def _sort_global_items(items: list[dict]) -> list[dict]:
    option = getattr(config, 'global_sort_option', 'name_asc') or 'name_asc'
    reverse = option in ('name_desc', 'size_desc')

    if option.startswith('size_'):
        return sorted(
            items,
            key=lambda item: (
                int(item.get('size_bytes') or 0),
                str(item.get('display_name') or '').lower(),
                str(item.get('platform_label') or '').lower(),
            ),
            reverse=reverse,
        )

    return sorted(
        items,
        key=lambda item: (
            str(item.get('display_name') or '').lower(),
            str(item.get('platform_label') or '').lower(),
            int(item.get('size_bytes') or 0),
        ),
        reverse=reverse,
    )

def _get_global_sort_index(option: str | None = None) -> int:
    target = option or getattr(config, 'global_sort_option', 'name_asc')
    for index, (key, _) in enumerate(GLOBAL_SORT_OPTIONS):
        if key == target:
            return index
    return 0

def _sort_local_games(items: list[Game]) -> list[Game]:
    option = getattr(config, 'global_sort_option', 'name_asc')
    return sort_games_list(items, option)

def _apply_sorted_active_filters() -> list[Game]:
    if hasattr(config, 'game_filter_obj') and config.game_filter_obj and config.game_filter_obj.is_active():
        platform = config.platforms[config.current_platform]
        platform_name = config.platform_names.get(platform, platform)
        return _sort_local_games(config.game_filter_obj.apply_filters(config.games, platform_name))
    return config.games

def _is_windows_os() -> bool:
    return str(getattr(config, 'OPERATING_SYSTEM', '') or '').lower() == "windows" or os.name == 'nt'

def _is_windows_drive_root(path: str) -> bool:
    if not _is_windows_os() or not path:
        return False
    normalized = os.path.normpath(path)
    drive, tail = os.path.splitdrive(normalized)
    return bool(drive) and tail in ('\\', '/')

def _get_available_windows_drives() -> list[str]:
    drives = []
    for letter in "ABCDEFGHIJKLMNOPQRSTUVWXYZ":
        drive = f"{letter}:\\"
        if os.path.isdir(drive):
            drives.append(drive)
    return drives

def _load_folder_browser_items(path: str) -> list[str]:
    if _is_windows_os() and not path:
        return _get_available_windows_drives()

    target_path = path
    if not target_path:
        target_path = "/"

    items = [".."]
    try:
        for item in sorted(os.listdir(target_path)):
            full_path = os.path.join(target_path, item)
            if os.path.isdir(full_path):
                items.append(item)
    except Exception as e:
        logger.error(f"Erreur lecture dossier {target_path}: {e}")
        return [".."] if target_path else []
    return items

def _set_folder_browser_location(path: str | None, reset_selection: bool = True) -> None:
    if _is_windows_os():
        normalized_path = os.path.normpath(path) if path else ""
        if normalized_path in ('\\', '/'):
            normalized_path = ""
        if normalized_path and not os.path.isdir(normalized_path):
            normalized_path = ""
    else:
        normalized_path = path or "/"
        if not os.path.isdir(normalized_path):
            normalized_path = "/"

    config.folder_browser_path = normalized_path
    config.folder_browser_items = _load_folder_browser_items(normalized_path)

    if reset_selection:
        config.folder_browser_selection = 0
        config.folder_browser_scroll_offset = 0
    else:
        max_index = max(0, len(config.folder_browser_items) - 1)
        config.folder_browser_selection = max(0, min(config.folder_browser_selection, max_index))
        max_scroll = max(0, len(config.folder_browser_items) - max(1, int(getattr(config, 'folder_browser_visible_items', 10) or 10)))
        config.folder_browser_scroll_offset = max(0, min(config.folder_browser_scroll_offset, max_scroll))

def _build_filter_menu_entries(context: str) -> list[dict[str, str]]:
    global_search_label = 'Recherche globale' if (_ is None or _("global_search_title") == "global_search_title") else _("global_search_title").format("").replace(" : ", "").rstrip(': ')
    platform_search_label = 'Recherche sur cette plateforme' if (_ is None or _("platform_search_title") == "platform_search_title") else _("platform_search_title")
    advanced_filter_label = 'Filtrer' if (_ is None or _("filter_advanced") == "filter_advanced") else _("filter_advanced")
    sort_label = 'Trier' if (_ is None or _("web_sort") == "web_sort") else _("web_sort")
    back_label = 'Retour' if (_ is None or _("menu_back") == "menu_back") else _("menu_back")

    entries = []
    if context == 'game':
        entries.extend([
            {
                'key': 'platform_search',
                'label': platform_search_label,
            },
            {
                'key': 'global_sort',
                'label': sort_label,
            },
            {
                'key': 'global_search',
                'label': global_search_label,
            },
            {
                'key': 'global_filter',
                'label': advanced_filter_label,
            },
        ])
    else:
        entries.extend([
            {
                'key': 'global_search',
                'label': global_search_label,
            },
            {
                'key': 'global_filter',
                'label': advanced_filter_label,
            },
            {
                'key': 'global_sort',
                'label': sort_label,
            },
        ])

    entries.append({
        'key': 'back',
        'label': back_label,
    })
    return entries

def open_unified_filter_menu(source_state: str) -> None:
    context = 'game' if source_state == 'game' else 'global'
    config.filter_menu_context = context
    config.filter_menu_entries = _build_filter_menu_entries(context)
    config.filter_menu_return_state = validate_menu_state(source_state)
    config.selected_filter_choice = 0
    config.previous_menu_state = source_state
    config.download_all_focus = False  # Faz 9: filtre menüsü açılınca "Tümünü İndir" odaktan çıkar
    config.menu_state = 'filter_menu_choice'
    config.needs_redraw = True
    logger.debug(f"Ouverture du menu filtre unifie depuis {source_state}")

VALID_STATES = [
    "platform", "game", "confirm_exit",
    "extension_warning", "pause_menu", "controls_help", "history", "controls_mapping",
    "reload_games_data", "restart_popup", "error", "loading", "confirm_clear_history",
    "reset_settings_confirm",
    "language_select", "filter_platforms", "display_menu", "confirm_cancel_download",
    "gamelist_update_prompt", "platform_folder_config",
    # Nouveaux sous-menus hiérarchiques (refonte pause menu)
    "pause_controls_menu",      # sous-menu Controls (aide, remap)
    "pause_display_menu",       # sous-menu Display (layout, font size, unsupported, unknown ext, filter)
    "pause_display_layout_menu",# sous-menu Display > Layout (disposition avec visualisation)
    "pause_display_font_menu",  # sous-menu Display > Font (taille police + footer)
    "pause_games_menu",         # sous-menu Games (source mode, update/redownload cache)
    "pause_settings_menu",      # sous-menu Settings (music on/off, symlink toggle, api keys status)
    "pause_api_keys_status",    # sous-menu API Keys (affichage statut des clés)
    "pause_qbt_password",       # sous-menu qBittorrent WebUI şifresi (keyboard girişi)
    "pause_connection_status",  # sous-menu Connection status (statut accès sites)
    # Nouveaux menus historique
    "history_game_options",     # menu options pour un jeu de l'historique
    "history_show_folder",      # afficher le dossier de téléchargement
    "history_scraper_info",     # info scraper non implémenté
    "scraper",                  # écran du scraper avec métadonnées
    "history_error_details",    # détails de l'erreur
    "history_confirm_delete",   # confirmation suppression jeu
    "history_extract_archive",  # extraction d'archive
    "text_file_viewer",         # visualiseur de fichiers texte
    # Nouveaux menus filtrage avancé
    "filter_menu_choice",       # menu de choix entre recherche et filtrage avancé
    "filter_search",            # recherche par nom (existant, mais renommé)
    "filter_advanced",          # filtrage avancé par région, etc.
    "filter_priority_config",   # configuration priorité régions pour one-rom-per-game
    "global_sort_menu",         # menu de tri global
    "platform_search",          # recherche globale inter-plateformes
    "platform_folder_config",   # configuration du dossier personnalisé pour une plateforme
    "folder_browser",           # navigateur de dossiers intégré
    "folder_browser_new_folder", # création d'un nouveau dossier
]

def validate_menu_state(state):
    if not state:
        return "platform"
    if state not in VALID_STATES:
        logger.debug(f"État invalide {state}, retour à platform")
        return "platform"
    return state

