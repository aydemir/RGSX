
"""grid module."""

import math
import os
import pygame  # type: ignore

import config

from language import _

from utils import (load_system_image, truncate_text_end, get_platform_source_badge_key, get_platform_source_badge_surface, get_disk_usage)

from .colors import THEME_COLORS
from .components import draw_header_badge, measure_header_badge, fit_badge_lines

import logging
logger = logging.getLogger(__name__)

platform_images_cache = {}
def get_platform_header_info_lines(max_badge_width=None, include_details=True):
    """Retourne les lignes du cartouche version/controleur/IP, adaptees a une largeur max."""
    lines = [f"v{config.app_version}"]

    if not include_details:
        return fit_badge_lines(lines, config.tiny_font, max_badge_width, padding_x=12)

    device_name = (getattr(config, 'controller_device_name', '') or '').strip()
    if device_name:
        lines.append(device_name)

    network_ip = ""
    system_info = getattr(config, 'SYSTEM_INFO', None)
    if isinstance(system_info, dict):
        network_ip = (system_info.get('network_ip', '') or '').strip()
    if network_ip:
        # Faz: son kullanıcıya bağlantı ipucu için ip:port formatında göster.
        # Port kaynağı: manager_port (config ya da rgsx_settings fallback).
        manager_port = getattr(config, 'manager_port', 0) or 0
        if not manager_port:
            try:
                from rgsx_settings import get_manager_port
                manager_port = get_manager_port()
            except Exception:
                manager_port = 0
        if manager_port and ':' not in network_ip:
            network_ip = f"{network_ip}:{manager_port}"
        lines.append(network_ip)

    return fit_badge_lines(lines, config.tiny_font, max_badge_width, padding_x=12)

def format_disk_size_gb(size_bytes):
    gb_value = size_bytes / (1024 ** 3)
    if gb_value >= 100:
        return f"{gb_value:.0f} GB"
    if gb_value >= 10:
        return f"{gb_value:.1f} GB"
    return f"{gb_value:.2f} GB"

def get_default_disk_space_line():
    """Retourne l'espace disque libre du dossier ROMs par defaut sous forme 'Disk : libre/total(percent libre)'."""
    try:
        target_path = getattr(config, 'ROMS_FOLDER', '') or ''
        if not target_path:
            return ""

        resolved_path = os.path.abspath(target_path)
        while resolved_path and not os.path.exists(resolved_path):
            parent_path = os.path.dirname(resolved_path)
            if not parent_path or parent_path == resolved_path:
                break
            resolved_path = parent_path

        if not os.path.exists(resolved_path):
            return ""

        usage = get_disk_usage(resolved_path)
        if usage is None:
            return ""
        free_bytes = max(0, usage.free)
        free_percent = int(round((free_bytes / usage.total) * 100)) if usage.total > 0 else 0
        free_label = _("disk_percent_free") if _ else "free"
        return f"[HDD] {format_disk_size_gb(free_bytes)}/{format_disk_size_gb(usage.total)} ({free_percent}% {free_label})"
    except Exception:
        return ""

def get_display_resolution_line():
    """Retourne la resolution d'affichage pour le cartouche gauche de la page plateformes."""
    try:
        system_info = getattr(config, 'SYSTEM_INFO', None)
        if isinstance(system_info, dict):
            display_resolution = (system_info.get('display_resolution', '') or '').strip()
            if display_resolution:
                return f"Res : {display_resolution}"
    except Exception:
        pass

    try:
        if getattr(config, 'screen_width', 0) and getattr(config, 'screen_height', 0):
            return f"Res : {config.screen_width}x{config.screen_height}"
    except Exception:
        pass

    return ""

def draw_platform_source_badge(screen, platform_name, container_rect):
    source_key = get_platform_source_badge_key(platform_name)
    if not source_key:
        return

    badge_size = max(20, min(int(min(container_rect.width, container_rect.height) * 0.24), 44))
    badge_surface = get_platform_source_badge_surface(source_key, badge_size)
    if badge_surface is None:
        return

    inset = max(5, badge_size // 6)
    badge_x = container_rect.right - badge_size - inset
    badge_y = container_rect.top + inset
    screen.blit(badge_surface, (badge_x, badge_y))

def draw_platform_header_info(screen, light_mode=False, badge_x=None, max_badge_width=None, include_details=True):
    """Affiche version, controleur connecte et IP reseau dans un cartouche en haut a droite."""
    lines = get_platform_header_info_lines(max_badge_width, include_details=include_details)
    badge_width, _, fitted_lines = measure_header_badge(lines, font=config.tiny_font, max_badge_width=max_badge_width)
    if not fitted_lines:
        return
    if badge_x is None:
        badge_x = config.screen_width - badge_width - 14
    badge_y = 10
    draw_header_badge(screen, fitted_lines, badge_x, badge_y, light_mode, font=config.tiny_font, max_badge_width=max_badge_width)

def get_platform_header_badge_layout(screen_width, left_lines=None, right_lines=None, center_min_width=None, header_margin_x=14, header_gap=None):
    """Calcule une repartition responsive des 3 cartouches d'en-tete avec priorite au cartouche droit."""
    if header_gap is None:
        header_gap = max(10, int(screen_width * 0.01))
    if center_min_width is None:
        center_min_width = max(160, int(screen_width * 0.18))

    left_lines = left_lines or []
    right_lines = right_lines or []

    available_width = screen_width - 2 * header_margin_x
    gap_count = (1 if left_lines else 0) + (1 if right_lines else 0)
    available_without_gaps = max(120, available_width - gap_count * header_gap)

    left_target = max(160, int(screen_width * 0.28)) if left_lines else 0
    right_target = max(220, int(screen_width * 0.26)) if right_lines else 0

    if left_lines and right_lines:
        max_side_total = max(120, available_without_gaps - center_min_width)
        desired_side_total = left_target + right_target
        if desired_side_total > max_side_total:
            scale = max_side_total / desired_side_total if desired_side_total > 0 else 1.0
            left_target = max(140, int(left_target * scale))
            right_target = max(180, int(right_target * scale))

            overflow = left_target + right_target - max_side_total
            if overflow > 0:
                left_reduction = min(overflow, max(0, left_target - 140))
                left_target -= left_reduction
                overflow -= left_reduction
            if overflow > 0:
                right_target = max(160, right_target - overflow)

    elif left_lines:
        left_target = max(160, min(left_target, available_without_gaps - center_min_width))
    elif right_lines:
        right_target = max(180, min(right_target, available_without_gaps - center_min_width))

    return {
        "header_gap": header_gap,
        "center_min_width": center_min_width,
        "left_max_width": left_target,
        "right_max_width": right_target,
    }

def draw_platform_grid(screen):
    """Affiche la grille des plateformes avec un style moderne et fluide."""
    global platform_images_cache
    
    # Vérifier si le mode performance est activé
    from rgsx_settings import get_light_mode
    light_mode = get_light_mode()
    
    if not config.platforms or config.selected_platform >= len(config.platforms):
        platform_name = _("platform_no_platform")
        logger.warning("Aucune plateforme ou selected_platform hors limites")
    else:
        platform = config.platforms[config.selected_platform]
        platform_name = config.platform_names.get(platform, platform)
    
    # Affichage du titre avec animation subtile
    # Afficher le nombre total de jeux disponibles (tous systèmes) pour cohérence avec l'écran jeux
    # Nombre de jeux pour la plateforme sélectionnée (utilise le cache pre-calculé si disponible)
    game_count = 0
    try:
        if hasattr(config, 'games_count') and isinstance(config.games_count, dict):
            game_count = config.games_count.get(platform_name, 0)
        # Fallback local sans fetch réseau pour éviter un chargement implicite pendant la navigation.
        if game_count == 0 and hasattr(config, 'platform_dict_by_name'):
            from utils import get_platform_game_count  # import local pour éviter import circulaire global
            game_count = get_platform_game_count(platform_name, allow_torrent_manifest_fetch=False)
    except Exception:
        game_count = 0
    title_text = f"{platform_name}  ({game_count})" if game_count > 0 else f"{platform_name}"

    header_margin_x = 14
    center_badge_min_width = max(160, int(config.screen_width * 0.18))
    header_y = 10
    num_cols = getattr(config, 'GRID_COLS', 3)
    num_rows = getattr(config, 'GRID_ROWS', 4)

    total_pages = 0
    left_badge_lines = []
    left_badge_width = 0
    left_badge_height = 0
    page_indicator_text = ""

    # Effet de pulsation subtil pour le titre - calculé une seule fois par frame
    current_time = pygame.time.get_ticks()

    visible_platforms = list(config.platforms)

    # Ajuster selected_platform et current_platform/page si liste réduite
    if config.selected_platform >= len(visible_platforms):
        config.selected_platform = max(0, len(visible_platforms) - 1)
    systems_per_page = num_cols * num_rows
    if systems_per_page <= 0:
        systems_per_page = 1
    config.current_page = config.selected_platform // systems_per_page if systems_per_page else 0

    total_pages = (len(visible_platforms) + systems_per_page - 1) // systems_per_page
    left_badge_candidate_lines = []
    if total_pages > 1:
        page_indicator_text = _("platform_page").format(config.current_page + 1, total_pages)
        left_badge_candidate_lines.append(page_indicator_text)

    disk_space_line = get_default_disk_space_line()
    if disk_space_line:
        left_badge_candidate_lines.append(disk_space_line)

    display_resolution_line = get_display_resolution_line()
    if display_resolution_line:
        left_badge_candidate_lines.append(display_resolution_line)

    right_badge_raw_lines = get_platform_header_info_lines(None, include_details=True)
    header_layout = get_platform_header_badge_layout(
        config.screen_width,
        left_lines=left_badge_candidate_lines,
        right_lines=right_badge_raw_lines,
        center_min_width=center_badge_min_width,
        header_margin_x=header_margin_x,
    )
    header_gap = header_layout["header_gap"]
    left_badge_max_width = header_layout["left_max_width"]
    right_badge_max_width = header_layout["right_max_width"]

    left_badge_width, left_badge_height, left_badge_lines = measure_header_badge(
        left_badge_candidate_lines,
        font=config.tiny_font,
        max_badge_width=left_badge_max_width,
    )

    right_badge_lines = get_platform_header_info_lines(right_badge_max_width, include_details=True)
    right_badge_width, right_badge_height, right_badge_lines = measure_header_badge(
        right_badge_lines,
        font=config.tiny_font,
        max_badge_width=right_badge_max_width,
    )

    center_left = header_margin_x + (left_badge_width + header_gap if left_badge_lines else 0)
    center_right = config.screen_width - header_margin_x - (right_badge_width + header_gap if right_badge_lines else 0)
    center_badge_max_width = max(center_badge_min_width, center_right - center_left)

    center_font_candidates = [config.title_font, config.search_font, config.font, config.small_font]
    center_font = config.small_font
    center_line = title_text
    center_padding_x = 18
    center_padding_y = 10
    center_line_gap = 4

    for candidate_font in center_font_candidates:
        raw_width = candidate_font.size(title_text)[0] + center_padding_x * 2
        if raw_width <= center_badge_max_width:
            center_font = candidate_font
            center_line = title_text
            break
    else:
        center_font = center_font_candidates[-1]
        center_line = truncate_text_end(title_text, center_font, max(80, center_badge_max_width - center_padding_x * 2))

    title_surface = center_font.render(center_line, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect()
    title_rect_inflated = title_rect.inflate(center_padding_x * 2, center_padding_y * 2)
    title_rect_inflated.x = center_left + max(0, (center_badge_max_width - title_rect_inflated.width) // 2)
    title_rect_inflated.y = header_y
    title_rect.center = title_rect_inflated.center

    if not light_mode:
        # Mode normal : effets visuels complets
        pulse_factor = 0.08 * (1 + math.sin(current_time / 400))
        
        # Ombre portée pour le titre
        shadow_surf = pygame.Surface((title_rect_inflated.width + 12, title_rect_inflated.height + 12), pygame.SRCALPHA)
        pygame.draw.rect(shadow_surf, (0, 0, 0, 140), (6, 6, title_rect_inflated.width, title_rect_inflated.height), border_radius=16)
        screen.blit(shadow_surf, (title_rect_inflated.left - 6, title_rect_inflated.top - 6))
        
        # Glow multicouche pour le titre
        for i in range(2):
            glow_size = title_rect_inflated.inflate(15 + i * 8, 15 + i * 8)
            title_glow = pygame.Surface((glow_size.width, glow_size.height), pygame.SRCALPHA)
            alpha = int((30 + 20 * pulse_factor) * (1 - i / 2))
            pygame.draw.rect(title_glow, (*THEME_COLORS["neon"][:3], alpha), 
                            title_glow.get_rect(), border_radius=16 + i * 2)
            screen.blit(title_glow, (title_rect_inflated.left - 8 - i * 4, title_rect_inflated.top - 8 - i * 4))
        
        # Fond du titre avec dégradé
        title_bg = pygame.Surface((title_rect_inflated.width, title_rect_inflated.height), pygame.SRCALPHA)
        for i in range(title_rect_inflated.height):
            ratio = i / title_rect_inflated.height
            alpha = int(THEME_COLORS["button_idle"][3] * (1 + ratio * 0.1))
            pygame.draw.line(title_bg, (*THEME_COLORS["button_idle"][:3], alpha), 
                            (0, i), (title_rect_inflated.width, i))
        screen.blit(title_bg, title_rect_inflated.topleft)
        
        # Reflet en haut du titre
        highlight = pygame.Surface((title_rect_inflated.width - 8, title_rect_inflated.height // 3), pygame.SRCALPHA)
        highlight.fill((255, 255, 255, 25))
        screen.blit(highlight, (title_rect_inflated.left + 4, title_rect_inflated.top + 4))
        
        pygame.draw.rect(screen, THEME_COLORS["border"], title_rect_inflated, 2, border_radius=14)
    else:
        # Mode performance : rendu simplifié
        pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_rect_inflated, border_radius=14)
        pygame.draw.rect(screen, THEME_COLORS["border"], title_rect_inflated, 2, border_radius=14)
    
    screen.blit(title_surface, title_rect)

    # Configuration de la grille - calculée une seule fois
    margin_left = int(config.screen_width * 0.026)
    margin_right = int(config.screen_width * 0.026)
    header_bottom = title_rect_inflated.bottom
    if left_badge_lines:
        header_bottom = max(header_bottom, header_y + left_badge_height)
    if right_badge_lines:
        header_bottom = max(header_bottom, header_y + right_badge_height)
    header_clearance = max(20, int(config.screen_height * 0.03))
    margin_top = max(int(config.screen_height * 0.140), header_bottom + header_clearance)
    footer_height = 70
    min_footer_gap = max(12, int(config.screen_height * 0.018))
    footer_reserved = max(footer_height + min_footer_gap, int(config.screen_height * 0.118))
    margin_bottom = footer_reserved
    systems_per_page = num_cols * num_rows

    available_width = config.screen_width - margin_left - margin_right
    available_height = config.screen_height - margin_top - margin_bottom

    # Calculer la taille des cellules en tenant compte de l'espace nécessaire pour le glow
    # Réduire la taille effective pour laisser de l'espace entre les éléments
    col_width = available_width // num_cols
    row_height = available_height // num_rows
    
    # Calculer la taille du container basée sur la cellule la plus petite
    # avec marges pour éviter les chevauchements (20% de marge)
    cell_size = min(col_width, row_height)
    container_size = int(cell_size * 0.70)  # 70% de la cellule pour laisser de l'espace
    
    # Espacement entre les cellules pour éviter les chevauchements
    cell_padding = int(cell_size * 0.15)  # 15% d'espacement

    x_positions = [margin_left + col_width * i + col_width // 2 for i in range(num_cols)]

    first_row_center = margin_top + row_height // 2
    last_row_center = config.screen_height - margin_bottom - row_height // 2
    if num_rows <= 1:
        y_positions = [margin_top + available_height // 2]
    elif last_row_center <= first_row_center:
        y_positions = [margin_top + row_height * i + row_height // 2 for i in range(num_rows)]
    else:
        row_step = (last_row_center - first_row_center) / (num_rows - 1)
        y_positions = [int(first_row_center + row_step * i) for i in range(num_rows)]

    if left_badge_lines:
        draw_header_badge(
            screen,
            left_badge_lines,
            header_margin_x,
            header_y,
            light_mode,
            font=config.tiny_font,
            max_badge_width=left_badge_max_width,
        )

    if right_badge_lines:
        right_badge_x = config.screen_width - right_badge_width - header_margin_x
        draw_platform_header_info(
            screen,
            light_mode,
            badge_x=right_badge_x,
            max_badge_width=right_badge_max_width,
            include_details=True,
        )

    # Calculer une seule fois la pulsation pour les éléments sélectionnés (réduite)
    if not light_mode:
        pulse = 0.05 * math.sin(current_time / 300)  # Réduit de 0.1 à 0.05
        glow_intensity = 40 + int(30 * math.sin(current_time / 300))
    else:
        pulse = 0
        glow_intensity = 0
    
    # Pré-calcul des images pour optimiser le rendu
    start_idx = config.current_page * systems_per_page
    for idx in range(start_idx, start_idx + systems_per_page):
        if idx >= len(visible_platforms):
            break
        grid_idx = idx - start_idx
        row = grid_idx // num_cols
        col = grid_idx % num_cols
        x = x_positions[col]
        y = y_positions[row]
        
        # Animation fluide pour l'item sélectionné (réduite pour éviter chevauchement)
        is_selected = idx == config.selected_platform
        if light_mode:
            # Mode performance : pas d'animation, taille fixe
            scale_base = 1.0
            scale = 1.0
        else:
            # Mode normal : animation réduite
            scale_base = 1.15 if is_selected else 1.0  # Réduit de 1.5 à 1.15
            scale = scale_base + pulse if is_selected else scale_base
            
        # Récupération robuste du dict via nom
        display_name = visible_platforms[idx]
        platform_dict = getattr(config, 'platform_dict_by_name', {}).get(display_name)
        if not platform_dict:
            # Fallback index brut
            # Chercher en parcourant platform_dicts pour correspondance nom
            for pd in config.platform_dicts:
                n = pd.get("platform_name") or pd.get("platform")
                if n == display_name:
                    platform_dict = pd
                    break
            else:
                continue
        platform_id = platform_dict.get("platform_name") or platform_dict.get("platform") or display_name
        
        # Utiliser le cache d'images pour éviter de recharger/redimensionner à chaque frame
        cache_key = f"{platform_id}_{scale:.2f}_{container_size}"
        if cache_key not in platform_images_cache:
            image = load_system_image(platform_dict)
            if image:
                orig_width, orig_height = image.get_width(), image.get_height()
                
                # Taille normalisée basée sur container_size calculé en fonction de la grille
                # Le scale affecte uniquement l'item sélectionné
                # Adapter la largeur en fonction du nombre de colonnes pour occuper ~25-30% de l'écran
                if num_cols == 3:
                    # En 3 colonnes, augmenter significativement la largeur (15% de l'écran par carte)
                    actual_container_width = int(config.screen_width * 0.15 * scale)
                elif num_cols == 4:
                    # En 4 colonnes, largeur plus modérée (10% de l'écran par carte)
                    actual_container_width = int(config.screen_width * 0.15 * scale)
                else:
                    # Par défaut, utiliser container_size * 1.3
                    actual_container_width = int(container_size * scale * 1.3)
                
                actual_container_height = int(container_size * scale)  # Hauteur normale
                
                # Calculer le ratio pour fit dans le container en gardant l'aspect ratio
                ratio = min(actual_container_width / orig_width, actual_container_height / orig_height)
                new_width = int(orig_width * ratio)
                new_height = int(orig_height * ratio)
                
                scaled_image = pygame.transform.smoothscale(image, (new_width, new_height))
                platform_images_cache[cache_key] = {
                    "image": scaled_image,
                    "width": new_width,
                    "height": new_height,
                    "container_width": actual_container_width,
                    "container_height": actual_container_height,
                    "last_used": current_time
                }
            else:
                continue
        
        # Récupérer les données du cache (que ce soit nouveau ou existant)
        if cache_key in platform_images_cache:
            platform_images_cache[cache_key]["last_used"] = current_time
            scaled_image = platform_images_cache[cache_key]["image"]
            new_width = platform_images_cache[cache_key]["width"]
            new_height = platform_images_cache[cache_key]["height"]
            container_width = platform_images_cache[cache_key]["container_width"]
            container_height = platform_images_cache[cache_key]["container_height"]
        else:
            continue
        
        image_rect = scaled_image.get_rect(center=(x, y))


        # Effet visuel moderne similaire au titre pour toutes les images
        border_radius = 12
        padding = 12
        
        # Utiliser la taille du container normalisé au lieu de la taille variable de l'image
        rect_width = container_width + 2 * padding
        rect_height = container_height + 2 * padding
        
        # Centrer le conteneur sur la position (x, y)
        container_left = x - rect_width // 2
        container_top = y - rect_height // 2
        
        if not light_mode:
            # Mode normal : effets visuels complets
            # Ombre portée
            shadow_surf = pygame.Surface((rect_width + 12, rect_height + 12), pygame.SRCALPHA)
            pygame.draw.rect(shadow_surf, (0, 0, 0, 160), (6, 6, rect_width, rect_height), border_radius=border_radius + 4)
            screen.blit(shadow_surf, (container_left - 6, container_top - 6))
            
            # Effet de glow multicouche pour l'item sélectionné
            if is_selected:
                neon_color = THEME_COLORS["neon"]
                
                # Glow multicouche (2 couches pour effet profondeur)
                for i in range(2):
                    glow_size = (rect_width + 15 + i * 8, rect_height + 15 + i * 8)
                    glow_surf = pygame.Surface(glow_size, pygame.SRCALPHA)
                    alpha = int((glow_intensity + 40) * (1 - i / 2))
                    pygame.draw.rect(glow_surf, neon_color + (alpha,), glow_surf.get_rect(), border_radius=border_radius + i * 2)
                    screen.blit(glow_surf, (container_left - 8 - i * 4, container_top - 8 - i * 4))
            
            # Fond avec dégradé vertical (similaire au titre)
            bg_surface = pygame.Surface((rect_width, rect_height), pygame.SRCALPHA)
            base_color = THEME_COLORS["button_idle"] if is_selected else THEME_COLORS["fond_image"]
            
            for i in range(rect_height):
                ratio = i / rect_height
                # Dégradé du haut (plus clair) vers le bas (plus foncé)
                alpha = int(base_color[3] * (1 + ratio * 0.15)) if len(base_color) > 3 else int(200 * (1 + ratio * 0.15))
                color = (*base_color[:3], min(255, alpha))
                pygame.draw.line(bg_surface, color, (0, i), (rect_width, i))
            
            screen.blit(bg_surface, (container_left, container_top))
            
            # Reflet en haut (highlight pour effet glossy)
            highlight_height = rect_height // 3
            highlight = pygame.Surface((rect_width - 8, highlight_height), pygame.SRCALPHA)
            highlight.fill((255, 255, 255, 35 if is_selected else 20))
            screen.blit(highlight, (container_left + 4, container_top + 4))
        else:
            # Mode performance : fond simple sans effets
            bg_color = THEME_COLORS["button_idle"] if is_selected else THEME_COLORS["fond_image"]
            pygame.draw.rect(screen, bg_color, (container_left, container_top, rect_width, rect_height), border_radius=border_radius)
        
        # Bordure
        if light_mode and is_selected:
            # Mode performance : bordure épaisse et très visible pour l'item sélectionné
            border_color = THEME_COLORS["neon"]  # Couleur verte bien visible
            border_width = 4  # Bordure plus épaisse
        elif not light_mode and is_selected:
            # Mode normal : bordure neon
            border_color = THEME_COLORS["neon"]
            border_width = 2
        else:
            # Non sélectionné : bordure standard
            border_color = THEME_COLORS["border"]
            border_width = 2
        
        border_rect = pygame.Rect(container_left, container_top, rect_width, rect_height)
        pygame.draw.rect(screen, border_color, border_rect, border_width, border_radius=border_radius)

        # Centrer l'image dans le container (l'image peut être plus petite que le container)
        centered_image_rect = scaled_image.get_rect(center=(x, y))
        
        # Affichage de l'image
        if light_mode:
            # Mode performance : pas d'effet de transparence
            screen.blit(scaled_image, centered_image_rect)
        else:
            # Mode normal : effet de transparence pour les items non sélectionnés
            if not is_selected:
                temp_image = scaled_image.copy()
                temp_image.set_alpha(220)
                screen.blit(temp_image, centered_image_rect)
            else:
                screen.blit(scaled_image, centered_image_rect)

        draw_platform_source_badge(screen, display_name, border_rect)
    
    # Nettoyer le cache périodiquement (garder seulement les images utilisées récemment)
    if len(platform_images_cache) > 50:  # Limite arbitraire pour éviter une croissance excessive
        current_time = pygame.time.get_ticks()
        cache_timeout = 30000  # 30 secondes
        keys_to_remove = [k for k, v in platform_images_cache.items() 
                         if current_time - v["last_used"] > cache_timeout]
        for key in keys_to_remove:
            del platform_images_cache[key]
