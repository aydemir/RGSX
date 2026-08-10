# -*- coding: utf-8 -*-
"""RGSXHandler mixin: routes settings / system info / update-cache / support."""
import os
import sys
import logging
import time
import threading
import shutil
import tempfile
import urllib.request
import copy

import config
from .cache import source_cache, cache_lock, generate_etag, _now_utc, invalidate_all_caches
from utils import load_sources, extract_data, request_torrent_manifest_refresh, _redact_settings_file_text

from . import logger


class SettingsMixin:
    def _api_settings_get(self):
        try:
            from rgsx_settings import load_rgsx_settings, get_auto_extract
            from utils import check_web_service_status, check_custom_dns_status, load_api_keys
            settings = load_rgsx_settings()

            # Ajouter les options dynamiques
            settings['auto_extract'] = get_auto_extract()

            # Options Linux/Batocera
            if config.OPERATING_SYSTEM == "Linux":
                settings['web_service_at_boot'] = check_web_service_status()
                settings['custom_dns_at_boot'] = check_custom_dns_status()

            # API Keys (filtrer la clé 'reloaded' qui n'est pas utile pour l'UI)
            api_keys_data = load_api_keys()
            settings['api_keys'] = {
                '1fichier': api_keys_data.get('1fichier', ''),
                'alldebrid': api_keys_data.get('alldebrid', ''),
                'debridlink': api_keys_data.get('debridlink', ''),
                'realdebrid': api_keys_data.get('realdebrid', ''),
                'torbox': api_keys_data.get('torbox', '')
            }

            self._send_json({
                'success': True,
                'settings': settings,
                'system_info': {
                    'system': config.OPERATING_SYSTEM,
                    'roms_folder': config.ROMS_FOLDER,
                    'platforms_count': len(config.platforms) if hasattr(config, 'platforms') else 0
                }
            })
        except Exception as e:
            logger.error(f"Erreur lors de la lecture des settings: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_system_info(self):
        try:
            # Rafraîchir les informations système avant de les renvoyer
            config.get_batocera_system_info()

            self._send_json({
                'success': True,
                'system_info': config.SYSTEM_INFO
            })
        except Exception as e:
            logger.error(f"Erreur lors de la récupération des infos système: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_update_cache(self):
        try:
            # Chemins à supprimer (utiliser les constantes de config)
            sources_file = config.SOURCES_FILE  # systems_list.json
            games_folder = config.GAMES_FOLDER
            images_folder = config.IMAGES_FOLDER

            deleted = []

            # Supprimer systems_list.json
            if os.path.exists(sources_file):
                os.remove(sources_file)
                deleted.append('systems_list.json')
                logger.info(f"✅ Fichier systems_list.json supprimé")

            # Supprimer dossier games/
            if os.path.exists(games_folder):
                shutil.rmtree(games_folder)
                deleted.append('games/')
                logger.info(f"✅ Dossier games/ supprimé")

            # Supprimer dossier images/
            if os.path.exists(images_folder):
                shutil.rmtree(images_folder)
                deleted.append('images/')
                logger.info(f"✅ Dossier images/ supprimé")

            # IMPORTANT: Télécharger et extraire games.zip depuis le serveur OTA
            logger.info("🔄 Téléchargement de games.zip depuis le serveur...")
            try:
                # URL du ZIP
                games_zip_url = config.OTA_data_ZIP  # https://retrogamesets.fr/softs/games.zip

                # Télécharger dans un fichier temporaire
                with tempfile.NamedTemporaryFile(delete=False, suffix='.zip') as tmp_file:
                    zip_path = tmp_file.name

                # Télécharger le ZIP
                logger.info(f"Téléchargement depuis {games_zip_url}...")
                urllib.request.urlretrieve(games_zip_url, zip_path)
                logger.info(f"✅ ZIP téléchargé: {os.path.getsize(zip_path)} octets")

                # Extraire dans SAVE_FOLDER
                logger.info(f"📂 Extraction vers {config.SAVE_FOLDER}...")
                success, message = extract_data(zip_path, config.SAVE_FOLDER, games_zip_url)

                # Supprimer le ZIP temporaire
                if os.path.exists(zip_path):
                    os.remove(zip_path)

                if success:
                    from rgsx_settings import get_remote_gamelist_timestamp, set_last_gamelist_update

                    remote_update_dt = get_remote_gamelist_timestamp(games_zip_url)
                    set_last_gamelist_update(remote_update_dt)
                    logger.info(f"✅ Extraction réussie: {message}")
                    deleted.append(f'extracted: {message}')

                    # Maintenant charger les sources
                    invalidate_all_caches(reason='update-cache refresh')
                    logger.info("🔄 Chargement des plateformes...")
                    request_torrent_manifest_refresh()
                    refreshed_sources = load_sources(allow_torrent_manifest_fetch=True)
                    if refreshed_sources is not None:
                        with cache_lock:
                            source_cache.update({
                                'data': copy.deepcopy(refreshed_sources),
                                'timestamp': time.time(),
                                'etag': generate_etag(refreshed_sources),
                                'last_modified': _now_utc(),
                            })
                    platforms_count = len(getattr(config, 'platforms', []))
                    logger.info(f"✅ {platforms_count} plateformes chargées")
                    deleted.append(f'loaded: {platforms_count} platforms')
                else:
                    raise Exception(f"Échec extraction: {message}")

            except Exception as reload_error:
                logger.error(f"❌ Erreur lors du téléchargement/extraction: {reload_error}")
                deleted.append(f'error: {str(reload_error)}')

            if deleted:
                self._send_json({
                    'success': True,
                    'message': 'Cache cleared and data reloaded successfully.',
                    'deleted': deleted
                })
            else:
                self._send_json({
                    'success': True,
                    'message': 'No cache found.',
                    'deleted': []
                })

        except Exception as e:
            logger.error(f"❌ Erreur lors du nettoyage du cache: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_settings_post(self, data):
        try:
            from rgsx_settings import save_rgsx_settings, set_auto_extract
            from utils import toggle_web_service_at_boot, toggle_custom_dns_at_boot, save_api_keys

            settings = data.get('settings')
            if not settings:
                self._send_json({
                    'success': False,
                    'error': 'Paramètre "settings" manquant'
                }, status=400)
                return

            # Gérer auto_extract séparément
            if 'auto_extract' in settings:
                set_auto_extract(settings['auto_extract'])
                del settings['auto_extract']  # Ne pas sauvegarder dans le fichier principal

            # Gérer web_service_at_boot (Linux only)
            if 'web_service_at_boot' in settings:
                if config.OPERATING_SYSTEM == "Linux":
                    try:
                        toggle_web_service_at_boot(settings['web_service_at_boot'])
                    except Exception as e:
                        logger.error(f"Erreur toggle web service: {e}")
                del settings['web_service_at_boot']

            # Gérer custom_dns_at_boot (Linux only)
            if 'custom_dns_at_boot' in settings:
                if config.OPERATING_SYSTEM == "Linux":
                    try:
                        toggle_custom_dns_at_boot(settings['custom_dns_at_boot'])
                    except Exception as e:
                        logger.error(f"Erreur toggle custom DNS: {e}")
                del settings['custom_dns_at_boot']

            # Gérer API keys séparément
            if 'api_keys' in settings:
                try:
                    save_api_keys(settings['api_keys'])
                except Exception as e:
                    logger.error(f"Erreur sauvegarde API keys: {e}")
                del settings['api_keys']

            save_rgsx_settings(settings)

            self._send_json({
                'success': True,
                'message': 'Paramètres sauvegardés avec succès'
            })

        except Exception as e:
            logger.error(f"Erreur lors de la sauvegarde des settings: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_save_filters(self, data):
        try:
            from rgsx_settings import load_rgsx_settings, save_rgsx_settings

            # Charger les settings actuels
            current_settings = load_rgsx_settings()

            # Mettre à jour seulement les filtres
            if 'game_filters' not in current_settings:
                current_settings['game_filters'] = {}

            current_settings['game_filters']['region_filters'] = data.get('region_filters', {})
            current_settings['game_filters']['hide_non_release'] = data.get('hide_non_release', False)
            current_settings['game_filters']['one_rom_per_game'] = data.get('one_rom_per_game', False)
            current_settings['game_filters']['hide_downloaded'] = data.get('hide_downloaded', False)
            current_settings['game_filters']['regex_mode'] = data.get('regex_mode', False)
            current_settings['game_filters']['region_priority'] = data.get('region_priority', ['USA', 'Canada', 'World', 'Europe', 'Japan', 'Other'])

            # Sauvegarder
            save_rgsx_settings(current_settings)

            # Mettre à jour config.game_filter_obj
            if getattr(config, 'game_filter_obj', None) is not None:
                config.game_filter_obj.region_filters = data.get('region_filters', {})
                config.game_filter_obj.hide_non_release = data.get('hide_non_release', False)
                config.game_filter_obj.one_rom_per_game = data.get('one_rom_per_game', False)
                config.game_filter_obj.hide_downloaded = data.get('hide_downloaded', False)
                config.game_filter_obj.regex_mode = data.get('regex_mode', False)
                config.game_filter_obj.region_priority = data.get('region_priority', ['USA', 'Canada', 'World', 'Europe', 'Japan', 'Other'])

            self._send_json({
                'success': True,
                'message': 'Filtres sauvegardés'
            })

        except Exception as e:
            logger.error(f"Erreur lors de la sauvegarde des filtres: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_clear_history(self):
        try:
            from history import clear_history

            clear_history()
            config.history = []  # Vider aussi la liste en mémoire

            self._send_json({
                'success': True,
                'message': 'Historique vidé avec succès'
            })

        except Exception as e:
            logger.error(f"Erreur lors du vidage de l\\'historique: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_restart(self):
        try:
            logger.info("Demande de redémarrage via l'interface web")

            # Importer restart_application depuis utils
            from utils import restart_application

            # Envoyer la réponse avant de redémarrer
            self._send_json({
                'success': True,
                'message': 'Redémarrage en cours...'
            })

            # Flush les logs
            for handler in logging.root.handlers:
                handler.flush()

            # Programmer le redémarrage dans 2 secondes
            logger.info("Redémarrage programmé dans 2 secondes")
            def delayed_restart():
                time.sleep(2)
                logger.info("Lancement du redémarrage...")
                restart_application(0)

            restart_thread = threading.Thread(target=delayed_restart, daemon=True)
            restart_thread.start()

        except Exception as e:
            logger.error(f"Erreur lors du redémarrage: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_support(self):
        try:
            import zipfile
            import tempfile
            from datetime import datetime

            logger.info("Génération d'un fichier de support")

            # Créer un fichier ZIP temporaire
            timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
            zip_filename = f"rgsx_support_{timestamp}.zip"
            zip_path = os.path.join(tempfile.gettempdir(), zip_filename)

            # Liste des fichiers à inclure
            files_to_include = []

            # Ajouter les fichiers de configuration
            if hasattr(config, 'CONTROLS_CONFIG_PATH') and os.path.exists(config.CONTROLS_CONFIG_PATH):
                files_to_include.append(('controls.json', config.CONTROLS_CONFIG_PATH))

            if hasattr(config, 'HISTORY_PATH') and os.path.exists(config.HISTORY_PATH):
                files_to_include.append(('history.json', config.HISTORY_PATH))

            if hasattr(config, 'RGSX_SETTINGS_PATH') and os.path.exists(config.RGSX_SETTINGS_PATH):
                files_to_include.append(('rgsx_settings.json', config.RGSX_SETTINGS_PATH))

            # Ajouter les fichiers de log
            if hasattr(config, 'log_file') and os.path.exists(config.log_file):
                files_to_include.append(('RGSX.log', config.log_file))

            # Log du serveur web
            web_log = os.path.join(config.log_dir, 'rgsx_web.log')
            if os.path.exists(web_log):
                files_to_include.append(('rgsx_web.log', web_log))

            # Log de démarrage du serveur web
            web_startup_log = os.path.join(config.log_dir, 'rgsx_web_startup.log')
            if os.path.exists(web_startup_log):
                files_to_include.append(('rgsx_web_startup.log', web_startup_log))

            # Créer le fichier ZIP
            with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
                for archive_name, file_path in files_to_include:
                    try:
                        if archive_name == 'rgsx_settings.json':
                            zipf.writestr(archive_name, _redact_settings_file_text(file_path))
                        else:
                            zipf.write(file_path, archive_name)
                        logger.debug(f"Ajouté au ZIP: {archive_name}")
                    except Exception as e:
                        logger.warning(f"Impossible d'ajouter {archive_name}: {e}")

                # Ajouter un fichier README avec des informations système
                readme_content = f"""RGSX Support Package
Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

System Information:
- OS: {config.OPERATING_SYSTEM}
- Python: {sys.version}
- Platform: {sys.platform}

Included Files:
"""
                for archive_name, _ in files_to_include:
                    readme_content += f"- {archive_name}\n"

                readme_content += """
Instructions:
1. Join RGSX Discord server
2. Describe your issue in the support channel
3. Upload this ZIP file to help the team diagnose your problem

DO NOT share this file publicly as it may contain sensitive information.
Sensitive values (passwords, API keys, tokens) are redacted.
"""
                zipf.writestr('README.txt', readme_content)

            # Lire le fichier ZIP pour l'envoyer
            with open(zip_path, 'rb') as f:
                zip_data = f.read()

            # Supprimer le fichier temporaire
            os.remove(zip_path)

            # Envoyer le fichier ZIP
            self.send_response(200)
            self.send_header('Content-Type', 'application/zip')
            self.send_header('Content-Disposition', f'attachment; filename="{zip_filename}"')
            self.send_header('Content-Length', str(len(zip_data)))
            self.end_headers()
            self.wfile.write(zip_data)

            logger.info(f"Fichier de support généré: {zip_filename} ({len(zip_data)} bytes)")

        except Exception as e:
            logger.error(f"Erreur lors de la génération du fichier de support: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)
