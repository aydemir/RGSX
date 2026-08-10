"""network.helpers — Genel yardimcilar (history feedback, disk, postprocess, torrent helper).

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import os
import re
import time
import tempfile
import logging
import requests  # type: ignore
import config
import qbittorrent_backend
from history import save_history, check_history_write_access, get_history_write_status
from display import show_toast
from language import _  # Import de la fonction de traduction
from utils import (
    extract_zip,
    extract_rar,
    extract_7z,
    handle_ps3,
    load_games,
    get_disk_usage,
)
from network.updates import _format_size
logger = logging.getLogger("network")


class InsufficientDiskSpaceError(Exception):
    """Disk alani yetersiz oldugunda indirme hatasi (pre-existing NameError fix: bu
    isim eski network.py'de raise/except icinde kullaniliyordu ama hic tanimli degildi)."""
    pass


def _is_arm_device() -> bool:
    architecture = str(getattr(config, "SYSTEM_INFO", {}).get("architecture") or "").lower().strip()
    if not architecture:
        try:
            import platform as _platform
            architecture = (_platform.machine() or "").lower().strip()
        except Exception:
            architecture = ""
    return any(token in architecture for token in ("arm", "aarch64", "arm64", "armv7", "armv8"))
def _warn_history_write_issue(context=""):
    status = get_history_write_status() or {}
    message = status.get("message") or "Erreur ecriture history.json. Le telechargement continue sans historique temps reel."
    if context:
        logger.error(f"[{context}] {message}")
    else:
        logger.error(message)

    try:
        config.history_write_ok = False
        config.history_write_error = message
    except Exception:
        pass

    now = time.time()
    last_toast = float(getattr(config, "history_write_last_toast_at", 0.0) or 0.0)
    if now - last_toast >= 8.0:
        try:
            show_toast(message, duration=5000)
        except Exception:
            pass
        try:
            config.history_write_last_toast_at = now
        except Exception:
            pass
def _save_history_with_feedback(context=""):
    saved = save_history(config.history)
    if not saved:
        _warn_history_write_issue(context)
    return saved
def _check_history_access_before_download(context="download"):
    ok, _ = check_history_write_access(force=True)
    if not ok:
        _warn_history_write_issue(f"{context}:precheck")
    return ok
def _update_history_local_target(url: str, task_id: str, dest_path: str) -> None:
    if not isinstance(config.history, list) or not dest_path:
        return

    absolute_path = os.path.abspath(dest_path)
    basename = os.path.basename(absolute_path)
    normalized_task_id = str(task_id or "")

    for entry in config.history:
        if entry.get("url") != url:
            continue
        entry_task_id = str(entry.get("task_id") or "")
        if normalized_task_id and entry_task_id and entry_task_id != normalized_task_id:
            continue

        entry["local_path"] = absolute_path
        entry["local_filename"] = basename

        moved_paths = entry.get("moved_paths")
        if not isinstance(moved_paths, list):
            moved_paths = []
        if absolute_path not in moved_paths:
            moved_paths.insert(0, absolute_path)
        entry["moved_paths"] = moved_paths
        _save_history_with_feedback("download:local_target")
        break
def _is_ps3_redump_target(platform_folder, platform) -> bool:
    try:
        ps3_platforms = {"ps3", "PlayStation 3"}
        return platform_folder == "ps3" or platform in ps3_platforms
    except Exception:
        return False
def _postprocess_downloaded_file(dest_path: str, dest_dir: str, url: str, game_name: str, is_ps3_target: bool):
    extension = os.path.splitext(dest_path)[1].lower()
    is_vimm_source = 'vimm.net' in str(url or '').lower()

    def _append_ps3_suffix_for_vimm_folder() -> tuple[bool, str]:
        archive_base = os.path.splitext(os.path.basename(dest_path))[0].strip()
        if not archive_base:
            return False, ""

        direct_candidate = os.path.join(dest_dir, archive_base)
        source_dir = ""
        if os.path.isdir(direct_candidate):
            source_dir = direct_candidate
        else:
            try:
                for entry in os.listdir(dest_dir):
                    candidate = os.path.join(dest_dir, entry)
                    if os.path.isdir(candidate) and entry.lower() == archive_base.lower():
                        source_dir = candidate
                        break
            except Exception:
                source_dir = ""

        if not source_dir:
            return False, ""

        if source_dir.lower().endswith('.ps3'):
            return False, source_dir

        target_dir = source_dir + '.ps3'
        if os.path.exists(target_dir):
            logger.warning(f"Suffixe .ps3 non appliqué: dossier cible déjà existant ({target_dir})")
            return False, target_dir

        os.replace(source_dir, target_dir)
        return True, target_dir

    if extension == ".zip":
        success, msg = extract_zip(dest_path, dest_dir, url)
    elif extension == ".rar":
        success, msg = extract_rar(dest_path, dest_dir, url)
    elif extension == ".7z":
        success, msg = extract_7z(dest_path, dest_dir, url)
    elif extension == ".iso" and is_ps3_target:
        logger.debug(f"Traitement PS3 direct ISO déclenché pour {dest_path}")
        success, msg = handle_ps3(
            dest_dir=dest_dir,
            new_dirs=[],
            extracted_basename=os.path.splitext(os.path.basename(dest_path))[0],
            url=url,
            archive_name=os.path.basename(dest_path),
        )
    else:
        logger.warning(f"Type d'archive non supporté: {extension}")
        return True, _("network_download_ok").format(game_name)

    if success:
        if is_ps3_target and is_vimm_source and extension in {'.zip', '.rar', '.7z'}:
            try:
                renamed, renamed_path = _append_ps3_suffix_for_vimm_folder()
                if renamed:
                    logger.info(f"PS3 Vimm: dossier extrait renommé avec suffixe .ps3 -> {renamed_path}")
                else:
                    logger.debug("PS3 Vimm: aucun renommage .ps3 nécessaire (dossier introuvable, déjà suffixé, ou cible existante)")
            except Exception as rename_exc:
                logger.warning(f"PS3 Vimm: impossible d'ajouter le suffixe .ps3: {rename_exc}")

        logger.debug(f"Post-traitement réussi pour {dest_path}: {msg}")
        return True, _("network_download_extract_ok").format(game_name)

    logger.error(f"Erreur post-traitement pour {dest_path}: {msg}")
    return False, _("network_extraction_failed").format(msg)
def _should_prefer_qbittorrent_backend() -> bool:
    """Indique si le backend qBittorrent doit être essayé en priorité pour un torrent."""
    if config.OPERATING_SYSTEM not in ("Windows", "Linux"):
        return False
    return qbittorrent_backend.is_available()
def _download_torrent_manifest_to_file(source_url: str) -> str:
    from network.http_download import _build_browser_download_headers  # lazy: network.helpers <-> 1fichier/http_download dongusunu onler
    headers = _build_browser_download_headers(referer="https://archive.org/", accept="*/*")
    response = requests.get(source_url, timeout=30, headers=headers)
    response.raise_for_status()
    fd, temp_path = tempfile.mkstemp(prefix="rgsx_torrent_", suffix=".torrent")
    with os.fdopen(fd, 'wb') as handle:
        handle.write(response.content)
    return temp_path
def _find_torrent_downloaded_file(download_root: str, relative_path: str, fallback_name: str) -> str | None:
    normalized_parts = [p for p in relative_path.replace('\\', '/').split('/') if p not in ('', '.')]
    expected = os.path.join(download_root, *normalized_parts) if normalized_parts else os.path.join(download_root, fallback_name)
    if os.path.exists(expected):
        return expected

    # Recherche stricte sur le nom exact (avec extension)
    exact_filename = os.path.basename(relative_path) or fallback_name
    for current_root, _, files in os.walk(download_root):
        for file_name in files:
            if file_name == exact_filename:
                return os.path.join(current_root, file_name)
    return None
def _strip_ansi_escape_codes(text: str) -> str:
    if not text:
        return ""
    return re.sub(r'\x1B\[[0-?]*[ -/]*[@-~]', '', text)
def _parse_known_size_to_bytes(value) -> int:
    if isinstance(value, (int, float)):
        return max(0, int(value))
    if not isinstance(value, str):
        return 0

    text = value.strip().replace(',', '.')
    if not text:
        return 0

    match = re.match(r'^([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z]+)?$', text)
    if not match:
        return 0

    amount = float(match.group(1))
    unit = (match.group(2) or 'B').strip().lower()
    multipliers = {
        'b': 1,
        'byte': 1,
        'bytes': 1,
        'octet': 1,
        'octets': 1,
        'kb': 1024,
        'kib': 1024,
        'ko': 1024,
        'mb': 1024 ** 2,
        'mib': 1024 ** 2,
        'mo': 1024 ** 2,
        'gb': 1024 ** 3,
        'gib': 1024 ** 3,
        'go': 1024 ** 3,
        'tb': 1024 ** 4,
        'tib': 1024 ** 4,
        'to': 1024 ** 4,
        'pb': 1024 ** 5,
        'pib': 1024 ** 5,
        'po': 1024 ** 5,
    }
    return int(amount * multipliers.get(unit, 0)) if unit in multipliers else 0
def _resolve_existing_path_for_usage(path: str) -> str:
    resolved_path = os.path.abspath(path or "")
    while resolved_path and not os.path.exists(resolved_path):
        parent_path = os.path.dirname(resolved_path)
        if not parent_path or parent_path == resolved_path:
            break
        resolved_path = parent_path
    return resolved_path
def _get_free_disk_bytes(path: str) -> int | None:
    try:
        usage_path = _resolve_existing_path_for_usage(path)
        if not usage_path or not os.path.exists(usage_path):
            return None
        usage = get_disk_usage(usage_path)
        return int(usage.free) if usage else None
    except Exception as exc:
        logger.debug(f"Impossible de lire l'espace disque libre pour {path}: {exc}")
        return None
def _build_low_disk_space_message(free_bytes: int, required_bytes: int, popup: bool = False) -> str:
    free_text = _format_size(max(0, int(free_bytes or 0)))
    required_text = _format_size(max(0, int(required_bytes or 0)))
    key = "popup_low_disk_space" if popup else "error_low_disk_space"
    template = _(key) if _ else ""
    if template and template != key:
        try:
            return template.format(free=free_text, required=required_text)
        except Exception:
            try:
                return template.format(free_text, required_text)
            except Exception:
                pass
    if popup:
        return f"Download blocked: low disk space ({free_text} free / {required_text} required)"
    return f"Low disk space ({free_text} free / {required_text} required)"
def _notify_low_disk_space(required_bytes: int, free_bytes: int) -> str:
    popup_message = _build_low_disk_space_message(free_bytes, required_bytes, popup=True)
    history_message = _build_low_disk_space_message(free_bytes, required_bytes, popup=False)
    try:
        config.popup_message = popup_message
        config.popup_timer = 5000
        config.needs_redraw = True
    except Exception:
        pass
    try:
        show_toast(popup_message, duration=5000)
    except Exception:
        pass
    return history_message
def _ensure_sufficient_disk_space(dest_dir: str, required_bytes: int) -> tuple[bool, str | None]:
    # Unit safety: both required_bytes and free bytes are compared in raw bytes.
    required = max(0, int(required_bytes or 0))
    if required <= 0:
        return True, None

    free_bytes = _get_free_disk_bytes(dest_dir)
    if free_bytes is None:
        return True, None

    try:
        from utils import get_disk_usage as _get_disk_usage_for_log
        _get_disk_usage_for_log(dest_dir, log=True)
    except Exception:
        pass

    if free_bytes < required:
        return False, _notify_low_disk_space(required, free_bytes)

    return True, None
def _lookup_known_game_size(platform: str, game_name: str, url: str | None = None) -> int:
    try:
        for game in load_games(platform):
            if url and game.url == url:
                return _parse_known_size_to_bytes(game.size)
            if game.name == game_name:
                return _parse_known_size_to_bytes(game.size)
    except Exception as exc:
        logger.debug(f"Impossible de déterminer la taille connue pour {platform}/{game_name}: {exc}")
    return 0
