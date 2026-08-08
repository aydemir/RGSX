"""
Display package - UI rendering modules for RGSX TVUI.

This package decomposes the original display.py (6800+ lines) into focused modules
following Single Responsibility Principle.
"""

# Core theme & background
from .colors import (
    THEME_COLORS,
    BACKGROUND_THEME_PRESETS,
    get_background_theme_colors,
    get_background_theme_label,
)
from .background import draw_gradient, draw_app_background

# Fonts & icons
from .fonts import get_badge_font
from .icons import (
    get_help_icon_surface,
    clear_help_icon_cache,
    render_icons_line,
    render_icons_line_singleline,
    render_combined_footer_controls,
)

# Components
from .components import (
    draw_stylized_button,
    draw_shadow,
    draw_glow_effect,
    draw_header_badge,
    measure_header_badge,
    get_adaptive_badge_layout,
    fit_badge_lines,
)

# Controls
from .controls import get_control_display, draw_controls

# Screens
from .screens import (
    draw_loading_screen,
    draw_error_screen,
    draw_popup,
    draw_toast,
    show_toast,
)

# Transitions
from .transitions import draw_validation_transition

# Grid & platform rendering
from .grid import (
    draw_platform_grid,
    draw_platform_header_info,
    get_platform_header_info_lines,
    get_platform_header_badge_layout,
    draw_platform_source_badge,
    get_default_disk_space_line,
    get_display_resolution_line,
    format_disk_size_gb,
)

# Game list
from .game_list import draw_game_list, draw_game_scrollbar, get_display_extension

# Global search
from .global_search import draw_global_search_list

# History
from .history import (
    draw_history_list,
    draw_history_scrollbar,
    draw_clear_history_dialog,
    draw_cancel_download_dialog,
    draw_history_game_options,
    draw_history_show_folder,
    draw_history_scraper_info,
    draw_history_error_details,
    draw_history_confirm_delete,
    draw_history_extract_archive,
)

# Virtual keyboard
from .virtual_keyboard import draw_virtual_keyboard

# Progress
from .progress import draw_progress_screen, draw_extension_warning

# Menus
from .menus import (
    draw_pause_menu,
    draw_language_menu,
    draw_display_menu,
    draw_pause_controls_menu,
    draw_pause_display_menu,
    draw_pause_display_layout_menu,
    draw_pause_display_font_menu,
    draw_pause_games_menu,
    draw_pause_settings_menu,
    draw_pause_api_keys_status,
    draw_pause_qbt_password,
    draw_pause_connection_status,
    draw_filter_platforms_menu,
    draw_controls_help,
    draw_confirm_dialog,
    draw_reload_games_data_dialog,
    draw_reset_settings_confirm_dialog,
    draw_gamelist_update_prompt,
    draw_platform_folder_config_dialog,
)

# Folder browser
from .folder_browser import draw_folder_browser, draw_folder_browser_new_folder

# Support
from .support import draw_support_dialog

# Text viewer
from .text_viewer import draw_text_file_viewer

# Scraper
from .scraper import draw_scraper_screen

# Filter
from .filter import (
    draw_filter_menu_choice,
    draw_global_sort_menu,
    draw_filter_advanced,
    draw_filter_priority_config,
)

# Initialize display (must be called before other functions)
from .core import init_display, sync_display_metrics, get_overlay, OVERLAY

__all__ = [
    # Theme
    "THEME_COLORS",
    "BACKGROUND_THEME_PRESETS",
    "get_background_theme_colors",
    "get_background_theme_label",
    # Background
    "draw_gradient",
    "draw_app_background",
    # Fonts
    "get_badge_font",
    # Icons
    "get_help_icon_surface",
    "clear_help_icon_cache",
    "render_icons_line",
    "render_icons_line_singleline",
    "render_combined_footer_controls",
    # Components
    "draw_stylized_button",
    "draw_shadow",
    "draw_glow_effect",
    "draw_header_badge",
    "measure_header_badge",
    "get_adaptive_badge_layout",
    "fit_badge_lines",
    # Controls
    "get_control_display",
    "draw_controls",
    # Screens
    "draw_loading_screen",
    "draw_error_screen",
    "draw_popup",
    "draw_toast",
    "show_toast",
    # Transitions
    "draw_validation_transition",
    # Grid
    "draw_platform_grid",
    "draw_platform_header_info",
    "get_platform_header_info_lines",
    "get_platform_header_badge_layout",
    "draw_platform_source_badge",
    "get_default_disk_space_line",
    "get_display_resolution_line",
    "format_disk_size_gb",
    # Game list
    "draw_game_list",
    "draw_game_scrollbar",
    "get_display_extension",
    # Global search
    "draw_global_search_list",
    # History
    "draw_history_list",
    "draw_history_scrollbar",
    "draw_clear_history_dialog",
    "draw_cancel_download_dialog",
    "draw_history_game_options",
    "draw_history_show_folder",
    "draw_history_scraper_info",
    "draw_history_error_details",
    "draw_history_confirm_delete",
    "draw_history_extract_archive",
    # Virtual keyboard
    "draw_virtual_keyboard",
    # Progress
    "draw_progress_screen",
    "draw_extension_warning",
    # Menus
    "draw_pause_menu",
    "draw_language_menu",
    "draw_display_menu",
    "draw_pause_controls_menu",
    "draw_pause_display_menu",
    "draw_pause_display_layout_menu",
    "draw_pause_display_font_menu",
    "draw_pause_games_menu",
    "draw_pause_settings_menu",
    "draw_pause_api_keys_status",
    "draw_pause_qbt_password",
    "draw_pause_connection_status",
    "draw_filter_platforms_menu",
    "draw_controls_help",
    "draw_confirm_dialog",
    "draw_reload_games_data_dialog",
    "draw_reset_settings_confirm_dialog",
    "draw_gamelist_update_prompt",
    "draw_platform_folder_config_dialog",
    # Folder browser
    "draw_folder_browser",
    "draw_folder_browser_new_folder",
    # Support
    "draw_support_dialog",
    # Text viewer
    "draw_text_file_viewer",
    # Scraper
    "draw_scraper_screen",
    # Filter
    "draw_filter_menu_choice",
    "draw_global_sort_menu",
    "draw_filter_advanced",
    "draw_filter_priority_config",
    # Core
    "init_display",
    "sync_display_metrics",
    "get_overlay",
    "OVERLAY",
]