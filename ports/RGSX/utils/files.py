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

from utils.text import sanitize_filename


DiskUsage = collections.namedtuple("DiskUsage", "total used free")



def get_disk_usage(path: str, log: bool = False) -> "DiskUsage | None":
    """Retourne (total, used, free) en octets pour le point de montage de `path`.

    Sur certains montages (overlayfs, NFS, quotas), `f_bavail` (blocs libres pour un
    utilisateur non privilégié) peut remonter à 0 alors que `f_bfree` (blocs libres
    incluant la réserve root) est correct : on prend le plus grand des deux pour éviter
    de considérer le disque comme plein à tort.
    """
    try:
        usage = shutil.disk_usage(path)
    except Exception as exc:
        logger.debug(f"Impossible de lire l'espace disque pour {path}: {exc}")
        return None

    if usage.free > 0 or platform.system() == "Windows" or not hasattr(os, "statvfs"):
        if log:
            logger.info(f"[HDD] Espace Disque disponible : {usage}")
        return usage

    try:
        st = os.statvfs(path)
        bfree_bytes = st.f_bfree * st.f_frsize
        if bfree_bytes > usage.free:
            logger.warning(
                f"Espace disque libre incohérent pour {path}: f_bavail=0 mais f_bfree={bfree_bytes} octets, utilisation de f_bfree"
            )
            return DiskUsage(usage.total, usage.total - bfree_bytes, bfree_bytes)
    except Exception as exc:
        logger.debug(f"Fallback statvfs impossible pour {path}: {exc}")
    return usage



def _get_dest_folder_name(platform_key: str) -> str:
    """Retourne le nom du dossier de destination pour une plateforme (basename du dossier)."""
    dest_dir = None
    for platform_dict in config.platform_dicts:
        if platform_dict.get("platform_name") == platform_key:
            folder = platform_dict.get("folder")
            if folder:
                dest_dir = os.path.join(config.ROMS_FOLDER, folder)
            break
    if not dest_dir:
        dest_dir = os.path.join(os.path.dirname(os.path.dirname(config.APP_FOLDER)), platform_key)
    return os.path.basename(dest_dir)




def normalize_platform_name(platform):
    """Normalise un nom de plateforme en supprimant espaces et convertissant en minuscules."""
    return platform.lower().replace(" ", "")



def resolve_platform_folder(platform):
    """Résout le dossier ROM d'une plateforme de façon fiable, même si config.platform_dicts
    n'est pas encore chargé en mémoire (ex: reprise de téléchargement au tout début du
    démarrage, avant l'étape 'load_sources').

    Ordre de résolution :
    1) config.platform_dicts (rapide, déjà en mémoire si chargé)
    2) Lecture directe de config.SOURCES_FILE (systems_list.json) sur disque
    3) normalize_platform_name(platform) en dernier recours

    Utiliser cette fonction partout où le dossier ROM est déterminé à partir du nom de
    plateforme, pour garantir un chemin identique quel que soit le moment de l'appel.
    """
    try:
        for platform_dict in getattr(config, 'platform_dicts', None) or []:
            if platform_dict.get("platform_name") == platform:
                folder = platform_dict.get("folder") or platform_dict.get("dossier")
                if folder:
                    return folder
    except Exception as e:
        logger.debug(f"resolve_platform_folder: erreur lecture config.platform_dicts: {e}")

    try:
        sources_file = getattr(config, 'SOURCES_FILE', '')
        if sources_file and os.path.isfile(sources_file):
            with open(sources_file, 'r', encoding='utf-8') as f:
                sources = json.load(f)
            if isinstance(sources, list):
                for entry in sources:
                    if isinstance(entry, dict) and entry.get("platform_name") == platform:
                        folder = entry.get("folder") or entry.get("dossier")
                        if folder:
                            return folder
    except Exception as e:
        logger.debug(f"resolve_platform_folder: erreur lecture {getattr(config, 'SOURCES_FILE', '')}: {e}")

    logger.warning(f"resolve_platform_folder: aucun dossier trouvé pour '{platform}', repli sur normalize_platform_name")
    return normalize_platform_name(platform)



def find_matching_files(base_path, filename):
    """Return all matching files for a requested download name within a ROM folder."""
    if not base_path or not os.path.exists(base_path):
        return []

    raw_filename = str(filename or "")
    candidate_names = []
    for candidate in (Path(raw_filename).name, sanitize_filename(raw_filename)):
        if candidate and candidate not in candidate_names:
            candidate_names.append(candidate)

    if not candidate_names:
        return []

    requested_variants = []
    for candidate_name in candidate_names:
        requested_stem, requested_ext = os.path.splitext(candidate_name)
        requested_normalized = re.sub(r'\s+', ' ', re.sub(r'\s*[\[(][^\])]*[\])]', '', requested_stem)).strip().lower()
        requested_variants.append((candidate_name, requested_stem, requested_ext, requested_normalized))

    archive_exts = {'.zip', '.7z', '.rar', '.tar', '.gz', '.xz', '.bz2'}
    matches = []
    seen_paths = set()

    for candidate_name, _, _, _ in requested_variants:
        full_path = os.path.join(base_path, candidate_name)
        if os.path.exists(full_path) and os.path.isfile(full_path):
            normalized_path = os.path.normcase(full_path)
            if normalized_path not in seen_paths:
                seen_paths.add(normalized_path)
                matches.append((1000, candidate_name, full_path))

    for existing_file in os.listdir(base_path):
        existing_path = os.path.join(base_path, existing_file)
        if not os.path.isfile(existing_path):
            continue

        normalized_path = os.path.normcase(existing_path)
        if normalized_path in seen_paths:
            continue

        existing_stem, existing_ext = os.path.splitext(existing_file)
        score = None

        existing_normalized = re.sub(r'\s+', ' ', re.sub(r'\s*[\[(][^\])]*[\])]', '', existing_stem)).strip().lower()
        for _, requested_stem, requested_ext, requested_normalized in requested_variants:
            candidate_score = None
            if requested_stem and existing_stem == requested_stem:
                candidate_score = 900
            elif requested_normalized and existing_normalized and existing_normalized == requested_normalized:
                candidate_score = 0
                if requested_ext and existing_ext.lower() == requested_ext.lower():
                    candidate_score += 4
                if existing_ext.lower() not in archive_exts:
                    candidate_score += 3
                candidate_score -= abs(len(existing_stem) - len(requested_stem))

            if candidate_score is not None:
                score = candidate_score if score is None else max(score, candidate_score)

        if score is not None:
            seen_paths.add(normalized_path)
            matches.append((score, existing_file, existing_path))

    matches.sort(key=lambda item: item[0], reverse=True)
    return [(actual_filename, actual_path) for _, actual_filename, actual_path in matches]



def move_files_to_directory(file_paths, destination_dir):
    """Move files to a destination directory, avoiding name collisions."""
    if not destination_dir:
        return False, [], "Destination directory is empty"

    if not any(file_paths or []):
        return False, [], "No files to move"

    try:
        os.makedirs(destination_dir, exist_ok=True)
    except Exception as e:
        logger.error(f"Impossible de créer le dossier de destination {destination_dir}: {e}")
        return False, [], str(e)

    moved_matches = []
    seen_sources = set()
    reserved_targets = set()

    for raw_source in file_paths:
        if not raw_source:
            continue

        source_path = os.path.abspath(str(raw_source))
        normalized_source = os.path.normcase(source_path)
        if normalized_source in seen_sources:
            continue
        seen_sources.add(normalized_source)

        if not os.path.isfile(source_path):
            error_message = f"File not found: {source_path}"
            logger.warning(error_message)
            return False, moved_matches, error_message

        source_name = os.path.basename(source_path)
        target_path = os.path.join(destination_dir, source_name)
        target_root, target_ext = os.path.splitext(target_path)
        suffix = 1

        while os.path.normcase(target_path) in reserved_targets or (
            os.path.exists(target_path)
            and os.path.normcase(target_path) != os.path.normcase(source_path)
        ):
            target_path = f"{target_root} ({suffix}){target_ext}"
            suffix += 1

        reserved_targets.add(os.path.normcase(target_path))

        try:
            if os.path.normcase(source_path) != os.path.normcase(target_path):
                shutil.move(source_path, target_path)
                logger.info(f"Fichier déplacé: {source_path} -> {target_path}")
            else:
                logger.debug(f"Déplacement ignoré, même chemin source/destination: {source_path}")
            moved_matches.append((os.path.basename(target_path), target_path))
        except Exception as e:
            logger.error(f"Erreur lors du déplacement de {source_path} vers {target_path}: {e}")
            return False, moved_matches, str(e)

    return True, moved_matches, None



def find_file_with_or_without_extension(base_path, filename):
    """
    Cherche un fichier, avec son extension ou sans (cherche jeuxxx.* si jeuxxx.zip n'existe pas).
    Retourne (file_exists, actual_filename, actual_path).
    """
    candidate_name = Path(str(filename or "")).name
    full_path = os.path.join(base_path, candidate_name)
    matches = find_matching_files(base_path, candidate_name)
    if matches:
        actual_filename, actual_path = matches[0]
        return True, actual_filename, actual_path

    return False, candidate_name, full_path
