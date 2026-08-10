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

from utils.text import _format_size_bytes

_torrent_manifest_cache = {}

_torrent_manifest_cache_loaded = False

_torrent_manifest_cache_lock = threading.Lock()

_TORRENT_DOWNLOAD_SCHEME = "rgsx+torrent"



def _load_persistent_torrent_manifest_cache() -> None:
    global _torrent_manifest_cache_loaded, _torrent_manifest_cache
    if _torrent_manifest_cache_loaded:
        return
    with _torrent_manifest_cache_lock:
        if _torrent_manifest_cache_loaded:
            return
        cache_path = getattr(config, 'TORRENT_MANIFEST_CACHE_PATH', '')
        loaded_cache = {}
        try:
            if cache_path and os.path.exists(cache_path):
                with open(cache_path, 'r', encoding='utf-8') as handle:
                    payload = json.load(handle)
                if isinstance(payload, dict):
                    entries = payload.get('entries') if isinstance(payload.get('entries'), dict) else payload
                    if isinstance(entries, dict):
                        for source_url, cached_entries in entries.items():
                            if not isinstance(source_url, str) or not isinstance(cached_entries, list):
                                continue
                            safe_entries = []
                            for entry in cached_entries:
                                if not isinstance(entry, dict):
                                    continue
                                safe_entries.append({
                                    'name': str(entry.get('name') or ''),
                                    'path': str(entry.get('path') or ''),
                                    'download_path': str(entry.get('download_path') or entry.get('path') or ''),
                                    'index': int(entry.get('index') or 1),
                                    'size_bytes': int(entry.get('size_bytes') or 0),
                                    'source_url': str(entry.get('source_url') or source_url),
                                })
                            if safe_entries:
                                loaded_cache[source_url] = safe_entries
                logger.info(f"Cache torrent charge: {len(loaded_cache)} manifestes")
        except Exception as exc:
            logger.warning(f"Impossible de charger le cache torrent persistant: {exc}")
        _torrent_manifest_cache = loaded_cache
        _torrent_manifest_cache_loaded = True



def _save_persistent_torrent_manifest_cache() -> None:
    cache_path = getattr(config, 'TORRENT_MANIFEST_CACHE_PATH', '')
    if not cache_path:
        return
    with _torrent_manifest_cache_lock:
        try:
            os.makedirs(os.path.dirname(cache_path), exist_ok=True)
            payload = {
                'version': 1,
                'entries': _torrent_manifest_cache,
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
            logger.warning(f"Impossible de sauvegarder le cache torrent persistant: {exc}")
        finally:
            try:
                if 'temp_path' in locals() and os.path.exists(temp_path):
                    os.remove(temp_path)
            except Exception:
                pass



def clear_torrent_manifest_cache() -> None:
    global _torrent_manifest_cache, _torrent_manifest_cache_loaded
    with _torrent_manifest_cache_lock:
        _torrent_manifest_cache = {}
        _torrent_manifest_cache_loaded = True
        cache_path = getattr(config, 'TORRENT_MANIFEST_CACHE_PATH', '')
        try:
            if cache_path and os.path.exists(cache_path):
                os.remove(cache_path)
            logger.info("Cache torrent invalide")
        except Exception as exc:
            logger.warning(f"Impossible de supprimer le cache torrent: {exc}")



def request_torrent_manifest_refresh() -> None:
    marker_path = getattr(config, 'PENDING_TORRENT_REFRESH_MARKER_PATH', '')
    if not marker_path:
        return
    try:
        os.makedirs(os.path.dirname(marker_path), exist_ok=True)
        with open(marker_path, 'w', encoding='utf-8') as handle:
            handle.write(datetime.now().isoformat())
        logger.info("Rafraichissement complet des manifests torrent planifie")
    except Exception as exc:
        logger.warning(f"Impossible de planifier le rafraichissement torrent: {exc}")



def is_torrent_manifest_refresh_requested() -> bool:
    marker_path = getattr(config, 'PENDING_TORRENT_REFRESH_MARKER_PATH', '')
    return bool(marker_path and os.path.exists(marker_path))



def clear_torrent_manifest_refresh_request() -> None:
    marker_path = getattr(config, 'PENDING_TORRENT_REFRESH_MARKER_PATH', '')
    if not marker_path or not os.path.exists(marker_path):
        return
    try:
        os.remove(marker_path)
        logger.debug("Demande de rafraichissement torrent consommee")
    except Exception as exc:
        logger.warning(f"Impossible de supprimer le marqueur de rafraichissement torrent: {exc}")



def _get_torrent_entry_count(source_url: str, display_label: str | None = None, platform_id: str | None = None, allow_network_fetch: bool = True) -> int:
    _load_persistent_torrent_manifest_cache()
    cached = _torrent_manifest_cache.get(source_url)
    if cached is not None:
        return len(cached)
    if not allow_network_fetch:
        logger.debug(f"Comptage torrent differe (cache absent, fetch desactive): {platform_id or 'unknown'} -> {source_url}")
        return 0
    return len(_get_torrent_entries(source_url, display_label=display_label, platform_id=platform_id))



def _decode_bencode_text(value) -> str:
    if isinstance(value, bytes):
        for encoding in ("utf-8", "utf-8-sig", "latin-1"):
            try:
                return value.decode(encoding)
            except UnicodeDecodeError:
                continue
        return value.decode("utf-8", errors="replace")
    return str(value or "")



def _bdecode(data: bytes, index: int = 0):
    token = data[index:index + 1]
    if token == b"i":
        end = data.index(b"e", index)
        return int(data[index + 1:end]), end + 1
    if token == b"l":
        items = []
        index += 1
        while data[index:index + 1] != b"e":
            value, index = _bdecode(data, index)
            items.append(value)
        return items, index + 1
    if token == b"d":
        values = {}
        index += 1
        while data[index:index + 1] != b"e":
            key, index = _bdecode(data, index)
            value, index = _bdecode(data, index)
            values[key] = value
        return values, index + 1
    if token.isdigit():
        sep = data.index(b":", index)
        length = int(data[index:sep])
        start = sep + 1
        end = start + length
        return data[start:end], end
    raise ValueError(f"Invalid bencode token at offset {index}: {token!r}")



def is_torrent_manifest_url(url: str | None) -> bool:
    if not url or not isinstance(url, str):
        return False
    try:
        parsed = urllib.parse.urlparse(url.strip())
    except Exception:
        return False
    return (parsed.path or "").lower().endswith(".torrent")



def build_torrent_download_url(source_url: str, file_index: int, relative_path: str, size_bytes: int | None = None) -> str:
    params = {
        "source": source_url,
        "index": str(max(1, int(file_index))),
        "path": relative_path,
    }
    if isinstance(size_bytes, int) and size_bytes > 0:
        params["size"] = str(size_bytes)
    return f"{_TORRENT_DOWNLOAD_SCHEME}://download?{urllib.parse.urlencode(params, quote_via=urllib.parse.quote)}"



def is_torrent_download_url(url: str | None) -> bool:
    if not url or not isinstance(url, str):
        return False
    try:
        return urllib.parse.urlparse(url).scheme == _TORRENT_DOWNLOAD_SCHEME
    except Exception:
        return False



def parse_torrent_download_url(url: str | None) -> dict[str, str | int] | None:
    if not is_torrent_download_url(url):
        return None
    parsed = urllib.parse.urlparse(str(url))
    query = urllib.parse.parse_qs(parsed.query)
    source_url = (query.get("source") or [""])[0].strip()
    relative_path = (query.get("path") or [""])[0].strip()
    try:
        file_index = int((query.get("index") or ["1"])[0])
    except (TypeError, ValueError):
        file_index = 1
    try:
        size_bytes = int((query.get("size") or ["0"])[0])
    except (TypeError, ValueError):
        size_bytes = 0
    if not source_url or not relative_path:
        return None
    return {
        "source_url": source_url,
        "file_index": max(1, file_index),
        "relative_path": relative_path,
        "size_bytes": max(0, size_bytes),
    }



def _extract_torrent_entries_from_bytes(payload: bytes, source_url: str, display_label: str | None = None, platform_id: str | None = None) -> list[dict[str, str | int]]:
    from utils.games import _refresh_loading_feedback  # lazy: utils.torrent → utils.games döngüsünü önler
    torrent_data, next_index = _bdecode(payload)
    if not isinstance(torrent_data, dict):
        raise ValueError("Torrent root metadata is not a dictionary")

    info = torrent_data.get(b"info")
    if not isinstance(info, dict):
        raise ValueError("Torrent metadata does not contain an info dictionary")

    entries: list[dict[str, str | int]] = []
    files = info.get(b"files")
    root_name = _decode_bencode_text(info.get(b"name.utf-8") or info.get(b"name") or "").strip()
    if isinstance(files, list):
        total_files = len(files)
        resolved_label = display_label or root_name or Path(urllib.parse.urlparse(source_url).path).name or source_url
        for file_index, file_entry in enumerate(files, start=1):
            if not isinstance(file_entry, dict):
                continue
            if file_index == 1 or file_index == total_files or file_index % 25 == 0:
                detail_lines = []
                if platform_id:
                    detail_lines.append(_("loading_platform_name").format(platform_id))
                detail_lines.append(_("loading_torrent_files_progress").format(file_index, total_files))
                detail_lines.append(_("loading_torrent_manifest_analysis").format(resolved_label))
                _refresh_loading_feedback(detail_lines=detail_lines, force=True)
            path_parts = file_entry.get(b"path.utf-8") or file_entry.get(b"path") or []
            if not isinstance(path_parts, list):
                continue
            parts = [_decode_bencode_text(part).strip() for part in path_parts]
            parts = [part for part in parts if part]
            if not parts:
                continue
            full_path = "/".join(parts)
            download_path = "/".join([p for p in [root_name, full_path] if p])
            entries.append({
                "name": parts[-1],
                "path": full_path,
                "download_path": download_path or full_path,
                "index": file_index,
                "size_bytes": int(file_entry.get(b"length") or 0),
                "source_url": source_url,
            })
    else:
        if root_name:
            entries.append({
                "name": root_name,
                "path": root_name,
                "download_path": root_name,
                "index": 1,
                "size_bytes": int(info.get(b"length") or 0),
                "source_url": source_url,
            })

    duplicate_names = {}
    for entry in entries:
        name = str(entry["name"])
        duplicate_names[name] = duplicate_names.get(name, 0) + 1

    for entry in entries:
        if duplicate_names.get(str(entry["name"]), 0) > 1:
            entry["name"] = str(entry["path"])

    return entries



def _get_torrent_entries(source_url: str, display_label: str | None = None, platform_id: str | None = None) -> list[dict[str, str | int]]:
    _load_persistent_torrent_manifest_cache()
    cached = _torrent_manifest_cache.get(source_url)
    if cached is not None:
        return cached

    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36",
        "Accept": "*/*",
    }
    response = requests.get(source_url, headers=headers, timeout=30)
    response.raise_for_status()

    entries = _extract_torrent_entries_from_bytes(response.content, source_url, display_label=display_label, platform_id=platform_id)
    _torrent_manifest_cache[source_url] = entries
    _save_persistent_torrent_manifest_cache()
    return entries



def _extract_torrent_source(item) -> tuple[str, str] | None:
    if isinstance(item, (list, tuple)):
        if len(item) < 2:
            return None
        source_name = str(item[0] or "").strip()
        source_url = item[1] if isinstance(item[1], str) else None
        if source_url and is_torrent_manifest_url(source_url):
            return source_name, source_url.strip()
        return None

    if isinstance(item, dict):
        source_url = item.get("torrent_url") or item.get("url") or item.get("download") or item.get("link")
        if not isinstance(source_url, str) or not source_url.strip():
            return None
        source_type = str(item.get("type") or item.get("source_type") or item.get("source") or "").strip().lower()
        if source_type == "torrent" or is_torrent_manifest_url(source_url):
            source_name = item.get("game_name") or item.get("name") or item.get("title") or item.get("game") or item.get("label")
            if not source_name:
                parsed = urllib.parse.urlparse(source_url)
                source_name = urllib.parse.unquote(Path(parsed.path).name)
            return str(source_name or "").strip(), source_url.strip()

    return None



def _expand_torrent_source(item, platform_id: str) -> list[tuple[str, None, str | None]] | None:
    from utils.games import _refresh_loading_feedback  # lazy: utils.torrent → utils.games döngüsünü önler
    source = _extract_torrent_source(item)
    if not source:
        return None

    source_name, source_url = source
    try:
        _refresh_loading_feedback(
            detail_lines=[
                _("loading_platform_name").format(platform_id),
                _("loading_torrent_manifest_analysis").format(source_name or Path(urllib.parse.urlparse(source_url).path).name),
            ]
        )
        entries = _get_torrent_entries(source_url, display_label=source_name, platform_id=platform_id)
    except Exception as exc:
        label = source_name or source_url
        logger.error(f"Erreur chargement torrent pour {platform_id} ({label}): {exc}")
        return []

    expanded: list[tuple[str, None, str | None]] = []
    total_entries = max(1, len(entries))
    display_label = source_name or Path(urllib.parse.urlparse(source_url).path).name or source_url
    for position, entry in enumerate(entries, start=1):
        if position == 1 or position == total_entries or position % 25 == 0:
            _refresh_loading_feedback(
                detail_lines=[
                    _("loading_platform_name").format(platform_id),
                    _("loading_torrent_files_progress").format(position, total_entries),
                    _("loading_torrent_manifest_analysis").format(display_label),
                ],
                force=True,
            )
        game_name = str(entry.get("name") or "").strip()
        if not game_name:
            continue
        size_bytes = int(entry.get("size_bytes") or 0)
        file_index = int(entry.get("index") or 1)
        relative_path = str(entry.get("download_path") or entry.get("path") or game_name)
        download_url = build_torrent_download_url(source_url, file_index, relative_path, size_bytes)
        expanded.append((game_name, download_url, _format_size_bytes(size_bytes) if size_bytes > 0 else None))
    return expanded
