# -*- coding: utf-8 -*-
"""Cache sources/jeux (ETag + Last-Modified) + watchdog invalidation."""
import json
import time
import threading
import copy
import hashlib
import os
import logging
from datetime import datetime, timezone
from email.utils import formatdate, parsedate_to_datetime

import config
from utils import load_sources, load_games
from config import Game

try:
    from watchdog.observers import Observer  # type: ignore
    from watchdog.events import FileSystemEventHandler  # type: ignore
    WATCHDOG_AVAILABLE = True
except ImportError:  # pragma: no cover - optional dependency
    WATCHDOG_AVAILABLE = False

logger = logging.getLogger("rgsx_web")

# Cache configuration
CACHE_TTL_SECONDS = 60  # seconds

cache_lock = threading.RLock()

source_cache = {
    'data': None,
    'timestamp': 0.0,
    'etag': None,
    'last_modified': None,
}

games_cache = {}

watchdog_observer = None
watchdog_started = False


def _now_utc() -> datetime:
    """Return timezone-aware UTC datetime."""
    return datetime.now(timezone.utc)


def _httpdate(dt: datetime | None) -> str | None:
    """Convert datetime to an HTTP-date string."""
    if dt is None:
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    else:
        dt = dt.astimezone(timezone.utc)
    return formatdate(dt.timestamp(), usegmt=True)


def generate_etag(payload: object) -> str:
    """Generate a stable ETag for JSON-serialisable payloads."""
    try:
        serialized = json.dumps(payload, sort_keys=True, ensure_ascii=False, separators=(',', ':'), default=str)
    except TypeError:
        serialized = repr(payload)
    return hashlib.md5(serialized.encode('utf-8')).hexdigest()


def _ensure_datetime(value: datetime | str | None) -> datetime | None:
    """Return a timezone-aware datetime from mixed input."""
    if value is None:
        return None
    if isinstance(value, datetime):
        return value if value.tzinfo else value.replace(tzinfo=timezone.utc)
    try:
        dt = parsedate_to_datetime(value)
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt
    except (TypeError, ValueError):
        return None


def invalidate_all_caches(reason: str | None = None) -> None:
    """Drop all cached datasets."""
    with cache_lock:
        source_cache.update({'data': None, 'timestamp': 0.0, 'etag': None, 'last_modified': None})
        games_cache.clear()
    if reason and 'logger' in globals():
        logger.debug(f"Caches invalidated ({reason})")


def invalidate_games_cache(platform: str | None = None, reason: str | None = None) -> None:
    """Invalidate either a specific platform cache or all game caches."""
    with cache_lock:
        if platform is None:
            games_cache.clear()
        else:
            games_cache.pop(platform, None)
    if reason and 'logger' in globals():
        logger.debug(f"Games cache invalidated for {platform or 'ALL'} ({reason})")


def get_cached_sources() -> tuple[list[dict], str, datetime]:
    """Return cached platforms data with ETag and last modified timestamp."""
    now = time.time()
    with cache_lock:
        entry_data = source_cache['data']
        if entry_data is not None and now - source_cache['timestamp'] <= CACHE_TTL_SECONDS:
            return copy.deepcopy(entry_data), source_cache['etag'], source_cache['last_modified']

    platforms = load_sources()
    last_modified = _now_utc()
    etag = generate_etag(platforms)

    with cache_lock:
        source_cache.update({
            'data': copy.deepcopy(platforms),
            'timestamp': now,
            'etag': etag,
            'last_modified': last_modified,
        })

    return copy.deepcopy(platforms), etag, last_modified


def get_cached_games(platform: str) -> tuple[list[Game], str, datetime]:
    """Return cached games list for platform with metadata."""
    now = time.time()
    with cache_lock:
        entry = games_cache.get(platform)
        if entry and now - entry['timestamp'] <= CACHE_TTL_SECONDS:
            return copy.deepcopy(entry['data']), entry['etag'], entry['last_modified']

    games = load_games(platform)
    last_modified = _now_utc()
    etag = generate_etag(games)

    with cache_lock:
        games_cache[platform] = {
            'data': copy.deepcopy(games),
            'timestamp': now,
            'etag': etag,
            'last_modified': last_modified,
        }

    return copy.deepcopy(games), etag, last_modified


if WATCHDOG_AVAILABLE:

    class _CacheInvalidationHandler(FileSystemEventHandler):
        """Watchdog handler to invalidate caches when files change."""

        def on_any_event(self, event):  # type: ignore[override]
            if event.is_directory:
                return
            invalidate_all_caches(reason=f"filesystem event: {getattr(event, 'src_path', '')}")

else:

    class _CacheInvalidationHandler:  # pragma: no cover - fallback stub
        def __init__(self, *_, **__):
            pass


def start_cache_invalidation_watchdog() -> None:
    """Start filesystem watcher to keep caches in sync."""
    global watchdog_observer, watchdog_started

    if watchdog_started:
        return
    if not WATCHDOG_AVAILABLE:
        logger.info("watchdog package not available; relying on TTL cache invalidation")
        return

    observer = Observer()
    watched_paths = {
        os.path.dirname(config.SOURCES_FILE),
        config.GAMES_FOLDER,
        config.ROMS_FOLDER,
    }

    handler = _CacheInvalidationHandler()

    scheduled = False

    for path in watched_paths:
        if path and os.path.isdir(path):
            observer.schedule(handler, path=path, recursive=True)
            scheduled = True

    if scheduled:
        observer.daemon = True
        observer.start()
        watchdog_observer = observer
        watchdog_started = True
        logger.info("Cache invalidation watchdog started")
    else:
        logger.debug("No valid paths for cache watchdog; skipping watcher startup")
