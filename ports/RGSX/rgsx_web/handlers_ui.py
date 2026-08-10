# -*- coding: utf-8 -*-
"""RGSXHandler mixin: index HTML, assets statiques, images plateformes, favicon, browse."""
import os
import time
import mimetypes
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from pathlib import Path

import config
from utils import _resolve_platform_image_path

from . import logger


class UIMixin:
    def _asset_version(self, relative_path: str) -> str:
        """Retourne un identifiant de version basé sur la date de modification du fichier statique."""
        static_root = Path(config.APP_FOLDER) / 'static'
        asset_path = static_root / relative_path
        try:
            return str(int(asset_path.stat().st_mtime))
        except OSError:
            return str(int(time.time()))

    def _serve_static_file(self, path: str) -> None:
        """Servez un fichier statique avec gestion du cache HTTP."""
        if not path.startswith('/static/'):
            self._send_not_found()
            return

        relative_path = path[len('/static/'):]
        safe_relative = os.path.normpath(relative_path).replace('\\', '/')

        if safe_relative.startswith('../') or safe_relative.startswith('..') or safe_relative.startswith('/'):
            self._send_not_found()
            return

        static_root = Path(config.APP_FOLDER) / 'static'
        asset_path = static_root / safe_relative

        if not asset_path.is_file():
            self._send_not_found()
            return

        mime_type, _ = mimetypes.guess_type(str(asset_path))
        if not mime_type:
            mime_type = 'application/octet-stream'

        stat_result = asset_path.stat()
        last_modified = datetime.fromtimestamp(stat_result.st_mtime, timezone.utc)
        etag = f'W/"{stat_result.st_mtime_ns}-{stat_result.st_size}"'

        cache_headers = {'Cache-Control': 'public, max-age=86400'}

        client_etag = self.headers.get('If-None-Match')
        if client_etag == etag:
            self._set_headers(mime_type, status=304, etag=etag, last_modified=last_modified, extra_headers=cache_headers)
            return

        client_ims = self.headers.get('If-Modified-Since')
        if client_ims:
            try:
                client_dt = parsedate_to_datetime(client_ims)
                if client_dt.tzinfo is None:
                    client_dt = client_dt.replace(tzinfo=timezone.utc)
                if client_dt >= last_modified:
                    self._set_headers(mime_type, status=304, etag=etag, last_modified=last_modified, extra_headers=cache_headers)
                    return
            except (TypeError, ValueError):
                pass

        data = asset_path.read_bytes()
        payload_headers = {
            'Cache-Control': 'public, max-age=86400',
            'Content-Length': str(len(data)),
        }
        self._set_headers(mime_type, status=200, etag=etag, last_modified=last_modified, extra_headers=payload_headers)
        self.wfile.write(data)

    def _serve_platform_image(self, platform_name):
        """Sert l'image d'une plateforme en utilisant le mapping de systems_list.json"""
        try:
            # Trouver la plateforme dans platform_dicts pour obtenir le platform_image
            platform_dict = None
            for pd in config.platform_dicts:
                if pd.get('platform_name') == platform_name:
                    platform_dict = pd
                    break

            payload_platform = platform_dict or {'platform_name': platform_name}
            image_path = _resolve_platform_image_path(payload_platform)

            if image_path and os.path.exists(image_path):
                ext = os.path.splitext(image_path)[1].lower()
                mime_types = {
                    '.png': 'image/png',
                    '.jpg': 'image/jpeg',
                    '.jpeg': 'image/jpeg',
                    '.gif': 'image/gif',
                    '.webp': 'image/webp',
                }
                content_type = mime_types.get(ext, 'image/png')
                with open(image_path, 'rb') as f:
                    image_data = f.read()

                # Cache navigateur (1 heure) : la grille de plateformes n'est pas
                # re-téléchargée à chaque re-rendu. Le client ajoute une version par
                # session (?v=...) pour rafraîchir après un update-cache.
                self.send_response(200)
                self.send_header('Content-type', content_type)
                self.send_header('Cache-Control', 'public, max-age=3600')
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                self.wfile.write(image_data)
            else:
                # Image par défaut (pixel transparent)
                logger.warning(f"Aucune image trouvée pour {platform_name}, envoi PNG transparent")
                self.send_response(404)
                self.send_header('Content-type', 'image/png')
                self.send_header('Cache-Control', 'no-store, no-cache, must-revalidate, max-age=0')
                self.send_header('Pragma', 'no-cache')
                self.send_header('Expires', '0')
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                # PNG transparent 1x1 pixel
                transparent_png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82'
                self.wfile.write(transparent_png)

        except Exception as e:
            logger.error(f"Erreur lors du chargement de l'image {platform_name}: {e}", exc_info=True)
            self.send_response(500)
            self.send_header('Content-type', 'image/png')
            self.send_header('Cache-Control', 'no-store, no-cache, must-revalidate, max-age=0')
            self.send_header('Pragma', 'no-cache')
            self.send_header('Expires', '0')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            # PNG transparent en cas d'erreur
            transparent_png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82'
            self.wfile.write(transparent_png)

    def _serve_favicon(self):
        """Sert le favicon de l'application"""
        try:
            favicon_path = os.path.join(config.APP_FOLDER, 'assets', 'images', 'favicon_rgsx.ico')

            if os.path.exists(favicon_path):
                with open(favicon_path, 'rb') as f:
                    favicon_data = f.read()

                self.send_response(200)
                self.send_header('Content-type', 'image/x-icon')
                self.send_header('Cache-Control', 'public, max-age=86400')  # Cache 24h
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                self.wfile.write(favicon_data)
            else:
                logger.warning(f"Favicon non trouvé: {favicon_path}")
                self.send_response(404)
                self.end_headers()
        except Exception as e:
            logger.error(f"Erreur lors du chargement du favicon: {e}", exc_info=True)
            self.send_response(500)
            self.end_headers()

    def _list_directories(self, path: str):
        """Liste les répertoires pour le navigateur de fichiers"""
        try:
            # Si le chemin est vide, lister les lecteurs sur Windows ou / sur Linux
            if not path:
                if os.name == 'nt':
                    # Windows: lister les lecteurs
                    import string
                    drives = []
                    for letter in string.ascii_uppercase:
                        drive = f"{letter}:\\"
                        if os.path.exists(drive):
                            drives.append({
                                'name': drive,
                                'path': drive,
                                'is_drive': True
                            })
                    self._send_json({
                        'success': True,
                        'current_path': '',
                        'parent_path': None,
                        'directories': drives
                    })
                    return  # Important: arrêter ici pour Windows
                else:
                    # Linux/Mac: partir de la racine
                    path = '/'

            # Vérifier que le chemin existe
            if not os.path.isdir(path):
                self._send_json({
                    'success': False,
                    'error': 'Le chemin spécifié n\'existe pas'
                }, status=400)
                return

            # Lister les sous-répertoires
            directories = []
            try:
                for entry in os.listdir(path):
                    entry_path = os.path.join(path, entry)
                    if os.path.isdir(entry_path):
                        directories.append({
                            'name': entry,
                            'path': entry_path,
                            'is_drive': False
                        })
            except PermissionError:
                logger.warning(f"Accès refusé au répertoire: {path}")

            # Trier par nom
            directories.sort(key=lambda x: x['name'].lower())

            # Déterminer le chemin parent
            parent_path = None
            if path and path != '/':
                if os.name == 'nt':
                    # Sur Windows, si on est à la racine d'un lecteur (C:\), parent = ''
                    if len(path) == 3 and path[1] == ':' and path[2] == '\\':
                        parent_path = ''
                    else:
                        parent_path = os.path.dirname(path)
                else:
                    parent_path = os.path.dirname(path)
                    if not parent_path:
                        parent_path = '/'

            self._send_json({
                'success': True,
                'current_path': path,
                'parent_path': parent_path,
                'directories': directories
            })

        except Exception as e:
            logger.error(f"Erreur lors du listage des répertoires: {e}", exc_info=True)
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _get_index_html(self):
        """Retourne la page HTML d'accueil"""
        css_version = self._asset_version('css/app.css')
        js_version = self._asset_version('js/app.js')
        html = """
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="theme-color" content="#667eea">
    <meta name="color-scheme" content="light dark">
    <title>🎮 RGSX Web Interface</title>
    <link rel="icon" type="image/x-icon" href="/api/favicon">
    <link rel="stylesheet" href="/static/css/theme.css?v=__CSS_VERSION__">
    <link rel="stylesheet" href="/static/css/app.css?v=__CSS_VERSION__">
    <link rel="stylesheet" href="/static/css/accessibility.css?v=__CSS_VERSION__">
</head>
<body>
    <!-- Skip to main content link for keyboard users -->
    <a href="#main-content" class="skip-to-main">Skip to main content</a>

    <!-- Live region for screen reader announcements -->
    <div id="sr-announcements" role="status" aria-live="polite" aria-atomic="true" class="sr-only"></div>

    <div class="container" role="application" aria-label="RGSX Game Manager">
        <header role="banner">
            <h1 data-translate="web_title">RGSX Web Interface</h1>
            <p style="font-size: 0.85em; opacity: 0.8; margin-top: 5px;">v{version}</p>


            <!-- Navigation mobile avec icônes uniquement -->
            <nav class="mobile-tabs" role="navigation" aria-label="Main navigation (mobile)">
                <button class="mobile-tab active" data-tab="platforms" onclick="showTab('platforms')" data-translate-title="web_tooltip_platforms" title="Platforms list" aria-current="page">🎮</button>
                <button class="mobile-tab" data-tab="downloads" onclick="showTab('downloads')" data-translate-title="web_tooltip_downloads" title="Downloads">⬇️</button>
                <button class="mobile-tab" data-tab="queue" onclick="showTab('queue')" data-translate-title="web_tooltip_queue" title="Queue">📋</button>
                <button class="mobile-tab" data-tab="history" onclick="showTab('history')" data-translate-title="web_tooltip_history" title="History">📜</button>
                <button class="mobile-tab" data-tab="settings" onclick="showTab('settings')" data-translate-title="web_tooltip_settings" title="Settings">⚙️</button>
                <button class="mobile-tab" onclick="openQbittorrentWebUi()" title="Open qBittorrent WebUI">🌐</button>
                <button class="mobile-tab" onclick="updateGamesList()" data-translate-title="web_tooltip_update" title="Update games list">🔄</button>
                <button class="mobile-tab" onclick="generateSupportZip()" data-translate-title="web_support" title="Support">🆘</button>
            </nav>
        </header>

        <nav class="tabs" role="navigation" aria-label="Main navigation (desktop)">
            <button class="tab active" data-tab="platforms" onclick="showTab('platforms')" aria-current="page">🎮 <span data-translate="web_tab_platforms">Platforms List</span></button>
            <button class="tab" data-tab="downloads" onclick="showTab('downloads')">⬇️ <span data-translate="web_tab_downloads">Downloads</span></button>
            <button class="tab" data-tab="queue" onclick="showTab('queue')">📋 <span data-translate="web_tab_queue">Queue</span></button>
            <button class="tab" data-tab="history" onclick="showTab('history')">📜 <span data-translate="web_tab_history">History</span></button>
            <button class="tab" data-tab="settings" onclick="showTab('settings')">⚙️ <span data-translate="web_tab_settings">Settings</span></button>
            <button class="tab" onclick="openQbittorrentWebUi()">🌐 <span>qBittorrent</span></button>
            <button class="tab" onclick="updateGamesList()">🔄 <span data-translate="web_tab_update">Update games list</span></button>
            <button class="tab" onclick="generateSupportZip()">🆘 <span data-translate="web_support">Support</span></button>
        </nav>

        <main class="content" id="main-content" role="main">
            <div id="platforms-content" role="region" aria-label="Platforms section"></div>
            <div id="downloads-content" style="display:none;" role="region" aria-label="Downloads section"></div>
            <div id="queue-content" style="display:none;" role="region" aria-label="Queue section"></div>
            <div id="history-content" style="display:none;" role="region" aria-label="History section"></div>
            <div id="settings-content" style="display:none;" role="region" aria-label="Settings section"></div>
        </main>
    </div>

    <!-- qBittorrent Password Modal -->
    <div id="qb-password-modal" style="display: none; position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.5); z-index: 10000; justify-content: center; align-items: center;">
        <div style="background: white; padding: 20px; border-radius: 8px; max-width: 440px; width: 90%; box-shadow: 0 4px 20px rgba(0,0,0,0.3);">
            <h3 data-translate="web_qbt_password_title" style="margin-top: 0;">🔑 qBittorrent WebUI Password</h3>
            <p data-translate="web_qbt_password_desc" style="color: #666; font-size: 0.9em; margin-bottom: 15px;">
                Set a new password (at least 6 characters). It applies to both qBittorrent WebUI and RGSX settings.
            </p>
            <label data-translate="web_qbt_password_new" style="display: block; margin-bottom: 5px; font-weight: bold;">New Password</label>
            <input type="password" id="qb-new-password" autocomplete="new-password"
                   style="width: 100%; box-sizing: border-box; padding: 10px; margin-bottom: 12px; border: 1px solid #ccc; border-radius: 5px; font-size: 1em;">
            <label data-translate="web_qbt_password_confirm" style="display: block; margin-bottom: 5px; font-weight: bold;">Password (Repeat)</label>
            <input type="password" id="qb-new-password-2" autocomplete="new-password"
                   style="width: 100%; box-sizing: border-box; padding: 10px; margin-bottom: 15px; border: 1px solid #ccc; border-radius: 5px; font-size: 1em;">
            <div style="display: flex; gap: 10px; justify-content: flex-end;">
                <button data-translate="web_qbt_password_cancel" onclick="closeQbittorrentPasswordModal()" style="background: #6c757d; color: #fff; border: none; padding: 10px 18px; border-radius: 5px; cursor: pointer;">Cancel</button>
                <button data-translate="web_qbt_password_save" onclick="saveQbittorrentPassword()" style="background: #e0a800; color: #fff; border: none; padding: 10px 18px; border-radius: 5px; font-weight: bold; cursor: pointer;">Save</button>
            </div>
            <div id="qb-password-error" style="color: #dc3545; margin-top: 12px; font-size: 0.9em; display: none;"></div>
        </div>
    </div>

    <script src="/static/js/accessibility.js?v=__JS_VERSION__" defer></script>
    <script src="/static/js/app.js?v=__JS_VERSION__" defer></script>

    <!-- Region Priority Configuration Modal -->
    <div id="region-priority-modal" style="display: none; position: fixed; top: 0; left: 0; width: 100%; height: 100%; background: rgba(0,0,0,0.5); z-index: 10000; justify-content: center; align-items: center;">
        <div style="background: white; padding: 20px; border-radius: 8px; max-width: 500px; width: 90%; max-height: 80vh; overflow-y: auto;">
            <h3 style="margin-top: 0;">Region Priority Configuration</h3>
            <p style="color: #666; font-size: 0.9em; margin-bottom: 15px;">
                Configure the priority order for "One ROM Per Game" filter.
                Higher priority regions will be selected first when multiple versions exist.
            </p>
            <div id="region-priority-config"></div>
        </div>
    </div>
</body>
</html>
        """
        return (html
                .replace('__CSS_VERSION__', css_version)
                .replace('__JS_VERSION__', js_version)
                .replace('{version}', config.app_version))
