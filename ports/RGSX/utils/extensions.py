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

from utils.files import _get_dest_folder_name

from utils.text import sanitize_filename




_extensions_cache = None  # type: ignore

_extensions_json_regenerated = False



# Fonction pour charger le fichier JSON des extensions supportées
def load_extensions_json():
    """Charge le JSON des extensions supportées.
    - Régénère une seule fois par exécution (au premier appel ou si le fichier est absent).
    - Met en cache le résultat pour éviter les relectures et logs répétés.
    """
    global _extensions_cache, _extensions_json_regenerated
    try:
        # Retour immédiat si déjà en cache
        if _extensions_cache is not None:
            return _extensions_cache

        os.makedirs(os.path.dirname(config.JSON_EXTENSIONS), exist_ok=True)

        # Régénération unique au premier appel (ou si le fichier est manquant)
        if not _extensions_json_regenerated or not os.path.exists(config.JSON_EXTENSIONS):
            try:
                generated = generate_extensions_json_from_es_systems()
                if generated:
                    with open(config.JSON_EXTENSIONS, 'w', encoding='utf-8') as wf:
                        json.dump(generated, wf, ensure_ascii=False, indent=2)
                    logger.info(f"rom_extensions régénéré ({len(generated)} systèmes): {config.JSON_EXTENSIONS}")
                else:
                    logger.warning("Aucune donnée générée depuis es_systems.cfg; on conserve l'existant si présent")
                _extensions_json_regenerated = True
            except Exception as ge:
                logger.error(f"Échec lors de la régénération de {config.JSON_EXTENSIONS} depuis es_systems.cfg: {ge}")

        # Lecture du fichier (nouveau ou existant)
        if os.path.exists(config.JSON_EXTENSIONS):
            with open(config.JSON_EXTENSIONS, 'r', encoding='utf-8') as f:
                _extensions_cache = json.load(f)
                return _extensions_cache
        _extensions_cache = []
        return _extensions_cache
    except Exception as e:
        logger.error(f"Erreur lors de la lecture de {config.JSON_EXTENSIONS}: {e}")
        _extensions_cache = []
        return _extensions_cache


def _detect_es_systems_cfg_paths():
    """Retourne une liste de chemins possibles pour es_systems.cfg selon l'OS.
    - RetroBat (Windows): {config.USERDATA_FOLDER}\\system\\templates\\emulationstation\\es_systems.cfg
    - Batocera (Linux): /usr/share/emulationstation/es_systems.cfg
      Ajoute aussi les fichiers customs: /userdata/system/configs/emulationstation/es_systems_*.cfg
    """
    candidates = []
    try:
        if config.OPERATING_SYSTEM == 'Windows':
            base = getattr(config, 'USERDATA_FOLDER', None)
            if base:
                candidates.append(os.path.join(base, 'system', 'templates', 'emulationstation', 'es_systems.cfg'))
        else:
            # Batocera / Linux classiques
            candidates.append('/usr/share/emulationstation/es_systems.cfg')
            candidates.append('/etc/emulationstation/es_systems.cfg')
            # Batocera customs
            custom_dir = '/userdata/system/configs/emulationstation'
            try:
                for p in glob.glob(os.path.join(custom_dir, 'es_systems_*.cfg')):
                    candidates.append(p)
                direct_cfg = os.path.join(custom_dir, 'es_systems.cfg')
                if os.path.exists(direct_cfg):
                    candidates.append(direct_cfg)
            except Exception:
                pass
    except Exception:
        pass
    existing = [p for p in candidates if p and os.path.exists(p)]
    # Logs réduits: on ne conserve que les résumés plus loin
    return existing


def _parse_es_systems_cfg(cfg_path):
    """Parse un es_systems.cfg minimalement pour extraire (folder, extensions).
    Retourne une liste de dicts: { 'folder': <str>, 'extensions': [..] }
    - folder: dérivé de la balise <path> en prenant la partie après 'roms/' (ou '\\roms\\' sous Windows)
    - extensions: liste normalisée de .ext (point + minuscule)
    """
    try:
        # Lire tel quel (pas besoin d'un parseur XML strict, mais ElementTree suffit)
        import xml.etree.ElementTree as ET
    # Log détaillé supprimé pour alléger les traces
        tree = ET.parse(cfg_path)
        root = tree.getroot()
        out = []
        for sys_elem in root.findall('system'):
            path_text = (sys_elem.findtext('path') or '').strip()
            ext_text = (sys_elem.findtext('extension') or '').strip()
            if not path_text:
                continue
            # Extraire le dossier après 'roms'
            folder = None
            norm = path_text.replace('\\', '/').lower()
            marker = '/roms/'
            if marker in norm:
                after = norm.split(marker, 1)[1]
                folder = after.strip().strip('/\\')
            if not folder:
                # fallback: si le chemin finit par .../roms/<folder>
                parts = norm.strip('/').split('/')
                if len(parts) >= 2 and parts[-2] == 'roms':
                    folder = parts[-1]
            if not folder:
                continue

            # Extensions: split par espaces, normaliser en .ext
            exts = []
            for tok in ext_text.split():
                tok = tok.strip().lower()
                if not tok:
                    continue
                if not tok.startswith('.'):
                    # Certaines entrées peuvent omettre le point
                    tok = '.' + tok
                exts.append(tok)
            # Dédupliquer tout en conservant l'ordre
            seen = set()
            norm_exts = []
            for e in exts:
                if e not in seen:
                    seen.add(e)
                    norm_exts.append(e)
            out.append({'folder': folder, 'extensions': norm_exts})
    # Résumé final affiché ailleurs
        return out
    except Exception as e:
        logger.error(f"Erreur parsing es_systems.cfg ({cfg_path}): {e}")
        return []


def generate_extensions_json_from_es_systems():
    """Essaie de construire la liste des extensions à partir des es_systems.cfg disponibles.
    Priorité: RetroBat si présent, sinon Batocera. Fusionne si plusieurs trouvés, en préférant RetroBat.
    """
    combined = {}
    paths = _detect_es_systems_cfg_paths()
    if not paths:
        logger.warning("Aucun chemin es_systems.cfg détecté (RetroBat/Batocera)")
        return []
    # Prioriser RetroBat en tête si présent
    def score(p):
        return 0 if 'templates' in p.replace('\\', '/').lower() else 1
    for cfg in sorted(paths, key=score):
        if not os.path.exists(cfg):
            continue
        items = _parse_es_systems_cfg(cfg)
        for itm in items:
            folder = itm['folder']
            exts = itm['extensions']
            if folder in combined:
                # Fusionner: ajouter extensions manquantes
                present = set(combined[folder])
                for e in exts:
                    if e not in present:
                        combined[folder].append(e)
                        present.add(e)
            else:
                combined[folder] = list(exts)
    # Convertir en liste triée par dossier
    result = [{'folder': k, 'extensions': v} for k, v in sorted(combined.items(), key=lambda x: x[0])]
    logger.info(f"Extensions combinées totales: {len(result)} systèmes")
    return result

    
def check_extension_before_download(url, platform, game_name):
    """Vérifie l'extension avant de lancer le téléchargement et retourne un tuple de 4 éléments."""
    try:
        sanitized_name = sanitize_filename(game_name)
        extensions_data = load_extensions_json()
        # Si le cache des extensions est vide/introuvable, ne bloquez pas: traitez comme "inconnu"
        # afin d'afficher l'avertissement d'extension au lieu d'une erreur fatale.
        if not extensions_data:
            logger.warning(f"Fichier {config.JSON_EXTENSIONS} vide ou introuvable; poursuite avec extensions inconnues")
            extensions_data = []

        is_supported = is_extension_supported(sanitized_name, platform, extensions_data)
        extension = os.path.splitext(sanitized_name)[1].lower()
        is_archive = extension in (".zip", ".rar", ".7z")

        # Déterminer si le système (dossier) est connu dans extensions_data
        dest_folder_name = _get_dest_folder_name(platform)
        system_known = any(s.get("folder") == dest_folder_name for s in extensions_data)

        # Traitement spécifique BIOS: forcer extraction des archives même si le système n'est pas connu
        try:
            bios_like = {"BIOS", "- BIOS by TMCTV -", "- BIOS"}
            if (dest_folder_name == "bios" or platform in bios_like) and is_archive:
                logger.debug(f"Plateforme BIOS détectée pour {sanitized_name}, extraction auto forcée pour {extension}")
                return (url, platform, game_name, True)
        except Exception:
            pass

        # Traitement spécifique PS Vita: ne pas extraire les archives ZIP même si non supportées
        try:
            if dest_folder_name == "psvita" and extension == ".zip":
                logger.debug(f"Plateforme PS Vita détectée pour {sanitized_name}, pas d'extraction automatique pour {extension}")
                return (url, platform, game_name, False)
        except Exception:
            pass

        # Traitement spécifique DOS: forcer extraction des ZIP et RAR pour structurer en dossiers .pc
        try:
            if dest_folder_name == "dos" and is_archive:
                logger.debug(f"Plateforme DOS détectée pour {sanitized_name}, extraction forcée pour {extension}")
                return (url, platform, game_name, True)
        except Exception:
            pass

        if is_supported:
            logger.debug(f"L'extension de {sanitized_name} est supportée pour {platform}")
            return (url, platform, game_name, False)
        elif is_archive:
            # Même si le système n'est pas connu ou que l'extension n'est pas listée,
            # on force l'extraction des archives (ZIP/RAR) à la fin du téléchargement
            # puis suppression du fichier.
            logger.debug(f"Archive {extension.upper()} détectée pour {sanitized_name}, extraction automatique prévue (extension non listée)")
            return (url, platform, game_name, True)
        else:
            # Autoriser si l'utilisateur a choisi d'autoriser les extensions inconnues
            allow_unknown = False
            try:
                allow_unknown = get_allow_unknown_extensions()
            except Exception:
                allow_unknown = False
            if allow_unknown:
                logger.debug(f"Extension non supportée ({extension}) mais autorisée par l'utilisateur pour {sanitized_name}")
                return (url, platform, game_name, False)
            logger.debug(f"Extension non supportée ({extension}) pour {sanitized_name}, avertissement affiché")
            return (url, platform, game_name, False)
    except Exception as e:
        logger.error(f"Erreur vérification extension {url}: {str(e)}")
        return None


# Fonction pour vérifier si l'extension est supportée pour une plateforme donnée
def is_extension_supported(filename, platform_key, extensions_data):
    """Vérifie si l'extension du fichier est supportée pour la plateforme donnée.
    platform_key correspond maintenant à l'identifiant utilisé dans config.platforms (platform_name)."""
    extension = os.path.splitext(filename)[1].lower()

    dest_dir = None
    for platform_dict in config.platform_dicts:
        # Nouveau schéma: platform_name
        if platform_dict.get("platform_name") == platform_key:
            dest_dir = os.path.join(config.ROMS_FOLDER, platform_dict.get("folder"))
            break

    if not dest_dir:
        logger.warning(f"Aucun dossier 'folder' trouvé pour la plateforme {platform_key}")
        dest_dir = os.path.join(os.path.dirname(os.path.dirname(config.APP_FOLDER)), platform_key)
    
    dest_folder_name = os.path.basename(dest_dir)
    logger.debug(f"Vérification extension {extension} pour {filename} dans dossier {dest_folder_name}, {len(extensions_data)} systèmes disponibles")
    
    for i, system in enumerate(extensions_data):
        if system["folder"] == dest_folder_name:
            result = extension in system["extensions"]
            logger.debug(f"Système trouvé: {dest_folder_name}, extensions: {system['extensions']}, résultat: {result}")
            return result
    
    logger.warning(f"Aucun système trouvé pour le dossier {dest_dir}")
    return False
