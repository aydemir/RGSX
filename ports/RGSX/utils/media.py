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

# Désactiver les logs DEBUG de urllib3 e requests pour supprimer les messages de connexion HTTP

_SOURCE_BADGE_CACHE = {}



def _platform_image_folders():
    return [
        config.IMAGES_FOLDER,
        os.path.join(config.APP_FOLDER, "assets", "images"),
        os.path.join(config.APP_FOLDER, "images"),
    ]



def _resolve_platform_image_path(platform_dict):
    platform_name = platform_dict.get("platform_name", "unknown")
    folder_name = platform_dict.get("folder") or ""

    candidates = []
    platform_image_field = (platform_dict.get("platform_image") or "").strip()
    if platform_image_field:
        candidates.append(platform_image_field)
    candidates.append(platform_name)
    if folder_name:
        candidates.append(folder_name)

    image_path = None
    for candidate in candidates:
        candidate_base = os.path.splitext(candidate)[0]
        for folder in _platform_image_folders():
            if not os.path.exists(folder):
                continue
            for ext in ('.png', '.jpg', '.jpeg', '.gif', '.webp'):
                test_path = os.path.join(folder, candidate_base + ext)
                if os.path.exists(test_path):
                    image_path = test_path
                    break
            if image_path:
                break
        if image_path:
            break

    if image_path:
        return image_path

    for folder in _platform_image_folders():
        default_path = os.path.join(folder, 'default.png')
        if os.path.exists(default_path):
            return default_path
    return None



def get_platform_source_badge_key(platform_name: str):
    text = str(platform_name).strip()
    if not text:
        return None
    match = re.search(r'\(([^()]+)\)\s*$', text)
    if not match:
        return None
    source = match.group(1).strip().lower()
    mapping = {
        'archive': 'Archive',
        'lolroms': 'LolRoms',
        'torrent': 'Torrent',
        '1fichier': '1Fichier',
        'vimms': 'Vimms',
        'edgeemu': 'EdgeEmu',
        'edgeemu.net': 'EdgeEmu',
    }
    return mapping.get(source)



def _load_svg_badge_surface(svg_path: str, size: int):
    if pygame is None:
        return None
    try:
        surf = pygame.image.load(svg_path)
        width, height = surf.get_size()
        if width <= 0 or height <= 0:
            return None
        scale = min(size / width, size / height)
        scaled = pygame.transform.smoothscale(
            surf,
            (max(1, int(width * scale)), max(1, int(height * scale))),
        )
        return _safe_convert_alpha(scaled)
    except Exception as exc:
        logger.debug(f"Badge SVG load failed for {svg_path}: {exc}")
        return None



def _safe_convert_alpha(surface):
    if pygame is None or surface is None:
        return surface
    try:
        if pygame.display.get_surface() is not None:
            return surface.convert_alpha()
    except Exception:
        pass
    return surface.copy()



def _build_text_badge_icon(label: str, size: int, color):
    if pygame is None:
        return None
    try:
        if not pygame.font.get_init():
            pygame.font.init()
        font_size = max(11, int(size * (0.34 if len(label) <= 2 else 0.26)))
        font = pygame.font.SysFont('arial', font_size, bold=True)
        surface = font.render(label, True, color)
        return _safe_convert_alpha(surface)
    except Exception as exc:
        logger.debug(f"Badge text render failed for {label}: {exc}")
        return None



def _build_1fichier_badge_icon(size: int):
    if pygame is None:
        return None
    try:
        if not pygame.font.get_init():
            pygame.font.init()

        width = max(size, int(size * 0.98))
        height = max(16, int(size * 0.90))
        surface = pygame.Surface((width, height), pygame.SRCALPHA)

        box_width = max(16, int(width * 0.88))
        box_height = max(14, int(height * 0.78))
        box_left = (width - box_width) // 2
        box_top = (height - box_height) // 2

        front = [
            (box_left + box_width * 0.10, box_top + box_height * 0.24),
            (box_left + box_width * 0.74, box_top + box_height * 0.24),
            (box_left + box_width * 0.74, box_top + box_height * 0.86),
            (box_left + box_width * 0.10, box_top + box_height * 0.86),
        ]
        right = [
            (box_left + box_width * 0.74, box_top + box_height * 0.24),
            (box_left + box_width * 0.92, box_top + box_height * 0.12),
            (box_left + box_width * 0.92, box_top + box_height * 0.70),
            (box_left + box_width * 0.74, box_top + box_height * 0.86),
        ]
        top = [
            (box_left + box_width * 0.10, box_top + box_height * 0.24),
            (box_left + box_width * 0.30, box_top + box_height * 0.04),
            (box_left + box_width * 0.92, box_top + box_height * 0.12),
            (box_left + box_width * 0.74, box_top + box_height * 0.24),
        ]

        pygame.draw.polygon(surface, (229, 133, 20), [(int(x), int(y)) for x, y in front])
        pygame.draw.polygon(surface, (191, 104, 8), [(int(x), int(y)) for x, y in right])
        pygame.draw.polygon(surface, (248, 177, 63), [(int(x), int(y)) for x, y in top])
        pygame.draw.lines(surface, (111, 61, 6), True, [(int(x), int(y)) for x, y in front], 1)
        pygame.draw.lines(surface, (111, 61, 6), True, [(int(x), int(y)) for x, y in right], 1)
        pygame.draw.lines(surface, (111, 61, 6), True, [(int(x), int(y)) for x, y in top], 1)

        accent_radius = max(1, int(box_height * 0.08))
        pygame.draw.circle(surface, (48, 48, 48), (int(box_left + box_width * 0.88), int(box_top + box_height * 0.08)), accent_radius)
        pygame.draw.circle(surface, (48, 48, 48), (int(box_left + box_width * 0.95), int(box_top + box_height * 0.22)), accent_radius)

        one_font = pygame.font.SysFont('arial', max(12, int(box_height * 0.60)), bold=True)
        f_font = pygame.font.SysFont('arial', max(12, int(box_height * 0.64)), bold=True)
        one_surface = one_font.render('1', True, (92, 92, 92))
        f_outline_surface = f_font.render('F', True, (32, 32, 32))
        f_surface = f_font.render('F', True, (236, 129, 18))

        total_text_width = one_surface.get_width() + f_surface.get_width() - max(1, int(box_height * 0.06))
        text_x = int(box_left + box_width * 0.16)
        max_text_x = box_left + box_width - total_text_width - max(2, int(box_width * 0.08))
        text_x = min(text_x, max_text_x)
        text_y = int(box_top + (box_height - max(one_surface.get_height(), f_surface.get_height())) / 2)
        surface.blit(one_surface, (text_x, text_y))
        f_x = text_x + one_surface.get_width() - max(1, int(box_height * 0.06))
        f_y = text_y - 1
        for offset_x, offset_y in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
            surface.blit(f_outline_surface, (f_x + offset_x, f_y + offset_y))
        surface.blit(f_surface, (f_x, f_y))
        return _safe_convert_alpha(surface)
    except Exception as exc:
        logger.debug(f"1Fichier badge render failed: {exc}")
        return None



def get_platform_source_badge_surface(source_key: str, badge_size: int):
    cache_key = (source_key, badge_size)
    if cache_key in _SOURCE_BADGE_CACHE:
        return _SOURCE_BADGE_CACHE[cache_key]

    if pygame is None:
        _SOURCE_BADGE_CACHE[cache_key] = None
        return None

    style_map = { 
        'Archive': {
            'svg': 'archive.svg',
            'bg': (255, 255, 255, 242),
            'border': (48, 48, 48, 235),
            'label': 'AR',
            'text': (35, 35, 35),
        },
        'LolRoms': {
            'svg': 'lolroms.svg',
            'bg': (255, 255, 255, 242),
            'border': (0, 255, 255, 230),
            'label': 'LOL',
            'text': (61, 19, 110),
        },
        'Vimms': {
            'svg': 'vimms.svg',
            'bg': (255, 255, 255, 242),
            'border': (208, 208, 208, 235),
            'label': 'VL',
            'text': (24, 77, 176),
        },
        'Torrent': {
            'svg': 'torrent.svg',
            'bg': (255, 255, 255, 242),
            'border': (97, 164, 64, 235),
            'label': 'TOR',
            'text': (37, 90, 24),
        },
        '1Fichier': {
            'svg': None,
            'bg': (255, 255, 255, 242),
            'border': (208, 208, 208, 235),
            'label': '1F',
            'text': (24, 77, 176),
        },
        'EdgeEmu': {
            'svg': 'edgeemu.svg',
            'bg': (255, 255, 255, 242),
            'border': (41, 126, 196, 235),
            'label': 'EMU',
            'text': (24, 77, 176),
            'icon_scale': 0.90,
        },
    }
    style = style_map.get(source_key)
    if not style:
        _SOURCE_BADGE_CACHE[cache_key] = None
        return None

    badge_surface = pygame.Surface((badge_size, badge_size), pygame.SRCALPHA)
    border_radius = max(10, badge_size // 4)
    pygame.draw.rect(badge_surface, style['bg'], (0, 0, badge_size, badge_size), border_radius=border_radius)

    inner_margin = max(3, badge_size // 14)
    inner_rect = pygame.Rect(inner_margin, inner_margin, badge_size - inner_margin * 2, badge_size - inner_margin * 2)
    pygame.draw.rect(badge_surface, (255, 255, 255, 50), inner_rect, border_radius=max(8, border_radius - 3))

    highlight_height = max(8, badge_size // 3)
    highlight = pygame.Surface((badge_size - 8, highlight_height), pygame.SRCALPHA)
    highlight.fill((255, 255, 255, 34))
    badge_surface.blit(highlight, (4, 4))
    pygame.draw.rect(badge_surface, style['border'], (0, 0, badge_size, badge_size), max(2, badge_size // 18), border_radius=border_radius)

    icon_surface = None
    svg_name = style.get('svg')
    icon_scale = float(style.get('icon_scale', 0.76))
    target_icon_size = max(22, int(badge_size * icon_scale))
    if svg_name:
        svg_path = os.path.join(config.APP_FOLDER, 'assets', 'images', svg_name)
        if os.path.exists(svg_path):
            icon_surface = _load_svg_badge_surface(svg_path, target_icon_size)
    elif source_key == '1Fichier':
        icon_surface = _build_1fichier_badge_icon(target_icon_size)
    if icon_surface is None:
        icon_surface = _build_text_badge_icon(style['label'], badge_size, style['text'])

    if icon_surface is not None:
        try:
            icon_shadow = pygame.mask.from_surface(icon_surface).to_surface(
                setcolor=(0, 0, 0, 90),
                unsetcolor=(0, 0, 0, 0),
            )
            icon_shadow.set_colorkey((0, 0, 0))
            shadow_rect = icon_shadow.get_rect(center=(badge_size // 2 + 2, badge_size // 2 + 2))
            badge_surface.blit(icon_shadow, shadow_rect)
        except Exception:
            pass
        icon_rect = icon_surface.get_rect(center=(badge_size // 2, badge_size // 2))
        badge_surface.blit(icon_surface, icon_rect)

    _SOURCE_BADGE_CACHE[cache_key] = badge_surface
    return badge_surface



def _apply_platform_source_badge(base_surface, platform_name: str):
    if pygame is None or base_surface is None:
        return base_surface
    source_key = get_platform_source_badge_key(platform_name)
    if not source_key:
        return base_surface

    composed = _safe_convert_alpha(base_surface)
    width, height = composed.get_size()
    badge_size = max(34, min(int(min(width, height) * 0.34), 180))
    badge_surface = get_platform_source_badge_surface(source_key, badge_size)
    if badge_surface is None:
        return composed

    margin = max(5, badge_size // 10)
    shadow = pygame.Surface((badge_size + 16, badge_size + 16), pygame.SRCALPHA)
    pygame.draw.rect(shadow, (0, 0, 0, 115), (8, 8, badge_size, badge_size), border_radius=max(10, badge_size // 4))
    shadow_x = max(0, width - badge_size - margin - 8)
    shadow_y = margin - 3
    composed.blit(shadow, (shadow_x, shadow_y))

    badge_x = max(0, width - badge_size - margin)
    badge_y = margin
    composed.blit(badge_surface, (badge_x, badge_y))
    return composed



def _surface_to_png_bytes(surface):
    if pygame is None or surface is None:
        return None
    try:
        raw_bytes = pygame.image.tobytes(surface, 'RGBA')
        if Image is not None:
            image = Image.frombytes('RGBA', surface.get_size(), raw_bytes)
            buffer = io.BytesIO()
            image.save(buffer, format='PNG')
            return buffer.getvalue()
    except Exception as exc:
        logger.debug(f"Surface to PNG conversion failed: {exc}")

    tmp_path = None
    try:
        fd, tmp_path = tempfile.mkstemp(suffix='.png')
        os.close(fd)
        pygame.image.save(surface, tmp_path)
        with open(tmp_path, 'rb') as handle:
            return handle.read()
    except Exception as exc:
        logger.debug(f"Surface temp export failed: {exc}")
        return None
    finally:
        if tmp_path and os.path.exists(tmp_path):
            try:
                os.remove(tmp_path)
            except Exception:
                pass



def get_platform_image_payload(platform_dict):
    image_path = _resolve_platform_image_path(platform_dict)
    if not image_path:
        return None, None

    platform_name = platform_dict.get('platform_name', 'unknown')
    try:
        if pygame is not None and get_platform_source_badge_key(platform_name):
            surface = _safe_convert_alpha(pygame.image.load(image_path))
            surface = _apply_platform_source_badge(surface, platform_name)
            png_bytes = _surface_to_png_bytes(surface)
            if png_bytes:
                return png_bytes, 'image/png'
    except Exception as exc:
        logger.debug(f"Badged image payload failed for {platform_name}: {exc}")

    ext = os.path.splitext(image_path)[1].lower()
    mime_types = {
        '.png': 'image/png',
        '.jpg': 'image/jpeg',
        '.jpeg': 'image/jpeg',
        '.gif': 'image/gif',
        '.webp': 'image/webp',
    }
    try:
        with open(image_path, 'rb') as handle:
            return handle.read(), mime_types.get(ext, 'image/png')
    except Exception as exc:
        logger.error(f"Unable to read platform image payload {image_path}: {exc}")
        return None, None


# Helper pour vérifier si pygame.mixer est disponible
def is_mixer_available():
    """Vérifie si pygame.mixer est disponible et initialisé."""
    try:
        return pygame is not None and hasattr(pygame, 'mixer') and pygame.mixer.get_init() is not None
    except (AttributeError, NotImplementedError):
        return False

    
def load_system_image(platform_dict):
    """Charge une image système avec la priorité suivante:
    1. platform_image explicite s'il est défini
    2. <platform_name>.png
    3. <folder>.png si disponible
    4. Recherche fallback dans le dossier images de l'app (APP_FOLDER/images) avec le même ordre
    5. default.png (dans SAVE_FOLDER/images), sinon default.png de l'app

    Cela évite d'échouer lorsque le nom affiché ne correspond pas au fichier image
    et respecte un mapping explicite fourni par systems_list.json."""
    platform_name = platform_dict.get("platform_name", "unknown")
    try:
        image_path = _resolve_platform_image_path(platform_dict)
        if not image_path:
            logger.error(f"Aucune image trouvée pour {platform_name}.")
            return None

        return _safe_convert_alpha(pygame.image.load(image_path))
    except Exception as e:
        logger.error(f"Erreur lors du chargement de l'image pour {platform_name} : {str(e)}")
        return None




def play_random_music(music_files, music_folder, current_music=None):
    if not getattr(config, "music_enabled", True) or not is_mixer_available():
        if is_mixer_available():
            pygame.mixer.music.stop()
        return current_music
    if music_files:
        # Éviter de rejouer la même musique consécutivement
        available_music = [f for f in music_files if f != current_music]
        if not available_music:  # Si une seule musique, on la reprend
            available_music = music_files
        music_file = random.choice(available_music)
        music_path = os.path.join(music_folder, music_file)
        logger.debug(f"Lecture de la musique : {music_path}")
        
        def load_and_play_music():
            try:
                if is_mixer_available():
                    pygame.mixer.music.load(music_path)
                    pygame.mixer.music.set_volume(0.5)
                    pygame.mixer.music.play(loops=0)  # Jouer une seule fois
                    pygame.mixer.music.set_endevent(pygame.USEREVENT + 1)  # Événement de fin
                    set_music_popup(music_file)  # Afficher le nom de la musique dans la popup
            except Exception as e:
                logger.error(f"Erreur lors du chargement de la musique {music_path}: {str(e)}")
        
        # Charger et jouer la musique dans un thread séparé pour éviter le blocage
        music_thread = threading.Thread(target=load_and_play_music, daemon=True)
        music_thread.start()
        
        return music_file  # Retourner la nouvelle musique pour mise à jour
    else:
        logger.debug("Aucune musique trouvée dans /RGSX/assets/music")
        return current_music


def set_music_popup(music_name):
    """Définit le nom de la musique à afficher dans la popup."""
    config.current_music_name = f"♬ {os.path.splitext(music_name)[0]}"  # Utilise l'emoji ♬ directement
    config.music_popup_start_time = pygame.time.get_ticks() / 1000  # Temps actuel en secondes
    config.needs_redraw = True  # Forcer le redraw pour afficher le nom de la musique


def load_music_config():
    """Charge la configuration musique depuis rgsx_settings.json."""
    try:
        settings = load_rgsx_settings()
        config.music_enabled = settings.get("music_enabled", True)
        return config.music_enabled
    except Exception as e:
        logger.error(f"Erreur lors du chargement de la configuration musique: {str(e)}")
    config.music_enabled = True
    return True


def save_music_config():
    """Sauvegarde la configuration musique dans rgsx_settings.json."""
    try:
        settings = load_rgsx_settings()
        settings["music_enabled"] = config.music_enabled
        save_rgsx_settings(settings)
        logger.debug(f"Configuration musique sauvegardée: {config.music_enabled}")
    except Exception as e:
        logger.error(f"Erreur lors de la sauvegarde de la configuration musique: {str(e)}")
