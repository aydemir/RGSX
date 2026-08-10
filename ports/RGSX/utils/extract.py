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


# Liste globale pour stocker les systèmes avec une erreur 404
unavailable_systems = []



def _resolve_7z_command():
    if config.OPERATING_SYSTEM == "Windows":
        return config.SEVEN_Z_EXE

    candidates = []
    bundled = getattr(config, 'SEVEN_Z_LINUX', '')
    if bundled:
        candidates.append(bundled)
    for binary_name in ("7zz", "7z", "7zr"):
        resolved = shutil.which(binary_name)
        if resolved and resolved not in candidates:
            candidates.append(resolved)

    last_error = None
    for candidate in candidates:
        if not candidate:
            continue
        try:
            if os.path.isabs(candidate) and not os.path.exists(candidate):
                continue
            if os.path.exists(candidate) and not os.access(candidate, os.X_OK):
                logger.warning(f"{candidate} n'est pas exécutable, correction des permissions...")
                os.chmod(candidate, 0o755)
            probe = subprocess.run(
                [candidate],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=5,
                check=False,
            )
            if isinstance(probe.returncode, int):
                return candidate
        except OSError as exc:
            last_error = exc
            logger.warning(f"Binaire 7z ignoré ({candidate}): {exc}")
        except Exception as exc:
            last_error = exc
            logger.warning(f"Impossible d'utiliser le binaire 7z {candidate}: {exc}")

    if last_error is not None:
        logger.error(f"Aucun binaire 7z utilisable trouvé: {last_error}")
    return bundled or None


def extract_data(zip_path, dest_dir, url):
    """Extrait le contenu de ZIP de DATA dans le dossier config.SAVE_FOLDER sans progression a l'ecran"""
    logger.debug(f"Extraction de {zip_path} dans {dest_dir}")
    try:
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.testzip()  # Vérifier l'intégrité de l'archive
            for info in zip_ref.infolist():
                if info.is_dir():
                    continue
                file_path = os.path.join(dest_dir, info.filename)
                os.makedirs(os.path.dirname(file_path), exist_ok=True)
                with zip_ref.open(info) as source, open(file_path, 'wb') as dest:
                    shutil.copyfileobj(source, dest)
        logger.info(f"Extraction terminée de {zip_path}")
        return True, "Extraction terminée avec succès"
    except zipfile.BadZipFile as e:
        logger.error(f"Erreur: Archive ZIP corrompue: {str(e)}")
        return False, _("utils_corrupt_zip").format(str(e))


    

def _update_extraction_progress(url, extracted_size, total_size, lock, last_save_time_ref, save_interval=0.5):
    """Fonction utilitaire pour mettre à jour la progression d'extraction."""
    try:
        current_time = time.time()
        with lock:
            if isinstance(config.history, list):
                for entry in config.history:
                    if "status" in entry and entry["status"] in ["Téléchargement", "Extracting", "Downloading"]:
                        if "url" in entry and entry["url"] == url:
                            progress_percent = int(extracted_size / total_size * 100) if total_size > 0 else 0
                            progress_percent = max(0, min(100, progress_percent))
                            entry["status"] = "Extracting"
                            entry["progress"] = progress_percent
                            entry["message"] = "Extraction en cours"
                            if current_time - last_save_time_ref[0] >= save_interval:
                                save_history(config.history)
                                last_save_time_ref[0] = current_time
                            config.needs_redraw = True
                            break
    except Exception as e:
        logger.debug(f"Erreur mise à jour progression extraction: {e}")


def _finalize_extraction(archive_path, dest_dir, url):
    """Fonction utilitaire pour finaliser l'extraction (suppression fichier + historique).
    NOTE: Ne met PAS à jour l'historique - c'est le rôle de network.py après le retour.
    Cela évite les doublons d'entrées dans l'historique.
    """
    # Quelques tentatives avec un court délai : un antivirus/indexeur peut brièvement
    # garder le fichier ouvert juste après extraction (Windows). Sans ceci, l'archive
    # source resterait dupliquée sur le disque à côté des fichiers extraits.
    last_error = None
    for attempt in range(5):
        try:
            os.remove(archive_path)
            logger.info(f"Fichier {archive_path} extrait dans {dest_dir} et supprimé")

            # Mettre à jour l'état de progression à 100% (mais PAS l'historique)
            if url in getattr(config, 'download_progress', {}):
                try:
                    config.download_progress[url]["status"] = "Download_OK"
                    config.download_progress[url]["progress_percent"] = 100
                except Exception:
                    pass

            return True, _("utils_extracted").format(os.path.basename(archive_path))
        except Exception as e:
            last_error = e
            if attempt < 4:
                time.sleep(0.5)
    logger.error(f"Erreur lors de la finalisation de l'extraction: {str(last_error)}")
    return True, _("utils_extracted").format(os.path.basename(archive_path))


def _capture_directories_before_extraction(dest_dir):
    """Capture les dossiers existants avant extraction pour détection PS3."""
    try:
        return set([d for d in os.listdir(dest_dir) if os.path.isdir(os.path.join(dest_dir, d))])
    except Exception:
        return set()


def _capture_all_items_before_extraction(dest_dir):
    """Capture tous les éléments (fichiers et dossiers) existants avant extraction pour DOS."""
    try:
        return set(os.listdir(dest_dir))
    except Exception:
        return set()


def _handle_special_platforms(dest_dir, archive_path, before_dirs, iso_before=None, url=None, before_items=None):
    """Gère les traitements spéciaux Xbox, PS3 et DOS après extraction.
    
    Args:
        before_items: Set de tous les éléments (fichiers+dossiers) avant extraction (pour DOS)
    """
    # Xbox: conversion ISO
    # Gérer les deux cas: symlink activé (xbox/xbox) ou désactivé (xbox)
    xbox_dir_normal = os.path.join(config.ROMS_FOLDER, "xbox")
    xbox_dir_symlink = os.path.join(config.ROMS_FOLDER, "xbox", "xbox")
    is_xbox = (dest_dir == xbox_dir_normal or dest_dir == xbox_dir_symlink)
    
    if is_xbox and iso_before is not None:
        iso_after = set()
        for root, dirs, files in os.walk(dest_dir):
            for file in files:
                if file.lower().endswith('.iso'):
                    iso_after.add(os.path.abspath(os.path.join(root, file)))
        new_isos = list(iso_after - iso_before)
        if new_isos:
            success, error_msg = handle_xbox(dest_dir, new_isos, url)
            if not success:
                return False, error_msg
        else:
            logger.warning("Aucun nouvel ISO détecté après extraction pour conversion Xbox.")

    # Dossier PS3: traitement spécifique
    # Gérer les deux cas: symlink activé (ps3/ps3) ou désactivé (ps3)
    ps3_dir_normal = os.path.join(config.ROMS_FOLDER, "ps3")
    ps3_dir_symlink = os.path.join(config.ROMS_FOLDER, "ps3", "ps3")
    is_ps3 = (dest_dir == ps3_dir_normal or dest_dir == ps3_dir_symlink)
    
    if is_ps3:
        # PS3 Redump: décryptage et extraction
        logger.info("Détection PS3 Redump - lancement du traitement spécifique")
        
        # Calculer les nouveaux dossiers créés lors de l'extraction
        try:
            after_dirs = set([d for d in os.listdir(dest_dir) if os.path.isdir(os.path.join(dest_dir, d))])
        except Exception:
            after_dirs = set()
        
        ignore_names = {"ps3", "images", "videos", "manuals", "media"}
        new_dirs = [d for d in (after_dirs - before_dirs) if d not in ignore_names and not d.endswith('.ps3')]
        expected_base = os.path.splitext(os.path.basename(archive_path))[0]
        
        success, error_msg = handle_ps3(
            dest_dir=dest_dir,
            new_dirs=new_dirs,
            extracted_basename=expected_base,
            url=url,
            archive_name=os.path.basename(archive_path)
        )
        if not success:
            return False, error_msg
        return True, None

    # DOS: organisation en dossiers .pc
    dos_dir = os.path.join(config.ROMS_FOLDER, "dos")
    if dest_dir == dos_dir:
        expected_base = os.path.splitext(os.path.basename(archive_path))[0]
        # Utiliser before_items si fourni, sinon before_dirs pour rétro-compatibilité
        items_before = before_items if before_items is not None else before_dirs
        success, error_msg = handle_dos(dest_dir, items_before, extracted_basename=expected_base)
        if not success:
            return False, error_msg
    
    # ScummVM: organisation en dossiers + fichier .scummvm
    scummvm_dir = os.path.join(config.ROMS_FOLDER, "scummvm")
    if dest_dir == scummvm_dir:
        expected_base = os.path.splitext(os.path.basename(archive_path))[0]
        # Utiliser before_items si fourni, sinon before_dirs pour rétro-compatibilité
        items_before = before_items if before_items is not None else before_dirs
        success, error_msg = handle_scummvm(dest_dir, items_before, extracted_basename=expected_base)
        if not success:
            return False, error_msg
    
    # PSVita: extraction dans ux0/app + création fichier .psvita
    psvita_dir_normal = os.path.join(config.ROMS_FOLDER, "psvita")
    psvita_dir_symlink = os.path.join(config.ROMS_FOLDER, "psvita", "psvita")
    is_psvita = (dest_dir == psvita_dir_normal or dest_dir == psvita_dir_symlink)
    
    if is_psvita:
        expected_base = os.path.splitext(os.path.basename(archive_path))[0]
        items_before = before_items if before_items is not None else before_dirs
        success, error_msg = handle_psvita(dest_dir, items_before, extracted_basename=expected_base)
        if not success:
            return False, error_msg
    
    return True, None


def extract_zip(zip_path, dest_dir, url):
    """Extrait le contenu du fichier ZIP dans le dossier cible avec un suivi progressif de la progression."""
    logger.debug(f"Extraction de {zip_path} dans {dest_dir}")
    try:
        # Capture état initial
        before_dirs = _capture_directories_before_extraction(dest_dir)
        # Capture tous les items pour DOS
        before_items = _capture_all_items_before_extraction(dest_dir)
        iso_before = set()
        for root, dirs, files in os.walk(dest_dir):
            for file in files:
                if file.lower().endswith('.iso'):
                    iso_before.add(os.path.abspath(os.path.join(root, file)))

        # Vérification et extraction
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.testzip()  # Vérifier l'intégrité de l'archive
            
            # Pré-analyse pour détecter les conflits fichier/dossier
            all_paths = set()
            file_paths = set()
            for info in zip_ref.infolist():
                normalized = info.filename.replace('/', os.sep)
                if not info.is_dir():
                    file_paths.add(normalized)
                all_paths.add(normalized)
            
            # Identifier les conflits (un fichier existe avec un nom qui est aussi un dossier parent)
            conflicts = set()
            for file_path in file_paths:
                # Vérifier si un parent de ce fichier existe aussi comme fichier
                parts = file_path.split(os.sep)
                for i in range(1, len(parts)):
                    parent_path = os.sep.join(parts[:i])
                    if parent_path in file_paths:
                        conflicts.add(parent_path)
                        logger.warning(f"Conflit détecté: '{parent_path}' est à la fois un fichier et un dossier parent")
            
            total_size = sum(info.file_size for info in zip_ref.infolist() if not info.is_dir())
            logger.info(f"Taille totale à extraire: {total_size} octets")
            
            if total_size == 0:
                logger.warning("ZIP vide ou ne contenant que des dossiers")
                return True, "ZIP vide extrait avec succès"

            # Variables de progression
            extracted_size = 0
            lock = threading.Lock()
            last_save_time = [time.time()]  # Liste pour référence mutable
            chunk_size = 2048
            os.makedirs(dest_dir, exist_ok=True)

            # Trier les fichiers par profondeur (nombre de séparateurs) pour extraire les fichiers racine d'abord
            files_to_extract = [info for info in zip_ref.infolist() if not info.is_dir()]
            files_to_extract.sort(key=lambda x: x.filename.count('/'))
            
            # Extraction avec progression
            for info in files_to_extract:
                # Normaliser le chemin pour Windows (remplacer / par \)
                normalized_filename = info.filename.replace('/', os.sep)
                
                # Debug pour fichiers .nca
                if normalized_filename.endswith('.nca'):
                    logger.debug(f"Traitement fichier NCA: {normalized_filename}")
                
                # Ignorer les fichiers en conflit (ils sont des dossiers parents pour d'autres fichiers)
                if normalized_filename in conflicts:
                    logger.warning(f"Fichier ignoré (conflit avec dossier): {normalized_filename}")
                    continue
                
                file_path = os.path.join(dest_dir, normalized_filename)
                
                try:
                    # Créer uniquement le dossier parent, pas le fichier lui-même
                    parent_dir = os.path.dirname(file_path)
                    # Vérifier que parent_dir n'est pas vide et est différent de dest_dir
                    if parent_dir and parent_dir != dest_dir:
                        os.makedirs(parent_dir, exist_ok=True)
                except Exception as dir_err:
                    logger.error(f"Erreur création dossier parent pour {file_path}: {dir_err}")
                    raise
                
                try:
                    # Vérifier si un dossier existe avec le même nom (conflit)
                    if os.path.isdir(file_path):
                        logger.warning(f"Conflit: dossier existant avec le même nom que le fichier {file_path}, suppression du dossier")
                        try:
                            shutil.rmtree(file_path)
                        except Exception as rm_err:
                            logger.error(f"Impossible de supprimer le dossier {file_path}: {rm_err}")
                            raise
                    # Vérifier si le fichier existe déjà et est en lecture seule
                    elif os.path.exists(file_path):
                        try:
                            # Retirer l'attribut lecture seule si présent (Windows)
                            os.chmod(file_path, 0o644)
                        except Exception:
                            pass
                    
                    with zip_ref.open(info) as source, open(file_path, 'wb') as dest:
                        while True:
                            chunk = source.read(chunk_size)
                            if not chunk:
                                break
                            dest.write(chunk)
                            extracted_size += len(chunk)
                            _update_extraction_progress(url, extracted_size, total_size, lock, last_save_time)
                    
                    # Définir les permissions (skip sur Windows si erreur)
                    try:
                        os.chmod(file_path, 0o644)
                    except (OSError, PermissionError) as chmod_err:
                        logger.debug(f"Impossible de définir chmod pour {file_path}: {chmod_err}")
                except Exception as file_err:
                    logger.error(f"Erreur extraction fichier {info.filename} vers {file_path}: {file_err}")
                    raise

        # Gestion plateformes spéciales
        success, error_msg = _handle_special_platforms(dest_dir, zip_path, before_dirs, iso_before, url, before_items)
        if not success:
            return False, error_msg

        # Finalisation
        return _finalize_extraction(zip_path, dest_dir, url)

    except zipfile.BadZipFile as e:
        logger.error(f"Erreur: Archive ZIP corrompue: {str(e)}")
        return False, _("utils_corrupt_zip").format(str(e))
    except PermissionError as e:
        logger.error(f"Erreur: Permission refusée lors de l'extraction: {str(e)}")
        return False, _("utils_permission_denied").format(str(e))
    except Exception as e:
        logger.error(f"Erreur lors de l'extraction de {zip_path}: {str(e)}")
        return False, _("utils_extraction_failed").format(str(e))

     

# Fonction pour extraire le contenu d'un fichier RAR
def extract_rar(rar_path, dest_dir, url):
    """Extrait le contenu du fichier RAR dans le dossier cible."""
    try:
        os.makedirs(dest_dir, exist_ok=True)
        
        # Configuration commande unrar selon l'OS
        if config.OPERATING_SYSTEM == "Windows":
            unrar_cmd = [config.UNRAR_EXE]
        else:
            unrar_cmd = ["unrar"]

        # Vérification disponibilité unrar
        result = subprocess.run(unrar_cmd, capture_output=True, text=True)
        if result.returncode not in [0, 1]:
            logger.error("Commande unrar non disponible")
            return False, _("utils_unrar_unavailable")

        # Analyse contenu RAR
        result = subprocess.run(unrar_cmd + ['l', '-v', rar_path], capture_output=True, text=True)
        if result.returncode != 0:
            error_msg = result.stderr.strip()
            logger.error(f"Erreur lors de la liste des fichiers RAR: {error_msg}")
            return False, _("utils_rar_list_failed").format(error_msg)

        # Parse liste fichiers
        total_size = 0
        files_to_extract = []
        lines = result.stdout.splitlines()
        in_file_list = False
        for line in lines:
            if line.startswith("----"):
                in_file_list = not in_file_list
                continue
            if in_file_list:
                match = re.match(r'^\s*(\S+)\s+(\d+)\s+\d*\s*(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2})\s+(.+)$', line)
                if match:
                    attrs, file_size, file_date, file_name = match.groups()
                    if 'D' not in attrs:  # Pas un dossier
                        files_to_extract.append((file_name.strip(), int(file_size)))
                        total_size += int(file_size)

        logger.info(f"Taille totale à extraire (RAR): {total_size} octets")
        if total_size == 0:
            logger.warning("RAR vide, ne contenant que des dossiers, ou erreur de parsing")
            return False, "RAR vide ou erreur lors de la liste des fichiers"

        # Capture état initial
        before_dirs = _capture_directories_before_extraction(dest_dir)

        # Variables de progression
        lock = threading.Lock()
        last_save_time = [time.time()]  # Liste pour référence mutable

        # Initialisation progression
        if url not in getattr(config, 'download_progress', {}):
            config.download_progress[url] = {}
        config.download_progress[url].update({
            "downloaded_size": 0,
            "total_size": total_size,
            "status": "Extracting",
            "progress_percent": 0
        })
        config.needs_redraw = True

        # Extraction RAR
        process = subprocess.Popen(unrar_cmd + ['x', '-y', rar_path, dest_dir],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        stdout, stderr = process.communicate()

        if process.returncode != 0:
            logger.error(f"Erreur lors de l'extraction de {rar_path}: {stderr}")
            return False, f"Erreur lors de l'extraction: {stderr}"

        # Vérification et mise à jour progression des fichiers extraits
        extracted_size = 0
        total_files = len(files_to_extract)
        for i, (expected_file, file_size) in enumerate(files_to_extract):
            file_path = os.path.join(dest_dir, expected_file)
            if os.path.exists(file_path):
                extracted_size += file_size
                os.chmod(file_path, 0o644)
                
                # Mise à jour progression basée sur nombre de fichiers
                progress_percent = int(((i + 1) / total_files * 100)) if total_files > 0 else 0
                _update_extraction_progress(url, progress_percent * total_size // 100, total_size, lock, last_save_time)
                
                # Mise à jour config.download_progress
                with lock:
                    if url in config.download_progress:
                        config.download_progress[url]["downloaded_size"] = extracted_size
                        config.download_progress[url]["progress_percent"] = progress_percent
                        config.needs_redraw = True

        # Permissions dossiers
        for root, dirs, files in os.walk(dest_dir):
            for dir_name in dirs:
                os.chmod(os.path.join(root, dir_name), 0o755)

        # Gestion plateformes spéciales (uniquement PS3 pour RAR)
        success, error_msg = _handle_special_platforms(dest_dir, rar_path, before_dirs, url=url)
        if not success:
            return False, error_msg

        # Finalisation
        return _finalize_extraction(rar_path, dest_dir, url)
        
    except Exception as e:
        logger.error(f"Erreur lors de l'extraction de {rar_path}: {str(e)}")
        return False, f"Erreur lors de l'extraction: {str(e)}"
    finally:
        # Nettoyage en cas d'erreur
        if os.path.exists(rar_path):
            try:
                os.remove(rar_path)
                logger.info(f"Fichier RAR {rar_path} supprimé après échec de l'extraction")
            except Exception as e:
                logger.error(f"Erreur lors de la suppression de {rar_path}: {str(e)}")



def extract_7z(archive_path, dest_dir, url):
    """Extrait le contenu d'un fichier 7z dans le dossier cible."""
    try:
        os.makedirs(dest_dir, exist_ok=True)

        seven_z_cmd = _resolve_7z_command()

        if not seven_z_cmd or (os.path.isabs(seven_z_cmd) and not os.path.exists(seven_z_cmd)):
            return False, "7z non trouvé - vérifiez que 7z.exe (Windows) ou 7zz (Linux) est présent dans assets/progs"

        # Capture état initial
        before_dirs = _capture_directories_before_extraction(dest_dir)
        before_items = _capture_all_items_before_extraction(dest_dir)
        iso_before = set()
        for root, dirs, files in os.walk(dest_dir):
            for file in files:
                if file.lower().endswith('.iso'):
                    iso_before.add(os.path.abspath(os.path.join(root, file)))

        # Calcul taille totale via 7z l -slt (best effort)
        total_size = 0
        try:
            list_cmd = [seven_z_cmd, "l", "-slt", archive_path]
            result = subprocess.run(list_cmd, capture_output=True, text=True)
            if result.returncode == 0:
                current_size = None
                is_dir = False
                for line in result.stdout.splitlines():
                    line = line.strip()
                    if not line:
                        if current_size is not None and not is_dir:
                            total_size += current_size
                        current_size = None
                        is_dir = False
                        continue
                    if line.startswith("Attributes ="):
                        attrs = line.split("=", 1)[1].strip()
                        if "D" in attrs:
                            is_dir = True
                    elif line.startswith("Size ="):
                        try:
                            current_size = int(line.split("=", 1)[1].strip())
                        except Exception:
                            current_size = None
                if current_size is not None and not is_dir:
                    total_size += current_size
        except Exception as e:
            logger.debug(f"Impossible de calculer la taille 7z: {e}")

        if url not in getattr(config, 'download_progress', {}):
            config.download_progress[url] = {}
        config.download_progress[url].update({
            "downloaded_size": 0,
            "total_size": total_size,
            "status": "Extracting",
            "progress_percent": 0
        })
        config.needs_redraw = True

        extract_cmd = [seven_z_cmd, "x", archive_path, f"-o{dest_dir}", "-y"]
        logger.debug(f"Commande d'extraction 7z: {' '.join(extract_cmd)}")
        result = subprocess.run(extract_cmd, capture_output=True, text=True)
        if result.returncode > 2:
            error_msg = result.stderr.strip() or f"Erreur extraction 7z (code {result.returncode})"
            logger.error(error_msg)
            return False, error_msg
        if result.returncode != 0:
            logger.warning(f"7z a retourné un avertissement (code {result.returncode}): {result.stderr}")

        # Gestion plateformes spéciales
        success, error_msg = _handle_special_platforms(dest_dir, archive_path, before_dirs, iso_before, url, before_items)
        if not success:
            return False, error_msg

        return _finalize_extraction(archive_path, dest_dir, url)
    except Exception as e:
        logger.error(f"Erreur lors de l'extraction 7z: {str(e)}")
        return False, _("utils_extraction_failed").format(str(e))


def handle_ps3(dest_dir, new_dirs=None, extracted_basename=None, url=None, archive_name=None):
    """Gère le traitement spécifique des jeux PS3.
   PS3 Redump (ps3): Décryptage ISO + extraction dans dossier .ps3
    
    Args:
        dest_dir: Dossier de destination (ps3 ou ps3)
        new_dirs: Liste des nouveaux dossiers créés (mode classique)
        extracted_basename: Nom de base de l'archive extraite
        url: URL du jeu (nécessaire pour PS3 Redump)
        archive_name: Nom complet de l'archive avec extension (pour PS3 Redump)
    
    Returns:
        (success: bool, message: str)
    """
    logger.debug(f"Traitement spécifique PS3 dans: {dest_dir}")
    
    # Détection du mode PS3 - supporter les deux cas: symlink activé (ps3/ps3) ou désactivé (ps3)
    ps3_dir_normal = os.path.join(config.ROMS_FOLDER, "ps3")
    ps3_dir_symlink = os.path.join(config.ROMS_FOLDER, "ps3", "ps3")
    is_ps3 = (dest_dir == ps3_dir_normal or dest_dir == ps3_dir_symlink)
    
    if is_ps3:
        # ============================================
        # MODE PS3 : Décryptage et extraction
        # ============================================
        logger.info(f"Mode PS3  détecté pour: {archive_name}")

        # L'extraction de l'archive est terminée; basculer l'UI en mode conversion/décryptage.
        try:
            if url:
                if url not in getattr(config, 'download_progress', {}):
                    config.download_progress[url] = {}
                config.download_progress[url]["status"] = "Converting"
                config.download_progress[url]["progress_percent"] = 0
                config.needs_redraw = True

                if isinstance(config.history, list):
                    for entry in config.history:
                        if entry.get("url") == url and entry.get("status") in ("Extracting", "Téléchargement", "Downloading"):
                            entry["status"] = "Converting"
                            entry["progress"] = 0
                            entry["message"] = "PS3 conversion in progress"
                            save_history(config.history)
                            break
        except Exception as e:
            logger.debug(f"MAJ statut conversion PS3 ignorée: {e}")
        
        try:
            ps3_keys_base_url = "https://retrogamesets.fr/softs/ps3/"
            logger.debug(f"URL jeu: {url}")
            logger.debug(f"Base URL des clés PS3: {ps3_keys_base_url}")
            
            # Chercher uniquement l'ISO correspondant au fichier téléchargé/extrait.
            iso_files = [f for f in os.listdir(dest_dir) if f.endswith('.iso') and not f.endswith('_decrypted.iso')]
            if not iso_files:
                # Vérifier si le jeu est déjà extrait dans un dossier
                game_folders = []
                for f in os.listdir(dest_dir):
                    f_path = os.path.join(dest_dir, f)
                    if os.path.isdir(f_path):
                        if f.endswith('.ps3'):
                            game_folders.append(f)
                        elif os.path.exists(os.path.join(f_path, 'PS3_GAME')):
                            game_folders.append(f)
                if game_folders:
                    game_folder = game_folders[0]
                    game_folder_path = os.path.join(dest_dir, game_folder)
                    if not game_folder.endswith('.ps3'):
                        new_name = game_folder + '.ps3'
                        new_path = os.path.join(dest_dir, new_name)
                        os.rename(game_folder_path, new_path)
                        logger.info(f"Dossier renommé: {game_folder} -> {new_name}")
                        game_folder = new_name
                    logger.info(f"Dossier de jeu PS3 déjà présent: {game_folder}")
                    return True, f"Jeu PS3 déjà extrait dans {game_folder}"
                else:
                    return False, "Aucun fichier .iso trouvé après extraction"

            def _normalize_ps3_name(name: str) -> str:
                base_name = os.path.splitext(os.path.basename(name or ""))[0]
                return re.sub(r"\s+", " ", base_name).strip().casefold()

            candidate_bases = []

            def _add_candidate_base(base_name):
                if not base_name:
                    return
                cleaned = str(base_name).strip()
                if not cleaned:
                    return
                if cleaned.lower().endswith('.dkey'):
                    cleaned = cleaned[:-5]
                normalized = _normalize_ps3_name(cleaned)
                if normalized and normalized not in candidate_bases:
                    candidate_bases.append(normalized)

            if archive_name:
                _add_candidate_base(os.path.splitext(os.path.basename(archive_name))[0])
            if extracted_basename:
                _add_candidate_base(extracted_basename)

            matching_iso_files = []
            for iso_name in iso_files:
                if _normalize_ps3_name(iso_name) in candidate_bases:
                    matching_iso_files.append(iso_name)

            if len(matching_iso_files) == 1:
                iso_file = matching_iso_files[0]
            elif len(matching_iso_files) > 1:
                iso_file = sorted(matching_iso_files)[0]
                logger.warning(
                    "Plusieurs ISO PS3 correspondent au fichier téléchargé dans %s, sélection de %s",
                    dest_dir,
                    iso_file,
                )
            elif len(iso_files) == 1:
                iso_file = iso_files[0]
                logger.warning(
                    "Un seul ISO PS3 présent dans %s, sélection par défaut de %s car aucun nom ne correspond au téléchargement courant",
                    dest_dir,
                    iso_file,
                )
            else:
                logger.error(
                    "Plusieurs ISO PS3 présents dans %s mais aucun ne correspond au téléchargement courant (%s / %s)",
                    dest_dir,
                    archive_name,
                    extracted_basename,
                )
                return False, "Impossible d'identifier l'ISO PS3 correspondant au fichier téléchargé"

            iso_path = os.path.join(dest_dir, iso_file)
            logger.info(f"Fichier ISO correspondant trouvé: {iso_path}")
            
            # Étape 1: Télécharger directement la clé .dkey depuis la nouvelle source
            dkey_path = None
            logger.info("Téléchargement de la clé de décryption (.dkey)...")

            candidate_bases = []

            def _add_candidate_base(base_name):
                if not base_name:
                    return
                cleaned = str(base_name).strip()
                if not cleaned:
                    return
                if cleaned.lower().endswith('.dkey'):
                    cleaned = cleaned[:-5]
                if cleaned not in candidate_bases:
                    candidate_bases.append(cleaned)

            if archive_name:
                _add_candidate_base(os.path.splitext(os.path.basename(archive_name))[0])
            if extracted_basename:
                _add_candidate_base(extracted_basename)
            _add_candidate_base(os.path.splitext(os.path.basename(iso_file))[0])

            for base_name in candidate_bases:
                remote_name = f"{base_name}.dkey"
                encoded_name = remote_name.replace(" ", "%20")
                key_url = f"{ps3_keys_base_url}{encoded_name}"
                logger.debug(f"Tentative clé distante: {key_url}")
                try:
                    response = requests.get(key_url, stream=True, timeout=30)
                    if response.status_code != 200:
                        logger.debug(f"Clé distante introuvable ({response.status_code}): {remote_name}")
                        continue

                    local_dkey_path = os.path.join(dest_dir, remote_name)
                    with open(local_dkey_path, 'wb') as f:
                        for chunk in response.iter_content(chunk_size=8192):
                            if chunk:
                                f.write(chunk)

                    dkey_path = local_dkey_path
                    logger.info(f"Clé téléchargée: {dkey_path}")
                    break
                except Exception as e:
                    logger.warning(f"Échec téléchargement clé {remote_name}: {e}")
            
            # Chercher une clé .dkey si pas téléchargée
            if not dkey_path:
                dkey_files = [f for f in os.listdir(dest_dir) if f.endswith('.dkey')]
                if dkey_files:
                    dkey_path = os.path.join(dest_dir, dkey_files[0])
                    logger.info(f"Clé trouvée localement: {dkey_path}")
                else:
                    return False, "Aucune clé de décryption trouvée (.dkey)"
            
            # Étape 2: Décrypter l'ISO
            logger.info("Décryptage de l'ISO...")
            decrypted_iso_path = iso_path.replace('.iso', '_decrypted.iso')
            
            # Vérifier et corriger les permissions de ps3dec sur Linux
            if config.OPERATING_SYSTEM != "Windows":
                ps3dec_tool = config.PS3DEC_LINUX
                try:
                    if os.path.exists(ps3dec_tool):
                        current_perms = os.stat(ps3dec_tool).st_mode
                        if not os.access(ps3dec_tool, os.X_OK):
                            logger.warning(f"ps3dec_linux n'est pas exécutable, correction des permissions...")
                            os.chmod(ps3dec_tool, 0o755)
                            logger.info(f"Permissions corrigées pour {ps3dec_tool}")
                        else:
                            logger.debug(f"ps3dec_linux a déjà les permissions d'exécution")
                    else:
                        return False, f"ps3dec_linux non trouvé: {ps3dec_tool}"
                except Exception as e:
                    logger.error(f"Erreur lors de la vérification des permissions: {e}")
                    # Continuer quand même, l'erreur sera capturée plus tard
            
            if config.OPERATING_SYSTEM == "Windows":
                # Utiliser des guillemets doubles et échapper correctement pour PowerShell
                # Doubler les guillemets doubles internes pour l'échappement PowerShell
                dkey_escaped = dkey_path.replace('"', '""')
                ps3dec_escaped = config.PS3DEC_EXE.replace('"', '""')
                iso_escaped = iso_path.replace('"', '""')
                decrypted_escaped = decrypted_iso_path.replace('"', '""')
                
                cmd = [
                    "powershell", "-Command",
                    f'$key = [System.IO.File]::ReadAllText("{dkey_escaped}").Trim(); ' +
                    f'& "{ps3dec_escaped}" d key $key "{iso_escaped}" "{decrypted_escaped}"'
                ]
            else:  # Linux
                # Utiliser des guillemets doubles avec échappement pour bash
                # Échapper les caractères spéciaux: $, `, \, ", et !
                def bash_escape(path):
                    return path.replace('\\', '\\\\').replace('"', '\\"').replace('$', '\\$').replace('`', '\\`')
                
                dkey_escaped = bash_escape(dkey_path)
                ps3dec_escaped = bash_escape(config.PS3DEC_LINUX)
                iso_escaped = bash_escape(iso_path)
                decrypted_escaped = bash_escape(decrypted_iso_path)
                
                cmd = [
                    "bash", "-c",
                    f'key=$(cat "{dkey_escaped}" | tr -d \' \\n\\r\\t\'); ' +
                    f'"{ps3dec_escaped}" d key "$key" "{iso_escaped}" "{decrypted_escaped}"'
                ]
            
            logger.debug(f"Commande de décryptage: {' '.join(cmd)}")
            result = subprocess.run(cmd, capture_output=True, text=True)
            
            if result.returncode != 0:
                error_msg = f"Erreur lors du décryptage: {result.stderr}"
                logger.error(error_msg)
                return False, error_msg
            
            logger.info("Décryptage réussi")
            os.remove(iso_path)
            logger.debug(f"ISO original supprimé: {iso_path}")
            
            # Étape 3: Extraire l'ISO décrypté dans un dossier .ps3
            logger.info("Extraction de l'ISO décrypté...")
            game_folder_name = os.path.splitext(os.path.basename(iso_file))[0] + ".ps3"
            game_folder_path = os.path.join(dest_dir, game_folder_name)
            os.makedirs(game_folder_path, exist_ok=True)
            
            try:
                seven_z_cmd = _resolve_7z_command()
                if not seven_z_cmd or (os.path.isabs(seven_z_cmd) and not os.path.exists(seven_z_cmd)):
                    return False, f"7zz non trouvé: {config.SEVEN_Z_LINUX}"
                
                extract_cmd = [seven_z_cmd, "x", decrypted_iso_path, f"-o{game_folder_path}", "-y"]
                logger.debug(f"Commande d'extraction ISO: {' '.join(extract_cmd)}")
                result = subprocess.run(extract_cmd, capture_output=True, text=True)
                
                if result.returncode > 2:
                    error_msg = f"Erreur critique lors de l'extraction ISO (code {result.returncode}): {result.stderr}"
                    logger.error(error_msg)
                    return False, error_msg
                
                if result.returncode != 0:
                    logger.warning(f"7z a retourné un avertissement (code {result.returncode}): {result.stderr}")
                    logger.info("Extraction poursuivie malgré l'avertissement")
                
                logger.info(f"ISO extrait dans: {game_folder_path}")
                
            except FileNotFoundError:
                return False, "7z non trouvé - vérifiez que 7z.exe (Windows) ou 7zz (Linux) est présent dans assets/progs"
            except Exception as e:
                return False, f"Erreur lors de l'extraction ISO: {str(e)}"
            
            # Nettoyage
            os.remove(decrypted_iso_path)
            logger.debug(f"ISO décrypté supprimé: {decrypted_iso_path}")
            
            if dkey_path and os.path.exists(dkey_path):
                os.remove(dkey_path)
                logger.debug(f"Fichier .dkey supprimé: {dkey_path}")
            
            logger.info(f"Traitement PS3 Redump terminé avec succès: {game_folder_name}")
            return True, f"Jeu décrypté et extrait: {game_folder_name}"
            
        except Exception as e:
            error_msg = f"Erreur lors du traitement PS3 Redump: {str(e)}"
            logger.error(error_msg)
            return False, error_msg

    
def handle_dos(dest_dir, before_items, extracted_basename=None):
    """Gère l'organisation spécifique des dossiers DOS extraits.

    - Si le ZIP contient un seul dossier: extraire et renommer ce dossier en <nom_zip>.pc
    - Si le ZIP contient plusieurs fichiers/dossiers: déplacer tout dans un nouveau dossier <nom_zip>.pc
    
    Args:
        before_items: Set des éléments (fichiers+dossiers) présents avant extraction
    """
    logger.debug(f"Traitement spécifique DOS dans: {dest_dir}")
    time.sleep(2)  # petite latence post-extraction

    try:
        # Déterminer les nouveaux éléments extraits
        after_items = set(os.listdir(dest_dir))
    except Exception:
        after_items = set()

    ignore_names = {"dos", "images", "videos", "manuals", "media"}
    # Filtrer les nouveaux éléments (fichiers ou dossiers)
    new_items = [item for item in (after_items - before_items) 
                 if item not in ignore_names and not item.endswith('.pc')]

    if not new_items:
        logger.warning("Aucun nouveau contenu DOS détecté après extraction")
        return True, None

    if not extracted_basename:
        logger.warning("Nom de base du ZIP non fourni pour le traitement DOS")
        return True, None

    target_name = f"{extracted_basename}.pc"
    target_path = os.path.join(dest_dir, target_name)

    # Cas 1: Un seul dossier extrait -> le renommer en .pc
    if len(new_items) == 1:
        item_path = os.path.join(dest_dir, new_items[0])
        if os.path.isdir(item_path):
            logger.debug(f"DOS: Un seul dossier détecté '{new_items[0]}', renommage en '{target_name}'")
            max_retries = 3
            retry_delay = 2
            for attempt in range(max_retries):
                try:
                    # Fermer les handles potentiellement ouverts
                    for root, dirs, files in os.walk(item_path):
                        for f in files:
                            try:
                                os.chmod(os.path.join(root, f), 0o644)
                            except (OSError, PermissionError):
                                pass
                        for d in dirs:
                            try:
                                os.chmod(os.path.join(root, d), 0o755)
                            except (OSError, PermissionError):
                                pass

                    if os.path.exists(target_path):
                        shutil.rmtree(target_path, ignore_errors=True)
                        time.sleep(1)

                    os.rename(item_path, target_path)
                    logger.info(f"Dossier DOS renommé avec succès: {item_path} -> {target_path}")
                    return True, None

                except Exception as e:
                    logger.warning(f"Tentative {attempt + 1}/{max_retries} échouée: {str(e)}")
                    if attempt < max_retries - 1:
                        time.sleep(retry_delay)
                    else:
                        error_msg = f"Erreur lors du renommage DOS de {item_path} en {target_path}: {str(e)}"
                        logger.error(error_msg)
                        return False, error_msg

    # Cas 2: Un seul fichier OU plusieurs fichiers/dossiers -> créer un dossier .pc et tout y déplacer
    logger.debug(f"DOS: {len(new_items)} élément(s) détecté(s), création du dossier '{target_name}'")
    try:
        # Créer le dossier .pc s'il n'existe pas
        if os.path.exists(target_path):
            logger.warning(f"Le dossier {target_path} existe déjà, il sera remplacé")
            shutil.rmtree(target_path, ignore_errors=True)
            time.sleep(1)

        os.makedirs(target_path, exist_ok=True)

        # Déplacer tous les nouveaux éléments dans le dossier .pc
        for item in new_items:
            src_path = os.path.join(dest_dir, item)
            dst_path = os.path.join(target_path, item)
            
            try:
                if os.path.isdir(src_path):
                    shutil.move(src_path, dst_path)
                else:
                    shutil.move(src_path, dst_path)
                    os.chmod(dst_path, 0o644)
                logger.debug(f"Déplacé: {item} -> {target_name}/{item}")
            except Exception as e:
                logger.error(f"Erreur lors du déplacement de {item}: {str(e)}")
                return False, f"Erreur lors du déplacement de {item}: {str(e)}"

        logger.info(f"Contenu DOS organisé avec succès dans: {target_path}")
        return True, None

    except Exception as e:
        error_msg = f"Erreur lors de l'organisation DOS dans {target_path}: {str(e)}"
        logger.error(error_msg)
        return False, error_msg



def handle_scummvm(dest_dir, before_items, extracted_basename=None):
    """Gère l'organisation spécifique des jeux ScummVM extraits.
    
    - Crée un sous-dossier avec le nom du jeu (sans extension)
    - Extrait/déplace le contenu du ZIP dans ce dossier
    - Crée un fichier .scummvm vide avec le même nom
    
    Exemple: Freddi_fish_1.zip -> dossier Freddi_fish_1/ + fichier Freddi_fish_1.scummvm
    
    Args:
        dest_dir: Dossier de destination (scummvm)
        before_items: Set des éléments présents avant extraction
        extracted_basename: Nom de base du ZIP extrait (sans extension)
    """
    logger.debug(f"Traitement spécifique ScummVM dans: {dest_dir}")
    time.sleep(2)  # Petite latence post-extraction
    
    try:
        # Déterminer les nouveaux éléments extraits
        after_items = set(os.listdir(dest_dir))
    except Exception:
        after_items = set()
    
    ignore_names = {"scummvm", "images", "videos", "manuals", "media"}
    # Filtrer les nouveaux éléments (fichiers ou dossiers)
    new_items = [item for item in (after_items - before_items) 
                 if item not in ignore_names and not item.endswith('.scummvm')]
    
    if not new_items:
        logger.warning("Aucun nouveau contenu ScummVM détecté après extraction")
        return True, None
    
    if not extracted_basename:
        logger.warning("Nom de base du ZIP non fourni pour le traitement ScummVM")
        return True, None
    
    # Nom du dossier et du fichier .scummvm
    game_folder_name = extracted_basename
    game_folder_path = os.path.join(dest_dir, game_folder_name)
    scummvm_file_path = os.path.join(game_folder_path, f"{game_folder_name}.scummvm")
    
    try:
        # Créer le dossier du jeu s'il n'existe pas
        if os.path.exists(game_folder_path):
            logger.warning(f"Le dossier {game_folder_path} existe déjà, il sera utilisé")
        else:
            os.makedirs(game_folder_path, exist_ok=True)
            logger.debug(f"Dossier créé: {game_folder_path}")
        
        # Déplacer tous les nouveaux éléments dans le dossier du jeu
        for item in new_items:
            src_path = os.path.join(dest_dir, item)
            dst_path = os.path.join(game_folder_path, item)
            
            try:
                if os.path.isdir(src_path):
                    shutil.move(src_path, dst_path)
                else:
                    shutil.move(src_path, dst_path)
                logger.debug(f"Déplacé: {item} -> {game_folder_name}/{item}")
            except Exception as e:
                logger.error(f"Erreur déplacement {item}: {e}")
                return False, f"Erreur lors du déplacement de {item}: {str(e)}"
        
        # Créer le fichier .scummvm vide dans le sous-dossier
        try:
            with open(scummvm_file_path, 'w', encoding='utf-8') as f:
                pass  # Fichier vide
            logger.info(f"Fichier .scummvm créé: {scummvm_file_path}")
        except Exception as e:
            logger.error(f"Erreur création fichier .scummvm: {e}")
            return False, f"Erreur lors de la création du fichier .scummvm: {str(e)}"
        
        logger.info(f"Contenu ScummVM organisé avec succès: dossier {game_folder_name}/ avec fichier {game_folder_name}.scummvm à l'intérieur")
        return True, None
        
    except Exception as e:
        error_msg = f"Erreur lors de l'organisation ScummVM dans {game_folder_path}: {str(e)}"
        logger.error(error_msg)
        return False, error_msg



def handle_psvita(dest_dir, before_items, extracted_basename=None):
    """Gère l'organisation spécifique des jeux PSVita extraits.
    
    Structure attendue:
    - Archive RAR extraite → Dossier "Nom du jeu"/
    - Dans ce dossier → Fichier "IDJeu.zip" (ex: PCSE00890.zip)
    - Ce ZIP contient → Dossier "IDJeu" (ex: PCSE00890/)
    
    Actions:
    1. Créer fichier "Nom du jeu [IDJeu].psvita" dans dest_dir
    2. Extraire IDJeu.zip dans config.SAVE_FOLDER/psvita/ux0/app/
    3. Supprimer le dossier temporaire "Nom du jeu"/
    
    Args:
        dest_dir: Dossier de destination (psvita ou psvita/psvita)
        before_items: Set des éléments présents avant extraction
        extracted_basename: Nom de base de l'archive extraite (sans extension)
    """
    logger.debug(f"Traitement spécifique PSVita dans: {dest_dir}")
    time.sleep(2)  # Petite latence post-extraction
    
    try:
        after_items = set(os.listdir(dest_dir))
    except Exception:
        after_items = set()
    
    ignore_names = {"psvita", "images", "videos", "manuals", "media"}
    # Filtrer les nouveaux éléments (fichiers ou dossiers)
    new_items = [item for item in (after_items - before_items) 
                 if item not in ignore_names and not item.endswith('.psvita')]
    
    if not new_items:
        logger.warning("PSVita: Aucun nouveau dossier détecté après extraction")
        return True, None
    
    if not extracted_basename:
        extracted_basename = new_items[0] if new_items else "game"
    
    # Chercher le dossier du jeu (normalement il n'y en a qu'un)
    game_folder = None
    for item in new_items:
        item_path = os.path.join(dest_dir, item)
        if os.path.isdir(item_path):
            game_folder = item
            game_folder_path = item_path
            break
    
    if not game_folder:
        logger.error("PSVita: Aucun dossier de jeu trouvé après extraction")
        return False, "PSVita: Aucun dossier de jeu trouvé"
    
    logger.debug(f"PSVita: Dossier de jeu trouvé: {game_folder}")
    
    # Chercher le fichier ZIP à l'intérieur (IDJeu.zip)
    try:
        contents = os.listdir(game_folder_path)
        zip_files = [f for f in contents if f.lower().endswith('.zip')]
        
        if not zip_files:
            logger.error(f"PSVita: Aucun fichier ZIP trouvé dans {game_folder}")
            return False, f"PSVita: Aucun ZIP trouvé dans {game_folder}"
        
        # Prendre le premier ZIP trouvé
        zip_filename = zip_files[0]
        zip_path = os.path.join(game_folder_path, zip_filename)
        
        # Extraire l'ID du jeu (nom du ZIP sans extension)
        game_id = os.path.splitext(zip_filename)[0]
        logger.debug(f"PSVita: ZIP trouvé: {zip_filename}, ID du jeu: {game_id}")
        
        # 1. Créer le fichier .psvita dans dest_dir
        psvita_filename = f"{game_folder} [{game_id}].psvita"
        psvita_file_path = os.path.join(dest_dir, psvita_filename)
        
        try:
            # Créer un fichier vide .psvita
            with open(psvita_file_path, 'w', encoding='utf-8') as f:
                f.write(f"# PSVita Game\n")
                f.write(f"# Game: {game_folder}\n")
                f.write(f"# ID: {game_id}\n")
            logger.info(f"PSVita: Fichier .psvita créé: {psvita_filename}")
        except Exception as e:
            logger.error(f"PSVita: Erreur création fichier .psvita: {e}")
            return False, f"Erreur création {psvita_filename}: {e}"
        
        # 2. Extraire le ZIP dans le dossier parent de config.SAVE_FOLDER/psvita/ux0/app/
        save_parent2 = os.path.dirname(config.SAVE_FOLDER)
        save_parent = os.path.dirname(save_parent2)
        ux0_app_dir = os.path.join(save_parent, "psvita", "ux0", "app")
        os.makedirs(ux0_app_dir, exist_ok=True)
        
        logger.debug(f"PSVita: Extraction de {zip_filename} dans {ux0_app_dir}")
        
        try:
            import zipfile
            with zipfile.ZipFile(zip_path, 'r') as zip_ref:
                zip_ref.extractall(ux0_app_dir)
            logger.info(f"PSVita: ZIP extrait avec succès dans {ux0_app_dir}")
            
            # Vérifier que le dossier game_id existe bien
            game_id_path = os.path.join(ux0_app_dir, game_id)
            if not os.path.exists(game_id_path):
                logger.warning(f"PSVita: Le dossier {game_id} n'a pas été trouvé dans l'extraction")
            else:
                logger.info(f"PSVita: Dossier {game_id} confirmé dans ux0/app/")
        
        except zipfile.BadZipFile as e:
            logger.error(f"PSVita: Fichier ZIP corrompu: {e}")
            return False, f"ZIP corrompu: {zip_filename}"
        except Exception as e:
            logger.error(f"PSVita: Erreur extraction ZIP: {e}")
            return False, f"Erreur extraction {zip_filename}: {e}"
        
        # 3. Supprimer le dossier temporaire du jeu
        try:
            import shutil
            shutil.rmtree(game_folder_path)
            logger.info(f"PSVita: Dossier temporaire supprimé: {game_folder}")
        except Exception as e:
            logger.warning(f"PSVita: Impossible de supprimer {game_folder}: {e}")
            # Ne pas échouer pour ça, le jeu est quand même installé
        
        logger.info(f"PSVita: Traitement terminé avec succès - {psvita_filename} créé, {game_id} installé dans ux0/app/")
        return True, None
        
    except Exception as e:
        logger.error(f"PSVita: Erreur générale: {e}", exc_info=True)
        return False, f"Erreur PSVita: {str(e)}"



def handle_xbox(dest_dir, iso_files, url=None):
    """Gère la conversion des fichiers Xbox extraits et met à jour l'UI (Converting)."""
    logger.debug(f"Traitement spécifique Xbox dans: {dest_dir}")
    
    # Attendre un peu que tous les processus d'extraction se terminent
    time.sleep(2)
    if config.OPERATING_SYSTEM == "Windows":
        # Sur Windows; telecharger le fichier exe
        XISO_EXE = config.XISO_EXE
        extract_xiso_cmd = [XISO_EXE, "-r"]  # Liste avec 2 éléments

    else:
        # Linux/Batocera : télécharger le fichier xdvdfs  
        XISO_LINUX = config.XISO_LINUX
        try:
            stat_info = os.stat(XISO_LINUX)
            mode = stat_info.st_mode
            logger.debug(f"Permissions de {XISO_LINUX}: {oct(mode)}")
            logger.debug(f"Propriétaire: {stat_info.st_uid}, Groupe: {stat_info.st_gid}")
            
            # Vérifier si le fichier est exécutable
            if not os.access(XISO_LINUX, os.X_OK):
                logger.error(f"Le fichier {XISO_LINUX} n'est pas exécutable")
                try:
                    os.chmod(XISO_LINUX, 0o755)
                    logger.info(f"Permissions corrigées pour {XISO_LINUX}")
                except Exception as e:
                    logger.error(f"Impossible de modifier les permissions: {str(e)}")
                    return False, "Erreur de permissions sur xdvdfs"
        except Exception as e:
            logger.error(f"Erreur lors de la vérification des permissions: {str(e)}")
    
        extract_xiso_cmd = [XISO_LINUX, "-r"]  # Liste avec 2 éléments

    try:
        # Utiliser uniquement la liste fournie (nouveaux ISO extraits). Fallback scan uniquement si liste vide.
        provided_list = iso_files
        iso_files = []
        if isinstance(provided_list, (list, tuple)) and len(provided_list) > 0:
            # Normaliser/filtrer
            for p in provided_list:
                try:
                    if isinstance(p, str) and p.lower().endswith('.iso') and os.path.exists(p):
                        iso_files.append(os.path.abspath(p))
                except Exception:
                    continue
        else:
            # Fallback: scan (ancienne logique)
            for root, dirs, files in os.walk(dest_dir):
                for file in files:
                    if file.lower().endswith('.iso'):
                        iso_files.append(os.path.join(root, file))

        if not iso_files:
            logger.warning("Aucun fichier ISO xbox trouvé")
            return True, None

        total = len(iso_files)
        # Marquer l'état comme Conversion en cours (0%)
        try:
            if url:
                # Progress dict (pour l'écran en cours)
                if url not in config.download_progress:
                    config.download_progress[url] = {}
                config.download_progress[url]["status"] = "Converting"
                config.download_progress[url]["progress_percent"] = 0
                config.needs_redraw = True
                # Historique
                if isinstance(config.history, list):
                    for entry in config.history:
                        if entry.get("url") == url and entry.get("status") in ["Extracting", "Téléchargement", "Downloading"]:
                            entry["status"] = "Converting"
                            entry["progress"] = 0
                            entry["message"] = "Xbox conversion in progress"
                            save_history(config.history)
                            break
        except Exception as e:
            logger.debug(f"MAJ statut conversion ignorée: {e}")

        logger.info(f"Démarrage conversion Xbox: {total} ISO(s)")
        for idx, iso_xbox_source in enumerate(iso_files, start=1):
            logger.debug(f"Traitement de l'ISO Xbox: {iso_xbox_source}")
            
            # extract-xiso -r repackage l'ISO en place
            # Il faut exécuter la commande depuis le dossier contenant l'ISO
            iso_dir = os.path.dirname(iso_xbox_source)
            iso_filename = os.path.basename(iso_xbox_source)
            
            # Utiliser le nom de fichier relatif et définir le répertoire de travail
            cmd = extract_xiso_cmd + [iso_filename]
            logger.debug(f"Exécution de la commande: {' '.join(cmd)} (cwd: {iso_dir})")
            
            process = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=iso_dir
            )

            if process.returncode != 0:
                err_msg = f"Erreur lors de la conversion de l'ISO: {process.stderr}"
                logger.error(err_msg)
                # Mettre à jour les statuts pour éviter de rester bloqué en 'Converting'
                try:
                    if url:
                        if url not in config.download_progress:
                            config.download_progress[url] = {}
                        config.download_progress[url]["status"] = "Error"
                        config.download_progress[url]["message"] = process.stderr
                        config.download_progress[url]["progress_percent"] = 0
                        config.needs_redraw = True
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if entry.get("url") == url and entry.get("status") in ("Converting", "Extracting", "Téléchargement", "Downloading"):
                                    entry["status"] = "Error"
                                    entry["message"] = process.stderr
                                    save_history(config.history)
                                    break
                except Exception:
                    pass
                return False, err_msg

            # Vérifier que l'ISO existe toujours (extract-xiso le modifie en place)
            if os.path.exists(iso_xbox_source):
                logger.info(f"ISO repackagé avec succès: {iso_xbox_source}")
                logger.debug(f"ISO converti au format XISO en place")
                
                # Supprimer le fichier .old créé par extract-xiso (backup)
                old_file = iso_xbox_source + ".old"
                if os.path.exists(old_file):
                    try:
                        os.remove(old_file)
                        logger.debug(f"Fichier backup .old supprimé: {old_file}")
                    except Exception as e:
                        logger.warning(f"Impossible de supprimer le fichier .old: {e}")
                
                # Mise à jour progression de conversion (coarse-grain)
                try:
                    percent = int(idx / total * 100) if total > 0 else 100
                    if url:
                        if url not in config.download_progress:
                            config.download_progress[url] = {}
                        config.download_progress[url]["status"] = "Converting"
                        config.download_progress[url]["progress_percent"] = percent
                        config.needs_redraw = True
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if entry.get("url") == url and entry.get("status") == "Converting":
                                    entry["progress"] = percent
                                    save_history(config.history)
                                    break
                except Exception:
                    pass
            else:
                err_msg = f"L'ISO source a disparu après conversion: {iso_xbox_source}"
                logger.error(err_msg)
                try:
                    if url:
                        if url not in config.download_progress:
                            config.download_progress[url] = {}
                        config.download_progress[url]["status"] = "Error"
                        config.download_progress[url]["message"] = err_msg
                        config.download_progress[url]["progress_percent"] = 0
                        config.needs_redraw = True
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if entry.get("url") == url and entry.get("status") in ("Converting", "Extracting", "Téléchargement", "Downloading"):
                                    entry["status"] = "Error"
                                    entry["message"] = err_msg
                                    save_history(config.history)
                                    break
                except Exception:
                    pass
                return False, "Échec de la conversion de l'ISO"

        # Conversion terminée avec succès - mettre à jour le statut final
        try:
            if url:
                if url not in config.download_progress:
                    config.download_progress[url] = {}
                config.download_progress[url]["status"] = "Download_OK"
                config.download_progress[url]["progress_percent"] = 100
                config.needs_redraw = True
                if isinstance(config.history, list):
                    for entry in config.history:
                        if entry.get("url") == url and entry.get("status") == "Converting":
                            entry["status"] = "Download_OK"
                            entry["progress"] = 100
                            entry["message"] = "Xbox conversion completed successfully"
                            save_history(config.history)
                            break
        except Exception as e:
            logger.debug(f"MAJ statut final conversion ignorée: {e}")

        return True, "Conversion Xbox terminée avec succès"

    except Exception as e:
        logger.error(f"Erreur lors de la conversion Xbox: {str(e)}")
        return False, f"Erreur lors de la conversion: {str(e)}"
