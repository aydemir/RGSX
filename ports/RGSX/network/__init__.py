"""network paketi — eski tek network.py dosyasinin modul bolunmus hali.

Faz 6-2: davranis degismez. Modul-seviyesi state (progress_queues, cancel_events,
pause_events, download_threads, torrent_temp_roots, _app_shutting_down,
urls_in_progress, urls_lock, url_results, url_done_events) AYNI obje kimligiyle
burada tutulur — thread_safety.py'deki `from network import pause_events` vs. ayni
objeyi gormeye devam eder. Tum isimler alt modullerden re-export edilir;
logger kimligi korunur (logger = logging.getLogger("network")).
"""

import logging
import threading

logger = logging.getLogger("network")

# --- modul-seviyesi state (kimlik korunur) ---
progress_queues = {}
cancel_events = {}
pause_events = {}  # {task_id: threading.Event} - Event is set when paused
download_threads = {}
# Chemins temp_root des téléchargements torrent en cours, indexés par task_id.
torrent_temp_roots: dict[str, str] = {}
# Flag global : True quand l'application est en cours d'arrêt propre.
_app_shutting_down: bool = False
# URLs actuellement en cours de téléchargement (pour éviter les doublons)
urls_in_progress = set()
urls_lock = threading.Lock()
# Résultats des URLs en cours de téléchargement (pour les doublons)
url_results = {}  # {url: (success, message)}
# Événements pour synchroniser les appels doublons (attendre la fin du premier)
url_done_events = {}  # {url: threading.Event}



# --- alt moduller (import sira: bagimlilik duzeni) ---
from network.helpers import (
    InsufficientDiskSpaceError,
    _is_arm_device,
    _warn_history_write_issue,
    _save_history_with_feedback,
    _check_history_access_before_download,
    _update_history_local_target,
    _is_ps3_redump_target,
    _postprocess_downloaded_file,
    _should_prefer_qbittorrent_backend,
    _download_torrent_manifest_to_file,
    _find_torrent_downloaded_file,
    _strip_ansi_escape_codes,
    _parse_known_size_to_bytes,
    _resolve_existing_path_for_usage,
    _get_free_disk_bytes,
    _build_low_disk_space_message,
    _notify_low_disk_space,
    _ensure_sufficient_disk_space,
    _lookup_known_game_size,
)

from network.updates import (
    cache,
    CACHE_TTL,
    test_internet,
    _normalize_release_notes,
    _extract_changelog_section,
    _fetch_recent_release_changelogs,
    _build_recent_changelog_text,
    _format_size,
    _set_loading_details,
    _safe_remove_file,
    _schedule_windows_file_replace_when_unlocked,
    _copy_windows_update_tree,
    _apply_pending_windows_update,
    apply_pending_update,
    check_for_updates,
    extract_update,
)

from network.upnp import (
    _reserve_ephemeral_tcp_port,
    _get_local_ip_for_route,
    _upnp_discover_igd,
    _upnp_get_control_url,
    _upnp_soap,
    _upnp_open_port,
    _upnp_close_port,
    _download_torrent_with_aria2,
    _update_seeding_status,
    _stop_seeding_status,
    _start_pending_torrent_seed_if_any,
    _discard_pending_torrent_seed,
)

from network.http_download import (
    _build_browser_download_headers,
    _default_referer_for_url,
    _is_browser_challenge_response,
    _redact_headers,
    _extract_vimm_download_info,
    _fetch_vimm_download_info,
    _get_vimm_file_size,
    _http_part_path,
    _http_resume_offset,
    _http_parse_content_range,
    _stream_response_to_path,
)

from network.lolroms import (
    _is_lolroms_url,
    _normalize_lolroms_url,
    _extract_reported_total_size,
    _build_lolroms_parent_url,
    _looks_like_html_or_challenge,
    _should_accept_partial_archive,
    _matches_expected_archive_signature,
    _resolve_lolroms_external_command,
    _probe_lolroms_remote_size,
    _download_lolroms_with_external_tool,
)

from network.archive_org import (
    _split_archive_org_path,
    _normalize_archive_org_download_path,
    _try_archive_org_alternate_urls,
)

from network.one_fichier import (
    WAIT_REGEXES_1F,
    extract_wait_seconds_1f,
    _extract_visible_text_from_html,
    _normalize_1fichier_text,
    _translate_free_mode_message,
    _append_1fichier_upgrade_advice,
    _extract_1fichier_free_mode_block_reason,
    download_1fichier_free_mode,
    download_from_1fichier,
    is_1fichier_url,
)

from network.queue import (
    download_queue_worker,
    notify_download_finished,
    request_cancel,
    _find_stray_torrent_temp_roots,
    cleanup_torrent_temp,
    _cleanup_torrent_resume_artifacts,
    _cleanup_seeder_local_artifacts,
    stop_active_seeder,
    toggle_pause_download,
    is_download_paused,
    _set_bulk_history_status,
    pause_all_downloads,
    resume_all_downloads,
    is_any_download_paused,
    cancel_all_downloads,
    shutdown_downloads,
    download_rom,
)

from network.download_state import (
    DownloadJob,
    DownloadState,
    DownloadEvent,
    IllegalTransitionError,
    classify_error,
    emit_state_event,
    get_state_emitter,
    is_active_state,
    legacy_history_status,
    retry_backoff_seconds,
    retryable,
    set_state_emitter,
    state_from_legacy,
    transition,
)

