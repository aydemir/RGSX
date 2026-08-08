"""
Platform selection transition animation.
"""

import math
import pygame  # type: ignore

import config
from utils import load_system_image
from .background import draw_app_background
from .colors import THEME_COLORS


def draw_validation_transition(screen: pygame.Surface, platform_index: int) -> None:
    """Show smooth transition animation for platform selection."""
    if platform_index < 0 or platform_index >= len(config.platforms):
        return
    platform_name = config.platforms[platform_index]
    platform_dict = getattr(config, 'platform_dict_by_name', {}).get(platform_name)
    if not platform_dict:
        try:
            platform_dict = config.platform_dicts[platform_index]
        except Exception:
            return
    image = load_system_image(platform_dict)
    if not image:
        return

    orig_width, orig_height = image.get_width(), image.get_height()
    base_size = int(config.screen_width * 0.0781)
    ratio = min(base_size / orig_width, base_size / orig_height)
    base_width = int(orig_width * ratio)
    base_height = int(orig_height * ratio)

    start_time = pygame.time.get_ticks()
    duration = 1000
    fps = 60
    frame_time = 1000 / fps

    while pygame.time.get_ticks() - start_time < duration:
        draw_app_background(screen)

        elapsed = pygame.time.get_ticks() - start_time
        progress = elapsed / duration
        scale = 1.5 + 1.0 * math.sin(math.pi * progress)
        new_width = int(base_width * scale)
        new_height = int(base_height * scale)

        scaled_image = pygame.transform.smoothscale(image, (new_width, new_height))
        image_rect = scaled_image.get_rect(center=(config.screen_width // 2, config.screen_height // 2))

        alpha = int(128 + 127 * math.cos(math.pi * progress))
        scaled_image.set_alpha(alpha)

        neon_color = THEME_COLORS["neon"]
        padding = 24
        neon_surface = pygame.Surface((new_width + 2 * padding, new_height + 2 * padding), pygame.SRCALPHA)
        pygame.draw.rect(neon_surface, neon_color + (40,), neon_surface.get_rect(), border_radius=24)
        pygame.draw.rect(neon_surface, neon_color + (100,), neon_surface.get_rect().inflate(-10, -10), border_radius=18)
        screen.blit(neon_surface, (image_rect.left - padding, image_rect.top - padding), special_flags=pygame.BLEND_RGBA_ADD)

        screen.blit(scaled_image, image_rect)
        pygame.display.flip()

        pygame.time.wait(int(frame_time))

    draw_app_background(screen)
    final_image = pygame.transform.smoothscale(image, (base_width, base_height))
    final_image.set_alpha(255)
    final_rect = final_image.get_rect(center=(config.screen_width // 2, config.screen_height // 2))
    screen.blit(final_image, final_rect)
    pygame.display.flip()