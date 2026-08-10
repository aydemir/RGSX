import json
import logging
import os
from pathlib import Path

import config
from config import Game
from display import draw_validation_transition
from history import scan_platform_roms_on_enter
from language import _
from rgsx_settings import get_allow_unknown_extensions
from utils import (
    check_extension_before_download,
    get_clean_display_name,
    is_extension_supported,
    load_extensions_json,
    load_games,
    parse_game_size_to_bytes,
    sanitize_filename,
    _refresh_loading_feedback,
)

from controls.downloads import (
    _has_download_url,
    _launch_next_queued_download,
    _queue_download,
    start_or_queue_download,
)
from controls.menus import _sort_global_items, _sort_local_games, validate_menu_state

logger = logging.getLogger("controls")

def filter_games_by_search_query() -> list[Game]:
    base_games = config.games
    if config.game_filter_obj and config.game_filter_obj.is_active():
        platform = config.platforms[config.current_platform]
        platform_name = config.platform_names.get(platform, platform)
        base_games = config.game_filter_obj.apply_filters(config.games, platform_name)
  
    filtered_games = []
    for game in base_games:
        game_name = game.display_name 
        if config.search_query.lower() in game_name.lower():
            filtered_games.append(game)

    return _sort_local_games(filtered_games)

GLOBAL_SEARCH_KEYBOARD_LAYOUT = [
    ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
    ['A', 'Z', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
    ['Q', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M'],
    ['W', 'X', 'C', 'V', 'B', 'N']
]

def _get_platform_id(platform) -> str:
    return platform.get("name") if isinstance(platform, dict) else str(platform)

def _get_platform_label(platform_id: str) -> str:
    return config.platform_names.get(platform_id, platform_id)

def _build_global_search_loading_title() -> str:
    fallback = "Loading..."
    if _ is None:
        return fallback
    try:
        text = _("global_search_title").format("").replace(" : ", "").rstrip(': ')
    except Exception:
        text = ""
    return text or fallback

def build_global_search_index() -> list[dict]:
    indexed_games = []
    total_platforms = max(1, len(config.platforms))
    for platform_index, platform in enumerate(config.platforms):
        platform_id = _get_platform_id(platform)
        platform_label = _get_platform_label(platform_id)
        _refresh_loading_feedback(
            current_system=_build_global_search_loading_title(),
            progress=((platform_index / total_platforms) * 100.0),
            detail_lines=[
                _("loading_platform_counter").format(platform_index + 1, total_platforms) if _ else f"Platform {platform_index + 1}/{total_platforms}",
                _("loading_platform_name").format(platform_label) if _ else f"Platform: {platform_label}",
                _("loading_read_games_resolve_sources") if _ else "Reading games and resolving sources...",
            ],
            force=True,
        )
        for game in load_games(platform_id):
            display_name = game.display_name or Path(game.name).stem
            indexed_games.append({
                "platform_id": platform_id,
                "platform_label": platform_label,
                "platform_index": platform_index,
                "game_name": game.name,
                "display_name": display_name,
                "search_name": display_name.lower(),
                "url": game.url,
                "size": game.size,
                "size_bytes": parse_game_size_to_bytes(game.size),
                "game_obj": game,
            })

    _refresh_loading_feedback(
        current_system=_build_global_search_loading_title(),
        progress=100.0,
        detail_lines=[
            _("loading_platform_counter").format(total_platforms, total_platforms) if _ else f"Platform {total_platforms}/{total_platforms}",
        ],
        force=True,
    )

    return _sort_global_items(indexed_games)

def _load_embedded_global_search_index() -> list[dict] | None:
    cache_path = getattr(config, 'GLOBAL_SEARCH_INDEX_CACHE_PATH', '')
    if not cache_path or not os.path.exists(cache_path):
        return None

    try:
        with open(cache_path, 'r', encoding='utf-8') as handle:
            payload = json.load(handle)
    except Exception as exc:
        logger.warning(f"Impossible de charger l'index global embarque: {exc}")
        return None

    raw_entries = payload.get('entries') if isinstance(payload, dict) else None
    if not isinstance(raw_entries, list):
        return None

    platform_order: dict[str, int] = {}
    for index, platform in enumerate(config.platforms):
        platform_order[_get_platform_id(platform)] = index

    indexed_games = []
    for raw_entry in raw_entries:
        if not isinstance(raw_entry, dict):
            continue
        platform_id = str(raw_entry.get('platform_id') or '').strip()
        if not platform_id or platform_id not in platform_order:
            continue

        game_name = str(raw_entry.get('game_name') or '').strip()
        if not game_name:
            continue

        display_name = str(raw_entry.get('display_name') or '').strip() or Path(game_name).stem
        url = str(raw_entry.get('url') or '').strip() or None
        size = str(raw_entry.get('size') or '').strip() or None
        try:
            size_bytes = int(raw_entry.get('size_bytes') or 0)
        except (TypeError, ValueError):
            size_bytes = 0

        game_obj = Game(name=game_name, url=url, size=size, display_name=display_name)
        indexed_games.append({
            'platform_id': platform_id,
            'platform_label': _get_platform_label(platform_id),
            'platform_index': platform_order[platform_id],
            'game_name': game_name,
            'display_name': display_name,
            'search_name': display_name.lower(),
            'url': url,
            'size': size,
            'size_bytes': size_bytes,
            'game_obj': game_obj,
        })

    if indexed_games:
        logger.info(f"Index global charge depuis le cache embarque: {len(indexed_games)} jeux")
        return _sort_global_items(indexed_games)
    return None

def _ensure_global_search_index(operation_title: str | None = None) -> None:
    index_signature = tuple(config.platforms)
    if getattr(config, 'global_search_index', None) and getattr(config, 'global_search_index_signature', None) == index_signature:
        return

    embedded_index = _load_embedded_global_search_index()
    if embedded_index is not None:
        config.global_search_index = embedded_index
        config.global_search_index_signature = index_signature
        return

    previous_menu_state = getattr(config, 'menu_state', 'platform')
    previous_loading_system = getattr(config, 'current_loading_system', '')
    previous_loading_progress = getattr(config, 'loading_progress', 0.0)
    previous_loading_detail_lines = list(getattr(config, 'loading_detail_lines', []) or [])

    config.menu_state = "loading"
    config.current_loading_system = operation_title or _build_global_search_loading_title()
    config.loading_progress = 0.0
    config.loading_detail_lines = [config.current_loading_system]
    config.needs_redraw = True
    _refresh_loading_feedback(force=True)

    try:
        config.global_search_index = build_global_search_index()
        config.global_search_index_signature = index_signature
    finally:
        config.menu_state = previous_menu_state
        config.current_loading_system = previous_loading_system
        config.loading_progress = previous_loading_progress
        config.loading_detail_lines = previous_loading_detail_lines
        config.needs_redraw = True

def refresh_global_search_results(reset_selection: bool = True) -> None:
    query = (config.global_search_query or "").strip().lower()
    items = list(getattr(config, 'global_search_index', []) or [])

    filter_obj = getattr(config, 'game_filter_obj', None)
    if filter_obj and filter_obj.is_active():
        # Gérer hide_downloaded par item (car chaque item a son platform_label)
        if getattr(filter_obj, 'hide_downloaded', False):
            from history import is_game_downloaded
            items = [item for item in items if not is_game_downloaded(item.get('platform_label', ''), item.get('game_name', ''))]
        
        item_by_game = {id(item.get('game_obj')): item for item in items}
        filtered_games = filter_obj.apply_filters([item.get('game_obj') for item in items if item.get('game_obj') is not None], platform_name=None)
        items = [item_by_game[id(game)] for game in filtered_games if id(game) in item_by_game]

    if query:
        items = [
            item for item in items
            if query in item.get("search_name", item["display_name"].lower())
        ]
    elif not getattr(config, 'global_search_allow_empty', False):
        items = []

    config.global_search_results = _sort_global_items(items)

    if reset_selection:
        config.global_search_selected = 0
        config.global_search_scroll_offset = 0
    else:
        max_index = max(0, len(config.global_search_results) - 1)
        config.global_search_selected = max(0, min(config.global_search_selected, max_index))
        config.global_search_scroll_offset = max(0, min(config.global_search_scroll_offset, config.global_search_selected))

def enter_global_search() -> None:
    _ensure_global_search_index(_build_global_search_loading_title())
    config.global_search_query = ""
    config.global_search_results = []
    config.global_search_selected = 0
    config.global_search_scroll_offset = 0
    config.global_search_editing = bool(getattr(config, 'joystick', False))
    config.global_search_allow_empty = False
    config.global_search_title_override = _("global_search_title").format("").replace(" : ", "").rstrip(': ') if _ else 'Recherche globale'
    config.selected_key = (0, 0)
    config.menu_state = "platform_search"
    config.needs_redraw = True
    logger.debug("Entree en recherche globale inter-plateformes")

def enter_global_filtered_results() -> None:
    _ensure_global_search_index(_("filter_advanced") if _ else "Loading...")
    config.global_search_query = ""
    config.global_search_selected = 0
    config.global_search_scroll_offset = 0
    config.global_search_editing = False
    config.global_search_allow_empty = True
    config.global_search_title_override = _("filter_advanced") if _ else 'Filtrer'
    refresh_global_search_results(reset_selection=True)
    config.menu_state = "platform_search"
    config.needs_redraw = True
    logger.debug(f"Affichage des resultats globaux filtres: {len(config.global_search_results)}")

def enter_global_sorted_results() -> None:
    _ensure_global_search_index(_("web_sort") if _ else "Loading...")
    config.global_search_query = ""
    config.global_search_selected = 0
    config.global_search_scroll_offset = 0
    config.global_search_editing = False
    config.global_search_allow_empty = True
    config.global_search_title_override = _("web_sort") if _ else 'Trier'
    refresh_global_search_results(reset_selection=True)
    config.menu_state = "platform_search"
    config.needs_redraw = True
    logger.debug(f"Affichage des resultats globaux tries ({config.global_sort_option}): {len(config.global_search_results)}")

def exit_global_search() -> None:
    config.global_search_query = ""
    config.global_search_results = []
    config.global_search_selected = 0
    config.global_search_scroll_offset = 0
    config.global_search_editing = False
    config.global_search_allow_empty = False
    config.global_search_title_override = ""
    config.selected_key = (0, 0)
    config.menu_state = validate_menu_state(getattr(config, 'global_search_return_state', None) or getattr(config, 'previous_menu_state', None))
    config.needs_redraw = True

def open_global_search_result(screen) -> None:
    if not config.global_search_results:
        return

    result = config.global_search_results[config.global_search_selected]
    platform_index = result.get("platform_index", 0)
    if platform_index < 0 or platform_index >= len(config.platforms):
        return

    config.current_platform = platform_index
    config.selected_platform = platform_index
    config.current_page = platform_index // max(1, config.GRID_COLS * config.GRID_ROWS)

    platform_id = result["platform_id"]
    config.games = load_games(platform_id)
    scan_platform_roms_on_enter(platform_id)
    config.filtered_games = config.games
    config.search_mode = False
    config.search_query = ""
    config.filter_active = False

    target_name = result["game_name"]
    target_display_name = result["display_name"]
    target_index = 0
    for index, game in enumerate(config.games):
        if game.name == target_name:
            target_index = index
            break
        if game.display_name == target_display_name:
            target_index = index

    config.current_game = target_index
    config.scroll_offset = 0
    config.global_search_editing = False

    from rgsx_settings import get_light_mode
    if not get_light_mode():
        draw_validation_transition(screen, config.current_platform)

    config.menu_state = "game"
    config.needs_redraw = True
    logger.debug(f"Ouverture du resultat global {target_display_name} sur {platform_id}")

def trigger_global_search_download(queue_only: bool = False) -> None:
    if not config.global_search_results:
        return

    result = config.global_search_results[config.global_search_selected]
    url = result.get("url")
    platform = result.get("platform_id")
    game_name = result.get("game_name")
    display_name = result.get("display_name") or get_clean_display_name(game_name, platform)

    if not platform or not game_name:
        logger.error(f"Resultat de recherche globale invalide: {result}")
        return
    if not _has_download_url(url, game_name):
        return

    pending_download = check_extension_before_download(url, platform, game_name)
    if not pending_download:
        logger.error(f"config.pending_download est None pour {game_name}")
        config.needs_redraw = True
        return

    is_supported = is_extension_supported(
        sanitize_filename(game_name),
        platform,
        load_extensions_json()
    )
    zip_ok = bool(pending_download[3])
    allow_unknown = get_allow_unknown_extensions()

    if (not is_supported and not zip_ok) and not allow_unknown:
        config.pending_download = pending_download
        config.pending_download_is_queue = queue_only
        config.previous_menu_state = config.menu_state
        config.menu_state = "extension_warning"
        config.extension_confirm_selection = 0
        config.needs_redraw = True
        logger.debug(f"Extension non supportee, passage a extension_warning pour {game_name}")
        return

    if queue_only:
        _queue_download(url, platform, game_name, pending_download[3], display_name)
        _launch_next_queued_download()
        return

    start_or_queue_download(url, platform, game_name, pending_download[3], display_name)

