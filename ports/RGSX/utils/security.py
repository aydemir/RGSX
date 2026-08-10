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



_REDACTED_PLACEHOLDER = "<redacted>"

_SENSITIVE_SETTING_KEY_RE = re.compile(
    r"(password|passwd|secret|token|credential|api[_-]?key|(?:^|[_\-])key$)",
    re.IGNORECASE,
)



def _is_sensitive_setting_key(key) -> bool:
    """Hassas ayar anahtarı mı? (password/secret/token/api_key vb.)"""
    return bool(_SENSITIVE_SETTING_KEY_RE.search(str(key)))



def redact_sensitive_settings(data):
    """Ayarlar ağacındaki hassas alan değerlerini <redacted> ile değiştiren kopyayı döndürür."""
    if isinstance(data, dict):
        return {
            key: (_REDACTED_PLACEHOLDER if _is_sensitive_setting_key(key) else redact_sensitive_settings(value))
            for key, value in data.items()
        }
    if isinstance(data, list):
        return [redact_sensitive_settings(item) for item in data]
    return data



def _redact_settings_file_text(file_path: str) -> str:
    """rgsx_settings.json içeriğini redakte edilmiş JSON metnine dönüştürür (disk dosyası değişmez)."""
    with open(file_path, 'r', encoding='utf-8') as f:
        raw_text = f.read()
    try:
        data = json.loads(raw_text)
    except Exception as exc:
        logger.warning(f"generate_support_zip: {file_path} parse edilemedi, ham içerik eklenecek: {exc}")
        return raw_text
    return json.dumps(redact_sensitive_settings(data), indent=2, ensure_ascii=False)



def generate_support_zip():
    """Génère un fichier ZIP contenant tous les fichiers de support pour le diagnostic.
    
    Returns:
        tuple: (success: bool, message: str, zip_path: str ou None)
    """

    
    try:
        # Créer un fichier ZIP temporaire
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        zip_filename = f"rgsx_support_{timestamp}.zip"
        zip_path = os.path.join(config.SAVE_FOLDER, zip_filename)
        
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
        if hasattr(config, 'log_dir'):
            web_log = os.path.join(config.log_dir, 'rgsx_web.log')
            if os.path.exists(web_log):
                files_to_include.append(('rgsx_web.log', web_log))
            
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
        
        logger.info(f"Fichier de support généré: {zip_path}")
        return (True, f"Support file created: {zip_filename}", zip_path)
        
    except Exception as e:
        logger.error(f"Erreur lors de la génération du fichier de support: {e}")
        return (False, str(e), None)
