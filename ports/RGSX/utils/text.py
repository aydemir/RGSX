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



def get_clean_display_name(raw_name, platform_id=None):
    """Return a user-facing game title from a raw file/path entry."""
    text = str(raw_name or "").strip()
    if not text:
        return ""

    normalized = text.replace("\\", "/")
    leaf_name = normalized.rsplit("/", 1)[-1]
    display_name = Path(leaf_name).stem.strip()

    prefixes = []
    if platform_id:
        prefixes.append(str(platform_id).strip())
        platform_label = getattr(config, "platform_names", {}).get(platform_id)
        if platform_label:
            prefixes.append(str(platform_label).strip())

    for prefix in prefixes:
        if not prefix:
            continue
        pattern = rf"^{re.escape(prefix)}[\s\-_:]+"
        updated_name = re.sub(pattern, "", display_name, flags=re.IGNORECASE).strip()
        if updated_name:
            display_name = updated_name

    return display_name.strip(" -_/")


# Cache/process flags for extensions generation/loading


def _format_size_bytes(size_bytes: int) -> str:
    if size_bytes < 1024:
        return f"{size_bytes} B"

    value = float(size_bytes)
    for unit in ["KB", "MB", "GB", "TB", "PB"]:
        value /= 1024.0
        if value < 1024.0 or unit == "PB":
            return f"{value:.2f} {unit}"

    return f"{size_bytes} B"


def truncate_text_middle(text, font, max_width, is_filename=True):
    """Tronque le texte en insérant '...' au milieu, en préservant le début et la fin.
    Si is_filename=False, ne supprime pas l'extension."""
    # Supprimer l'extension uniquement si is_filename est True
    if is_filename:
        text = text.rsplit('.', 1)[0] if '.' in text else text
    text_width = font.size(text)[0]
    if text_width <= max_width:
        return text
    ellipsis = "..."
    ellipsis_width = font.size(ellipsis)[0]
    max_text_width = max_width - ellipsis_width
    if max_text_width <= 0:
        return ellipsis

    # Diviser la largeur disponible entre début et fin, en priorisant la fin
    chars = list(text)
    left = []
    right = []
    left_width = 0
    right_width = 0
    left_idx = 0
    right_idx = len(chars) - 1

    # Préserver plus de caractères à droite pour garder le '%'
    while left_idx <= right_idx and (left_width + right_width) < max_text_width:
        # Ajouter à droite en priorité
        if left_idx <= right_idx:
            right.insert(0, chars[right_idx])
            right_width = font.size(''.join(right))[0]
            if left_width + right_width > max_text_width:
                right.pop(0)
                break
            right_idx -= 1
        # Ajouter à gauche seulement si nécessaire
        if left_idx < right_idx:
            left.append(chars[left_idx])
            left_width = font.size(''.join(left))[0]
            if left_width + right_width > max_text_width:
                left.pop()
                break
            left_idx += 1

    # Reculer jusqu'à un espace pour éviter de couper un mot
    while left and left[-1] != ' ' and left_width + right_width > max_text_width:
        left.pop()
        left_width = font.size(''.join(left))[0] if left else 0
    while right and right[0] != ' ' and left_width + right_width > max_text_width:
        right.pop(0)
        right_width = font.size(''.join(right))[0] if right else 0

    return ''.join(left).rstrip() + ellipsis + ''.join(right).lstrip()


def truncate_text_end(text, font, max_width):
    """Tronque le texte à la fin pour qu'il tienne dans max_width avec la police donnée."""
    if not isinstance(text, str):
        logger.error(f"Texte non valide: {text}")
        return ""
    if not isinstance(font, pygame.font.Font):
        logger.error("Police non valide dans truncate_text_end")
        return text  # Retourne le texte brut si la police est invalide

    try:
        if font.size(text)[0] <= max_width:
            return text

        truncated = text
        while len(truncated) > 0 and font.size(truncated + "...")[0] > max_width:
            truncated = truncated[:-1]
        return truncated + "..." if len(truncated) < len(text) else text
    except Exception as e:
        logger.error(f"Erreur lors du rendu du texte '{text}': {str(e)}")
        return text  # Retourne le texte brut en cas d'erreur


def sanitize_filename(name):
    """Sanitise les noms de fichiers en remplaçant les caractères interdits."""
    return re.sub(r'[<>:"/\/\\|?*]', '_', name).strip()

    
def wrap_text(text, font, max_width):
    """Divise le texte en lignes pour respecter la largeur maximale, en coupant les mots longs si nécessaire."""
    if not isinstance(text, str):
        text = str(text) if text is not None else ""
    
    words = text.split(' ')
    lines = []
    current_line = ''
    
    for word in words:
        # Si le mot seul dépasse max_width, le couper caractère par caractère
        if font.render(word, True, (255, 255, 255)).get_width() > max_width:
            temp_line = current_line
            for char in word:
                test_line = temp_line + (' ' if temp_line else '') + char
                test_surface = font.render(test_line, True, (255, 255, 255))
                if test_surface.get_width() <= max_width:
                    temp_line = test_line
                else:
                    if temp_line:
                        lines.append(temp_line)
                    temp_line = char
            current_line = temp_line
        else:
            # Comportement standard pour les mots normaux
            test_line = current_line + (' ' if current_line else '') + word
            test_surface = font.render(test_line, True, (255, 255, 255))
            if test_surface.get_width() <= max_width:
                current_line = test_line
            else:
                if current_line:
                    lines.append(current_line)
                current_line = word
    
    if current_line:
        lines.append(current_line)
    
    return lines
