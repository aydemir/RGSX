from pathlib import Path
import collections
import io
import shutil
import requests  # type: ignore
import re
import json
import os
import logging
import platform
import subprocess
import urllib.parse
import config
from config import HEADLESS, Game
try:
    if not HEADLESS:
        import pygame  # type: ignore
    else:
        pygame = None  # type: ignore
except Exception:
    pygame = None  # type: ignore
import glob
import threading
from rgsx_settings import load_rgsx_settings, save_rgsx_settings, get_allow_unknown_extensions
import zipfile
import time
import random
import config
from history import save_history
from language import _ 
from datetime import datetime
import sys
import tempfile
try:
    from PIL import Image  # type: ignore
except Exception:
    Image = None  # type: ignore

logger = logging.getLogger("utils")


def load_api_keys(force: bool = False):
    """Charge les clés API (1fichier, AllDebrid, Debrid-Link, RealDebrid, TorBox) en une seule passe.

    - Crée les fichiers vides s'ils n'existent pas
    - Met à jour config.API_KEY_1FICHIER, config.API_KEY_ALLDEBRID, config.API_KEY_DEBRIDLINK, config.API_KEY_REALDEBRID, config.API_KEY_TORBOX
    - Utilise un cache basé sur le mtime pour éviter des relectures
    - force=True ignore le cache et relit systématiquement

    Retourne: { '1fichier': str, 'alldebrid': str, 'debridlink': str, 'realdebrid': str, 'torbox': str, 'reloaded': bool }
    """
    try:
        paths = {
            '1fichier': getattr(config, 'API_KEY_1FICHIER_PATH', ''),
            'alldebrid': getattr(config, 'API_KEY_ALLDEBRID_PATH', ''),
            'debridlink': getattr(config, 'API_KEY_DEBRIDLINK_PATH', ''),
            'realdebrid': getattr(config, 'API_KEY_REALDEBRID_PATH', ''),
            'torbox': getattr(config, 'API_KEY_TORBOX_PATH', ''),
        }
        cache_attr = '_api_keys_cache'
        if not hasattr(config, cache_attr):
            setattr(config, cache_attr, {'1fichier_mtime': None, 'alldebrid_mtime': None, 'debridlink_mtime': None, 'realdebrid_mtime': None, 'torbox_mtime': None})
        cache_data = getattr(config, cache_attr)
        reloaded = False

        for key_name, path in paths.items():
            if not path:
                continue
            # Création fichier vide si absent
            try:
                if not os.path.exists(path):
                    os.makedirs(os.path.dirname(path), exist_ok=True)
                    with open(path, 'w', encoding='utf-8') as f:
                        f.write("")
            except Exception as ce:
                logger.error(f"Impossible de préparer le fichier clé {key_name}: {ce}")
                continue
            try:
                mtime = os.path.getmtime(path)
            except Exception:
                mtime = None
            cache_key = f"{key_name}_mtime"
            if force or (mtime is not None and mtime != cache_data.get(cache_key)):
                # Lecture
                try:
                    with open(path, 'r', encoding='utf-8') as f:
                        value = f.read().strip()
                except Exception as re:
                    logger.error(f"Erreur lecture clé {key_name}: {re}")
                    value = ""
                # Assignation dans config
                if key_name == '1fichier':
                    config.API_KEY_1FICHIER = value
                elif key_name == 'alldebrid':
                    config.API_KEY_ALLDEBRID = value
                elif key_name == 'debridlink':
                    config.API_KEY_DEBRIDLINK = value
                elif key_name == 'realdebrid':
                    config.API_KEY_REALDEBRID = value
                elif key_name == 'torbox':
                    config.API_KEY_TORBOX = value
                cache_data[cache_key] = mtime
                reloaded = True
        return {
            '1fichier': getattr(config, 'API_KEY_1FICHIER', ''),
            'alldebrid': getattr(config, 'API_KEY_ALLDEBRID', ''),
            'debridlink': getattr(config, 'API_KEY_DEBRIDLINK', ''),
            'realdebrid': getattr(config, 'API_KEY_REALDEBRID', ''),
            'torbox': getattr(config, 'API_KEY_TORBOX', ''),
            'reloaded': reloaded
        }
    except Exception as e:
        logger.error(f"Erreur load_api_keys: {e}")
        return {
            '1fichier': getattr(config, 'API_KEY_1FICHIER', ''),
            'alldebrid': getattr(config, 'API_KEY_ALLDEBRID', ''),
            'debridlink': getattr(config, 'API_KEY_DEBRIDLINK', ''),
            'realdebrid': getattr(config, 'API_KEY_REALDEBRID', ''),
            'torbox': getattr(config, 'API_KEY_TORBOX', ''),
            'reloaded': False
        }



def load_archive_org_cookie(force: bool = False) -> str:
    """Charge le cookie Archive.org depuis un fichier texte.

    - Fichier: config.ARCHIVE_ORG_COOKIE_PATH
    - Accepte soit une ligne brute de cookie, soit une ligne "Cookie: ..."
    - Utilise un cache mtime pour éviter les relectures
    """
    try:
        path = getattr(config, 'ARCHIVE_ORG_COOKIE_PATH', '')
        if not path:
            return ""
        cache_attr = '_archive_cookie_cache'
        if not hasattr(config, cache_attr):
            setattr(config, cache_attr, {'mtime': None, 'value': ''})
        cache_data = getattr(config, cache_attr)

        # Créer le fichier vide si absent
        try:
            if not os.path.exists(path):
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, 'w', encoding='utf-8') as f:
                    f.write("")
        except Exception as ce:
            logger.error(f"Impossible de préparer le fichier cookie archive.org: {ce}")
            return ""

        try:
            mtime = os.path.getmtime(path)
        except Exception:
            mtime = None

        if force or (mtime is not None and mtime != cache_data.get('mtime')):
            try:
                with open(path, 'r', encoding='utf-8') as f:
                    value = f.read().strip()
            except Exception as re:
                logger.error(f"Erreur lecture cookie archive.org: {re}")
                value = ""

            if value.lower().startswith("cookie:"):
                value = value.split(":", 1)[1].strip()

            cache_data['mtime'] = mtime
            cache_data['value'] = value

        return cache_data.get('value', '') or ""
    except Exception as e:
        logger.error(f"Erreur load_archive_org_cookie: {e}")
        return ""



def save_api_keys(api_keys: dict):
    """Sauvegarde les clés API (1fichier, AllDebrid, Debrid-Link, RealDebrid, TorBox) dans leurs fichiers respectifs.

    Args:
        api_keys: dict avec les clés '1fichier', 'alldebrid', 'debridlink', 'realdebrid', 'torbox'
    
    Retourne: True si au moins une clé a été sauvegardée avec succès
    """
    if not api_keys:
        return False
    
    paths = {
        '1fichier': getattr(config, 'API_KEY_1FICHIER_PATH', ''),
        'alldebrid': getattr(config, 'API_KEY_ALLDEBRID_PATH', ''),
        'debridlink': getattr(config, 'API_KEY_DEBRIDLINK_PATH', ''),
        'realdebrid': getattr(config, 'API_KEY_REALDEBRID_PATH', ''),
        'torbox': getattr(config, 'API_KEY_TORBOX_PATH', ''),
    }
    
    saved_any = False
    
    for key_name, path in paths.items():
        if not path:
            continue
        
        # Récupérer la valeur (utiliser la clé telle quelle ou en minuscule)
        value = api_keys.get(key_name, api_keys.get(key_name.lower(), None))
        if value is None:
            continue  # Ne pas modifier si la clé n'est pas fournie
        
        try:
            # Créer le dossier si nécessaire
            os.makedirs(os.path.dirname(path), exist_ok=True)
            
            # Écrire la clé (valeur nettoyée)
            with open(path, 'w', encoding='utf-8') as f:
                f.write(value.strip())
            
            # Mettre à jour le cache config
            if key_name == '1fichier':
                config.API_KEY_1FICHIER = value.strip()
            elif key_name == 'alldebrid':
                config.API_KEY_ALLDEBRID = value.strip()
            elif key_name == 'debridlink':
                config.API_KEY_DEBRIDLINK = value.strip()
            elif key_name == 'realdebrid':
                config.API_KEY_REALDEBRID = value.strip()
            elif key_name == 'torbox':
                config.API_KEY_TORBOX = value.strip()
            
            # Invalider le cache mtime
            cache_attr = '_api_keys_cache'
            if hasattr(config, cache_attr):
                cache_data = getattr(config, cache_attr)
                cache_data[f"{key_name}_mtime"] = None
            
            saved_any = True
            logger.info(f"Clé API {key_name} sauvegardée avec succès")
            
        except Exception as e:
            logger.error(f"Erreur sauvegarde clé {key_name}: {e}")
    
    return saved_any



# Wrappers rétro-compatibilité (dépréciés)
def load_api_key_1fichier(force: bool = False):  # pragma: no cover
    return load_api_keys(force).get('1fichier', '')


def load_api_key_alldebrid(force: bool = False):  # pragma: no cover
    return load_api_keys(force).get('alldebrid', '')


def load_api_key_debridlink(force: bool = False):  # pragma: no cover
    return load_api_keys(force).get('debridlink', '')


def load_api_key_realdebrid(force: bool = False):  # pragma: no cover
    return load_api_keys(force).get('realdebrid', '')


def load_api_key_torbox(force: bool = False):  # pragma: no cover
    return load_api_keys(force).get('torbox', '')


# Ancien nom conservé comme alias
def ensure_api_keys_loaded(force: bool = False):  # pragma: no cover
    return load_api_keys(force)


# ------------------------------
# Helpers centralisés pour gestion des fournisseurs de téléchargement
# ------------------------------
def build_provider_paths_string():
    """Retourne une chaîne listant les chemins des fichiers de clés pour affichage/erreurs."""
    return f"{getattr(config, 'API_KEY_1FICHIER_PATH', '')} or {getattr(config, 'API_KEY_ALLDEBRID_PATH', '')} or {getattr(config, 'API_KEY_DEBRIDLINK_PATH', '')} or {getattr(config, 'API_KEY_REALDEBRID_PATH', '')} or {getattr(config, 'API_KEY_TORBOX_PATH', '')}"


def ensure_download_provider_keys(force: bool = False):  # pragma: no cover
    """S'assure que les clés 1fichier/AllDebrid/Debrid-Link/RealDebrid/TorBox sont chargées et retourne le dict.

    Utilise load_api_keys (cache mtime). force=True invalide le cache.
    """
    return load_api_keys(force)


def missing_all_provider_keys():  # pragma: no cover
    """True si aucune des clés premium n'est définie."""
    keys = load_api_keys(False)
    return not keys.get('1fichier') and not keys.get('alldebrid') and not keys.get('debridlink') and not keys.get('realdebrid') and not keys.get('torbox')


def provider_keys_status():  # pragma: no cover
    """Retourne un dict de présence pour debug/log."""
    keys = load_api_keys(False)
    return {
        '1fichier': bool(keys.get('1fichier')),
        'alldebrid': bool(keys.get('alldebrid')),
        'debridlink': bool(keys.get('debridlink')),
        'realdebrid': bool(keys.get('realdebrid')),
        'torbox': bool(keys.get('torbox')),
    }
