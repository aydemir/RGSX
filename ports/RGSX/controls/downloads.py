import asyncio
import datetime
import json
import logging
import threading

import pygame

import config
from display import show_toast
from history import save_history
from language import _
from network import download_from_1fichier, download_rom, is_1fichier_url
from utils import ensure_download_provider_keys, get_clean_display_name, missing_all_provider_keys

logger = logging.getLogger("controls")

ARCHIVE_EXTENSIONS = {'.zip', '.7z', '.rar', '.tar', '.gz', '.xz', '.bz2'}

def _notify_torrent_in_maintenance(game_name: str | None = None) -> None:
    # Fonction devenue inutile, ne fait plus rien
    pass

def _has_download_url(url, game_name: str | None = None) -> bool:
    if isinstance(url, str) and url.strip():
        return True

    config.needs_redraw = True
    return False

def _launch_next_queued_download(force: bool = False):
    """Lance le(s) prochain(s) téléchargement(s) de la queue selon les slots disponibles.
    Si force=True, ignore la limite (Force Download depuis l'UI).
    Peut être appelée plusieurs fois pour remplir tous les slots libres.
    """
    max_dl = getattr(config, 'max_simultaneous_downloads', 5)
    active = getattr(config, 'active_download_count', 0)
    if not force and active >= max_dl:
        return
    if not config.download_queue:
        return

    queue_item = config.download_queue.pop(0)
    config.active_download_count = active + 1
    config.download_active = True

    url = queue_item['url']
    platform = queue_item['platform']
    game_name = queue_item['game_name']
    is_zip_non_supported = queue_item['is_zip_non_supported']
    is_1fichier = queue_item['is_1fichier']
    task_id = queue_item['task_id']

    # Mettre à jour le statut dans l'historique: queued -> Downloading
    for entry in config.history:
        if entry.get('task_id') == task_id and entry.get('status') == 'Queued':
            entry['status'] = 'Downloading'
            entry['message'] = _("download_in_progress")
            save_history(config.history)
            break

    logger.info(f"📋 Lancement téléchargement (slot {config.active_download_count}/{max_dl}): {game_name} (task_id={task_id})")

    # Lancer le téléchargement de manière asynchrone avec callback
    try:
        if is_1fichier:
            task = asyncio.create_task(download_from_1fichier(url, platform, game_name, is_zip_non_supported, task_id))
        else:
            task = asyncio.create_task(download_rom(url, platform, game_name, is_zip_non_supported, task_id))
        _register_download_task(task_id, task, url, game_name, platform)
        
    except Exception as e:
        logger.error(f"Erreur lancement queue download: {e}")
        config.active_download_count = max(0, getattr(config, 'active_download_count', 1) - 1)
        config.download_active = config.active_download_count > 0
        # Mettre à jour l'historique en erreur
        for entry in config.history:
            if entry.get('task_id') == task_id:
                entry['status'] = 'Erreur'
                entry['message'] = str(e)
                save_history(config.history)
                break
        # Relancer le suivant
        _launch_next_queued_download()

def _register_download_task(task_id: str, task, url: str, game_name: str, platform: str, increment_slot: bool = False):
    """Enregistre une tâche de téléchargement et branche la relance de queue à la fin."""
    if increment_slot:
        config.active_download_count = getattr(config, 'active_download_count', 0) + 1
        config.download_active = True

    config.download_tasks[task_id] = (task, url, game_name, platform)

    def on_task_done(t):
        try:
            t.result()
        except asyncio.CancelledError:
            logger.info(f"Tâche annulée pour {game_name} (task_id={task_id})")
        except Exception as e:
            logger.error(f"Erreur tâche download {game_name}: {e}")
        finally:
            # Le décrément des slots est géré dans network.notify_download_finished().
            config.download_active = getattr(config, 'active_download_count', 0) > 0
            _launch_next_queued_download()

    task.add_done_callback(on_task_done)

def _queue_download(url: str, platform: str, game_name: str, is_zip_non_supported: bool, display_name: str | None = None) -> str:
    """Ajoute un téléchargement à la file d'attente et l'historique."""
    task_id = str(pygame.time.get_ticks())
    queue_item = {
        'url': url,
        'platform': platform,
        'game_name': game_name,
        'is_zip_non_supported': is_zip_non_supported,
        'is_1fichier': is_1fichier_url(url),
        'task_id': task_id,
        'status': 'Queued'
    }
    config.download_queue.append(queue_item)

    shown_name = display_name or get_clean_display_name(game_name, platform)
    config.history.append({
        'platform': platform,
        'game_name': game_name,
        'display_name': shown_name,
        'status': 'Queued',
        'url': url,
        'progress': 0,
        'message': _("download_queued"),
        'timestamp': datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        'downloaded_size': 0,
        'total_size': 0,
        'task_id': task_id
    })
    save_history(config.history)
    show_toast(f"{shown_name}\n{_('download_queued')}")
    config.needs_redraw = True
    logger.info(f"{game_name} ajouté à la file d'attente. Queue size: {len(config.download_queue)}")
    return task_id

def _delegate_download_to_manager(url: str, platform: str, game_name: str, display_name: str | None = None):
    """Délègue un téléchargement au manager RGSX via HTTP (en arrière-plan)."""
    import urllib.request
    port = getattr(config, 'manager_port', 5000)
    shown_name = display_name or get_clean_display_name(game_name, platform)

    def _post():
        try:
            body = json.dumps({
                'platform': platform,
                'game_name': game_name,
                'url': url,
                'mode': 'now',
            }).encode('utf-8')
            req = urllib.request.Request(
                f'http://127.0.0.1:{port}/api/download',
                data=body,
                headers={'Content-Type': 'application/json'},
            )
            with urllib.request.urlopen(req, timeout=15) as resp:
                data = json.loads(resp.read().decode('utf-8'))
            if data.get('success'):
                show_toast(f"{shown_name}\n{_('download_started') if _ else 'Download started'}")
            else:
                logger.error(f"[MANAGER] /api/download refusé: {data.get('error')}")
        except Exception as e:
            logger.error(f"[MANAGER] délégation échouée: {e}")

    threading.Thread(target=_post, daemon=True).start()
    config.needs_redraw = True
    return ("queued", "manager")

def start_or_queue_download(url: str, platform: str, game_name: str, is_zip_non_supported: bool, display_name: str | None = None, force_start: bool = False) -> tuple[str, str]:
    """Démarre un téléchargement si un slot est libre, sinon le place en queue."""
    # Mode manager: tous les téléchargements passent par le daemon (tray/web/TV unifiés).
    if getattr(config, 'manager_available', False):
        return _delegate_download_to_manager(url, platform, game_name, display_name)

    max_dl = getattr(config, 'max_simultaneous_downloads', 5)
    active = getattr(config, 'active_download_count', 0)

    if not force_start and active >= max_dl:
        task_id = _queue_download(url, platform, game_name, is_zip_non_supported, display_name)
        _launch_next_queued_download()
        return ("queued", task_id)

    if is_1fichier_url(url):
        ensure_download_provider_keys(False)
        if missing_all_provider_keys():
            logger.warning("Aucune clé API - Mode gratuit 1fichier sera utilisé (attente requise)")
        task_id = str(pygame.time.get_ticks())
        task = asyncio.create_task(download_from_1fichier(url, platform, game_name, is_zip_non_supported, task_id))
    else:
        task_id = str(pygame.time.get_ticks())
        task = asyncio.create_task(download_rom(url, platform, game_name, is_zip_non_supported, task_id))

    _register_download_task(task_id, task, url, game_name, platform, increment_slot=True)
    shown_name = display_name or get_clean_display_name(game_name, platform)
    show_toast(f"{_('download_started')}: {shown_name}")
    config.needs_redraw = True
    logger.info(f"Téléchargement démarré: {game_name} pour {platform}, task_id={task_id}")
    return ("started", task_id)

