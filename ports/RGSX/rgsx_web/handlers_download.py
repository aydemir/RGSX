# -*- coding: utf-8 -*-
"""RGSXHandler mixin: routes téléchargements / queue / progression / annulation."""
import os
import time
import threading
import asyncio

import config
from .cache import get_cached_games
from .i18n import TRANSLATIONS, get_translation
from history import load_history, save_history
from utils import get_clean_display_name
from network import download_rom, download_from_1fichier

from . import logger


class DownloadMixin:
    def _process_queued_download(self, queue_item):
        """Traite un élément de la queue de téléchargement"""
        game_url = queue_item['url']
        platform = queue_item['platform']
        game_name = queue_item['game_name']
        is_zip_non_supported = queue_item['is_zip_non_supported']
        is_1fichier = queue_item['is_1fichier']
        task_id = queue_item['task_id']

        config.active_download_count = getattr(config, 'active_download_count', 0) + 1
        config.download_active = True

        # Mettre à jour l'historique: queued -> Downloading
        config.history = load_history()
        for entry in config.history:
            if entry.get('task_id') == task_id and entry.get('status') == 'Queued':
                entry['status'] = 'Downloading'
                entry['message'] = get_translation('download_in_progress')
                save_history(config.history)
                logger.info(f"📋 Statut mis à jour de 'queued' à 'Downloading' pour {game_name} (task_id={task_id})")
                break

        if is_1fichier:
            download_func = download_from_1fichier
            logger.info(f"🔗 Queue: download_from_1fichier() pour {game_name}, extraction={is_zip_non_supported}")
        else:
            download_func = download_rom
            logger.info(f"📦 Queue: Téléchargement {game_name}, extraction={is_zip_non_supported}")

        def run_download():
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                loop.run_until_complete(
                    download_func(game_url, platform, game_name, is_zip_non_supported, task_id)
                )
            finally:
                loop.close()
                # Après le téléchargement, traiter la queue selon les slots disponibles.
                config.download_active = getattr(config, 'active_download_count', 0) > 0
                max_dl = getattr(config, 'max_simultaneous_downloads', 5)
                active_now = getattr(config, 'active_download_count', 0)
                if active_now < max_dl and config.download_queue:
                    next_item = config.download_queue.pop(0)
                    logger.info(f"📋 Traitement du prochain élément de la queue: {next_item['game_name']}")
                    # Relancer de manière asynchrone
                    threading.Thread(target=lambda: self._process_queued_download(next_item), daemon=True).start()

        thread = threading.Thread(target=run_download, daemon=True)
        thread.start()

    def _api_progress(self):
        # Lire depuis history.json - filtrer seulement les téléchargements en cours
        history = load_history() or []

        print(f"\n[DEBUG PROGRESS] history.json charge avec {len(history)} entrees totales")

        # Filtrer les entrées avec status "Downloading", "Téléchargement", "Connecting", "Try X/Y"
        in_progress_statuses = ["Downloading", "Téléchargement", "Downloading", "Connecting", "Extracting"]

        downloads = {}
        for entry in history:
            status = entry.get('status', '')
            # Inclure aussi les status qui commencent par "Try" (ex: "Try 1/4")
            if status in in_progress_statuses or status.startswith('Try '):
                url = entry.get('url', '')
                if url:
                    downloads[url] = {
                        'downloaded_size': entry.get('downloaded_size', 0),
                        'total_size': entry.get('total_size', 0),
                        'status': status,
                        'progress_percent': entry.get('progress', 0),
                        'speed': entry.get('speed', 0),
                        'seeds': entry.get('seeds', 0),
                        'connections': entry.get('connections', 0),
                        'game_name': entry.get('game_name', ''),
                        'platform': entry.get('platform', ''),
                        'timestamp': entry.get('timestamp', '')
                    }
            else:
                # Debug: afficher les premiers status qui ne matchent pas
                if len(downloads) < 3:
                    print(f"  [DEBUG] Ignore - Status: '{status}', Game: {entry.get('game_name', '')[:50]}")

        print(f"[DEBUG PROGRESS] {len(downloads)} telechargements en cours trouves")
        if downloads:
            for url, data in list(downloads.items())[:2]:
                print(f"  - URL: {url[:80]}...")
                print(f"    Status: {data.get('status')}, Progress: {data.get('progress_percent')}%")

        self._send_json({
            'success': True,
            'downloads': downloads
        })

    def _api_game_status(self):
        try:
            import json as _json

            history = load_history() or []

            # İndirilen oyunlar (config'den al, dosyadan değil)
            downloaded = getattr(config, 'downloaded_games', {})

            # Aktif indirmeler (progress)
            in_progress = {}
            for entry in history:
                status = entry.get('status', '')
                if status in ("Downloading", "Téléchargement", "Connecting", "Extracting") or status.startswith('Try '):
                    game_name = entry.get('game_name', '')
                    if game_name:
                        stem = os.path.splitext(game_name)[0]
                        in_progress[stem.lower()] = {
                            'status': 'downloading',
                            'progress': entry.get('progress', 0)
                        }

            # Başarısız indirmeler
            failed = {}
            for entry in reversed(history):
                status = entry.get('status', '')
                if status in ("Erreur", "Error"):
                    game_name = entry.get('game_name', '')
                    platform = entry.get('platform', '')
                    stem = os.path.splitext(game_name)[0]
                    key = f"{platform}::{stem.lower()}"
                    if key not in failed:
                        failed[key] = {'status': 'failed'}

            # Tüm durumları birleştir
            result = {}
            for platform_name, games in downloaded.items():
                for game_name in games:
                    stem = os.path.splitext(game_name)[0]
                    result[stem.lower()] = {
                        'status': 'downloaded',
                        'platform': platform_name
                    }

            # Aktif indirmeleri ekle/override et
            for stem_lower, info in in_progress.items():
                result[stem_lower] = info

            # Başarısızları ekle (sadece indirilmemiş olanlar)
            for key, info in failed.items():
                platform, stem_lower = key.split('::', 1)
                if stem_lower not in result:
                    result[stem_lower] = info

            self._send_json({
                'success': True,
                'statuses': result
            })
        except Exception as e:
            logger.error(f"Erreur /api/game-status: {e}")
            self._send_json({
                'success': False,
                'error': str(e),
                'statuses': {}
            })

    def _api_history(self):
        # Lire depuis history.json - filtrer pour inclure en cours ET terminés
        history = load_history() or []

        # Inclure: statuts terminés + en queue + en cours
        included_statuses = [
            "Download_OK", "Erreur", "error", "Canceled", "Already_Present",  # Terminés
            "Queued", "Downloading", "Téléchargement", "Downloading", "Connecting", "Extracting",  # En cours
        ]
        # Inclure aussi les statuts "Try X/Y" (tentatives)
        visible_history = [
            entry for entry in history
            if entry.get('status', '') in included_statuses or
               str(entry.get('status', '')).startswith('Try ')
        ]

        # Trier par timestamp (plus récent en premier)
        visible_history.sort(
            key=lambda x: x.get('timestamp', ''),
            reverse=True
        )

        self._send_json({
            'success': True,
            'count': len(visible_history),
            'history': visible_history
        })

    def _api_queue_get(self):
        try:
            queue_status = {
                'success': True,
                'active': config.download_active,
                'queue': config.download_queue,
                'queue_size': len(config.download_queue)
            }
            self._send_json(queue_status)
        except Exception as e:
            logger.error(f"Erreur lors de la récupération de la queue: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_download(self, data):
        platform = data.get('platform')
        game_index = data.get('game_index')
        game_name_param = data.get('game_name')  # Nouveau: chercher par nom
        mode = data.get('mode', 'now')  # 'now' ou 'queue'

        if not platform or (game_index is None and not game_name_param):
            self._send_json({
                'success': False,
                'error': 'Paramètres manquants: platform et (game_index ou game_name) requis'
            }, status=400)
            return

        # Charger les jeux de la plateforme (cache)
        games, _, _ = get_cached_games(platform)

        # Si game_name est fourni, chercher l'index correspondant
        if game_name_param and game_index is None:
            game_index = None
            for idx, game in enumerate(games):
                current_game_name = game.name
                if current_game_name == game_name_param:
                    game_index = idx
                    break

            if game_index is None:
                self._send_json({
                    'success': False,
                    'error': f'Jeu non trouvé: {game_name_param}'
                }, status=400)
                return

        # Vérifier que game_index est valide (après recherche ou direct)
        if game_index is None or game_index < 0 or game_index >= len(games):
            self._send_json({
                'success': False,
                'error': f'Index de jeu invalide: {game_index}'
            }, status=400)
            return

        game = games[game_index]
        game_name = game.name
        game_url = game.url

        if not game_url:
            torrent_message = TRANSLATIONS.get('popup_torrent_in_maintenance', 'torrent in maintence')
            self._send_json({
                'success': False,
                'error': torrent_message
            }, status=400)
            return

        # Suppression du blocage torrent : on laisse passer les URLs rgsx+torrent

        # Vérifier l'extension et déterminer si extraction nécessaire
        from utils import check_extension_before_download
        check_result = check_extension_before_download(game_url, platform, game_name)

        if not check_result:
            self._send_json({
                'success': False,
                'error': 'Extension non supportée ou erreur de vérification'
            }, status=400)
            return

        # check_result est un tuple: (url, platform, game_name, is_zip_non_supported)
        is_zip_non_supported = check_result[3] if len(check_result) > 3 else False

        # Détecter si c'est un lien 1fichier et utiliser la fonction appropriée
        is_1fichier = "1fichier.com" in game_url

        task_id = f"web_{int(time.time() * 1000)}"

        # Déterminer si on doit ajouter à la queue selon le mode et les slots disponibles.
        max_dl = getattr(config, 'max_simultaneous_downloads', 5)
        active_count = getattr(config, 'active_download_count', 0)
        queue_full = active_count >= max_dl
        should_queue = (mode == 'queue' and config.download_active) or queue_full

        if mode == 'now':
            # mode='now' lance immédiatement uniquement si un slot est disponible.
            if queue_full:
                queue_item = {
                    'url': game_url,
                    'platform': platform,
                    'game_name': game_name,
                    'is_zip_non_supported': is_zip_non_supported,
                    'is_1fichier': is_1fichier,
                    'task_id': task_id,
                    'status': 'Queued'
                }
                config.download_queue.append(queue_item)
                import datetime
                queue_history_entry = {
                    'platform': platform,
                    'game_name': game_name,
                    'display_name': get_clean_display_name(game_name, platform),
                    'status': 'Queued',
                    'url': game_url,
                    'progress': 0,
                    'message': get_translation('download_queued'),
                    'timestamp': datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                    'downloaded_size': 0,
                    'total_size': 0,
                    'task_id': task_id
                }
                config.history.append(queue_history_entry)
                save_history(config.history)
                self._send_json({
                    'success': True,
                    'message': f"{game_name} ajouté à la file d'attente",
                    'task_id': task_id,
                    'game_name': game_name,
                    'platform': platform,
                    'queued': True,
                    'queue_position': len(config.download_queue)
                })
                return

            logger.info(f"⚡ Téléchargement immédiat lancé en parallèle (mode=now): {game_name}")

            if is_1fichier:
                download_func = download_from_1fichier
                logger.info(f"🔗 Détection 1fichier, utilisation de download_from_1fichier() pour {game_name}, extraction={is_zip_non_supported}")
            else:
                download_func = download_rom
                logger.info(f"📦 Téléchargement {game_name}, extraction={is_zip_non_supported}")

            def run_download_now():
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                try:
                    loop.run_until_complete(
                        download_func(game_url, platform, game_name, is_zip_non_supported, task_id)
                    )
                finally:
                    loop.close()
                    config.download_active = getattr(config, 'active_download_count', 0) > 0
                    max_dl_now = getattr(config, 'max_simultaneous_downloads', 5)
                    active_now = getattr(config, 'active_download_count', 0)
                    if active_now < max_dl_now and config.download_queue:
                        next_item = config.download_queue.pop(0)
                        threading.Thread(target=lambda: self._process_queued_download(next_item), daemon=True).start()

            config.active_download_count = getattr(config, 'active_download_count', 0) + 1
            config.download_active = True

            thread = threading.Thread(target=run_download_now, daemon=True)
            thread.start()

            self._send_json({
                'success': True,
                'message': f'Téléchargement de {game_name} lancé',
                'task_id': task_id,
                'game_name': game_name,
                'platform': platform,
                'is_1fichier': is_1fichier
            })

        elif should_queue:
            # mode='queue' ET un téléchargement est actif -> ajouter à la queue
            queue_item = {
                'url': game_url,
                'platform': platform,
                'game_name': game_name,
                'is_zip_non_supported': is_zip_non_supported,
                'is_1fichier': is_1fichier,
                'task_id': task_id,
                'status': 'Queued'
            }
            config.download_queue.append(queue_item)

            # Ajouter une entrée à l'historique avec status "queued"
            import datetime
            queue_history_entry = {
                'platform': platform,
                'game_name': game_name,
                'display_name': get_clean_display_name(game_name, platform),
                'status': 'Queued',
                'url': game_url,
                'progress': 0,
                'message': get_translation('download_queued'),
                'timestamp': datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                'downloaded_size': 0,
                'total_size': 0,
                'task_id': task_id
            }
            config.history.append(queue_history_entry)

            # Sauvegarder l'historique
            save_history(config.history)

            logger.info(f"📋 {game_name} ajouté à la file d'attente (mode=queue, active={active_count}/{max_dl})")

            self._send_json({
                'success': True,
                'message': f'{game_name} ajouté à la file d\'attente',
                'task_id': task_id,
                'game_name': game_name,
                'platform': platform,
                'queued': True,
                'queue_position': len(config.download_queue)
            })
        else:
            # mode='queue' MAIS pas de téléchargement actif -> lancer immédiatement (premier élément)
            config.active_download_count = getattr(config, 'active_download_count', 0) + 1
            config.download_active = True
            logger.info(f"🚀 Lancement du premier élément de la queue: {game_name}")

            # Ajouter une entrée à l'historique avec status "Downloading"
            # (pas "queued" car on lance immédiatement)
            import datetime
            download_history_entry = {
                'platform': platform,
                'game_name': game_name,
                'display_name': get_clean_display_name(game_name, platform),
                'status': 'Downloading',
                'url': game_url,
                'progress': 0,
                'message': get_translation('download_in_progress'),
                'timestamp': datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                'downloaded_size': 0,
                'total_size': 0,
                'task_id': task_id
            }
            config.history.append(download_history_entry)
            save_history(config.history)

            if is_1fichier:
                download_func = download_from_1fichier
                logger.info(f"🔗 Détection 1fichier, utilisation de download_from_1fichier() pour {game_name}, extraction={is_zip_non_supported}")
            else:
                download_func = download_rom
                logger.info(f"📦 Téléchargement {game_name}, extraction={is_zip_non_supported}")

            def run_download_queue():
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                try:
                    loop.run_until_complete(
                        download_func(game_url, platform, game_name, is_zip_non_supported, task_id)
                    )
                finally:
                    loop.close()
                    # Mode queue: traiter le suivant uniquement si un slot est libre.
                    config.download_active = getattr(config, 'active_download_count', 0) > 0
                    max_dl_queue = getattr(config, 'max_simultaneous_downloads', 5)
                    active_now = getattr(config, 'active_download_count', 0)
                    if active_now < max_dl_queue and config.download_queue:
                        next_item = config.download_queue.pop(0)
                        logger.info(f"📋 Traitement du prochain élément de la queue: {next_item['game_name']}")
                        # Relancer de manière asynchrone
                        threading.Thread(target=lambda: self._process_queued_download(next_item), daemon=True).start()

            thread = threading.Thread(target=run_download_queue, daemon=True)
            thread.start()

            self._send_json({
                'success': True,
                'message': f'Téléchargement de {game_name} lancé',
                'task_id': task_id,
                'game_name': game_name,
                'platform': platform,
                'is_1fichier': is_1fichier
            })

    def _api_cancel(self, data):
        url = data.get('url')

        if not url:
            self._send_json({
                'success': False,
                'error': 'Paramètre manquant: url requis'
            }, status=400)
            return

        try:
            from network import request_cancel

            # Trouver le task_id correspondant à l'URL dans l'historique
            history = load_history() or []
            task_id = None

            for entry in history:
                if entry.get('url') == url and entry.get('status') in ['Downloading', 'Téléchargement', 'Downloading', 'Connecting']:
                    # Mettre à jour le statut dans l'historique
                    entry['status'] = 'Canceled'
                    entry['progress'] = 0
                    entry['message'] = get_translation('web_download_canceled')

                    # Récupérer le task_id depuis l'entrée (il a été sauvegardé lors du démarrage du téléchargement)
                    task_id = entry.get('task_id')
                    break

            if task_id:
                # Tenter d'annuler le téléchargement
                cancel_success = request_cancel(task_id)
                logger.info(f"Annulation demandée pour task_id={task_id}, success={cancel_success}")
            else:
                logger.warning(f"Impossible de trouver task_id pour l'URL: {url}")

            # Sauvegarder l'historique modifié
            save_history(history)

            # Réinitialiser le flag de téléchargement actif et lancer le prochain
            config.download_active = False
            if config.download_queue:
                next_item = config.download_queue.pop(0)
                logger.info(f"📋 Traitement du prochain élément de la queue après annulation: {next_item['game_name']}")
                # Relancer de manière asynchrone
                # Créer une référence à self pour utiliser dans la lambda
                handler = self
                threading.Thread(target=lambda: handler._process_queued_download(next_item), daemon=True).start()

            self._send_json({
                'success': True,
                'message': 'Téléchargement annulé',
                'url': url,
                'task_id': task_id
            })

        except Exception as e:
            logger.error(f"Erreur lors de l\\'annulation du téléchargement: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_queue_post(self, data):
        try:
            queue_status = {
                'success': True,
                'active': config.download_active,
                'queue': config.download_queue,
                'queue_size': len(config.download_queue)
            }
            self._send_json(queue_status)
        except Exception as e:
            logger.error(f"Erreur lors de la récupération de la queue: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_queue_clear(self):
        try:
            cleared_count = len(config.download_queue)
            config.download_queue.clear()

            # Mettre à jour l'historique pour annuler les téléchargements en statut "Queued"
            history = load_history()
            for entry in history:
                if entry.get("status") == "Queued":
                    entry["status"] = "Canceled"
                    entry["message"] = get_translation('download_canceled')
                    logger.info(f"Téléchargement en attente annulé : {entry.get('game_name', '?')}")
            save_history(history)

            logger.info(f"📋 Queue vidée ({cleared_count} éléments supprimés)")
            self._send_json({
                'success': True,
                'message': f'{cleared_count} éléments supprimés de la queue',
                'cleared_count': cleared_count
            })
        except Exception as e:
            logger.error(f"Erreur lors du nettoyage de la queue: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_queue_remove(self, data):
        try:
            task_id = data.get('task_id')
            if not task_id:
                self._send_json({
                    'success': False,
                    'error': 'Paramètre manquant: task_id requis'
                }, status=400)
                return

            # Chercher et supprimer l'élément
            found = False
            for idx, item in enumerate(config.download_queue):
                if item.get('task_id') == task_id:
                    removed_item = config.download_queue.pop(idx)
                    logger.info(f"📋 {removed_item['game_name']} supprimé de la queue")
                    found = True

                    # Mettre à jour l'historique pour cet élément
                    history = load_history()
                    for entry in history:
                        if entry.get('task_id') == task_id and entry.get('status') == 'Queued':
                            entry['status'] = 'Canceled'
                            entry['message'] = get_translation('download_canceled')
                            logger.info(f"Téléchargement en attente annulé dans l'historique : {entry.get('game_name', '?')}")
                            break
                    save_history(history)
                    break

            if found:
                self._send_json({
                    'success': True,
                    'message': f'Élément supprimé de la queue',
                    'task_id': task_id
                })
            else:
                self._send_json({
                    'success': False,
                    'error': f'Élément non trouvé: {task_id}'
                }, status=404)
        except Exception as e:
            logger.error(f"Erreur lors de la suppression d'un élément de la queue: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)
