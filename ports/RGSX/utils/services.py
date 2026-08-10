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

from utils.media import is_mixer_available



VERSIONCLEAN_SERVICE_NAME = "versionclean"

VERSIONCLEAN_BACKUP_PATH = "/usr/bin/batocera-version.bak"



CONNECTION_STATUS_TTL_SECONDS = 120


 
def restart_application(delay_ms: int = 2000):
    """Schedule a restart with a visible popup; actual restart happens in the main loop.

    - Sets popup_restarting and schedules config.pending_restart_at = now + delay_ms.
    - Main loop (__main__) detects pending_restart_at and calls restart_application(0) to perform the execl.
    """
    try:
        # Show popup and schedule
        if hasattr(config, 'popup_message'):
            try:
                config.popup_message = _("popup_restarting")
            except Exception:
                config.popup_message = "Restarting..."
            config.popup_timer = max(config.popup_timer, int(delay_ms)) if hasattr(config, 'popup_timer') else int(delay_ms)
            config.menu_state = getattr(config, 'menu_state', 'restart_popup') or 'restart_popup'
            config.needs_redraw = True
        # Schedule actual restart in main loop
        now = pygame.time.get_ticks() if hasattr(pygame, 'time') else 0
        config.pending_restart_at = now + max(0, int(delay_ms))
        logger.debug(f"Redémarrage planifié dans {delay_ms} ms (pending_restart_at={getattr(config, 'pending_restart_at', 0)})")

        # If delay_ms is 0, perform immediately here
        if int(delay_ms) <= 0:
            try:
                try:
                    if is_mixer_available():
                        pygame.mixer.music.stop()
                except Exception:
                    pass
                try:
                    pygame.quit()
                except Exception:
                    pass
                exe = sys.executable or "python"
                os.execl(exe, exe, *sys.argv)
            except Exception as e:
                logger.exception(f"Failed to restart immediately: {e}")
    except Exception as e:
        logger.exception(f"Failed to schedule restart: {e}")



def _get_enabled_services():
    """Retourne la liste des services activés dans batocera-settings, ou None si indisponible."""
    try:
        result = subprocess.run(
            ["batocera-settings-get", "system.services"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode != 0:
            logger.warning(f"batocera-settings-get failed: {result.stderr}")
            return None
        return result.stdout.split()
    except FileNotFoundError:
        logger.warning("batocera-settings-get command not found")
        return None
    except Exception as e:
        logger.warning(f"Failed to read enabled services: {e}")
        return None



def _ensure_versionclean_service():
    """Installe et active versionclean si nécessaire.

    - Installe uniquement si le service n'est pas déjà présent.
    - Active uniquement si le service n'est pas déjà activé.
    - Démarre uniquement si le nettoyage n'est pas déjà appliqué.
    """
    try:
        if config.OPERATING_SYSTEM != "Linux":
            return (True, "Versionclean skipped (non-Linux)")

        services_dir = "/userdata/system/services"
        service_file = os.path.join(services_dir, VERSIONCLEAN_SERVICE_NAME)
        source_file = os.path.join(config.APP_FOLDER, "assets", "progs", VERSIONCLEAN_SERVICE_NAME)

        if not os.path.exists(service_file):
            try:
                os.makedirs(services_dir, exist_ok=True)
            except Exception as e:
                error_msg = f"Failed to create services directory: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)

            if not os.path.exists(source_file):
                error_msg = f"Source service file not found: {source_file}"
                logger.error(error_msg)
                return (False, error_msg)

            try:
                shutil.copy2(source_file, service_file)
                os.chmod(service_file, 0o755)
                logger.info(f"Versionclean service installed: {service_file}")
            except Exception as e:
                error_msg = f"Failed to copy versionclean service file: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
        else:
            logger.debug("Versionclean service already present, skipping install")

        enabled_services = _get_enabled_services()
        if enabled_services is None or VERSIONCLEAN_SERVICE_NAME not in enabled_services:
            try:
                result = subprocess.run(
                    ["batocera-services", "enable", VERSIONCLEAN_SERVICE_NAME],
                    capture_output=True,
                    text=True,
                    timeout=10,
                )
                if result.returncode != 0:
                    error_msg = f"batocera-services enable versionclean failed: {result.stderr}"
                    logger.error(error_msg)
                    return (False, error_msg)
                logger.debug(f"Versionclean enabled: {result.stdout}")
            except FileNotFoundError:
                error_msg = "batocera-services command not found"
                logger.error(error_msg)
                return (False, error_msg)
            except Exception as e:
                error_msg = f"Failed to enable versionclean: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
        else:
            logger.debug("Versionclean already enabled, skipping enable")

        if os.path.exists(VERSIONCLEAN_BACKUP_PATH):
            logger.debug("Versionclean already active (backup present), skipping start")
            return (True, "Versionclean already active")

        try:
            result = subprocess.run(
                ["batocera-services", "start", VERSIONCLEAN_SERVICE_NAME],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if result.returncode != 0:
                logger.warning(f"batocera-services start versionclean warning: {result.stderr}")
            else:
                logger.debug(f"Versionclean started: {result.stdout}")
        except Exception as e:
            logger.warning(f"Failed to start versionclean (non-critical): {str(e)}")

        return (True, "Versionclean ensured")

    except Exception as e:
        error_msg = f"Unexpected versionclean error: {str(e)}"
        logger.exception(error_msg)
        return (False, error_msg)



def toggle_web_service_at_boot(enable: bool):
    """Active ou désactive le service web au démarrage de Batocera.
    
    Args:
        enable: True pour activer, False pour désactiver
        
    Returns:
        tuple: (success: bool, message: str)
    """

    
    try:
        # Vérifier si on est sur un système compatible (Linux avec batocera-services)
        if config.OPERATING_SYSTEM != "Linux":
            return (False, "Web service auto-start is only available on Batocera/Linux systems")
        
        services_dir = "/userdata/system/services"
        service_file = os.path.join(services_dir, "rgsx_web")
        source_file = os.path.join(config.APP_FOLDER, "assets", "progs", "rgsx_web")
        
        if enable:
            # Mode ENABLE
            logger.debug("Activation du service web au démarrage...")
            
            # 1. Créer le dossier services s'il n'existe pas
            try:
                os.makedirs(services_dir, exist_ok=True)
                logger.debug(f"Dossier services vérifié/créé: {services_dir}")
            except Exception as e:
                error_msg = f"Failed to create services directory: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)

            # 1b. Assurer versionclean (install/enable/start si nécessaire)
            ensure_ok, ensure_msg = _ensure_versionclean_service()
            if not ensure_ok:
                return (False, ensure_msg)
            
            # 2. Copier le fichier rgsx_web
            try:
                if not os.path.exists(source_file):
                    error_msg = f"Source service file not found: {source_file}"
                    logger.error(error_msg)
                    return (False, error_msg)
                
                shutil.copy2(source_file, service_file)
                os.chmod(service_file, 0o755)  # Rendre exécutable
                logger.debug(f"Fichier service copié et rendu exécutable: {service_file}")
            except Exception as e:
                error_msg = f"Failed to copy service file: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            # 3. Activer le service avec batocera-services
            try:
                result = subprocess.run(
                    ['batocera-services', 'enable', 'rgsx_web'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    error_msg = f"batocera-services enable failed: {result.stderr}"
                    logger.error(error_msg)
                    return (False, error_msg)
                logger.debug(f"Service activé: {result.stdout}")
            except FileNotFoundError:
                error_msg = "batocera-services command not found"
                logger.error(error_msg)
                return (False, error_msg)
            except Exception as e:
                error_msg = f"Failed to enable service: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            # 4. Démarrer le service immédiatement
            try:
                result = subprocess.run(
                    ['batocera-services', 'start', 'rgsx_web'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    # Le service peut ne pas démarrer si déjà en cours, ce n'est pas grave
                    logger.warning(f"batocera-services start warning: {result.stderr}")
                else:
                    logger.debug(f"Service démarré: {result.stdout}")
            except Exception as e:
                logger.warning(f"Failed to start service (non-critical): {str(e)}")
            
            success_msg = _("settings_web_service_success_enabled") if _ else "Web service enabled at boot"
            logger.info(success_msg)
            
            # Sauvegarder l'état dans rgsx_settings.json            
            settings = load_rgsx_settings()
            settings["web_service_at_boot"] = True
            save_rgsx_settings(settings)
            
            return (True, success_msg)
            
        else:
            # Mode DISABLE
            logger.debug("Désactivation du service web au démarrage...")
            
            # 1. Désactiver le service avec batocera-services
            try:
                result = subprocess.run(
                    ['batocera-services', 'disable', 'rgsx_web'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    error_msg = f"batocera-services disable failed: {result.stderr}"
                    logger.error(error_msg)
                    return (False, error_msg)
                logger.debug(f"Service désactivé: {result.stdout}")
            except FileNotFoundError:
                error_msg = "batocera-services command not found"
                logger.error(error_msg)
                return (False, error_msg)
            except Exception as e:
                error_msg = f"Failed to disable service: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            success_msg = _("settings_web_service_success_disabled") if _ else "✓ Web service disabled at boot"
            logger.info(success_msg)
            
            # Sauvegarder l'état dans rgsx_settings.json
            settings = load_rgsx_settings()
            settings["web_service_at_boot"] = False
            save_rgsx_settings(settings)
            
            return (True, success_msg)
            
    except Exception as e:
        error_msg = f"Unexpected error: {str(e)}"
        logger.exception(error_msg)
        return (False, error_msg)



def toggle_custom_dns_at_boot(enable: bool):
    """Active ou désactive le service custom DNS au démarrage de Batocera.
    
    Args:
        enable: True pour activer, False pour désactiver
        
    Returns:
        tuple: (success: bool, message: str)
    """
    try:
        # Vérifier si on est sur un système compatible (Linux avec batocera-services)
        if config.OPERATING_SYSTEM != "Linux":
            return (False, "Custom DNS service is only available on Batocera/Linux systems")
        
        services_dir = "/userdata/system/services"
        service_file = os.path.join(services_dir, "custom_dns")
        source_file = os.path.join(config.APP_FOLDER, "assets", "progs", "custom_dns")
        
        if enable:
            # Mode ENABLE
            logger.debug("Activation du service custom DNS au démarrage...")
            
            # 1. Créer le dossier services s'il n'existe pas
            try:
                os.makedirs(services_dir, exist_ok=True)
                logger.debug(f"Dossier services vérifié/créé: {services_dir}")
            except Exception as e:
                error_msg = f"Failed to create services directory: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)

            # 1b. Assurer versionclean (install/enable/start si nécessaire)
            ensure_ok, ensure_msg = _ensure_versionclean_service()
            if not ensure_ok:
                return (False, ensure_msg)
            
            # 2. Copier le fichier custom_dns
            try:
                if not os.path.exists(source_file):
                    error_msg = f"Source service file not found: {source_file}"
                    logger.error(error_msg)
                    return (False, error_msg)
                
                shutil.copy2(source_file, service_file)
                os.chmod(service_file, 0o755)  # Rendre exécutable
                logger.debug(f"Fichier service copié et rendu exécutable: {service_file}")
            except Exception as e:
                error_msg = f"Failed to copy service file: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            # 3. Activer le service avec batocera-services
            try:
                result = subprocess.run(
                    ['batocera-services', 'enable', 'custom_dns'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    error_msg = f"batocera-services enable failed: {result.stderr}"
                    logger.error(error_msg)
                    return (False, error_msg)
                logger.debug(f"Service activé: {result.stdout}")
            except FileNotFoundError:
                error_msg = "batocera-services command not found"
                logger.error(error_msg)
                return (False, error_msg)
            except Exception as e:
                error_msg = f"Failed to enable service: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            # 4. Démarrer le service immédiatement
            try:
                result = subprocess.run(
                    ['batocera-services', 'start', 'custom_dns'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    # Le service peut ne pas démarrer si déjà en cours, ce n'est pas grave
                    logger.warning(f"batocera-services start warning: {result.stderr}")
                else:
                    logger.debug(f"Service démarré: {result.stdout}")
            except Exception as e:
                logger.warning(f"Failed to start service (non-critical): {str(e)}")
            
            success_msg = _("settings_custom_dns_success_enabled") if _ else "Custom DNS enabled at boot"
            logger.info(success_msg)
            
            # Sauvegarder l'état dans rgsx_settings.json
            settings = load_rgsx_settings()
            settings["custom_dns_at_boot"] = True
            save_rgsx_settings(settings)
            
            return (True, success_msg)
            
        else:
            # Mode DISABLE
            logger.debug("Désactivation du service custom DNS au démarrage...")
            
            # 1. Désactiver le service avec batocera-services
            try:
                result = subprocess.run(
                    ['batocera-services', 'disable', 'custom_dns'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    error_msg = f"batocera-services disable failed: {result.stderr}"
                    logger.error(error_msg)
                    return (False, error_msg)
                logger.debug(f"Service désactivé: {result.stdout}")
            except FileNotFoundError:
                error_msg = "batocera-services command not found"
                logger.error(error_msg)
                return (False, error_msg)
            except Exception as e:
                error_msg = f"Failed to disable service: {str(e)}"
                logger.error(error_msg)
                return (False, error_msg)
            
            # 2. Arrêter le service immédiatement
            try:
                result = subprocess.run(
                    ['batocera-services', 'stop', 'custom_dns'],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode != 0:
                    logger.warning(f"batocera-services stop warning: {result.stderr}")
                else:
                    logger.debug(f"Service arrêté: {result.stdout}")
            except Exception as e:
                logger.warning(f"Failed to stop service (non-critical): {str(e)}")
            
            success_msg = _("settings_custom_dns_success_disabled") if _ else "✓ Custom DNS disabled at boot"
            logger.info(success_msg)
            
            # Sauvegarder l'état dans rgsx_settings.json
            settings = load_rgsx_settings()
            settings["custom_dns_at_boot"] = False
            save_rgsx_settings(settings)
            
            return (True, success_msg)
            
    except Exception as e:
        error_msg = f"Unexpected error: {str(e)}"
        logger.exception(error_msg)
        return (False, error_msg)



def check_custom_dns_status():
    """Vérifie si le service custom DNS est activé au démarrage.
    
    Returns:
        bool: True si activé, False sinon
    """
    try:
        if config.OPERATING_SYSTEM != "Linux":
            return False
        
        # Lire l'état depuis rgsx_settings.json
        settings = load_rgsx_settings()
        return settings.get("custom_dns_at_boot", False)
        
    except Exception as e:
        logger.debug(f"Failed to check custom DNS status: {e}")
        return False



def get_connection_status_targets():
    """Retourne la liste des sites à vérifier pour le status de connexion."""
    default_targets = [
        {
            "key": "retrogamesets",
            "label": "Retrogamesets.fr",
            "url": "https://retrogamesets.fr",
            "category": "updates",
        },
        {
            "key": "github",
            "label": "GitHub.com",
            "url": "https://github.com",
            "category": "updates",
        },
        {
            "key": "myrient",
            "label": "Myrient.erista.me",
            "url": "https://myrient.erista.me",
            "category": "sources",
        },
        {
            "key": "1fichier",
            "label": "1fichier.com",
            "url": "https://1fichier.com",
            "category": "sources",
        },
        {
            "key": "archive",
            "label": "Archive.org",
            "url": "https://archive.org",
            "category": "sources",
        },
    ]

    configured = getattr(config, "CONNECTION_STATUS_TARGETS", None)
    if not isinstance(configured, list):
        return default_targets

    normalized = []
    seen_keys = set()
    for index, item in enumerate(configured):
        if not isinstance(item, dict):
            continue
        raw_key = str(item.get("key", "")).strip().lower()
        key = raw_key if raw_key else f"target_{index + 1}"
        if key in seen_keys:
            continue

        url = str(item.get("url", "")).strip()
        if not url:
            continue

        label = str(item.get("label", "")).strip() or url
        category = str(item.get("category", "sources")).strip().lower() or "sources"

        normalized.append({
            "key": key,
            "label": label,
            "url": url,
            "category": category,
        })
        seen_keys.add(key)

    return normalized if normalized else default_targets



def _check_url_connectivity(url: str, timeout: int = 6) -> bool:
    """Teste rapidement la connectivité à une URL (DNS + HTTPS)."""
    headers = {"User-Agent": "RGSX-Connectivity/1.0"}
    try:
        try:
           

            try:
                response = requests.head(url, timeout=timeout, allow_redirects=True, headers=headers)
                if response.status_code < 500:
                    return True
            except Exception:
                pass
            try:
                response = requests.get(url, timeout=timeout, allow_redirects=True, stream=True, headers=headers)
                return response.status_code < 500
            except Exception:
                return False
        except Exception:
            import urllib.request

            try:
                req = urllib.request.Request(url, method="HEAD", headers=headers)
                with urllib.request.urlopen(req, timeout=timeout) as resp:
                    return resp.status < 500
            except Exception:
                try:
                    req = urllib.request.Request(url, method="GET", headers=headers)
                    with urllib.request.urlopen(req, timeout=timeout) as resp:
                        return resp.status < 500
                except Exception:
                    return False
    except Exception:
        return False



def start_connection_status_check(force: bool = False) -> None:
    """Lance un check asynchrone des sites (avec cache/TTL)."""
    try:
        now = time.time()
        if getattr(config, "connection_status_in_progress", False):
            return
        last_ts = getattr(config, "connection_status_timestamp", 0.0) or 0.0
        if not force and last_ts and now - last_ts < CONNECTION_STATUS_TTL_SECONDS:
            return

        targets = get_connection_status_targets()
        status = getattr(config, "connection_status", {})
        if not isinstance(status, dict):
            status = {}
        if not status:
            for item in targets:
                status[item["key"]] = None
        config.connection_status = status
        config.connection_status_in_progress = True
        config.connection_status_progress = {"done": 0, "total": len(targets)}

        def _worker():
            try:
                results = {}
                done = 0
                total = len(targets)
                for item in targets:
                    results[item["key"]] = _check_url_connectivity(item["url"])
                    done += 1
                    config.connection_status_progress = {"done": done, "total": total}
                    try:
                        config.needs_redraw = True
                    except Exception:
                        pass
                config.connection_status.update(results)
                config.connection_status_timestamp = time.time()
                try:
                    summary = ", ".join([f"{k}={'OK' if v else 'FAIL'}" for k, v in results.items()])
                    logger.info(f"Connection status results: {summary}")
                except Exception:
                    pass
                try:
                    config.needs_redraw = True
                except Exception:
                    pass
            except Exception as e:
                logger.debug(f"Connection status check failed: {e}")
            finally:
                config.connection_status_in_progress = False

        threading.Thread(target=_worker, daemon=True).start()
    except Exception as e:
        logger.debug(f"Failed to start connection status check: {e}")



def get_connection_status_snapshot():
    """Retourne (status_dict, timestamp, in_progress, progress)."""
    status = getattr(config, "connection_status", {})
    if not isinstance(status, dict):
        status = {}
    ts = getattr(config, "connection_status_timestamp", 0.0) or 0.0
    in_progress = getattr(config, "connection_status_in_progress", False)
    progress = getattr(config, "connection_status_progress", {"done": 0, "total": 0})
    if not isinstance(progress, dict):
        progress = {"done": 0, "total": 0}
    return status, ts, in_progress, progress


def check_web_service_status():
    """Vérifie si le service web est activé au démarrage.
    
    Returns:
        bool: True si activé, False sinon
    """
    try:
        if config.OPERATING_SYSTEM != "Linux":
            return False
        
        # Lire l'état depuis rgsx_settings.json
       
        settings = load_rgsx_settings()
        return settings.get("web_service_at_boot", False)
        
    except Exception as e:
        logger.debug(f"Failed to check web service status: {e}")
        return False
