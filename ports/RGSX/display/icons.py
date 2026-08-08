"""
SVG icon loading and footer control rendering.
"""

import io
import os
import pygame  # type: ignore

import config
from utils import truncate_text_end, wrap_text
from .fonts import get_badge_font
from .colors import THEME_COLORS

_HELP_ICON_CACHE: dict = {}


def clear_help_icon_cache() -> None:
    """Clear the help icon surface cache (call after icon mapping changes)."""
    _HELP_ICON_CACHE.clear()


def _images_base_dir() -> str:
    """Get base directory for SVG icon assets."""
    try:
        return os.path.join(os.path.dirname(__file__), "..", "assets", "images")
    except Exception:
        return "assets/images"


def _action_icon_filename(action_name: str) -> str | None:
    """Map action name to SVG filename based on Nintendo layout setting."""
    is_nintendo = getattr(config, 'nintendo_layout', False)
    if is_nintendo:
        mapping = {
            "up": "dpad_up.svg",
            "down": "dpad_down.svg",
            "left": "dpad_left.svg",
            "right": "dpad_right.svg",
            "confirm": "buttons_east.svg",
            "cancel": "buttons_south.svg",
            "clear_history": "buttons_west.svg",
            "history": "buttons_north.svg",
            "start": "button_start.svg",
            "filter": "button_select.svg",
            "delete": "button_l.svg",
            "space": "button_r.svg",
            "page_up": "button_lt.svg",
            "page_down": "button_rt.svg",
        }
    else:
        mapping = {
            "up": "dpad_up.svg",
            "down": "dpad_down.svg",
            "left": "dpad_left.svg",
            "right": "dpad_right.svg",
            "confirm": "buttons_south.svg",
            "cancel": "buttons_east.svg",
            "clear_history": "buttons_north.svg",
            "history": "buttons_west.svg",
            "start": "button_start.svg",
            "filter": "button_select.svg",
            "delete": "button_l.svg",
            "space": "button_r.svg",
            "page_up": "button_lt.svg",
            "page_down": "button_rt.svg",
        }
    return mapping.get(action_name)


def _load_svg_icon_surface(svg_path: str, size: int) -> pygame.Surface | None:
    """Load and rasterize SVG icon to pygame Surface at given size."""
    try:
        try:
            import cairosvg  # type: ignore
        except Exception:
            cairosvg = None
        if cairosvg is not None:
            with open(svg_path, "rb") as f:
                svg_bytes = f.read()
            png_bytes = cairosvg.svg2png(bytestring=svg_bytes, output_width=size, output_height=size)
            return pygame.image.load(io.BytesIO(png_bytes), "icon.png").convert_alpha()
        surf = pygame.image.load(svg_path)
        w, h = surf.get_size()
        if w != size or h != size:
            scale = min(size / max(w, 1), size / max(h, 1))
            new_w = max(1, int(w * scale))
            new_h = max(1, int(h * scale))
            surf = pygame.transform.smoothscale(surf, (new_w, new_h))
        return surf.convert_alpha()
    except Exception:
        return None


def get_help_icon_surface(action_name: str, size: int) -> pygame.Surface | None:
    """Get cached icon surface for action at given size."""
    key = (action_name, size)
    if key in _HELP_ICON_CACHE:
        return _HELP_ICON_CACHE[key]
    filename = _action_icon_filename(action_name)
    if not filename:
        _HELP_ICON_CACHE[key] = None
        return None
    full_path = os.path.join(_images_base_dir(), filename)
    if not os.path.exists(full_path):
        _HELP_ICON_CACHE[key] = None
        return None
    surf = _load_svg_icon_surface(full_path, size)
    _HELP_ICON_CACHE[key] = surf
    return surf


def _render_icons_line(
    actions: list[str],
    text: str,
    target_col_width: int,
    font: pygame.font.Font,
    text_color: tuple,
    icon_size: int = 28,
    icon_gap: int = 8,
    icon_text_gap: int = 12,
) -> pygame.Surface:
    """Compose a line with icon row (actions) and text on the right."""
    if not getattr(config, 'joystick', True):
        from .controls import get_control_display
        action_labels = [f"[{get_control_display(a, a.upper())}]" for a in actions]
        full_text = " ".join(action_labels) + " : " + text
        try:
            lines = wrap_text(full_text, font, target_col_width)
        except Exception:
            lines = [full_text]
        line_surfs = [font.render(l, True, text_color) for l in lines]
        width = max((s.get_width() for s in line_surfs), default=1)
        height = sum(s.get_height() for s in line_surfs) + max(0, (len(line_surfs) - 1)) * 4
        surf = pygame.Surface((width, height), pygame.SRCALPHA)
        y = 0
        for s in line_surfs:
            surf.blit(s, (0, y))
            y += s.get_height() + 4
        return surf

    icon_surfs = []
    for a in actions:
        surf = get_help_icon_surface(a, icon_size)
        if surf is not None:
            icon_surfs.append(surf)

    if not icon_surfs:
        try:
            lines = wrap_text(text, font, target_col_width)
        except Exception:
            lines = [text]
        line_surfs = [font.render(l, True, text_color) for l in lines]
        width = max((s.get_width() for s in line_surfs), default=1)
        height = sum(s.get_height() for s in line_surfs) + max(0, (len(line_surfs) - 1)) * 4
        surf = pygame.Surface((width, height), pygame.SRCALPHA)
        y = 0
        for s in line_surfs:
            surf.blit(s, (0, y))
            y += s.get_height() + 4
        return surf

    icons_width = sum(s.get_width() for s in icon_surfs) + (len(icon_surfs) - 1) * icon_gap
    if icons_width + icon_text_gap > target_col_width:
        scale = (target_col_width - icon_text_gap) / max(1, icons_width)
        scale = max(0.6, min(1.0, scale))
        icon_surfs = [pygame.transform.smoothscale(s, (max(1, int(s.get_width() * scale)), max(1, int(s.get_height() * scale)))) for s in icon_surfs]
        icons_width = sum(s.get_width() for s in icon_surfs) + (len(icon_surfs) - 1) * icon_gap

    text_area_width = max(60, target_col_width - icons_width - icon_text_gap)
    try:
        lines = wrap_text(text, font, text_area_width)
    except Exception:
        lines = [text]
    line_surfs = [font.render(l, True, text_color) for l in lines]
    text_block_width = max((s.get_width() for s in line_surfs), default=1)
    text_block_height = sum(s.get_height() for s in line_surfs) + max(0, (len(line_surfs) - 1)) * 4

    total_width = min(target_col_width, icons_width + icon_text_gap + text_block_width)
    total_height = max(max((s.get_height() for s in icon_surfs), default=0), text_block_height)
    surf = pygame.Surface((total_width, total_height), pygame.SRCALPHA)

    x = 0
    icon_y_center = total_height // 2
    for idx, s in enumerate(icon_surfs):
        r = s.get_rect()
        y = icon_y_center - r.height // 2
        surf.blit(s, (x, y))
        x += r.width + (icon_gap if idx < len(icon_surfs) - 1 else 0)

    text_x = x + icon_text_gap
    y = (total_height - text_block_height) // 2
    for ls in line_surfs:
        surf.blit(ls, (text_x, y))
        y += ls.get_height() + 4
    return surf


def _render_icons_line_singleline(
    actions: list[str],
    text: str,
    target_col_width: int,
    font: pygame.font.Font,
    text_color: tuple,
    icon_size: int = 28,
    icon_gap: int = 8,
    icon_text_gap: int = 12,
) -> pygame.Surface:
    """Single-line version for footer: scales down then truncates, no wrapping."""
    if not getattr(config, 'joystick', True):
        from .controls import get_control_display
        action_labels = [f"[{get_control_display(action_name, action_name.upper())}]" for action_name in actions]
        full_text = " ".join(action_labels) + " : " + text
        fitted_text = truncate_text_end(full_text, font, target_col_width)
        text_surface = font.render(fitted_text, True, text_color)
        surf = pygame.Surface(text_surface.get_size(), pygame.SRCALPHA)
        surf.blit(text_surface, (0, 0))
        return surf

    icon_surfs = []
    for action_name in actions:
        surf = get_help_icon_surface(action_name, icon_size)
        if surf is not None:
            icon_surfs.append(surf)

    if not icon_surfs:
        fitted_text = truncate_text_end(text, font, target_col_width)
        text_surface = font.render(fitted_text, True, text_color)
        surf = pygame.Surface(text_surface.get_size(), pygame.SRCALPHA)
        surf.blit(text_surface, (0, 0))
        return surf

    icons_width = sum(s.get_width() for s in icon_surfs) + (len(icon_surfs) - 1) * icon_gap
    if icons_width + icon_text_gap > target_col_width:
        scale = (target_col_width - icon_text_gap) / max(1, icons_width)
        scale = max(0.5, min(1.0, scale))
        icon_surfs = [pygame.transform.smoothscale(s, (max(1, int(s.get_width() * scale)), max(1, int(s.get_height() * scale)))) for s in icon_surfs]
        icons_width = sum(s.get_width() for s in icon_surfs) + (len(icon_surfs) - 1) * icon_gap

    text_area_width = max(24, target_col_width - icons_width - icon_text_gap)
    fitted_text = truncate_text_end(text, font, text_area_width)
    text_surface = font.render(fitted_text, True, text_color)

    total_width = min(target_col_width, icons_width + icon_text_gap + text_surface.get_width())
    total_height = max(max((s.get_height() for s in icon_surfs), default=0), text_surface.get_height())
    surf = pygame.Surface((total_width, total_height), pygame.SRCALPHA)

    x = 0
    icon_y_center = total_height // 2
    for idx, icon_surf in enumerate(icon_surfs):
        rect = icon_surf.get_rect()
        y = icon_y_center - rect.height // 2
        surf.blit(icon_surf, (x, y))
        x += rect.width + (icon_gap if idx < len(icon_surfs) - 1 else 0)

    text_x = x + icon_text_gap
    text_y = (total_height - text_surface.get_height()) // 2
    surf.blit(text_surface, (text_x, text_y))
    return surf


def render_combined_footer_controls(
    all_controls: list[tuple],
    max_width: int,
    text_color: tuple,
) -> pygame.Surface:
    """Render all footer controls combined on one line with adaptive font scaling."""
    footer_scale = config.accessibility_settings.get("footer_font_scale", 1.0)
    nominal_size = max(10, int(20 * footer_scale))
    candidate_sizes = []
    for size in range(nominal_size, 9, -2):
        if size not in candidate_sizes:
            candidate_sizes.append(size)
    if 10 not in candidate_sizes:
        candidate_sizes.append(10)

    for font_size in candidate_sizes:
        font = get_badge_font(font_size)
        ratio = font_size / max(1, nominal_size)
        icon_size = max(12, int(20 * footer_scale * ratio))
        icon_gap = max(2, int(6 * ratio))
        icon_text_gap = max(4, int(10 * ratio))
        control_gap = max(8, int(20 * ratio))

        rendered_controls = []
        total_width = 0
        for _, actions, label in all_controls:
            surf = _render_icons_line_singleline(actions, label, max_width, font, text_color, icon_size=icon_size, icon_gap=icon_gap, icon_text_gap=icon_text_gap)
            rendered_controls.append(surf)
            total_width += surf.get_width()

        total_width += max(0, len(rendered_controls) - 1) * control_gap
        if total_width <= max_width:
            total_height = max((surf.get_height() for surf in rendered_controls), default=1)
            combined = pygame.Surface((total_width, total_height), pygame.SRCALPHA)
            x_pos = 0
            for idx, surf in enumerate(rendered_controls):
                combined.blit(surf, (x_pos, (total_height - surf.get_height()) // 2))
                x_pos += surf.get_width() + (control_gap if idx < len(rendered_controls) - 1 else 0)
            return combined

    font = get_badge_font(candidate_sizes[-1])
    icon_size = 12
    icon_gap = 2
    icon_text_gap = 4
    control_gap = 8
    remaining_width = max_width
    rendered_controls = []
    for idx, (_, actions, label) in enumerate(all_controls):
        controls_left = len(all_controls) - idx
        target_width = max(40, remaining_width // max(1, controls_left))
        surf = _render_icons_line_singleline(actions, label, target_width, font, text_color, icon_size=icon_size, icon_gap=icon_gap, icon_text_gap=icon_text_gap)
        rendered_controls.append(surf)
        remaining_width -= surf.get_width() + control_gap

    total_width = min(max_width, sum(surf.get_width() for surf in rendered_controls) + max(0, len(rendered_controls) - 1) * control_gap)
    total_height = max((surf.get_height() for surf in rendered_controls), default=1)
    combined = pygame.Surface((total_width, total_height), pygame.SRCALPHA)
    x_pos = 0
    for idx, surf in enumerate(rendered_controls):
        if x_pos + surf.get_width() > total_width:
            break
        combined.blit(surf, (x_pos, (total_height - surf.get_height()) // 2))
        x_pos += surf.get_width() + (control_gap if idx < len(rendered_controls) - 1 else 0)
    return combined


def render_icons_line(
    actions: list[str],
    text: str,
    target_col_width: int,
    font: pygame.font.Font,
    text_color: tuple,
    icon_size: int = 28,
    icon_gap: int = 8,
    icon_text_gap: int = 12,
) -> pygame.Surface:
    """Public wrapper for _render_icons_line."""
    return _render_icons_line(actions, text, target_col_width, font, text_color, icon_size, icon_gap, icon_text_gap)


def render_icons_line_singleline(
    actions: list[str],
    text: str,
    target_col_width: int,
    font: pygame.font.Font,
    text_color: tuple,
    icon_size: int = 28,
    icon_gap: int = 8,
    icon_text_gap: int = 12,
) -> pygame.Surface:
    """Public wrapper for _render_icons_line_singleline."""
    return _render_icons_line_singleline(actions, text, target_col_width, font, text_color, icon_size, icon_gap, icon_text_gap)