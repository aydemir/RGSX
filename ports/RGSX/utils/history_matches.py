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

from utils.files import _get_dest_folder_name

_HISTORY_MATCH_NO_MATCH_LOG_COOLDOWN_SEC = 5.0

_HISTORY_MATCH_NO_MATCH_LOG_LAST_TS: dict[str, float] = {}

_HISTORY_MATCH_NO_MATCH_LOG_LOCK = threading.Lock()



def _history_no_match_log_key(game_name, platform_name) -> str:
    return f"{str(platform_name or '').strip().lower()}::{str(game_name or '').strip().lower()}"



def _log_history_no_match_once(game_name, platform_name) -> None:
    """Log no-match once per key during a short cooldown to avoid spam."""
    key = _history_no_match_log_key(game_name, platform_name)
    now_ts = time.time()
    should_log = False

    with _HISTORY_MATCH_NO_MATCH_LOG_LOCK:
        last_ts = _HISTORY_MATCH_NO_MATCH_LOG_LAST_TS.get(key, 0.0)
        if (now_ts - last_ts) >= _HISTORY_MATCH_NO_MATCH_LOG_COOLDOWN_SEC:
            _HISTORY_MATCH_NO_MATCH_LOG_LAST_TS[key] = now_ts
            should_log = True

    if should_log:
        logger.debug(
            "[HISTORY_MATCH_LOOKUP] no_match game=%s platform=%s",
            game_name,
            platform_name,
        )



def _clear_history_no_match_log_cooldown(game_name, platform_name) -> None:
    key = _history_no_match_log_key(game_name, platform_name)
    with _HISTORY_MATCH_NO_MATCH_LOG_LOCK:
        _HISTORY_MATCH_NO_MATCH_LOG_LAST_TS.pop(key, None)



def get_existing_history_matches(entry):
    """Return persisted moved paths that still exist for a history entry."""
    if not isinstance(entry, dict):
        return []

    moved_paths = entry.get("moved_paths", []) or []
    local_path = entry.get("local_path")
    local_filename = entry.get("local_filename")
    game_name = entry.get("game_name", "")
    if local_path:
        moved_paths = [local_path, *moved_paths]

    direct_matches = []
    direct_candidates = []
    if local_path:
        direct_candidates.append(str(local_path))

    platform_name = (entry.get("platform") or "").strip()
    base_path = None
    if platform_name:
        try:
            base_path = os.path.join(config.ROMS_FOLDER, _get_dest_folder_name(platform_name))
        except Exception:
            base_path = None

    if local_filename and base_path:
        direct_candidates.append(os.path.join(base_path, str(local_filename)))

    seen_direct = set()
    for candidate in direct_candidates:
        actual_path = os.path.abspath(str(candidate))
        normalized_path = os.path.normcase(actual_path)
        if normalized_path in seen_direct:
            continue
        seen_direct.add(normalized_path)
        exists = os.path.isfile(actual_path)
        if exists:
            direct_matches.append((os.path.basename(actual_path), actual_path))

    if direct_matches:
        _clear_history_no_match_log_cooldown(game_name, platform_name)
        return direct_matches

    candidate_paths = []
    for raw_path in moved_paths:
        if raw_path:
            candidate_paths.append(str(raw_path))
    if local_filename and base_path:
        candidate_paths.insert(0, os.path.join(base_path, str(local_filename)))

    matches = []
    seen_paths = set()

    for raw_path in candidate_paths:
        if not raw_path:
            continue

        raw_path = str(raw_path)
        fallback_paths = [os.path.abspath(raw_path)]
        if base_path:
            fallback_paths.append(os.path.join(base_path, os.path.basename(raw_path)))

        for actual_path in fallback_paths:
            normalized_path = os.path.normcase(actual_path)
            exists = os.path.isfile(actual_path)
            if normalized_path in seen_paths or not exists:
                continue

            seen_paths.add(normalized_path)
            matches.append((os.path.basename(actual_path), actual_path))
            break

    if not matches:
        _log_history_no_match_once(game_name, platform_name)
    else:
        _clear_history_no_match_log_cooldown(game_name, platform_name)

    return matches



def remember_history_local_match(entry, actual_filename, actual_path):
    """Persist a resolved local path for a history entry so later lookups are exact."""
    if not isinstance(entry, dict) or not actual_path:
        return False

    absolute_path = os.path.abspath(str(actual_path))
    filename = actual_filename or os.path.basename(absolute_path)
    changed = False

    if entry.get("local_path") != absolute_path:
        entry["local_path"] = absolute_path
        changed = True
    if entry.get("local_filename") != filename:
        entry["local_filename"] = filename
        changed = True

    moved_paths = entry.get("moved_paths")
    if not isinstance(moved_paths, list):
        moved_paths = []
    if absolute_path not in moved_paths:
        moved_paths.insert(0, absolute_path)
        changed = True
    entry["moved_paths"] = moved_paths

    if changed:
        try:
            from history import save_history

            save_history(config.history)
        except Exception as e:
            logger.debug(f"Impossible de mémoriser le chemin local de l'historique: {e}")
    return changed
