# -*- coding: utf-8 -*-
"""RGSXHandler: dispatch do_GET/do_POST + méthodes de réponse communes."""
import json
import urllib.parse
from datetime import timezone
from email.utils import parsedate_to_datetime
from http.server import BaseHTTPRequestHandler

from . import logger
from .cache import _httpdate, _ensure_datetime
from .handlers_ui import UIMixin
from .handlers_games import GamesMixin
from .handlers_download import DownloadMixin
from .handlers_settings import SettingsMixin


class RGSXHandler(UIMixin, GamesMixin, DownloadMixin, SettingsMixin, BaseHTTPRequestHandler):
    """Handler HTTP pour les requêtes RGSX"""

    def log_message(self, format, *args):
        """Override pour logger proprement (désactivé pour réduire verbosité)"""
        pass  # Logs désactivés pour éviter la pollution des logs

    def _set_headers(self, content_type='application/json', status=200, etag=None, last_modified=None, extra_headers=None):
        """Définit les headers de réponse"""
        self.send_response(status)
        self.send_header('Content-type', content_type)
        self.send_header('Access-Control-Allow-Origin', '*')  # CORS pour dev
        if etag:
            self.send_header('ETag', etag)
        if last_modified:
            http_date = _httpdate(_ensure_datetime(last_modified)) if not isinstance(last_modified, str) else last_modified
            if http_date:
                self.send_header('Last-Modified', http_date)
        if extra_headers:
            for header, value in extra_headers.items():
                self.send_header(header, value)
        self.end_headers()

    def _send_json(self, data, status=200, etag=None, last_modified=None):
        """Envoie une réponse JSON"""
        cached_dt = _ensure_datetime(last_modified)
        client_etag = self.headers.get('If-None-Match') if etag else None
        client_ims = self.headers.get('If-Modified-Since') if cached_dt else None

        if etag and client_etag == etag:
            self._set_headers('application/json', status=304, etag=etag, last_modified=cached_dt)
            return

        if cached_dt and client_ims:
            try:
                client_dt = parsedate_to_datetime(client_ims)
                if client_dt.tzinfo is None:
                    client_dt = client_dt.replace(tzinfo=timezone.utc)
                if client_dt >= cached_dt:
                    self._set_headers('application/json', status=304, etag=etag, last_modified=cached_dt)
                    return
            except (TypeError, ValueError):
                pass

        try:
            self._set_headers('application/json', status, etag=etag, last_modified=cached_dt)
            self.wfile.write(json.dumps(data, ensure_ascii=False).encode('utf-8'))
        except (ConnectionAbortedError, BrokenPipeError) as e:
            logger.debug(f"Connexion fermée par le client pendant l'envoi JSON: {e}")
            return

    def _send_html(self, html, status=200, etag=None, last_modified=None):
        """Envoie une réponse HTML"""
        try:
            self._set_headers('text/html; charset=utf-8', status, etag=etag, last_modified=last_modified)
            self.wfile.write(html.encode('utf-8'))
        except (ConnectionAbortedError, BrokenPipeError) as e:
            # La connexion a été fermée par le client, ce n'est pas une erreur critique
            logger.debug(f"Connexion fermée par le client pendant l'envoi HTML: {e}")
            pass

    def _send_not_found(self):
        """Répond avec un 404 générique."""
        self._set_headers('text/plain; charset=utf-8', status=404)
        self.wfile.write(b'Not found')

    def _get_language_from_cookies(self):
        """Récupère la langue depuis les cookies ou retourne 'en' par défaut"""
        cookie_header = self.headers.get('Cookie', '')
        if cookie_header:
            # Parser les cookies
            cookies = {}
            for cookie in cookie_header.split(';'):
                cookie = cookie.strip()
                if '=' in cookie:
                    key, value = cookie.split('=', 1)
                    cookies[key] = value
            return cookies.get('language', 'en')
        return 'en'

    def do_GET(self):
        """Traite les requêtes GET"""
        # Parser l'URL
        parsed_path = urllib.parse.urlparse(self.path)
        path = parsed_path.path

        # Logs de requêtes désactivés pour réduire verbosité

        try:
            if path.startswith('/static/'):
                self._serve_static_file(path)
                return

            # Route: Page d'accueil (avec ou sans paramètres pour navigation)
            if path == '/' or path == '/index.html' or path.startswith('/platform/') or path in ['/downloads', '/history', '/settings']:
                self._send_html(self._get_index_html())

            # Route: API - Liste des plateformes
            elif path == '/api/platforms':
                self._api_platforms()

            # Route: API - Recherche universelle (systèmes + jeux)
            elif path == '/api/search':
                self._api_search(parsed_path)

            # Route: API - Traductions
            elif path == '/api/translations':
                self._api_translations()

            # Route: API - Liste des jeux d'une plateforme
            elif path.startswith('/api/games/'):
                self._api_games(path)

            # Route: API - Progression des téléchargements (en cours seulement)
            elif path == '/api/progress':
                self._api_progress()

            # Route: API - Durum göstergeleri (indirilen/indiriliyor/başarısız)
            elif path == '/api/game-status':
                self._api_game_status()

            # Route: API - Historique (téléchargements terminés ET en queue/cours)
            elif path == '/api/history':
                self._api_history()

            # Route: API - Queue (lecture)
            elif path == '/api/queue':
                self._api_queue_get()

            # Route: API - Settings (lecture)
            elif path == '/api/settings':
                self._api_settings_get()

            # Route: API - System Info (informations système Batocera)
            elif path == '/api/system_info':
                self._api_system_info()

            # Route: API - Update games list (clear cache)
            elif path == '/api/update-cache':
                self._api_update_cache()

            # Route: Images des plateformes
            elif path.startswith('/api/image/'):
                platform_name = path.split('/api/image/')[-1]
                platform_name = urllib.parse.unquote(platform_name)
                self._serve_platform_image(platform_name)

            # Route: Favicon
            elif path == '/api/favicon':
                self._serve_favicon()

            # Route: Browse directories
            elif path == '/api/browse-directories':
                parsed_qs = urllib.parse.parse_qs(parsed_path.query)
                current_path = parsed_qs.get('path', [''])[0]
                self._list_directories(current_path)

            # Route inconnue
            else:
                self._send_json({
                    'success': False,
                    'error': 'Route non trouvée',
                    'path': path
                }, status=404)

        except Exception as e:
            print(f"[ERROR] Exception: {e}", flush=True)  # DEBUG
            logger.error(f"Erreur lors du traitement de {path}: {e}", exc_info=True)
            try:
                self._send_json({
                    'success': False,
                    'error': str(e)
                }, status=500)
            except:
                pass  # Éviter le crash si la réponse échoue

    def do_POST(self):
        """Traite les requêtes POST"""
        parsed_path = urllib.parse.urlparse(self.path)
        path = parsed_path.path

        print(f"[DEBUG] POST Requête: {path}", flush=True)
        logger.info(f"POST {path}")

        try:
            # Lire le corps de la requête
            content_length = int(self.headers.get('Content-Length', 0))
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data.decode('utf-8')) if content_length > 0 else {}

            # Route: Lancer un téléchargement
            if path == '/api/download':
                self._api_download(data)

            # Route: Toplu indirme (Faz 9) — filtrelenmiş listeyi kuyruğa al
            elif path == '/api/download/batch':
                self._api_download_batch(data)

            # Route: Annuler un téléchargement
            elif path == '/api/cancel':
                self._api_cancel(data)

            # Route: Obtenir l'état de la queue
            elif path == '/api/queue':
                self._api_queue_post(data)

            # Route: Vider la queue (sauf le premier élément en cours)
            elif path == '/api/queue/clear':
                self._api_queue_clear()

            # Route: Supprimer un élément de la queue
            elif path == '/api/queue/remove':
                self._api_queue_remove(data)

            # Route: Sauvegarder les settings
            elif path == '/api/settings':
                self._api_settings_post(data)

            # Route: Sauvegarder seulement les filtres (sauvegarde rapide)
            elif path == '/api/save_filters':
                self._api_save_filters(data)

            # Route: Vider l'historique
            elif path == '/api/clear-history':
                self._api_clear_history()

            # Route: Redémarrer l'application
            elif path == '/api/restart':
                self._api_restart()

            # Route: Générer un fichier ZIP de support
            elif path == '/api/support':
                self._api_support()

            # Route inconnue
            else:
                self._send_json({
                    'success': False,
                    'error': 'Route non trouvée',
                    'path': path
                }, status=404)

        except Exception as e:
            print(f"[ERROR] POST Exception: {e}", flush=True)
            logger.error(f"Erreur POST {path}: {e}", exc_info=True)
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)
