"""
Control display names and footer controls rendering.
"""

import pygame  # type: ignore

import config
from language import _
from .icons import render_combined_footer_controls, get_help_icon_surface
from .colors import THEME_COLORS


def get_control_display(action: str, default: str) -> str:
    """Get display name for a control action from controls_config."""
    keyboard_defaults = {
        "confirm": "Enter",
        "cancel": "Esc/Echap",
        "left": "←",
        "right": "→",
        "up": "↑",
        "down": "↓",
        "start": "AltGR",
        "clear_history": "X",
        "history": "H",
        "page_up": "Page+",
        "page_down": "Page-",
        "filter": "F",
        "delete": "Backspace",
        "space": "Espace",
    }
    keyboard_default = keyboard_defaults.get(action)
    if not config.controls_config:
        return keyboard_default or default

    control_config = config.controls_config.get(action, {})
    control_type = control_config.get('type', '')

    if getattr(config, 'keyboard', False) and control_type != 'key' and keyboard_default:
        return keyboard_default

    custom_label = control_config.get('display')
    if isinstance(custom_label, str) and custom_label.strip():
        return custom_label

    if control_type == 'key':
        key_code = control_config.get('key')
        key_names = {
            pygame.K_RETURN: "Enter",
            pygame.K_ESCAPE: "Esc/Echap",
            pygame.K_SPACE: "Espace",
            pygame.K_UP: "↑",
            pygame.K_DOWN: "↓",
            pygame.K_LEFT: "←",
            pygame.K_RIGHT: "→",
            pygame.K_BACKSPACE: "Backspace",
            pygame.K_TAB: "Tab",
            pygame.K_LALT: "Alt",
            pygame.K_RALT: "AltGR",
            pygame.K_LCTRL: "LCtrl",
            pygame.K_RCTRL: "RCtrl",
            pygame.K_LSHIFT: "LShift",
            pygame.K_RSHIFT: "RShift",
            pygame.K_LMETA: "LMeta",
            pygame.K_RMETA: "RMeta",
            pygame.K_CAPSLOCK: "Verr Maj",
            pygame.K_NUMLOCK: "Verr Num",
            pygame.K_SCROLLOCK: "Verr Def",
        }
        for i in range(pygame.K_a, pygame.K_z + 1):
            key_names[i] = chr(i).upper()
        for i in range(pygame.K_0, pygame.K_9 + 1):
            key_names[i] = str(i - pygame.K_0)
        return key_names.get(key_code, f"Key{key_code}")

    elif control_type == 'button':
        button_id = control_config.get('button')
        button_names = {
            0: "A", 1: "B", 2: "X", 3: "Y",
            4: "LB", 5: "RB",
            6: "Select", 7: "Start",
            8: "Select", 9: "Start",
            10: "L3", 11: "R3",
        }
        return button_names.get(button_id, f"Btn{button_id}")

    elif control_type == 'hat':
        hat_value = control_config.get('value', (0, 0))
        hat_names = {
            (0, 1): "D↑", (0, -1): "D↓",
            (-1, 0): "D←", (1, 0): "D→"
        }
        return hat_names.get(tuple(hat_value) if isinstance(hat_value, list) else hat_value, "D-Pad")

    elif control_type == 'axis':
        axis_id = control_config.get('axis')
        direction = control_config.get('direction')
        axis_names = {
            (0, -1): "J←", (0, 1): "J→",
            (1, -1): "J↑", (1, 1): "J↓"
        }
        return axis_names.get((axis_id, direction), f"Joy{axis_id}")

    return control_config.get('display', default)


def draw_controls(screen: pygame.Surface, menu_state: str, current_music_name: str | None = None, music_popup_start_time: int = 0) -> None:
    """Draw contextual controls at bottom of screen based on menu_state."""
    if menu_state == "platform_search" and getattr(config, 'joystick', False) and getattr(config, 'global_search_editing', False):
        menu_state = "platform_search_edit"

    controls_map = {
        "platform": [
            ("history", _("controls_action_history")),
            ("filter", _("controls_filter_search")),
            ("confirm", _("controls_confirm_select")),
            ("confirm", _("controls_longpress_confirm")),
            ("start", _("controls_action_start")),
        ],
        "platform_search": [
            ("confirm", _("controls_confirm_select")),
            ("clear_history", _("controls_action_queue")),
            (("page_up", "page_down"), _("controls_pages")),
            ("filter", _("controls_action_edit_search")),
            ("cancel", _("controls_cancel_back")),
        ],
        "platform_search_edit": [
            ("confirm", _("controls_action_select_char")),
            ("delete", _("controls_action_delete")),
            ("space", _("controls_action_space")),
            ("filter", _("controls_action_show_results")),
            ("cancel", _("controls_cancel_back")),
        ],
        "game": [
            ("confirm", _("controls_confirm_select")),
            ("clear_history", _("controls_action_queue")),
            (("page_up", "page_down"), _("controls_pages")),
            ("filter", _("controls_filter_search")),
            ("history", _("controls_action_history")),
        ],
        "history": [
            ("confirm", _("history_game_options_title")),
            ("clear_history", _("controls_action_clear_history")),
            ("history", _("controls_action_close_history")),
            ("cancel", _("controls_cancel_back")),
        ],
        "history_show_folder": [
            ("confirm", _("button_OK")),
            ("clear_history", _("history_move_action")),
            ("cancel", _("controls_cancel_back")),
        ],
        "scraper": [
            ("confirm", _("controls_confirm_select")),
            ("cancel", _("controls_cancel_back")),
        ],
        "error": [
            ("confirm", _("controls_confirm_select")),
        ],
        "confirm_exit": [
            ("confirm", _("controls_confirm_select")),
            ("cancel", _("controls_cancel_back")),
        ],
        "extension_warning": [
            ("confirm", _("controls_confirm_select")),
        ],
        "folder_browser": [
            ("confirm", _("folder_browser_enter")),
            (("page_up", "page_down"), _("controls_pages")),
            ("history", _("folder_browser_select")),
            ("clear_history", _("folder_new_folder")),
            ("cancel", _("controls_cancel_back")),
        ],
        "folder_browser_new_folder": [
            ("confirm", _("controls_action_select_char")),
            ("delete", _("controls_action_delete")),
            ("space", _("controls_action_space")),
            ("history", _("folder_new_confirm")),
            ("cancel", _("controls_cancel_back")),
        ],
        "platform_folder_config": [
            ("confirm", _("controls_confirm_select")),
            ("cancel", _("controls_cancel_back")),
        ],
        "pause_settings_roms_folder": [
            ("confirm", _("folder_browser_browse")),
            ("clear_history", _("settings_roms_folder_default")),
            ("cancel", _("controls_cancel_back")),
        ],
        "pause_connection_status": [
            ("cancel", _("controls_cancel_back")),
        ],
        "filter_platforms": [
            ("confirm", _("controls_confirm_select")),
            (("left", "right"), (_("filter_expand_collapse") if _ and _("filter_expand_collapse") != "filter_expand_collapse" else "Expand/Collapse")),
            (("page_up", "page_down"), f"{_('filter_all')} / {_('filter_none')}"),
            ("history", _("filter_apply")),
            ("cancel", _("controls_cancel_back")),
        ],
        "support_dialog": [
            ("start", _("controls_cancel_back")),
        ],
    }

    if menu_state == "pause_settings_menu":
        roms_folder_index = 3
        if getattr(config, 'pause_settings_selection', 0) == roms_folder_index:
            menu_state = "pause_settings_roms_folder"

    controls_list = controls_map.get(menu_state, [
        ("confirm", _("controls_confirm_select")),
        ("cancel", _("controls_cancel_back")),
    ])

    icon_lines = []

    if menu_state == "loading":
        icon_lines.append(f"RGSX v{config.app_version}")
    else:
        all_controls = []
        for action, label in controls_list:
            if isinstance(action, tuple):
                all_controls.append(("icons", list(action), label))
            else:
                all_controls.append(("icons", [action], label))
        icon_lines.append(("icons_combined", all_controls))

    max_width = config.screen_width - 40
    icon_surfs = []

    footer_scale = config.accessibility_settings.get("footer_font_scale", 1.0)
    base_icon_size = 20
    scaled_icon_size = int(base_icon_size * footer_scale)
    base_icon_gap = 6
    scaled_icon_gap = int(base_icon_gap * footer_scale)
    base_icon_text_gap = 10
    scaled_icon_text_gap = int(base_icon_text_gap * footer_scale)

    for line_data in icon_lines:
        if isinstance(line_data, tuple) and len(line_data) >= 2:
            if line_data[0] == "icons_combined":
                all_controls = line_data[1]
                try:
                    final_surf = render_combined_footer_controls(all_controls, max_width - 20, THEME_COLORS["text"])
                    icon_surfs.append(final_surf)
                except Exception:
                    pass
            elif line_data[0] == "icons" and len(line_data) == 3:
                ignored, actions, label = line_data
                try:
                    surf = get_help_icon_surface(actions[0], scaled_icon_size)  # simplified
                    if surf:
                        from .icons import _render_icons_line
                        surf = _render_icons_line(actions, label, max_width, config.tiny_font, THEME_COLORS["text"], icon_size=scaled_icon_size, icon_gap=scaled_icon_gap, icon_text_gap=scaled_icon_text_gap)
                    icon_surfs.append(surf)
                except Exception:
                    text_surface = config.tiny_font.render(f"{label}", True, THEME_COLORS["text"])
                    icon_surfs.append(text_surface)
        else:
            text_surface = config.tiny_font.render(line_data, True, THEME_COLORS["text"])
            icon_surfs.append(text_surface)

    total_height = sum(s.get_height() for s in icon_surfs) + max(0, (len(icon_surfs) - 1)) * 4 + 8
    rect_height = total_height
    rect_y = config.screen_height - rect_height - 5
    rect_x = (config.screen_width - max_width) // 2

    pygame.draw.rect(screen, THEME_COLORS["button_idle"], (rect_x, rect_y, max_width, rect_height), border_radius=8)
    pygame.draw.rect(screen, THEME_COLORS["border"], (rect_x, rect_y, max_width, rect_height), 1, border_radius=8)

    y = rect_y + 4
    for surf in icon_surfs:
        x_centered = rect_x + (max_width - surf.get_width()) // 2
        screen.blit(surf, (x_centered, y))
        y += surf.get_height() + 4