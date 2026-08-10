"""network.queue — Indirme kuyrugu/worker + pause/resume/cancel/shutdown + download_rom.

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import asyncio
import os
import queue
import shutil
import threading
import time
import urllib.parse
import logging
import datetime
from datetime import datetime
import requests  # type: ignore
import config
import qbittorrent_backend
from history import load_history, save_history
from display import show_toast
from language import _  # Import de la fonction de traduction
from utils import (
    sanitize_filename,
    load_archive_org_cookie,
    resolve_platform_folder,
    get_clean_display_name,
    parse_torrent_download_url,
)
from network import (
    progress_queues,
    cancel_events,
    pause_events,
    download_threads,
    torrent_temp_roots,
    urls_in_progress,
    urls_lock,
    url_results,
    url_done_events,
)
from network.helpers import (
    InsufficientDiskSpaceError,
    _check_history_access_before_download,
    _ensure_sufficient_disk_space,
    _is_arm_device,
    _is_ps3_redump_target,
    _lookup_known_game_size,
    _postprocess_downloaded_file,
    _save_history_with_feedback,
    _should_prefer_qbittorrent_backend,
    _update_history_local_target,
)
from network.http_download import (
    _default_referer_for_url,
    _fetch_vimm_download_info,
    _get_vimm_file_size,
    _http_parse_content_range,
    _http_part_path,
    _http_resume_offset,
    _is_browser_challenge_response,
    _redact_headers,
    _stream_response_to_path,
)
from network.lolroms import (
    _download_lolroms_with_external_tool,
    _is_lolroms_url,
    _looks_like_html_or_challenge,
    _matches_expected_archive_signature,
    _probe_lolroms_remote_size,
    _should_accept_partial_archive,
)
from network.archive_org import (
    _split_archive_org_path,
    _try_archive_org_alternate_urls,
)
from network.updates import _safe_remove_file


logger = logging.getLogger("network")

def download_queue_worker():
    """Worker qui surveille la file d'attente et lance des téléchargements dans la limite des slots disponibles."""
    from network.one_fichier import download_from_1fichier, is_1fichier_url  # lazy: network.queue <-> 1fichier/http_download dongusunu onler
    import time
    while True:
        try:
            max_dl = getattr(config, 'max_simultaneous_downloads', 5)
            active = getattr(config, 'active_download_count', 0)
            if active < max_dl and config.download_queue:
                job = config.download_queue.pop(0)
                config.active_download_count = active + 1
                config.download_active = True
                logger.info(f"[QUEUE] Lancement téléchargement (slot {active+1}/{max_dl}): {job.get('game_name','?')}")
                url = job['url']
                platform = job['platform']
                game_name = job['game_name']
                is_zip_non_supported = job.get('is_zip_non_supported', False)
                task_id = job.get('task_id') or f"queue_{int(time.time()*1000)}"
                if is_1fichier_url(url):
                    t = threading.Thread(target=lambda: asyncio.run(download_from_1fichier(url, platform, game_name, is_zip_non_supported, task_id)), daemon=True)
                else:
                    t = threading.Thread(target=lambda: asyncio.run(download_rom(url, platform, game_name, is_zip_non_supported, task_id)), daemon=True)
                t.start()
            time.sleep(1)
        except Exception as e:
            logger.error(f"[QUEUE] Erreur dans le worker de file d'attente: {e}")
            time.sleep(2)
def notify_download_finished():
    config.active_download_count = max(0, getattr(config, 'active_download_count', 1) - 1)
    config.download_active = config.active_download_count > 0
def request_cancel(task_id: str) -> bool:
    """Request cancellation for a running download task by its task_id."""
    ev = cancel_events.get(task_id)
    if ev is not None:
        try:
            ev.set()
            logger.debug(f"Cancel requested for task_id={task_id}")
            return True
        except Exception as e:
            logger.debug(f"Failed to set cancel for task_id={task_id}: {e}")
            return False
    logger.debug(f"No cancel event found for task_id={task_id}")
    return False
def _find_stray_torrent_temp_roots(stable_key: str) -> list[str]:
    """Recherche tous les dossiers '.rgsx_torrent/<stable_key>' existants sous les
    dossiers de plateformes de ROMS_FOLDER.

    Utile car le dossier de destination résolu pour une plateforme peut varier
    d'une session à l'autre (ex: bug historique de résolution de dossier avant
    chargement de config.platform_dicts), laissant des dossiers de reprise
    orphelins dans un ancien emplacement. On les recherche tous pour un nettoyage
    complet, plutôt que de se fier uniquement au dernier temp_root connu.
    """
    found: list[str] = []
    if not stable_key:
        return found
    roms_root = getattr(config, 'ROMS_FOLDER', '') or ''
    if not roms_root or not os.path.isdir(roms_root):
        return found
    try:
        for entry in os.listdir(roms_root):
            platform_dir = os.path.join(roms_root, entry)
            if not os.path.isdir(platform_dir):
                continue
            candidate = os.path.join(platform_dir, ".rgsx_torrent", stable_key)
            if os.path.isdir(candidate):
                found.append(candidate)
    except Exception as e:
        logger.debug(f"_find_stray_torrent_temp_roots: erreur scan {roms_root}: {e}")
    return found
def cleanup_torrent_temp(task_id: str) -> bool:
    """Supprime le dossier temporaire torrent associé à task_id.
    
    À appeler lors d'une annulation explicite par l'utilisateur (via l'UI),
    notamment quand cancel_events ne contient plus le task_id (téléchargement
    terminé côté thread mais statut UI encore 'Téléchargement').
    Retourne True si un dossier a été supprimé.
    """
    temp_root = torrent_temp_roots.pop(task_id, None)
    removed_any = False
    if temp_root and os.path.isdir(temp_root):
        try:
            shutil.rmtree(temp_root, ignore_errors=True)
            logger.debug(f"cleanup_torrent_temp: dossier supprimé {temp_root}")
            removed_any = True
        except Exception as e:
            logger.debug(f"cleanup_torrent_temp: erreur suppression {temp_root}: {e}")

    # Nettoyer aussi les éventuels dossiers orphelins du même torrent situés sous
    # un autre dossier de plateforme (ex: résolution de dossier différente d'une
    # session à l'autre).
    if temp_root:
        stable_key = os.path.basename(temp_root.rstrip("\\/"))
        for stray_root in _find_stray_torrent_temp_roots(stable_key):
            if os.path.normcase(os.path.abspath(stray_root)) == os.path.normcase(os.path.abspath(temp_root)):
                continue
            try:
                shutil.rmtree(stray_root, ignore_errors=True)
                logger.debug(f"cleanup_torrent_temp: dossier orphelin supprimé {stray_root}")
                removed_any = True
            except Exception as e:
                logger.debug(f"cleanup_torrent_temp: erreur suppression orpheline {stray_root}: {e}")

    return removed_any
def _cleanup_torrent_resume_artifacts(source_url: str | None, file_index: int | None, dest_path: str | None) -> bool:
    """Supprime les artefacts de reprise torrent (.rgsx_torrent/*) pour un fichier terminé."""
    if not source_url or not dest_path:
        return False

    removed_any = False
    try:
        import hashlib as _hashlib
        stable_key = _hashlib.md5(f"{source_url}|{int(file_index or 1)}".encode()).hexdigest()[:12]
        dest_dir = os.path.dirname(dest_path) or os.path.abspath(config.ROMS_FOLDER)
        rom_root = os.path.abspath(dest_dir)
        temp_parent = os.path.join(rom_root, ".rgsx_torrent")
        temp_root = os.path.join(temp_parent, stable_key)

        if os.path.isdir(temp_root):
            shutil.rmtree(temp_root, ignore_errors=True)
            removed_any = True
            logger.debug(f"_cleanup_torrent_resume_artifacts: dossier supprimé {temp_root}")

        if os.path.isdir(temp_parent):
            try:
                if not os.listdir(temp_parent):
                    os.rmdir(temp_parent)
                    logger.debug(f"_cleanup_torrent_resume_artifacts: dossier vide supprimé {temp_parent}")
            except Exception:
                pass

        # Nettoyer aussi les dossiers orphelins du même torrent sous un autre
        # dossier de plateforme (résolution de dossier incohérente d'une session
        # à l'autre).
        for stray_root in _find_stray_torrent_temp_roots(stable_key):
            if os.path.normcase(os.path.abspath(stray_root)) == os.path.normcase(os.path.abspath(temp_root)):
                continue
            try:
                shutil.rmtree(stray_root, ignore_errors=True)
                removed_any = True
                logger.debug(f"_cleanup_torrent_resume_artifacts: dossier orphelin supprimé {stray_root}")
                stray_parent = os.path.dirname(stray_root)
                if os.path.isdir(stray_parent) and not os.listdir(stray_parent):
                    os.rmdir(stray_parent)
            except Exception as e:
                logger.debug(f"_cleanup_torrent_resume_artifacts: erreur suppression orpheline {stray_root}: {e}")
    except Exception as exc:
        logger.debug(f"_cleanup_torrent_resume_artifacts: erreur nettoyage: {exc}")

    return removed_any
def _cleanup_seeder_local_artifacts(dest_path: str | None, relative_path: str | None, seed_work_dir: str | None, temp_manifest: str | None) -> bool:
    """Supprime les artefacts locaux d'un seeding (fichiers .aria2, liens et manifest)."""
    removed_any = False

    candidates: set[str] = set()
    if dest_path:
        candidates.add(f"{dest_path}.aria2")
        if relative_path:
            torrent_basename = os.path.basename(relative_path)
            if torrent_basename:
                candidates.add(os.path.join(os.path.dirname(dest_path), f"{torrent_basename}.aria2"))

    for candidate in candidates:
        try:
            if os.path.isfile(candidate):
                os.remove(candidate)
                removed_any = True
                logger.debug(f"_cleanup_seeder_local_artifacts: fichier supprimé {candidate}")
        except Exception as exc:
            logger.debug(f"_cleanup_seeder_local_artifacts: erreur suppression {candidate}: {exc}")

    if temp_manifest:
        try:
            if os.path.isfile(temp_manifest):
                os.remove(temp_manifest)
                removed_any = True
                logger.debug(f"_cleanup_seeder_local_artifacts: manifest supprimé {temp_manifest}")
        except Exception as exc:
            logger.debug(f"_cleanup_seeder_local_artifacts: erreur suppression manifest {temp_manifest}: {exc}")

    if seed_work_dir and os.path.isdir(seed_work_dir):
        try:
            shutil.rmtree(seed_work_dir, ignore_errors=True)
            removed_any = True
            logger.debug(f"_cleanup_seeder_local_artifacts: dossier seed supprimé {seed_work_dir}")
        except Exception as exc:
            logger.debug(f"_cleanup_seeder_local_artifacts: erreur suppression dossier seed {seed_work_dir}: {exc}")

    return removed_any
def stop_active_seeder(task_id: str | None = None, original_history_url: str | None = None) -> bool:
    """Stoppe un seed qBittorrent actif par task_id ou URL historique."""
    try:
        if qbittorrent_backend.has_active_seed(task_id, original_history_url):
            return qbittorrent_backend.stop_seed(task_id=task_id, original_history_url=original_history_url)
    except Exception as exc:
        logger.debug(f"stop_active_seeder: arrêt seed qBittorrent échoué: {exc}")
    return False
def toggle_pause_download(task_id: str) -> bool:
    """Toggle pause state for a running download task. Returns True if now paused, False if resumed."""
    ev = pause_events.get(task_id)
    if ev is None:
        # Créer l'événement de pause s'il n'existe pas
        pause_events[task_id] = threading.Event()
        ev = pause_events[task_id]
    
    if ev.is_set():
        # Actuellement en pause, reprendre
        ev.clear()
        logger.debug(f"Download resumed for task_id={task_id}")
        return False  # Retourne False = pas en pause (repris)
    else:
        # Actuellement actif, mettre en pause
        ev.set()
        logger.debug(f"Download paused for task_id={task_id}")
        return True  # Retourne True = en pause
def is_download_paused(task_id: str) -> bool:
    """Check if a download is currently paused."""
    ev = pause_events.get(task_id)
    if ev is not None:
        return ev.is_set()
    return False
def _set_bulk_history_status(from_statuses: tuple, to_status: str, message: str | None) -> int:
    """Met à jour le statut des entrées d'historique (chargé depuis le disque)."""
    try:
        history = load_history() or []
    except Exception:
        return 0
    changed = 0
    for entry in history:
        if entry.get("status") in from_statuses:
            entry["status"] = to_status
            if message:
                entry["message"] = message
            changed += 1
    if changed:
        try:
            save_history(history)
            if isinstance(getattr(config, "history", None), list):
                config.history = history
        except Exception as e:
            logger.debug(f"_set_bulk_history_status: save échec: {e}")
    return changed
def pause_all_downloads() -> int:
    """Met en pause tous les téléchargements actifs (HTTP direct + torrent qBittorrent)."""
    task_ids = set()
    for tid in list(download_threads.keys()):
        task_ids.add(tid)
    for entry in list(getattr(config, "history", []) or []):
        if entry.get("status") in ("Downloading", "Téléchargement", "Connecting", "Extracting"):
            tid = entry.get("task_id")
            if tid:
                task_ids.add(tid)
    paused = 0
    for tid in task_ids:
        try:
            pause_events.setdefault(tid, threading.Event()).set()
            paused += 1
        except Exception:
            pass
    if paused:
        try:
            _set_bulk_history_status(
                ("Downloading", "Téléchargement", "Connecting", "Extracting"),
                "Paused",
                (_("download_paused") if _ else "Download paused"),
            )
        except Exception:
            pass
        logger.info(f"[PAUSE] {paused} téléchargement(s) mis en pause")
    return paused
def resume_all_downloads() -> int:
    """Reprend tous les téléchargements mis en pause."""
    task_ids = set()
    for tid in list(download_threads.keys()):
        task_ids.add(tid)
    for entry in list(getattr(config, "history", []) or []):
        if entry.get("status") == "Paused":
            tid = entry.get("task_id")
            if tid:
                task_ids.add(tid)
    resumed = 0
    for tid in task_ids:
        ev = pause_events.get(tid)
        if ev is not None:
            try:
                ev.clear()
                resumed += 1
            except Exception:
                pass
    if resumed:
        try:
            _set_bulk_history_status(("Paused",), "Downloading", None)
        except Exception:
            pass
        logger.info(f"[PAUSE] {resumed} téléchargement(s) repris")
    return resumed
def is_any_download_paused() -> bool:
    """True si au moins un téléchargement actif est en pause."""
    for tid in list(download_threads.keys()):
        ev = pause_events.get(tid)
        if ev is not None and ev.is_set():
            return True
    return False
def cancel_all_downloads():
    """Cancel all active downloads and queued downloads, and attempt to stop threads quickly."""
    # Annuler tous les téléchargements actifs via cancel_events
    for tid, ev in list(cancel_events.items()):
        try:
            ev.set()
        except Exception:
            pass
    # Optionally join threads briefly
    for tid, th in list(download_threads.items()):
        try:
            if th.is_alive():
                th.join(timeout=0.2)
        except Exception:
            pass
    
    # Vider la file d'attente des téléchargements
    config.download_queue.clear()
    config.download_active = False
    
    # Mettre à jour l'historique pour annuler les téléchargements en statut "Queued"
    try:
        history = load_history()
        for entry in history:
            if entry.get("status") == "Queued":
                entry["status"] = "Canceled"
                entry["message"] = _("download_canceled")
                logger.info(f"Téléchargement en attente annulé : {entry.get('game_name', '?')}")
        save_history(history)
    except Exception as e:
        logger.error(f"Erreur lors de l'annulation des téléchargements en attente : {e}")
def shutdown_downloads():
    """Appelée au moment de quitter l'application proprement.
    Arrête les téléchargements actifs et laisse qBittorrent gérer son propre état de reprise."""
    global _app_shutting_down
    _app_shutting_down = True
    # Vider la file d'attente (pas de téléchargements futurs)
    config.download_queue.clear()
    config.download_active = False
    try:
        qbittorrent_backend.shutdown()
    except Exception as exc:
        logger.debug(f"shutdown_downloads: arrêt qBittorrent échoué: {exc}")
    logger.debug("shutdown_downloads: _app_shutting_down=True, file d'attente vidée.")
async def download_rom(url, platform, game_name, is_zip_non_supported=False, task_id=None):
    logger.debug(f"Début téléchargement: {game_name} depuis {url}, zip non supporté={is_zip_non_supported}, task_id={task_id}")
    
    # Sauvegarder l'URL originale pour les mises à jour d'historique
    original_history_url = url
    # Correction : détecter les URLs rgsx+torrent même si parse_torrent_download_url retourne None (blocage désactivé)
    torrent_meta = None
    if url and isinstance(url, str) and url.startswith("rgsx+torrent://"):
        # Forcer la reconstruction des métadonnées torrent
        from utils import is_torrent_download_url
        if is_torrent_download_url(url):
            # Recréer le dict attendu par _download_torrent_with_aria2
            from utils import parse_torrent_download_url as _parse_torrent_download_url
            torrent_meta = _parse_torrent_download_url.__wrapped__(url) if hasattr(_parse_torrent_download_url, '__wrapped__') else _parse_torrent_download_url(url)
    else:
        torrent_meta = parse_torrent_download_url(url)

    result = [None, None]
    
    # Vérifier si cette URL est déjà en cours de téléchargement (prévenir les doublons)
    with urls_lock:
        if url in urls_in_progress:
            logger.warning(f"⚠️ Un téléchargement pour cette URL est déjà en cours, attente du résultat: {url}")
            # Créer un événement d'attente si ce n'est pas déjà fait
            if url not in url_done_events:
                url_done_events[url] = threading.Event()
            done_event = url_done_events[url]
        else:
            # Ajouter l'URL au set en cours
            urls_in_progress.add(url)
            done_event = None
    
    # Si on attendait un doublon, on attend ici
    if done_event is not None:
        logger.debug(f"Attente de la fin du téléchargement en doublon pour {url}")
        # Attendre de manière asynchrone l'événement (timeout de 30 minutes pour les gros fichiers)
        start_wait = time.time()
        while not done_event.is_set():
            if time.time() - start_wait > 1800:  # 30 minutes timeout
                logger.warning(f"Timeout d'attente pour le doublon de {url}")
                break
            await asyncio.sleep(0.1)
        # Vérifier si on a un résultat en cache
        if url in url_results:
            logger.info(f"Résultat en cache pour {url}: {url_results[url]}")
            try:
                notify_download_finished()
            except Exception:
                pass
            return url_results[url]
        else:
            # Fallback: retourner un message de succès (le premier téléchargement a probablement réussi)
            try:
                notify_download_finished()
            except Exception:
                pass
            return (True, _("network_download_ok").format(game_name))
    
    # Créer une queue/cancel spécifique pour cette tâche
    if task_id not in progress_queues:
        progress_queues[task_id] = queue.Queue()
    if task_id not in cancel_events:
        cancel_events[task_id] = threading.Event()
    
    def download_thread():
        nonlocal url
        nonlocal torrent_meta
        try:
            _check_history_access_before_download("download_rom")
            known_total_size = _lookup_known_game_size(platform, game_name, original_history_url)
            # IMPORTANT: Créer l'entrée dans config.history dès le début avec status "Downloading"
            # pour que l'interface web puisse afficher le téléchargement en cours
            
            # TOUJOURS charger l'historique existant depuis le fichier pour éviter d'écraser les anciennes entrées
            config.history = load_history()
            
            # Vérifier si l'entrée existe déjà
            entry_exists = False
            for entry in config.history:
                if entry.get("url") == original_history_url:
                    entry_exists = True
                    # Réinitialiser le status à "Downloading"
                    entry["status"] = "Downloading"
                    entry["progress"] = 0
                    entry["downloaded_size"] = 0
                    entry["platform"] = platform
                    entry["game_name"] = game_name
                    entry["display_name"] = get_clean_display_name(game_name, platform)
                    entry["timestamp"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                    entry["task_id"] = task_id
                    if int(entry.get("total_size", 0) or 0) <= 0 and known_total_size > 0:
                        entry["total_size"] = known_total_size
                    break
            
            # Si l'entrée n'existe pas, la créer
            if not entry_exists:
                config.history.append({
                    "platform": platform,
                    "game_name": game_name,
                    "display_name": get_clean_display_name(game_name, platform),
                    "url": original_history_url,
                    "status": "Downloading",
                    "progress": 0,
                    "downloaded_size": 0,
                    "total_size": known_total_size,
                    "speed": 0,
                    "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                    "message": f"Téléchargement de {game_name}",
                    "task_id": task_id
                })
            
            # Sauvegarder history.json immédiatement
            _save_history_with_feedback("download_rom:init")
            
            cancel_ev = cancel_events.get(task_id)
            # Use symlink path if enabled
            from rgsx_settings import apply_symlink_path, get_platform_custom_path
            
            # Vérifier si un dossier personnalisé est configuré pour cette plateforme
            custom_path = get_platform_custom_path(platform)
            if custom_path and os.path.isdir(custom_path):
                dest_dir = custom_path
                platform_folder = os.path.basename(dest_dir)
                logger.debug(f"Utilisation du dossier personnalisé pour {platform}: {dest_dir}")
            else:
                dest_dir = None
                platform_folder = None
                for platform_dict in config.platform_dicts:
                    if platform_dict.get("platform_name") == platform:
                        # Priorité: clé 'folder'; fallback legacy: 'dossier'; sinon normalisation du nom de plateforme
                        platform_folder = platform_dict.get("folder") or platform_dict.get("dossier") or resolve_platform_folder(platform)
                        dest_dir = apply_symlink_path(config.ROMS_FOLDER, platform_folder)
                        logger.debug(f"Répertoire de destination trouvé pour {platform}: {dest_dir}")
                        break
                if not dest_dir:
                    platform_folder = resolve_platform_folder(platform)
                    dest_dir = apply_symlink_path(config.ROMS_FOLDER, platform_folder)

            # Spécifique: si le système est "BIOS" on force le dossier BIOS
            if platform_folder == "bios" or platform == "BIOS" or platform == "- BIOS by TMCTV -":
                dest_dir = config.USERDATA_FOLDER
                logger.debug(f"Plateforme 'BIOS' détectée, destination forcée vers USERDATA_FOLDER: {dest_dir}")
            
            os.makedirs(dest_dir, exist_ok=True)
            if not os.access(dest_dir, os.W_OK):
                raise PermissionError(f"Pas de permission d'écriture dans {dest_dir}")
                
            sanitized_name = sanitize_filename(game_name)
            dest_path = os.path.join(dest_dir, f"{sanitized_name}")
            logger.info(f"Chemin destination: {dest_path}")
            _update_history_local_target(original_history_url, task_id, dest_path)

            expected_size_before_start = int(torrent_meta.get("size_bytes") or 0) if torrent_meta is not None else int(known_total_size or 0)
            has_space, low_space_message = _ensure_sufficient_disk_space(dest_dir, expected_size_before_start)
            if not has_space:
                raise InsufficientDiskSpaceError(low_space_message)

            torrent_meta = parse_torrent_download_url(url)
            if torrent_meta is not None:
                if _is_arm_device():
                    raise RuntimeError("Les téléchargements torrent ne sont pas encore disponibles sur les appareils ARM pour le moment.")
                if url not in config.download_progress:
                    config.download_progress[url] = {
                        "downloaded_size": 0,
                        "total_size": int(torrent_meta.get("size_bytes") or 0),
                        "status": "Downloading",
                        "progress_percent": 0,
                        "speed": 0,
                        "game_name": game_name,
                        "platform": platform,
                    }

                if os.path.exists(dest_path):
                    local_size = os.path.getsize(dest_path)
                    expected_size = int(torrent_meta.get("size_bytes") or 0)
                    if expected_size <= 0 or local_size == expected_size:
                        logger.info(f"Le fichier torrent {dest_path} existe déjà, téléchargement ignoré")
                        result[0] = True
                        result[1] = _("network_download_ok").format(game_name) + _("download_already_present")
                        for entry in config.history:
                            if entry.get("url") == original_history_url:
                                entry["status"] = "Download_OK"
                                entry["progress"] = 100
                                entry["message"] = result[1]
                                _save_history_with_feedback("download_rom:torrent_already_present")
                                break
                        try:
                            show_toast(result[1])
                        except Exception:
                            pass
                        with urls_lock:
                            urls_in_progress.discard(original_history_url)
                        try:
                            notify_download_finished()
                        except Exception:
                            pass
                        return result[0], result[1]

                torrent_expected_size = int(torrent_meta.get("size_bytes") or 0)
                has_space, low_space_message = _ensure_sufficient_disk_space(dest_dir, torrent_expected_size)
                if not has_space:
                    raise InsufficientDiskSpaceError(low_space_message)

                if not _should_prefer_qbittorrent_backend():
                    raise qbittorrent_backend.BackendUnavailableError("qBittorrent introuvable, non démarré ou non disponible")

                logger.info("Téléchargement torrent via qBittorrent")
                success, message = qbittorrent_backend.download_torrent_via_qbittorrent(
                    torrent_meta,
                    dest_dir,
                    dest_path,
                    task_id,
                    cancel_ev,
                    progress_queues[task_id],
                    original_history_url,
                    pause_ev=pause_events.setdefault(task_id, threading.Event()),
                )
                result[0] = success
                result[1] = message

                if success and os.path.exists(dest_path):
                    os.chmod(dest_path, 0o644)
                logger.debug(f"Téléchargement torrent terminé: {dest_path}")
            else:
            
                # Créer la session AVANT la vérification du fichier existant
                headers = {
                    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36',
                    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8',
                    'Accept-Language': 'en-US,en;q=0.5',
                    'Accept-Encoding': 'gzip, deflate',
                    'DNT': '1',
                    'Connection': 'keep-alive',
                    'Upgrade-Insecure-Requests': '1'
                }
                
                session = requests.Session()
                session.headers.update(headers)
                
                # Récupérer la taille du fichier pour vimm.net avant de commencer
                vimm_file_size = 0
                vimm_download_info = None
                if 'vimm.net' in url:
                    vimm_download_info = _fetch_vimm_download_info(url, session)
                    vimm_file_size = _get_vimm_file_size(url, session, vimm_download_info)
                    if vimm_file_size > 0:
                        logger.info(f"Taille du fichier vimm.net déterminée: {vimm_file_size} octets")
                        # Mettre à jour l'historique avec la taille connue
                        for entry in config.history:
                            if entry.get("url") == original_history_url:
                                entry["total_size"] = vimm_file_size
                                _save_history_with_feedback("download_rom:vimm_total_size")
                                break

                    # Utiliser le nom de fichier réel récupéré via Content-Disposition dans _get_vimm_file_size
                    if vimm_download_info and vimm_download_info.get('real_filename'):
                        real_filename = sanitize_filename(vimm_download_info['real_filename'])
                        if real_filename and real_filename != sanitized_name:
                            dest_path = os.path.join(dest_dir, real_filename)
                            logger.debug(f"Nom de fichier Vimm réel utilisé: {real_filename}")
                            _update_history_local_target(original_history_url, task_id, dest_path)
                            display_name = os.path.splitext(real_filename)[0]
                            for entry in config.history:
                                if entry.get('url') == original_history_url:
                                    entry['display_name'] = display_name
                                    _save_history_with_feedback("download_rom:vimm_display_name")
                                    break

                # Gestion spéciale pour vimm.net
                vimm_original_referer = None
                if 'vimm.net' in url:
                    try:
                        logger.debug("Détection URL vimm.net, récupération du mediaId...")
                        vimm_original_referer = url  # Sauvegarder l'URL originale pour le referer
                        if not vimm_download_info:
                            vimm_download_info = _fetch_vimm_download_info(url, session)
                        if not vimm_download_info:
                            raise ValueError("Formulaire de téléchargement introuvable")

                        media_id = str(vimm_download_info.get('media_id') or '').strip()
                        download_url = str(vimm_download_info.get('base_download_url') or '').strip()
                        final_download_url = str(vimm_download_info.get('download_url') or '').strip()
                        if not media_id:
                            raise ValueError("mediaId introuvable")
                        if not download_url or not final_download_url:
                            raise ValueError("URL de téléchargement vimm.net introuvable")

                        logger.debug(f"mediaId trouvé: {media_id}")
                        logger.debug(f"URL de téléchargement: {download_url}")

                        # Modifier l'URL pour le téléchargement direct
                        url = final_download_url
                        logger.debug(f"URL finale pour téléchargement: {url}")
                        
                    except Exception as e:
                        logger.error(f"Erreur lors du traitement vimm.net: {e}")
                        raise
                
                # Vérifier si le fichier existe déjà (exact ou avec autre extension)
                file_found = False
                if os.path.exists(dest_path):
                    logger.info(f"Le fichier {dest_path} existe déjà, vérification de la taille...")
                    
                    # Vérifier la taille du fichier local
                    local_size = os.path.getsize(dest_path)
                    logger.debug(f"Taille du fichier local: {local_size} octets")
                    
                    # Essayer de récupérer la taille du serveur
                    remote_size = None
                    if vimm_file_size > 0:
                        remote_size = vimm_file_size
                        logger.debug(f"Taille du fichier serveur via vimm.net: {remote_size} octets")
                    elif _is_lolroms_url(url):
                        probed_size = _probe_lolroms_remote_size(url)
                        if probed_size > 0:
                            remote_size = probed_size
                            logger.debug(f"Taille du fichier serveur via lolroms probe: {remote_size} octets")
                        else:
                            logger.warning("Impossible de vérifier la taille distante lolroms; le fichier local sera re-téléchargé par sécurité")
                    else:
                        try:
                            head_response = session.head(url, timeout=10, allow_redirects=True)
                            if head_response.status_code == 200:
                                content_length = head_response.headers.get('content-length')
                                if content_length:
                                    remote_size = int(content_length)
                                    logger.debug(f"Taille du fichier serveur: {remote_size} octets")
                        except Exception as e:
                            logger.debug(f"Impossible de vérifier la taille serveur: {e}")
                    
                    # Comparer les tailles si on a obtenu la taille distante
                    if remote_size is not None and local_size != remote_size:
                        logger.warning(f"Taille mismatch! Local: {local_size}, Remote: {remote_size} - le fichier sera re-téléchargé")
                        # Les tailles ne correspondent pas, il faut re-télécharger
                        try:
                            if os.path.exists(dest_path):
                                os.remove(dest_path)
                                logger.info(f"Fichier incomplet supprimé: {dest_path}")
                            else:
                                logger.debug(f"Fichier déjà supprimé par un autre thread: {dest_path}")
                        except FileNotFoundError:
                            logger.debug(f"Fichier déjà supprimé (ou n'existe plus): {dest_path}")
                        except Exception as e:
                            logger.error(f"Impossible de supprimer le fichier incomplet: {e}")
                            result[0] = False
                            result[1] = f"Erreur suppression fichier incomplet: {str(e)}"
                            with urls_lock:
                                urls_in_progress.discard(original_history_url)
                                logger.debug(f"URL supprimée du set des téléchargements en cours: {original_history_url} (URLs restantes: {len(urls_in_progress)})")
                            return
                        # Continuer le téléchargement normal (ne pas faire return)
                    elif _is_lolroms_url(url):
                        try:
                            if os.path.exists(dest_path):
                                os.remove(dest_path)
                                logger.info(f"Fichier lolroms local non vérifiable supprimé: {dest_path}")
                        except Exception as e:
                            logger.error(f"Impossible de supprimer le fichier lolroms non vérifiable: {e}")
                            result[0] = False
                            result[1] = f"Erreur suppression fichier incomplet: {str(e)}"
                            with urls_lock:
                                urls_in_progress.discard(original_history_url)
                            return
                    else:
                        # Les tailles correspondent ou on ne peut pas vérifier, considérer comme déjà téléchargé
                        logger.info(f"Le fichier {dest_path} existe déjà et la taille est correcte, téléchargement ignoré")
                        result[0] = True
                        result[1] = _("network_download_ok").format(game_name) + _("download_already_present")
                        
                        # Mettre à jour l'historique
                        for entry in config.history:
                            if entry.get("url") == original_history_url:
                                entry["status"] = "Download_OK"
                                entry["progress"] = 100
                                entry["message"] = result[1]
                                _save_history_with_feedback("download_rom:file_already_present")
                                break
                        
                        # Afficher un toast au lieu d'ouvrir l'historique
                        try:
                            show_toast(result[1])
                        except Exception as e:
                            logger.debug(f"Impossible d'afficher le toast: {e}")
                        with urls_lock:
                            urls_in_progress.discard(original_history_url)
                            logger.debug(f"URL supprimée du set des téléchargements en cours: {original_history_url} (URLs restantes: {len(urls_in_progress)})")
                        
                        # Libérer le slot de la queue
                        try:
                            notify_download_finished()
                        except Exception:
                            pass
                        
                        return result[0], result[1]
                    file_found = True
                
                # Vérifier si un fichier avec le même nom de base mais extension différente existe (SEULEMENT si fichier exact non trouvé)
                if not file_found:
                    base_name_no_ext = os.path.splitext(sanitized_name)[0]
                    if base_name_no_ext != sanitized_name:  # Seulement si une extension était présente
                        try:
                            # Lister tous les fichiers dans le répertoire de destination
                            if os.path.exists(dest_dir):
                                for existing_file in os.listdir(dest_dir):
                                    existing_base = os.path.splitext(existing_file)[0]
                                    if existing_base == base_name_no_ext:
                                        existing_path = os.path.join(dest_dir, existing_file)
                                        logger.info(f"Un fichier avec le même nom de base existe: {existing_path}, vérification de la taille...")
                                        
                                        # Vérifier la taille du fichier local
                                        local_size = os.path.getsize(existing_path)
                                        logger.debug(f"Taille du fichier local (extension différente): {local_size} octets")
                                        
                                        # Essayer de récupérer la taille du serveur
                                        remote_size = None
                                        if vimm_file_size > 0:
                                            remote_size = vimm_file_size
                                            logger.debug(f"Taille du fichier serveur via vimm.net (extension différente): {remote_size} octets")
                                        elif _is_lolroms_url(url):
                                            probed_size = _probe_lolroms_remote_size(url)
                                            if probed_size > 0:
                                                remote_size = probed_size
                                                logger.debug(f"Taille du fichier serveur via lolroms probe: {remote_size} octets")
                                            else:
                                                logger.warning("Impossible de vérifier la taille distante lolroms (extension différente); re-téléchargement par sécurité")
                                        else:
                                            try:
                                                head_response = session.head(url, timeout=10, allow_redirects=True)
                                                if head_response.status_code == 200:
                                                    content_length = head_response.headers.get('content-length')
                                                    if content_length:
                                                        remote_size = int(content_length)
                                                        logger.debug(f"Taille du fichier serveur: {remote_size} octets")
                                            except Exception as e:
                                                logger.debug(f"Impossible de vérifier la taille serveur: {e}")
                                        
                                        # Comparer les tailles si on a obtenu la taille distante
                                        if remote_size is not None and local_size != remote_size:
                                            logger.warning(f"Taille mismatch (extension différente)! Local: {local_size}, Remote: {remote_size} - re-téléchargement")
                                            # Continuer le téléchargement normal
                                            break
                                        elif _is_lolroms_url(url):
                                            logger.info(f"Fichier lolroms avec extension différente non vérifiable, re-téléchargement forcé: {existing_path}")
                                            break
                                        else:
                                            # Les tailles correspondent, fichier complet
                                            logger.info(f"Un fichier avec le même nom de base existe déjà: {existing_path}, téléchargement ignoré")
                                            result[0] = True
                                            result[1] = _("network_download_ok").format(game_name) + _("download_already_extracted")
                                            
                                            # Mettre à jour l'historique
                                            for entry in config.history:
                                                if entry.get("url") == original_history_url:
                                                    entry["status"] = "Download_OK"
                                                    entry["progress"] = 100
                                                    entry["message"] = result[1]
                                                    _save_history_with_feedback("download_rom:file_already_extracted")
                                                    break
                                            
                                            # Afficher un toast au lieu d'ouvrir l'historique
                                            try:
                                                show_toast(result[1])
                                            except Exception as e:
                                                logger.debug(f"Impossible d'afficher le toast: {e}")
                                            with urls_lock:
                                                urls_in_progress.discard(original_history_url)
                                                logger.debug(f"URL supprimée du set des téléchargements en cours: {original_history_url} (URLs restantes: {len(urls_in_progress)})")
                                            
                                            # Libérer le slot de la queue
                                            try:
                                                notify_download_finished()
                                            except Exception:
                                                pass
                                            
                                            return result[0], result[1]
                        except Exception as e:
                            logger.debug(f"Erreur lors de la vérification des fichiers existants: {e}")
            
                external_lolroms_downloaded = False
                total_size = 0

                if _is_lolroms_url(url):
                    if url not in config.download_progress:
                        config.download_progress[url] = {
                            "downloaded_size": 0,
                            "total_size": known_total_size,
                            "status": "Connecting",
                            "progress_percent": 0,
                            "speed": 0,
                            "game_name": game_name,
                            "platform": platform
                        }
                    if url in config.download_progress:
                        config.download_progress[url]["status"] = "lolroms external"
                        config.needs_redraw = True
                    external_success, external_message = _download_lolroms_with_external_tool(
                        url,
                        dest_path,
                        task_id,
                        cancel_ev=cancel_ev,
                        progress_queue=progress_queues.get(task_id),
                    )
                    if external_success is True:
                        external_lolroms_downloaded = True
                        total_size = os.path.getsize(dest_path) if os.path.exists(dest_path) else 0
                        if url in config.download_progress:
                            config.download_progress[url]["downloaded_size"] = total_size
                            config.download_progress[url]["total_size"] = total_size
                            config.download_progress[url]["progress_percent"] = 100
                            config.download_progress[url]["status"] = "Completed"
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == url:
                                    entry["total_size"] = total_size
                                    _save_history_with_feedback("download_rom:lolroms_total_size")
                                    break
                        logger.info(f"Téléchargement lolroms terminé via outil externe: {dest_path}")
                    elif external_success is False:
                        # Sur Windows, curl peut échouer avec schannel
                        # (ex: SEC_E_LOGON_DENIED). On tente alors le fallback
                        # requests/stream classique au lieu d'échouer immédiatement.
                        ext_err = (external_message or "lolroms external download failed").strip()
                        if url in config.download_progress:
                            config.download_progress[url]["status"] = "lolroms requests fallback"
                            config.needs_redraw = True
                        logger.warning(f"lolroms external tool failed, fallback requests: {ext_err}")

                if not external_lolroms_downloaded:
                    download_headers = headers.copy()
                    download_headers['Accept'] = 'application/octet-stream, */*'
                    # Utiliser le referer spécial pour vimm.net si défini
                    if vimm_original_referer:
                        download_headers['Referer'] = vimm_original_referer
                    else:
                        default_referer = _default_referer_for_url(url)
                        if default_referer:
                            download_headers['Referer'] = default_referer
                    archive_cookie = load_archive_org_cookie()
                    archive_alt_urls = []
                    meta_json = None

                    if 'archive.org/download/' in url:
                        try:
                            parsed = urllib.parse.urlsplit(url)
                            parts = parsed.path.split('/download/', 1)
                            pre_id = None
                            if len(parts) == 2:
                                after = parts[1]
                                pre_id = after.split('/', 1)[0]
                                logger.debug(f"URL archive.org conservée: {url}")
                            if not pre_id:
                                pre_id = url.split('/download/')[1].split('/')[0]

                            download_headers['Referer'] = f"https://archive.org/details/{pre_id}"
                            download_headers['Origin'] = 'https://archive.org'
                            if archive_cookie:
                                download_headers['Cookie'] = archive_cookie
                            if archive_cookie:
                                for pair in archive_cookie.split(';'):
                                    if '=' in pair:
                                        name, value = pair.split('=', 1)
                                        session.cookies.set(name.strip(), value.strip(), domain='.archive.org')

                            session.get('https://archive.org/robots.txt', timeout=20, headers={'Cookie': archive_cookie} if archive_cookie else None)
                            meta_resp = session.get(f'https://archive.org/metadata/{pre_id}', timeout=20, headers={'Cookie': archive_cookie} if archive_cookie else None)
                            if meta_resp.status_code == 200:
                                try:
                                    meta_json = meta_resp.json()
                                except Exception:
                                    meta_json = None
                            logger.debug(f"Pré-chargement cookies/metadata archive.org pour {pre_id}")

                            identifier, archive_name, inner_path = _split_archive_org_path(url)
                            if identifier and archive_name and inner_path:
                                if meta_json:
                                    server = meta_json.get('server')
                                    directory = meta_json.get('dir')
                                    if server and directory:
                                        archive_path = f"{directory}/{archive_name}"
                                        view_url = f"https://{server}/view_archive.php?archive=" + urllib.parse.quote(archive_path, safe='/') + "&file=" + urllib.parse.quote(inner_path, safe='/')
                                        archive_alt_urls.insert(0, view_url)
                        except Exception as e:
                            logger.debug(f"Pré-chargement archive.org ignoré: {e}")

                    if url not in config.download_progress:
                        config.download_progress[url] = {
                            "downloaded_size": 0,
                            "total_size": vimm_file_size if vimm_file_size > 0 else known_total_size,
                            "status": "Connecting",
                            "progress_percent": 0,
                            "speed": 0,
                            "game_name": game_name,
                            "platform": platform
                        }

                    header_variants = [download_headers]
                    if 'archive.org' in url:
                        header_variants.extend([
                            {
                                'User-Agent': headers.get('User-Agent', download_headers.get('User-Agent', 'Mozilla/5.0')),
                                'Accept': 'application/octet-stream,*/*;q=0.8',
                                'Accept-Language': headers.get('Accept-Language', 'en-US,en;q=0.5'),
                                'Connection': 'keep-alive',
                                **({'Cookie': archive_cookie} if archive_cookie else {})
                            },
                            {
                                'User-Agent': headers.get('User-Agent', download_headers.get('User-Agent', 'Mozilla/5.0')),
                                'Accept': '*/*',
                                'Referer': 'https://archive.org/',
                                **({'Cookie': archive_cookie} if archive_cookie else {})
                            }
                        ])
                    elif 'vimm.net' in url:
                        # dl2.vimm.net ferme parfois la connexion sans réponse ("RemoteDisconnected")
                        # de façon transitoire (charge serveur / anti-hotlinking ponctuel), même quand
                        # le même téléchargement fonctionne l'instant d'après. On retente avec
                        # Connection: close pour forcer une connexion TCP fraîche plutôt que de
                        # réutiliser une connexion du pool qui a pu être coupée côté serveur.
                        vimm_retry_headers = download_headers.copy()
                        vimm_retry_headers['Connection'] = 'close'
                        header_variants.extend([vimm_retry_headers, vimm_retry_headers.copy()])

                    response = None
                    last_status = None
                    last_error = None
                    last_error_type = None
                    browser_challenge_detected = False

                    # Le rate-limit 429 de vimm.net (dl2.vimm.net) peut durer plus longtemps que
                    # les quelques secondes couvertes par les variantes de headers habituelles ;
                    # on s'autorise donc des tentatives supplémentaires dédiées, avec un backoff
                    # exponentiel, spécifiquement en réaction à des 429 répétés.
                    extra_429_retries = 4 if 'vimm.net' in url else 0
                    total_max_attempts = len(header_variants) + extra_429_retries
                    rate_limit_hits = 0
                    resume_offset = _http_resume_offset(dest_path)
                    if resume_offset > 0:
                        logger.info(f"Reprise HTTP détectée: {resume_offset} octets déjà téléchargés dans {_http_part_path(dest_path)}")
                    attempt = 0
                    while attempt < total_max_attempts:
                        attempt += 1
                        hv = header_variants[min(attempt - 1, len(header_variants) - 1)]
                        if resume_offset > 0:
                            hv = dict(hv)
                            hv['Range'] = f'bytes={resume_offset}-'
                        try:
                            if url in config.download_progress:
                                config.download_progress[url]["status"] = f"Try {attempt}/{total_max_attempts}"
                                config.download_progress[url]["progress_percent"] = 0
                                config.needs_redraw = True

                            logger.debug(f"Tentative téléchargement {attempt}/{total_max_attempts} avec headers: {_redact_headers(hv)}")
                            timeout_val = (60, 90) if 'archive.org' in url else 30
                            r = session.get(url, stream=True, timeout=timeout_val, allow_redirects=True, headers=hv)
                            last_status = r.status_code
                            logger.debug(f"Status code tentative {attempt}: {r.status_code}")
                            if _is_browser_challenge_response(r):
                                browser_challenge_detected = True
                                try:
                                    snippet = r.text[:200]
                                    logger.debug(f"Challenge navigateur detecte tentative {attempt}: {snippet}")
                                except Exception:
                                    pass
                                break
                            if r.status_code in (401, 403):
                                try:
                                    snippet = r.text[:200]
                                    logger.debug(f"Réponse {r.status_code} snippet: {snippet}")
                                except Exception:
                                    pass
                                continue
                            r.raise_for_status()
                            response = r
                            break
                        except requests.Timeout as e:
                            last_error = str(e)
                            last_error_type = "timeout"
                            logger.debug(f"Timeout tentative {attempt}: {e}")
                        except requests.ConnectionError as e:
                            last_error = str(e)
                            last_error_type = "connection"
                            logger.debug(f"Erreur connexion tentative {attempt}: {e}")
                            if (('archive.org' in url) or ('vimm.net' in url)) and attempt < total_max_attempts:
                                time.sleep(2)
                        except requests.HTTPError as e:
                            last_error = str(e)
                            last_error_type = "http"
                            logger.debug(f"Erreur HTTP tentative {attempt}: {e}")
                            if last_status == 429:
                                # Rate limit : on respecte l'en-tête Retry-After si présent, sinon
                                # on patiente avec un backoff exponentiel (5s, 10s, 20s, 30s...)
                                # avant de retenter, car ce type de limite est généralement temporaire
                                # mais peut durer plus que quelques secondes.
                                retry_after = None
                                try:
                                    retry_after = float(r.headers.get('Retry-After', ''))
                                except Exception:
                                    retry_after = None
                                wait_time = min(max(retry_after or (5.0 * (2 ** rate_limit_hits)), 1.0), 30.0)
                                rate_limit_hits += 1
                                if attempt < total_max_attempts:
                                    logger.debug(f"429 Too Many Requests, attente {wait_time:.1f}s avant nouvelle tentative")
                                    time.sleep(wait_time)
                                continue
                            if last_status not in (401, 403):
                                break
                        except requests.RequestException as e:
                            last_error = str(e)
                            last_error_type = "request"
                            logger.debug(f"Erreur requête tentative {attempt}: {e}")
                            if isinstance(e, requests.HTTPError) and last_status not in (401, 403):
                                break
                            if 'archive.org' in url and attempt < total_max_attempts:
                                time.sleep(2)

                    if response is None:
                        if browser_challenge_detected:
                            raise requests.HTTPError(
                                "Access blocked by a browser challenge on the source host. This source requires an interactive browser session and cannot be downloaded by the embedded Python client."
                            )
                        if archive_alt_urls and (last_status in (401, 403) or last_error_type in ("timeout", "connection", "request")):
                            for alt_url in archive_alt_urls:
                                try:
                                    timeout_val = (45, 90)
                                    logger.debug(f"Tentative archive.org alt URL: {alt_url}")
                                    alt_headers = download_headers.copy()
                                    try:
                                        alt_host = urllib.parse.urlsplit(alt_url).netloc
                                        if alt_host.startswith("ia") and alt_host.endswith(".archive.org"):
                                            alt_headers["Referer"] = f"https://{alt_host}/"
                                            alt_headers["Origin"] = "https://archive.org"
                                    except Exception:
                                        pass
                                    r = session.get(alt_url, stream=True, timeout=timeout_val, allow_redirects=True, headers=alt_headers)
                                    if r.status_code not in (401, 403):
                                        r.raise_for_status()
                                        response = r
                                        url = alt_url
                                        break
                                except Exception as e:
                                    logger.debug(f"Alt URL archive.org échec: {e}")
                        if response is None and 'archive.org/download/' in url:
                            try:
                                identifier = url.split('/download/')[1].split('/')[0]
                                if meta_json is None:
                                    meta_resp = session.get(f'https://archive.org/metadata/{identifier}', timeout=30)
                                    if meta_resp.status_code == 200:
                                        meta_json = meta_resp.json()
                                if meta_json:
                                    if meta_json.get('is_dark'):
                                        raise requests.HTTPError(f"Item archive.org restreint (is_dark=true): {identifier}")
                                    if not meta_json.get('files'):
                                        raise requests.HTTPError(f"Item archive.org sans fichiers listés: {identifier}")
                                    available = [f.get('name') for f in meta_json.get('files', [])][:10]
                                    raise requests.HTTPError(f"Accès refusé (HTTP {last_status}). Fichiers disponibles exemples: {available}")
                                raise requests.HTTPError(f"HTTP {last_status} & metadata indisponible pour {identifier}")
                            except requests.HTTPError:
                                raise
                            except Exception as e:
                                raise requests.HTTPError(f"HTTP {last_status} après variations; metadata échec: {e}")

                        error_msg = None
                        if last_status:
                            if last_status == 401:
                                error_msg = _("network_auth_required").format(last_status) if _ else f"Authentication required (HTTP {last_status})"
                            elif last_status == 403:
                                error_msg = _("network_access_denied").format(last_status) if _ else f"Access denied (HTTP {last_status})"
                            elif last_status == 429 and 'vimm.net' in url:
                                # Vimm.net limite apparemment à un seul téléchargement simultané par IP :
                                # un 429 persistant après plusieurs tentatives est très probablement dû à
                                # un autre téléchargement vimm.net déjà en cours, pas à un vrai blocage.
                                error_msg = _("network_vimm_rate_limit") if _ else "Vimm.net limits downloads to one at a time per IP. Wait for the other download to finish and retry."
                            elif last_status >= 500:
                                error_msg = _("network_server_error").format(last_status) if _ else f"Server error (HTTP {last_status})"
                            else:
                                error_msg = _("network_http_error").format(last_status) if _ else f"HTTP error {last_status}"
                        elif last_error_type == "timeout":
                            error_msg = _("network_timeout_error") if _ else "Connection timeout"
                        elif last_error_type == "connection":
                            error_msg = _("network_connection_error") if _ else "Network connection error"
                        else:
                            error_msg = _("network_no_response") if _ else "No response from server"

                        attempts_count = attempt
                        full_error_msg = _("network_connection_failed").format(attempts_count) if _ else f"Connection failed after {attempts_count} attempts"
                        full_error_msg += f" - {error_msg}"
                        if last_error:
                            logger.error(f"Détails de l'erreur: {last_error}")
                        raise requests.HTTPError(full_error_msg)

                    response_content_type = (response.headers.get('content-type', '') or '').lower()
                    if 'vimm.net' in original_history_url and 'text/html' in response_content_type:
                        raise requests.HTTPError(
                            f"Vimm returned an HTML page instead of an archive (content-type={response_content_type})"
                        )

                    if url in config.download_progress:
                        config.download_progress[url]["status"] = "Downloading"
                        config.needs_redraw = True

                    content_range_total = _http_parse_content_range(response.headers.get('content-range'))
                    announced_total_size = content_range_total or int(response.headers.get('content-length', 0) or 0) or int(vimm_file_size or known_total_size or 0)
                    has_space, low_space_message = _ensure_sufficient_disk_space(dest_dir, announced_total_size)
                    if not has_space:
                        raise InsufficientDiskSpaceError(low_space_message)

                    transfer = _stream_response_to_path(response, dest_path, task_id, cancel_ev, progress_queues.get(task_id), fallback_total_size=vimm_file_size or known_total_size, resume_offset=resume_offset)
                    total_size = int(transfer['total_size'])
                    downloaded = int(transfer['downloaded'])
                    last_downloaded = int(transfer['last_downloaded'])
                    last_update_time = float(transfer['last_update_time'])
                    download_canceled = bool(transfer['download_canceled'])

                    logger.info(f"Taille totale: {total_size} octets")
                    logger.debug(f"Progression initiale envoyée: 0% pour {game_name}, task_id={task_id}")
                    if isinstance(config.history, list):
                        for entry in config.history:
                            if "url" in entry and entry["url"] == original_history_url:
                                entry["total_size"] = total_size
                                _save_history_with_feedback("download_rom:stream_total_size")
                                break
                    
                    # Mettre à jour la taille dans download_progress si elle n'était pas connue
                    if url in config.download_progress and config.download_progress[url]["total_size"] == 0:
                        config.download_progress[url]["total_size"] = total_size

                    if downloaded <= 0 and archive_alt_urls and 'archive.org' in url:
                        try:
                            response.close()
                        except Exception:
                            pass
                        response, fallback_url, fallback_transfer = _try_archive_org_alternate_urls(
                            session,
                            archive_alt_urls,
                            url,
                            download_headers,
                            dest_path,
                            task_id,
                            cancel_ev,
                            progress_queues.get(task_id),
                            fallback_total_size=known_total_size,
                            resume_offset=resume_offset,
                        )
                        if fallback_transfer is not None:
                            url = fallback_url
                            total_size = int(fallback_transfer['total_size'])
                            downloaded = int(fallback_transfer['downloaded'])
                            last_downloaded = int(fallback_transfer['last_downloaded'])
                            last_update_time = float(fallback_transfer['last_update_time'])
                            download_canceled = bool(fallback_transfer['download_canceled'])
                            logger.debug(f"Fallback Archive.org réussi: {url}")

                    if downloaded <= 0:
                        try:
                            if os.path.exists(dest_path):
                                os.remove(dest_path)
                        except Exception:
                            pass
                        content_type = response.headers.get('content-type', '') if response is not None else ''
                        raise requests.HTTPError(
                            f"Downloaded empty response from source (content-type={content_type or 'unknown'})"
                        )

                    # Garde-fous anti faux-positifs: certains hôtes peuvent
                    # renvoyer une page HTML/challenge avec un status 200.
                    archive_ext = os.path.splitext(dest_path)[1].lower()
                    if archive_ext in {'.7z', '.zip', '.rar'}:
                        if _looks_like_html_or_challenge(dest_path):
                            _safe_remove_file(dest_path)
                            raise requests.HTTPError(
                                "Downloaded HTML/challenge content instead of archive payload"
                            )
                        if not _matches_expected_archive_signature(dest_path):
                            _safe_remove_file(dest_path)
                            raise requests.HTTPError(
                                "Downloaded payload is not a valid archive"
                            )
                        if total_size > 0 and downloaded < total_size:
                            accepted, reason = _should_accept_partial_archive(downloaded, total_size, dest_path)
                            if not accepted:
                                _safe_remove_file(dest_path)
                                raise requests.HTTPError(reason)
                            logger.warning(
                                f"Archive partiellement incomplète mais acceptée malgré l'écart de taille: {reason}"
                            )

                    if downloaded > 0 and downloaded != last_downloaded:
                        current_time = time.time()
                        delta = downloaded - last_downloaded
                        elapsed = current_time - last_update_time
                        speed = delta / elapsed / (1024 * 1024) if elapsed > 0 else 0
                        progress_queues[task_id].put((task_id, downloaded, total_size, speed))
                        logger.debug(f"Mise à jour finale de progression: {downloaded}/{total_size} octets")

                    if download_canceled:
                        try:
                            notify_download_finished()
                        except Exception:
                            pass
                        return
            
            os.chmod(dest_path, 0o644)
            logger.info(f"Téléchargement terminé: {dest_path}")
            
            # Vérifier si l'extraction automatique est activée dans les paramètres
            from rgsx_settings import get_auto_extract
            auto_extract_enabled = get_auto_extract()
            
            # Forcer extraction si plateforme BIOS même si le pré-check ne l'avait pas marqué
            force_extract = is_zip_non_supported and auto_extract_enabled
            if not force_extract and auto_extract_enabled:
                try:
                    bios_like = {"BIOS", "- BIOS by TMCTV -", "- BIOS"}
                    if platform_folder == "bios" or platform in bios_like:
                        force_extract = True
                        logger.debug("Extraction forcée activée pour BIOS")
                except Exception:
                    pass
            
            # Forcer extraction pour PS3 Redump (déchiffrement et extraction ISO obligatoire)
            is_ps3_target = _is_ps3_redump_target(platform_folder, platform)
            if not force_extract and is_ps3_target:
                force_extract = True
                logger.debug("Extraction forcée activée pour PS3 Redump (déchiffrement ISO)")

            if force_extract:
                logger.debug(f"Extraction automatique nécessaire pour {dest_path}")
                if url in config.download_progress:
                    config.download_progress[url]["status"] = "Extracting"
                    config.download_progress[url]["progress_percent"] = 0
                    config.download_progress[url]["downloaded_size"] = 0
                    config.needs_redraw = True
                if isinstance(config.history, list):
                    for entry in config.history:
                        if "url" in entry and entry["url"] == original_history_url and entry["status"] in ["Downloading", "Téléchargement"]:
                            entry["status"] = "Extracting"
                            entry["progress"] = 0
                            entry["message"] = "Préparation de l'extraction..."
                            _save_history_with_feedback("download_rom:extracting")
                            config.needs_redraw = True
                            break
                try:
                    result[0], result[1] = _postprocess_downloaded_file(dest_path, dest_dir, url, game_name, is_ps3_target)
                except Exception as e:
                    logger.error(f"Exception lors du post-traitement: {str(e)}")
                    result[0] = False
                    result[1] = f"Erreur téléchargement {game_name}: {str(e)}"
            else:
                result[0] = True
                result[1] = _("network_download_ok").format(game_name)

        except InsufficientDiskSpaceError as e:
            logger.warning(f"Téléchargement annulé par manque d'espace disque pour {url}: {e}")
            result[0] = False
            result[1] = str(e)
        except Exception as e:
            logger.error(f"Erreur téléchargement {url}: {str(e)}")
            result[0] = False
            result[1] = _("network_download_error").format(game_name, str(e))
        
        # AVANT le finally : Mettre à jour la progression à 100% si succès
        if result[0] and url in config.download_progress:
            logger.info(f"[WEB PROGRESS] Mise à jour finale à 100% pour {game_name}")
            config.download_progress[url]["progress_percent"] = 100
            config.download_progress[url]["status"] = "Completed"
            config.download_progress[url]["downloaded_size"] = config.download_progress[url].get("total_size", 0)
                # Plus besoin de update_web_progress
            logger.info(f"[WEB PROGRESS] Attente 1.5s pour affichage...")
            time.sleep(1.5)  # Laisser l'interface afficher 100% pendant 1.5 secondes
            logger.info(f"[WEB PROGRESS] Fin de l'attente, envoi signal de fin")
        
        # Maintenant on peut envoyer le signal de fin à la boucle
        logger.debug(f"Thread téléchargement terminé pour {url}, task_id={task_id}")
        progress_queues[task_id].put((task_id, result[0], result[1]))
        logger.debug(f"Final result sent to queue: success={result[0]}, message={result[1]}, task_id={task_id}")

    thread = threading.Thread(target=download_thread, daemon=True)
    download_threads[task_id] = thread
    thread.start()
    last_saved_progress_percent = -1
    
    # Boucle principale pour mettre à jour la progression
    while thread.is_alive():
        try:
            task_queue = progress_queues.get(task_id)
            if task_queue:
                while not task_queue.empty():
                    data = task_queue.get()
                    #logger.debug(f"Progress queue data received: {data}")
                    if isinstance(data[1], bool):  # Fin du téléchargement
                        success, message = data[1], data[2]
                        
                        # Nettoyer download_progress et web_progress
                        if url in config.download_progress:
                            del config.download_progress[url]
                        # Plus besoin de remove_web_progress
                        
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == original_history_url and entry["status"] in ["Downloading", "Téléchargement", "Extracting", "Converting"]:
                                    current_progress = int(entry.get("progress", 0) or 0)
                                    entry["status"] = "Download_OK" if success else "Erreur"
                                    entry["progress"] = 100 if success else current_progress
                                    entry["message"] = message
                                    _save_history_with_feedback("download_rom:final")
                                    # Marquer le jeu comme téléchargé si succès
                                    if success:
                                        logger.debug(f"[WHILE_LOOP] Marking game as downloaded: platform={platform}, game={game_name}")
                                        from history import mark_game_as_downloaded
                                        file_size = entry.get("size", "N/A")
                                        mark_game_as_downloaded(platform, game_name, file_size)
                                    config.needs_redraw = True
                                    logger.debug(f"Final update in history: status={entry['status']}, progress={entry['progress']}%, message={message}, task_id={task_id}")
                                    break
                    else:
                        # logger.debug(f"[QUEUE] Traitement données progression: {data}, task_id={task_id}")
                        if len(data) >= 7:
                            downloaded, total_size, speed, seeds, connections, phase = data[1], data[2], data[3], data[4], data[5], data[6]
                        elif len(data) >= 6:
                            downloaded, total_size, speed, seeds, connections = data[1], data[2], data[3], data[4], data[5]
                            phase = "downloading" if (data[3] > 0.001 or data[1] > 0) else "connecting"
                        elif len(data) >= 4:
                            downloaded, total_size, speed = data[1], data[2], data[3]
                            seeds, connections = 0, 0
                            phase = "downloading" if data[3] > 0.001 else "connecting"
                        else:
                            downloaded, total_size = data[1], data[2]
                            speed, seeds, connections = 0.0, 0, 0
                            phase = "connecting"
                        display_seeds = seeds
                        progress_percent = int(downloaded / total_size * 100) if total_size > 0 else 0
                        progress_percent = max(0, min(100, progress_percent))
                        
                        # Mettre à jour config.download_progress pour compatibilité
                        if url in config.download_progress:
                            config.download_progress[url]["downloaded_size"] = downloaded
                            config.download_progress[url]["total_size"] = total_size
                            config.download_progress[url]["speed"] = speed
                            config.download_progress[url]["progress_percent"] = progress_percent
                            config.download_progress[url]["seeds"] = display_seeds
                            config.download_progress[url]["connections"] = connections
                            # Si 100%, afficher "Completed" au lieu de "Downloading"
                            if progress_percent >= 100:
                                config.download_progress[url]["status"] = "Completed"
                            elif display_seeds > 0 or connections > 0:
                                display_connections = connections if connections > 0 else display_seeds
                                config.download_progress[url]["status"] = f"CN:{display_connections}"
                            else:
                                config.download_progress[url]["status"] = "Downloading"
                            
                            # Mettre à jour l'historique
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == original_history_url:
                                    entry["progress"] = progress_percent
                                    entry["downloaded_size"] = downloaded
                                    entry["total_size"] = total_size
                                    entry["speed"] = speed
                                    entry["seeds"] = display_seeds
                                    entry["connections"] = connections
                                    entry["aria2_phase"] = phase
                                    entry["status"] = "Téléchargement"
                                    config.needs_redraw = True
                                    break
                        
                        # IMPORTANT: Mettre à jour config.history PENDANT le téléchargement aussi
                        # pour que l'interface web affiche la progression en temps réel
                        # NOTE: On ne touche PAS au timestamp qui doit rester celui de création
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == original_history_url and entry["status"] in ["Downloading", "Téléchargement"]:
                                    entry["downloaded_size"] = downloaded
                                    entry["total_size"] = total_size
                                    entry["speed"] = speed
                                    entry["seeds"] = display_seeds
                                    entry["connections"] = connections
                                    entry["progress"] = progress_percent
                                    entry["aria2_phase"] = phase
                                    entry["status"] = "Téléchargement"
                                    config.needs_redraw = True
                                    # Sauvegarder au plus une fois par palier de 5%
                                    if (progress_percent % 5 == 0 or progress_percent >= 99) and progress_percent != last_saved_progress_percent:
                                        if _save_history_with_feedback("download_rom:progress"):
                                            last_saved_progress_percent = progress_percent
                                    break
            try:
                await asyncio.sleep(0.1)
            except asyncio.CancelledError:
                # La tâche asyncio a été annulée depuis l'UI (Cancel Download). Le thread
                # d'arrière-plan a déjà reçu le signal coopératif via cancel_events
                # (request_cancel) et va s'arrêter de lui-même : on sort de la boucle
                # sans propager l'annulation, pour ne jamais sauter le nettoyage final
                # (thread.join, drain de la queue, notify_download_finished) qui libère
                # le slot de la file d'attente.
                logger.debug(f"Boucle de progression annulée (Cancel Download) pour task_id={task_id}")
                break
        except Exception as e:
            logger.error(f"Erreur mise à jour progression: {str(e)}")
    
    thread.join()
    try:
        download_threads.pop(task_id, None)
    except Exception:
        pass
    # Drain any remaining final message to ensure history is saved
    try:
        task_queue = progress_queues.get(task_id)
        if task_queue:
            while not task_queue.empty():
                data = task_queue.get()
                if isinstance(data[1], bool):
                    success, message = data[1], data[2]
                    logger.debug(f"[DRAIN_QUEUE] Processing final message: success={success}, message={message[:100] if message else 'None'}")
                    if isinstance(config.history, list):
                        for entry in config.history:
                            if "url" in entry and entry["url"] == original_history_url and entry["status"] in ["Downloading", "Téléchargement", "Extracting", "Converting"]:
                                entry["status"] = "Download_OK" if success else "Erreur"
                                entry["progress"] = 100 if success else 0
                                entry["message"] = message
                                _save_history_with_feedback("download_rom:drain")
                                # Marquer le jeu comme téléchargé si succès
                                if success:
                                    logger.debug(f"[DRAIN_QUEUE] Marking game as downloaded: platform={platform}, game={game_name}")
                                    from history import mark_game_as_downloaded
                                    file_size = entry.get("size", "N/A")
                                    mark_game_as_downloaded(platform, game_name, file_size)
                                break
    except Exception as e:
        logger.error(f"[DRAIN_QUEUE] Error processing final message: {e}")


    # Nettoyer la queue
    if task_id in progress_queues:
        del progress_queues[task_id]
    cancel_events.pop(task_id, None)
    
    # Sauvegarder le résultat AVANT de retirer l'URL du set (pour les doublons)
    with urls_lock:
        url_results[original_history_url] = (result[0], result[1])
        urls_in_progress.discard(original_history_url)
        logger.debug(f"URL supprimée du set des téléchargements en cours: {original_history_url} (URLs restantes: {len(urls_in_progress)})")
        # Signaler l'événement pour les appels doublons en attente
        if original_history_url in url_done_events:
            url_done_events[original_history_url].set()
    
    # Libérer le slot de la queue
    try:
        notify_download_finished()
    except Exception:
        pass
    
    return result[0], result[1]
