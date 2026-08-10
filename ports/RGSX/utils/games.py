from pathlib import Path
import collections
import io
import shutil
import requests  # type: ignore
import re
import json
import os
import logging
import platform
import subprocess
import urllib.parse
import config
from config import HEADLESS, Game
try:
    if not HEADLESS:
        import pygame  # type: ignore
    else:
        pygame = None  # type: ignore
except Exception:
    pygame = None  # type: ignore
import glob
import threading
from rgsx_settings import load_rgsx_settings, save_rgsx_settings, get_allow_unknown_extensions
import zipfile
import time
import random
import config
from history import save_history
from language import _ 
from datetime import datetime
import sys
import tempfile
try:
    from PIL import Image  # type: ignore
except Exception:
    Image = None  # type: ignore

logger = logging.getLogger("utils")

from utils.files import normalize_platform_name

from utils.sorting import sort_games_list

from utils.text import get_clean_display_name

from utils.torrent import (
    _expand_torrent_source,
    _extract_torrent_source,
    _get_torrent_entry_count,
    clear_torrent_manifest_refresh_request,
    is_torrent_manifest_refresh_requested,
)


_games_cache = {}

_platform_game_count_cache = {}

_platform_game_count_cache_loaded = False

_platform_game_count_cache_lock = threading.Lock()



def _load_persistent_platform_game_count_cache() -> None:
    global _platform_game_count_cache_loaded, _platform_game_count_cache
    if _platform_game_count_cache_loaded:
        return
    with _platform_game_count_cache_lock:
        if _platform_game_count_cache_loaded:
            return
        cache_path = getattr(config, 'PLATFORM_GAME_COUNT_CACHE_PATH', '')
        loaded_cache = {}
        try:
            if cache_path and os.path.exists(cache_path):
                with open(cache_path, 'r', encoding='utf-8') as handle:
                    payload = json.load(handle)
                entries = payload.get('entries') if isinstance(payload, dict) else payload
                if isinstance(entries, dict):
                    for platform_id, entry in entries.items():
                        if not isinstance(platform_id, str) or not isinstance(entry, dict):
                            continue
                        loaded_cache[platform_id] = {
                            'path': str(entry.get('path') or ''),
                            'mtime_ns': int(entry.get('mtime_ns') or 0),
                            'file_name': str(entry.get('file_name') or ''),
                            'size_bytes': int(entry.get('size_bytes') or 0),
                            'count': int(entry.get('count') or 0),
                        }
                logger.info(f"Cache compteurs plateformes charge: {len(loaded_cache)} entrees")
        except Exception as exc:
            logger.warning(f"Impossible de charger le cache compteurs plateformes: {exc}")
        _platform_game_count_cache = loaded_cache
        _platform_game_count_cache_loaded = True



def _save_persistent_platform_game_count_cache() -> None:
    cache_path = getattr(config, 'PLATFORM_GAME_COUNT_CACHE_PATH', '')
    if not cache_path:
        return
    with _platform_game_count_cache_lock:
        try:
            os.makedirs(os.path.dirname(cache_path), exist_ok=True)
            payload = {
                'version': 2,
                'entries': _platform_game_count_cache,
            }
            temp_path = f"{cache_path}.{os.getpid()}.{threading.get_ident()}.tmp"
            with open(temp_path, 'w', encoding='utf-8') as handle:
                json.dump(payload, handle, ensure_ascii=False, indent=2)
            last_error = None
            for attempt in range(5):
                try:
                    os.replace(temp_path, cache_path)
                    last_error = None
                    break
                except PermissionError as exc:
                    last_error = exc
                    time.sleep(0.15 * (attempt + 1))
            if last_error is not None:
                raise last_error
        except Exception as exc:
            logger.warning(f"Impossible de sauvegarder le cache compteurs plateformes: {exc}")
        finally:
            try:
                if 'temp_path' in locals() and os.path.exists(temp_path):
                    os.remove(temp_path)
            except Exception:
                pass



def clear_platform_game_count_cache() -> None:
    global _platform_game_count_cache, _platform_game_count_cache_loaded
    with _platform_game_count_cache_lock:
        _platform_game_count_cache = {}
        _platform_game_count_cache_loaded = True
        cache_path = getattr(config, 'PLATFORM_GAME_COUNT_CACHE_PATH', '')
        try:
            if cache_path and os.path.exists(cache_path):
                os.remove(cache_path)
            logger.info("Cache compteurs plateformes invalide")
        except Exception as exc:
            logger.warning(f"Impossible de supprimer le cache compteurs plateformes: {exc}")



def _resolve_game_file(platform_id: str):
    platform_dict = None
    for pd in config.platform_dicts:
        if pd.get("platform_name") == platform_id or pd.get("platform") == platform_id:
            platform_dict = pd
            break

    candidates = [os.path.join(config.GAMES_FOLDER, f"{platform_id}.json")]
    norm = normalize_platform_name(platform_id)
    if norm and norm != platform_id:
        candidates.append(os.path.join(config.GAMES_FOLDER, f"{norm}.json"))
    if platform_dict:
        folder_name = platform_dict.get("folder")
        if folder_name:
            candidates.append(os.path.join(config.GAMES_FOLDER, f"{folder_name}.json"))

    for candidate in candidates:
        if os.path.exists(candidate):
            return candidate, platform_dict, candidates
    return None, platform_dict, candidates



def _get_cached_platform_game_count(platform_id: str, game_file: str, game_mtime_ns: int, game_size_bytes: int) -> int | None:
    _load_persistent_platform_game_count_cache()
    cached_entry = _platform_game_count_cache.get(platform_id)
    if not isinstance(cached_entry, dict):
        return None
    cached_path = str(cached_entry.get('path') or '')
    cached_mtime_ns = int(cached_entry.get('mtime_ns') or 0)
    cached_file_name = str(cached_entry.get('file_name') or '')
    cached_size_bytes = int(cached_entry.get('size_bytes') or 0)

    if cached_path == game_file and cached_mtime_ns == int(game_mtime_ns):
        return max(0, int(cached_entry.get('count') or 0))

    # Portable fallback for caches embedded in games.zip.
    if cached_file_name and cached_file_name == os.path.basename(game_file) and cached_size_bytes == int(game_size_bytes):
        return max(0, int(cached_entry.get('count') or 0))

    return None



def _store_platform_game_count(platform_id: str, game_file: str, game_mtime_ns: int, game_size_bytes: int, count: int) -> None:
    _load_persistent_platform_game_count_cache()
    normalized_entry = {
        'path': game_file,
        'mtime_ns': int(game_mtime_ns),
        'file_name': os.path.basename(game_file),
        'size_bytes': int(game_size_bytes),
        'count': max(0, int(count)),
    }
    with _platform_game_count_cache_lock:
        existing_entry = _platform_game_count_cache.get(platform_id)
        if isinstance(existing_entry, dict):
            existing_normalized = {
                'path': str(existing_entry.get('path') or ''),
                'mtime_ns': int(existing_entry.get('mtime_ns') or 0),
                'file_name': str(existing_entry.get('file_name') or ''),
                'size_bytes': int(existing_entry.get('size_bytes') or 0),
                'count': int(existing_entry.get('count') or 0),
            }
            if existing_normalized == normalized_entry:
                return

        _platform_game_count_cache[platform_id] = normalized_entry
    _save_persistent_platform_game_count_cache()



def get_platform_game_count(platform_id: str, allow_torrent_manifest_fetch: bool = True) -> int:
    game_file, resolved_platform_dict, candidates = _resolve_game_file(platform_id)
    if not game_file:
        logger.warning(f"Aucun fichier de jeux trouvé pour {platform_id} (candidats: {candidates})")
        return 0

    game_stat = os.stat(game_file)
    game_mtime_ns = game_stat.st_mtime_ns
    game_size_bytes = game_stat.st_size
    cached_count = _get_cached_platform_game_count(platform_id, game_file, game_mtime_ns, game_size_bytes)
    if cached_count is not None:
        return cached_count

    with open(game_file, 'r', encoding='utf-8') as f:
        data = json.load(f)

    if isinstance(data, dict) and 'games' in data:
        data = data['games']

    count = 0

    def count_from_dict(d):
        torrent_source = _extract_torrent_source(d)
        if torrent_source is not None:
            source_name, source_url = torrent_source
            try:
                return _get_torrent_entry_count(
                    source_url,
                    display_label=source_name,
                    platform_id=platform_id,
                    allow_network_fetch=allow_torrent_manifest_fetch,
                )
            except Exception as exc:
                logger.error(f"Erreur comptage torrent pour {platform_id} ({source_name or source_url}): {exc}")
                return 0
        name = d.get('game_name') or d.get('name') or d.get('title') or d.get('game')
        return 1 if name else 0

    if isinstance(data, list):
        total_items = max(1, len(data))
        for item_index, item in enumerate(data, start=1):
            if item_index == 1 or item_index == total_items or item_index % 250 == 0:
                _refresh_loading_feedback(
                    detail_lines=[
                        _("loading_platform_name").format(platform_id),
                        _("loading_game_entries_progress").format(item_index, total_items),
                        _("loading_read_games_resolve_sources"),
                    ],
                    force=True,
                )
            if isinstance(item, dict):
                count += count_from_dict(item)
            elif isinstance(item, (list, tuple)):
                torrent_source = _extract_torrent_source(item)
                if torrent_source is not None:
                    source_name, source_url = torrent_source
                    try:
                        count += _get_torrent_entry_count(
                            source_url,
                            display_label=source_name,
                            platform_id=platform_id,
                            allow_network_fetch=allow_torrent_manifest_fetch,
                        )
                    except Exception as exc:
                        logger.error(f"Erreur comptage torrent pour {platform_id} ({source_name or source_url}): {exc}")
                elif len(item) > 0:
                    count += 1
            elif isinstance(item, str):
                count += 1
            elif item is not None:
                count += 1
    elif isinstance(data, dict):
        count += count_from_dict(data)
    else:
        logger.warning(f"Format de fichier jeux inattendu pour {platform_id}: {type(data)}")

    _store_platform_game_count(platform_id, game_file, game_mtime_ns, game_size_bytes, count)
    return count



def _refresh_loading_feedback(current_system: str | None = None, progress: float | None = None, detail_lines=None, force: bool = False):
    """Refresh the blocking startup loading screen with more granular details."""
    try:
        if current_system is not None:
            config.current_loading_system = current_system
        if progress is not None:
            config.loading_progress = max(0.0, min(100.0, float(progress)))
        if detail_lines is not None:
            config.loading_detail_lines = [str(line) for line in detail_lines if line]
        config.needs_redraw = True

        if getattr(config, 'menu_state', '') != 'loading' or pygame is None:
            return

        now = time.time()
        last_update = float(getattr(config, '_loading_feedback_last_update', 0.0) or 0.0)
        if not force and (now - last_update) < 0.12:
            return
        config._loading_feedback_last_update = now

        screen = pygame.display.get_surface()
        if screen is None:
            return

        from display import draw_app_background, draw_loading_screen, draw_controls

        draw_app_background(screen)
        draw_loading_screen(screen)
        draw_controls(screen, config.menu_state, getattr(config, 'current_music_name', None), getattr(config, 'music_popup_start_time', 0))
        pygame.display.flip()
        pygame.event.pump()
    except Exception as exc:
        logger.debug(f"Impossible de rafraichir le loading screen: {exc}")





# Fonction pour charger sources.json
def load_sources(allow_torrent_manifest_fetch: bool | None = None):
    try:
        if allow_torrent_manifest_fetch is None:
            allow_torrent_manifest_fetch = is_torrent_manifest_refresh_requested()
        logger.debug(
            "Chargement des sources (%s fetch manifest torrent)",
            "avec" if allow_torrent_manifest_fetch else "sans",
        )
        sources = []
        if os.path.exists(config.SOURCES_FILE):
            with open(config.SOURCES_FILE, 'r', encoding='utf-8') as f:
                sources = json.load(f)
            if not isinstance(sources, list):
                logger.error("systems_list.json n'est pas une liste JSON valide")
                sources = []
        else:
            logger.warning(f"Fichier systems_list absent: {config.SOURCES_FILE}")

        sources_file_changed = False
        normalized_sources = []
        # S'assurer que chaque entrée possède les clés attendues.
        for raw_entry in sources:
            if not isinstance(raw_entry, dict):
                sources_file_changed = True
                continue

            s = dict(raw_entry)

            if "platform_image" not in s:
                # Supporter ancienne clé system_image -> platform_image si présente
                legacy = s.pop("system_image", "")
                s["platform_image"] = legacy or ""
                sources_file_changed = True

            # Normaliser clé dossier -> folder si besoin (legacy francophone)
            if "folder" not in s:
                legacy_folder = s.get("dossier") or s.get("folder_name")
                if legacy_folder:
                    s["folder"] = legacy_folder
                    sources_file_changed = True

            normalized_sources.append(s)

        sources = normalized_sources

        existing_names = {
            str(s.get("platform_name", "")).strip()
            for s in sources
            if isinstance(s, dict) and str(s.get("platform_name", "")).strip()
        }
        added = []
        if os.path.isdir(config.GAMES_FOLDER):
            for fname in sorted(os.listdir(config.GAMES_FOLDER)):
                if not fname.lower().endswith('.json'):
                    continue
                pname = os.path.splitext(fname)[0]
                if not pname or pname in existing_names:
                    continue
                new_entry = {"platform_name": pname, "folder": pname, "platform_image": ""}
                sources.append(new_entry)
                added.append(pname)
                existing_names.add(pname)
                sources_file_changed = True

        # Déterminer les plateformes orphelines (fichier manquant)
        existing_files = set()
        if os.path.isdir(config.GAMES_FOLDER):
            existing_files = {os.path.splitext(f)[0] for f in os.listdir(config.GAMES_FOLDER) if f.lower().endswith('.json')}
        removed = []
        runtime_sources = []
        for entry in sources:
            pname = entry.get("platform_name", "")
            # En runtime, garder seulement si un fichier existe.
            # Important: on ne supprime plus ces entrées du fichier systems_list.json,
            # car l'absence peut être transitoire pendant une mise à jour/extraction.
            if pname in existing_files:
                runtime_sources.append(entry)
            else:
                if pname:
                    removed.append(pname)

        if added:
            logger.info(f"Plateformes ajoutées automatiquement: {', '.join(added)}")
        if removed:
            logger.info(f"Plateformes ignorées en runtime (fichiers absents): {', '.join(removed)}")

        # Persister uniquement les changements non destructifs (ajouts / normalisations de clés).
        if sources_file_changed:
            try:
                # Pas de tri avant persistance: conserver ordre d'origine + ajouts fins
                os.makedirs(os.path.dirname(config.SOURCES_FILE), exist_ok=True)
                with open(config.SOURCES_FILE, 'w', encoding='utf-8') as f:
                    json.dump(sources, f, ensure_ascii=False, indent=2)
                logger.info("systems_list.json mis à jour (ajouts/normalisations, ordre conservé)")
            except Exception as e:
                logger.error(f"Échec écriture systems_list.json après maj auto: {e}")

        # Pour l'affichage on veut un tri alphabétique sans toucher l'ordre de persistance
        sorted_for_display = sorted(runtime_sources, key=lambda x: x.get("platform_name", "").lower())

        # Construire structures config: platform_dicts = ordre fichier, platforms = tri (avec filtre masqués)
        config.platform_dicts = runtime_sources  # ordre runtime (fichiers présents)
        settings = load_rgsx_settings()
        hidden = set(settings.get("hidden_platforms", [])) if isinstance(settings, dict) else set()
        all_sorted_names = [s.get("platform_name", "") for s in sorted_for_display]
        visible_names = [n for n in all_sorted_names if n and n not in hidden]

        # Masquer automatiquement les systèmes dont le dossier ROM n'existe pas (selon le toggle)
        unsupported = []
        try:
            from rgsx_settings import get_show_unsupported_platforms
            show_unsupported = get_show_unsupported_platforms(settings)
            sources_by_name = {s.get("platform_name", ""): s for s in sources if isinstance(s, dict)}
            for name in list(visible_names):
                entry = sources_by_name.get(name) or {}
                folder = entry.get("folder")
                # Conserver BIOS même sans dossier, et ignorer entrées sans folder
                bios_name = name.strip()
                if not folder or bios_name == "- BIOS by TMCTV -" or bios_name == "- BIOS":
                    continue
                expected_dir = os.path.join(config.ROMS_FOLDER, folder)
                if not os.path.isdir(expected_dir):
                    unsupported.append(name)
            if show_unsupported:
                config.unsupported_platforms = unsupported
            else:
                if unsupported:
                    # Filtrer la liste visible
                    visible_names = [n for n in visible_names if n not in set(unsupported)]
                    config.unsupported_platforms = unsupported
                    # Log concis + détaillé en DEBUG uniquement
                    logger.info(f"Plateformes masquées (dossier rom absent): {len(unsupported)}")
                    logger.debug("Détails plateformes masquées: " + ", ".join(unsupported))
                else:
                    config.unsupported_platforms = []
        except Exception as e:
            logger.error(f"Erreur détection plateformes non supportées (dossiers manquants): {e}")
            config.unsupported_platforms = []

        config.platforms = visible_names
        config.platform_names = {p: p for p in config.platforms}
        # Nouveau mapping par nom pour éviter décalages index après tri d'affichage
        try:
            config.platform_dict_by_name = {d.get("platform_name", ""): d for d in config.platform_dicts}
        except Exception:
            config.platform_dict_by_name = {}
        _load_persistent_platform_game_count_cache()
        config.games_count = {}
        total_platforms = max(1, len(config.platforms))
        for index, platform_name in enumerate(config.platforms, start=1):
            progress_value = 80.0 + ((index - 1) / total_platforms) * 19.0
            _refresh_loading_feedback(
                current_system=_("loading_load_systems"),
                progress=progress_value,
                detail_lines=[
                    _("loading_platform_counter").format(index, total_platforms),
                    platform_name,
                    _("loading_read_games_resolve_sources"),
                ],
                force=True,
            )
            config.games_count[platform_name] = get_platform_game_count(
                platform_name,
                allow_torrent_manifest_fetch=allow_torrent_manifest_fetch,
            )
            _refresh_loading_feedback(
                current_system=_("loading_load_systems"),
                progress=80.0 + (index / total_platforms) * 19.0,
                detail_lines=[
                    _("loading_platform_counter").format(index, total_platforms),
                    platform_name,
                    _("loading_read_games_resolve_sources"),
                ],
            )
        if config.games_count:
            try:
                summary = ", ".join([f"{name}: {count}" for name, count in config.games_count.items()])
                logger.debug(f"Nombre de jeux par système: {summary}")
            except Exception:
                pass
        if allow_torrent_manifest_fetch:
            clear_torrent_manifest_refresh_request()
        _refresh_loading_feedback(detail_lines=[], force=True)
        return sources
    except Exception as e:
        logger.error(f"Erreur fusion systèmes + détection jeux: {e}")
        return []


def load_games(platform_id:str) -> list[Game]:
    try:
        try:
            from rgsx_settings import get_global_sort_option

            current_sort_option = get_global_sort_option()
        except Exception:
            current_sort_option = 'name_asc'

        game_file, resolved_platform_dict, candidates = _resolve_game_file(platform_id)
        if not game_file:
            _games_cache.pop(platform_id, None)
            logger.warning(f"Aucun fichier de jeux trouvé pour {platform_id} (candidats: {candidates})")
            return []

        game_stat = os.stat(game_file)
        game_mtime_ns = game_stat.st_mtime_ns
        game_size_bytes = game_stat.st_size
        cached_entry = _games_cache.get(platform_id)
        if cached_entry and cached_entry.get("path") == game_file and cached_entry.get("mtime_ns") == game_mtime_ns:
            if cached_entry.get("sort_option") != current_sort_option:
                cached_entry["games"] = sort_games_list(list(cached_entry.get("games") or []), current_sort_option)
                cached_entry["sort_option"] = current_sort_option
            return cached_entry["games"]

        with open(game_file, 'r', encoding='utf-8') as f:
            data = json.load(f)

        # Si dict avec clé 'games'
        if isinstance(data, dict) and 'games' in data:
            data = data['games']

        normalized = []  # (name, url, size)

        def extract_from_dict(d):
            torrent_rows = _expand_torrent_source(d, platform_id)
            if torrent_rows is not None:
                normalized.extend(torrent_rows)
                return
            name = d.get('game_name') or d.get('name') or d.get('title') or d.get('game')
            url = d.get('url') or d.get('download') or d.get('link') or d.get('href')
            size = d.get('size') or d.get('filesize') or d.get('length')
            if name:
                normalized.append((str(name), url if isinstance(url, str) and url.strip() else None, str(size) if size else None))

        if isinstance(data, list):
            total_items = max(1, len(data))
            for item_index, item in enumerate(data, start=1):
                torrent_source = _extract_torrent_source(item)
                if (item_index == 1 or item_index == total_items or item_index % 100 == 0) and torrent_source is None:
                    _refresh_loading_feedback(
                        detail_lines=[
                            _("loading_platform_name").format(platform_id),
                            _("loading_game_entries_progress").format(item_index, total_items),
                            _("loading_read_games_resolve_sources"),
                        ],
                        force=True,
                    )
                if isinstance(item, (list, tuple)):
                    torrent_rows = _expand_torrent_source(item, platform_id)
                    if torrent_rows is not None:
                        normalized.extend(torrent_rows)
                        continue
                    if len(item) == 0:
                        continue
                    name = str(item[0])
                    url = item[1] if len(item) > 1 and isinstance(item[1], str) and item[1].strip() else None
                    size = item[2] if len(item) > 2 and isinstance(item[2], str) and item[2].strip() else None
                    normalized.append((name, url, size))
                elif isinstance(item, dict):
                    extract_from_dict(item)
                elif isinstance(item, str):
                    normalized.append((item, None, None))
                else:
                    normalized.append((str(item), None, None))
        elif isinstance(data, dict):  # dict sans 'games'
            extract_from_dict(data)
        else:
            logger.warning(f"Format de fichier jeux inattendu pour {platform_id}: {type(data)}")

        if getattr(config, "games_count_log_verbose", False):
            logger.debug(f"{os.path.basename(game_file)}: {len(normalized)} jeux")

        games_list: list[Game] = []
        for name, url, size in normalized:
            display_name = get_clean_display_name(name, platform_id)
            games_list.append(Game(name=name, url=url, size=size, display_name=display_name))

        games_list = sort_games_list(games_list, current_sort_option)

        _games_cache[platform_id] = {
            "path": game_file,
            "mtime_ns": game_mtime_ns,
            "sort_option": current_sort_option,
            "games": games_list,
        }
        _store_platform_game_count(platform_id, game_file, game_mtime_ns, game_size_bytes, len(games_list))
        return games_list
    except Exception as e:
        _games_cache.pop(platform_id, None)
        logger.error(f"Erreur lors du chargement des jeux pour {platform_id}: {e}")
        return []
