# -*- coding: utf-8 -*-
"""RGSXHandler mixin: routes plateformes / recherche / jeux / traductions."""
import os
import urllib.parse

import config
from .cache import get_cached_sources, get_cached_games, generate_etag
from .i18n import TRANSLATIONS, load_translations, normalize_size
from history import is_game_downloaded, scan_platform_roms_on_enter
from rgsx_settings import get_language

from . import logger


class GamesMixin:
    def _api_platforms(self):
        platforms, _, source_last_modified = get_cached_sources()
        # Ajouter le nombre de jeux depuis config.games_count
        games_count_dict = getattr(config, 'games_count', {})

        # Filtrer les plateformes cachées selon config.filter_platforms_selection
        hidden_platforms = set()
        if hasattr(config, 'filter_platforms_selection') and config.filter_platforms_selection:
            hidden_platforms = {name for name, is_hidden in config.filter_platforms_selection if is_hidden}

        # Ajouter aussi les plateformes sans dossier ROM (si show_unsupported_platforms = False)
        from rgsx_settings import load_rgsx_settings, get_show_unsupported_platforms
        settings = load_rgsx_settings()
        show_unsupported = get_show_unsupported_platforms(settings)

        if not show_unsupported:
            # Masquer les plateformes dont le dossier ROM n'existe pas
            for platform in platforms:
                platform_name = platform.get('platform_name', '')
                folder = platform.get('folder', '')
                # Garder BIOS même sans dossier
                if platform_name and folder and platform_name not in ["- BIOS by TMCTV -", "- BIOS"]:
                    expected_dir = os.path.join(config.ROMS_FOLDER, folder)
                    if not os.path.isdir(expected_dir):
                        hidden_platforms.add(platform_name)

        filtered_platforms = []
        for platform in platforms:
            platform_name = platform.get('platform_name', '')
            if platform_name in hidden_platforms:
                continue
            platform_copy = dict(platform)
            platform_copy['games_count'] = games_count_dict.get(platform_name, 0)
            filtered_platforms.append(platform_copy)

        response_payload = {
            'success': True,
            'count': len(filtered_platforms),
            'platforms': filtered_platforms
        }
        response_etag = generate_etag(response_payload)

        self._send_json(response_payload, etag=response_etag, last_modified=source_last_modified)

    def _api_search(self, parsed_path):
        try:
            query_params = urllib.parse.parse_qs(parsed_path.query)
            search_term = query_params.get('q', [''])[0].lower().strip()
            search_words = [w for w in search_term.split() if w]

            if not search_term:
                self._send_json({
                    'success': True,
                    'search_term': '',
                    'results': {'platforms': [], 'games': []}
                })
                return

            # Charger toutes les plateformes (avec cache)
            platforms, _, source_last_modified = get_cached_sources()
            games_count_dict = getattr(config, 'games_count', {})

            # Filtrer les plateformes cachées selon config.filter_platforms_selection
            hidden_platforms = set()
            if hasattr(config, 'filter_platforms_selection') and config.filter_platforms_selection:
                hidden_platforms = {name for name, is_hidden in config.filter_platforms_selection if is_hidden}

            # Ajouter aussi les plateformes sans dossier ROM (si show_unsupported_platforms = False)
            from rgsx_settings import load_rgsx_settings, get_show_unsupported_platforms
            settings = load_rgsx_settings()
            show_unsupported = get_show_unsupported_platforms(settings)

            if not show_unsupported:
                # Masquer les plateformes dont le dossier ROM n'existe pas
                for platform in platforms:
                    platform_name = platform.get('platform_name', '')
                    folder = platform.get('folder', '')
                    # Garder BIOS même sans dossier
                    if platform_name and folder and platform_name not in ["- BIOS by TMCTV -", "- BIOS"]:
                        expected_dir = os.path.join(config.ROMS_FOLDER, folder)
                        if not os.path.isdir(expected_dir):
                            hidden_platforms.add(platform_name)

            matching_platforms = []
            matching_games = []
            latest_modified = source_last_modified

            # Rechercher dans les plateformes et leurs jeux
            for platform in platforms:
                platform_name = platform.get('platform_name', '')

                # Exclure les plateformes cachées
                if platform_name in hidden_platforms:
                    continue

                platform_name_lower = platform_name.lower()

                # Vérifier si le système correspond
                platform_matches = search_term in platform_name_lower

                if platform_matches:
                    matching_platforms.append({
                        'platform_name': platform_name,
                        'folder': platform.get('folder', ''),
                        'platform_image': platform.get('platform_image', ''),
                        'games_count': games_count_dict.get(platform_name, 0)
                    })

                # Rechercher dans les jeux de cette plateforme
                try:
                    games, _, games_last_modified = get_cached_games(platform_name)
                    if games_last_modified and latest_modified:
                        latest_modified = max(latest_modified, games_last_modified)
                    elif games_last_modified:
                        latest_modified = games_last_modified
                    for game in games:
                        game_name = game.name
                        game_name_lower = game_name.lower()
                        if all(word in game_name_lower for word in search_words):
                            matching_games.append({
                                'game_name': game_name,
                                'platform': platform_name,
                                'url': game.url,
                                'size': normalize_size(game.size, self._get_language_from_cookies()),
                                'downloaded': is_game_downloaded(platform_name, game_name)
                            })
                except Exception as e:
                    logger.debug(f"Erreur lors de la recherche dans {platform_name}: {e}")
                    continue

            response_payload = {
                'success': True,
                'search_term': search_term,
                'results': {
                    'platforms': matching_platforms,
                    'games': matching_games
                }
            }
            response_etag = generate_etag(response_payload)

            self._send_json(response_payload, etag=response_etag, last_modified=latest_modified)

        except Exception as e:
            logger.error(f"Erreur lors de la recherche: {e}")
            self._send_json({
                'success': False,
                'error': str(e)
            }, status=500)

    def _api_translations(self):
        # Ajouter le code de langue dans les traductions pour que JS puisse l'utiliser.
        # Dil dosyalari manager acikken guncellenebildigi icin TRANSLATIONS'u her
        # istekte diskten tazele (dil degisikligi / yeni anahtar restart istemez).
        try:
            fresh = load_translations()
        except Exception:
            fresh = TRANSLATIONS
        translations_with_lang = dict(fresh)
        translations_with_lang['_language'] = get_language()
        self._send_json({
            'success': True,
            'language': get_language(),
            'translations': translations_with_lang
        })

    def _api_games(self, path):
        platform_name = path.split('/api/games/')[-1]
        platform_name = urllib.parse.unquote(platform_name)

        # Récupérer la langue depuis les cookies ou utiliser 'en' par défaut
        lang = self._get_language_from_cookies()

        scan_platform_roms_on_enter(platform_name)

        games, _, games_last_modified = get_cached_games(platform_name)
        games_formatted = [
            {
                'name': g.name,
                'url': g.url,
                'size': normalize_size(g.size, lang),
                'downloaded': is_game_downloaded(platform_name, g.name)
            }
            for g in games
        ]

        response_payload = {
            'success': True,
            'platform': platform_name,
            'count': len(games_formatted),
            'games': games_formatted
        }
        response_etag = generate_etag(response_payload)

        self._send_json(response_payload, etag=response_etag, last_modified=games_last_modified)
