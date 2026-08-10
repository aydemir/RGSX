#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""RGSX Web Server - package principal.

rgsx_web est un package: __init__.py met en place le logging, charge les
données initiales, puis ré-exporte RGSXHandler et les fonctions de cache/i18n
pour la compatibilité avec rgsx_manager.py.

Modules extraits:
  - cache.py          : cache sources/jeux (ETag + Last-Modified) + watchdog
  - i18n.py           : traductions (TRANSLATIONS) + normalisation des tailles
  - handlers.py       : RGSXHandler (dispatch GET/POST + réponses communes)
  - handlers_ui.py    : UIMixin (index HTML, assets, images, favicon, browse)
  - handlers_games.py : GamesMixin (plateformes, recherche, jeux, traductions)
  - handlers_download.py : DownloadMixin (download, cancel, queue, progress)
  - handlers_settings.py : SettingsMixin (settings, system info, update-cache, support)
"""
import copy
import json
import logging
import os
import sys
import time
import urllib.request

import config
from history import load_downloaded_games
from utils import load_sources

# Ajouter le dossier parent au path pour imports
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# =========================================================================
# Configuration logging - Enregistrer dans rgsx_web.log
# =========================================================================
os.makedirs(config.log_dir, exist_ok=True)


# Supprimer les handlers existants pour éviter les doublons
for handler in logging.root.handlers[:]:
    logging.root.removeHandler(handler)

# IMPORTANT: Forcer le flush après chaque log
class FlushFileHandler(logging.FileHandler):
    def emit(self, record):
        super().emit(record)
        self.flush()

# Handler principal : rgsx_web.log avec rotation (20 MB max, 2 backups)
# pour éviter une croissance illimitée (mode 'a' ne suffit pas en daemon longue durée).
try:
    from logging.handlers import RotatingFileHandler as _RotatingFileHandler

    class _FlushRotatingFileHandler(_RotatingFileHandler):
        def emit(self, record):
            super().emit(record)
            self.flush()

    file_handler = _FlushRotatingFileHandler(
        config.log_file_web,
        maxBytes=20 * 1024 * 1024,  # 20 MB
        backupCount=2,
        encoding='utf-8',
    )
except Exception as e:
    logging.warning(f"RotatingFileHandler indisponible, repli sur FileHandler: {e}")
    file_handler = FlushFileHandler(config.log_file_web, mode='a', encoding='utf-8')
file_handler.setLevel(logging.DEBUG)
file_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))

# Handler crash : ne retient que les erreurs/critiques (diagnostic ciblé, petit volume)
try:
    from logging.handlers import RotatingFileHandler as _CrashRotatingFileHandler

    class _FlushCrashRotatingFileHandler(_CrashRotatingFileHandler):
        def emit(self, record):
            super().emit(record)
            self.flush()

    crash_handler = _FlushCrashRotatingFileHandler(
        config.log_file_crash,
        maxBytes=5 * 1024 * 1024,  # 5 MB
        backupCount=1,
        encoding='utf-8',
    )
except Exception as e:
    logging.warning(f"Crash log RotatingFileHandler indisponible, repli sur FileHandler: {e}")
    crash_handler = FlushFileHandler(config.log_file_crash, mode='a', encoding='utf-8')
crash_handler.setLevel(logging.ERROR)
crash_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))

# Créer le handler console
console_handler = logging.StreamHandler(sys.stdout)
console_handler.setLevel(logging.INFO)
console_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))

# Configurer le logger racine
logging.root.setLevel(logging.DEBUG)
logging.root.addHandler(file_handler)
logging.root.addHandler(crash_handler)
logging.root.addHandler(console_handler)

logger = logging.getLogger(__name__)

logger.info("=" * 60)
logger.info("RGSX Web Server - Démarrage du logging")
logger.info(f"Fichier de log: {config.log_file_web}")
logger.info(f"Répertoire de log: {config.log_dir}")
logger.info(f"Python version: {sys.version}")
logger.info(f"Plateforme: {sys.platform}")
logger.info(f"Répertoire de travail: {os.getcwd()}")
logger.info(f"Script: {__file__}")
logger.info("=" * 60)

# Force flush pour être sûr que ces logs sont écrits
for handler in logging.root.handlers:
    handler.flush()

# Test d'écriture pour vérifier que le fichier fonctionne
try:
    with open(config.log_file_web, 'a', encoding='utf-8') as test_file:
        test_file.write(f"\n{'='*60}\n")
        test_file.write(f"Test d'écriture directe - {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
        test_file.write(f"{'='*60}\n")
        test_file.flush()
    logger.info("Test d'écriture dans le fichier de log réussi")
except Exception as e:
    logger.error(f"Erreur lors du test d'écriture : {e}")
    print(f"ERREUR: Impossible d'ecrire dans {config.log_file_web}: {e}", file=sys.stderr)

# =========================================================================
# Import du cache + traductions (ré-export pour compatibilité)
# =========================================================================
from .cache import (
    CACHE_TTL_SECONDS,
    cache_lock,
    source_cache,
    games_cache,
    watchdog_observer,
    watchdog_started,
    _now_utc,
    _httpdate,
    generate_etag,
    _ensure_datetime,
    invalidate_all_caches,
    invalidate_games_cache,
    get_cached_sources,
    get_cached_games,
    start_cache_invalidation_watchdog,
)
from .i18n import load_translations, TRANSLATIONS, get_translation, normalize_size

# =========================================================================
# Initialiser les données au démarrage
# =========================================================================
logger.info("Chargement initial des données...")
try:
    initial_sources = load_sources()  # Initialise config.games_count
    logger.info(f"{len(getattr(config, 'platforms', []))} plateformes chargées")

    # Charger les jeux déjà téléchargés (pour les indicateurs de statut)
    config.downloaded_games = load_downloaded_games()
    logger.info(f"Jeux téléchargés chargés: {len(config.downloaded_games)} plateformes")

    # Initialiser filter_platforms_selection depuis les settings (pour filtrer les plateformes)
    from rgsx_settings import load_rgsx_settings
    settings = load_rgsx_settings()
    hidden = set(settings.get("hidden_platforms", [])) if isinstance(settings, dict) else set()

    if initial_sources is not None:
        with cache_lock:
            source_cache.update({
                'data': copy.deepcopy(initial_sources),
                'timestamp': time.time(),
                'etag': generate_etag(initial_sources),
                'last_modified': _now_utc(),
            })

    if not hasattr(config, 'filter_platforms_selection') or not config.filter_platforms_selection:
        all_platform_names = []
        for platform_entry in getattr(config, 'platforms', []):
            if isinstance(platform_entry, str):
                name = platform_entry
            elif isinstance(platform_entry, dict):
                name = platform_entry.get("platform_name", "")
            else:
                name = str(platform_entry)
            name = name.strip()
            if name:
                all_platform_names.append(name)
        all_platform_names = sorted(set(all_platform_names))
        config.filter_platforms_selection = [(name, name in hidden) for name in all_platform_names]
        logger.info(f"Filter platforms initialisé: {len(hidden)} plateformes cachées sur {len(all_platform_names)}")

    # Force flush
    for handler in logging.root.handlers:
        handler.flush()
except Exception as e:
    logger.error(f"Erreur lors du chargement initial: {e}")
    # Force flush
    for handler in logging.root.handlers:
        handler.flush()

# Lancer le watcher de cache si disponible
try:
    start_cache_invalidation_watchdog()
except Exception as watcher_error:  # pragma: no cover - watcher errors shouldn't crash server
    logger.warning(f"Cache watchdog startup failed: {watcher_error}")

# =========================================================================
# Handler HTTP + serveur (import après la définition du logger : `from . import logger`)
# =========================================================================
from .handlers import RGSXHandler
from .server import run_server, CURRENT_HTTPD


if __name__ == '__main__':
    # =========================================================================
    # SHIM: rgsx_web.py est un point d'entrée de compatibilité.
    # Tout le travail réel est fait par le daemon rgsx_manager.py qui héberge
    # le serveur HTTP + la queue de téléchargement + le tray. Ce shim garantit
    # qu'un manager est actif, puis quitte (le manager sert le port 5000).
    # =========================================================================
    print("="*60, flush=True)
    print("RGSX Web (shim) - verification du manager RGSX...", flush=True)
    print("="*60, flush=True)

    import argparse

    parser = argparse.ArgumentParser(description='RGSX Web Server (shim)')
    parser.add_argument('--host', default='0.0.0.0', help='Adresse IP (défaut: 0.0.0.0)')
    parser.add_argument('--port', type=int, default=5000, help='Port (défaut: 5000)')
    args = parser.parse_args()

    import subprocess
    port = args.port

    def _healthy(timeout=2.0):
        try:
            with urllib.request.urlopen(f'http://127.0.0.1:{port}/api/health', timeout=timeout) as resp:
                if resp.status != 200:
                    return False
                data = json.loads(resp.read().decode('utf-8'))
                return bool(data.get('success') and data.get('manager'))
        except Exception:
            return False

    if _healthy():
        print(f"RGSX Manager déjà actif sur http://localhost:{port}", flush=True)
        sys.exit(0)

    manager_script = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'rgsx_manager.py')
    if not os.path.exists(manager_script):
        print(f"ERREUR: {manager_script} introuvable. Le manager RGSX est requis.", file=sys.stderr, flush=True)
        sys.exit(1)

    exe = sys.executable or 'python'
    try:
        if os.name == 'nt':
            CREATE_NO_WINDOW = 0x08000000
            proc = subprocess.Popen(
                [exe, manager_script, f'--port={port}', '--minimized'],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                cwd=os.path.dirname(os.path.abspath(__file__)),
                creationflags=CREATE_NO_WINDOW,
            )
        else:
            proc = subprocess.Popen(
                [exe, manager_script, f'--port={port}', '--minimized'],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                cwd=os.path.dirname(os.path.abspath(__file__)),
            )
    except Exception as e:
        print(f"ERREUR: Impossible de lancer le manager RGSX: {e}", file=sys.stderr, flush=True)
        sys.exit(1)

    print("Manager RGSX lancé, attente du serveur HTTP...", flush=True)
    deadline = time.time() + 30
    while time.time() < deadline:
        time.sleep(0.5)
        if _healthy():
            print(f"RGSX Manager actif sur http://localhost:{port}", flush=True)
            sys.exit(0)
        if proc.poll() is not None:
            print(f"ERREUR: Le manager RGSX s'est arrêté (code {proc.returncode})", file=sys.stderr, flush=True)
            sys.exit(1)

    print(f"ERREUR: Le manager RGSX n'a pas démarré dans le délai imparti", file=sys.stderr, flush=True)
    sys.exit(1)
