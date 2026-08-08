
"""Display core: window init, metrics sync, shared OVERLAY."""

import logging
import os
import platform
import pygame  # type: ignore

import config
from rgsx_settings import load_rgsx_settings, get_display_fullscreen

logger = logging.getLogger(__name__)

OVERLAY = None  # set by init_display/sync_display_metrics


def get_overlay() -> pygame.Surface | None:
    """Return the current shared dim overlay surface (created by init_display/sync_display_metrics)."""
    return OVERLAY


def _get_windows_monitor_physical_sizes() -> list[tuple[int, int]]:
    """Return physical monitor resolutions from Win32, bypassing DPI-scaled SDL values."""
    if platform.system() != "Windows":
        return []

    try:
        import ctypes
        from ctypes import wintypes

        CCHDEVICENAME = 32
        ENUM_CURRENT_SETTINGS = -1

        class MONITORINFOEXW(ctypes.Structure):
            _fields_ = [
                ("cbSize", wintypes.DWORD),
                ("rcMonitor", wintypes.RECT),
                ("rcWork", wintypes.RECT),
                ("dwFlags", wintypes.DWORD),
                ("szDevice", wintypes.WCHAR * CCHDEVICENAME),
            ]

        class DEVMODEW(ctypes.Structure):
            _fields_ = [
                ("dmDeviceName", wintypes.WCHAR * CCHDEVICENAME),
                ("dmSpecVersion", wintypes.WORD),
                ("dmDriverVersion", wintypes.WORD),
                ("dmSize", wintypes.WORD),
                ("dmDriverExtra", wintypes.WORD),
                ("dmFields", wintypes.DWORD),
                ("dmPositionX", wintypes.LONG),
                ("dmPositionY", wintypes.LONG),
                ("dmDisplayOrientation", wintypes.DWORD),
                ("dmDisplayFixedOutput", wintypes.DWORD),
                ("dmColor", wintypes.SHORT),
                ("dmDuplex", wintypes.SHORT),
                ("dmYResolution", wintypes.SHORT),
                ("dmTTOption", wintypes.SHORT),
                ("dmCollate", wintypes.SHORT),
                ("dmFormName", wintypes.WCHAR * 32),
                ("dmLogPixels", wintypes.WORD),
                ("dmBitsPerPel", wintypes.DWORD),
                ("dmPelsWidth", wintypes.DWORD),
                ("dmPelsHeight", wintypes.DWORD),
                ("dmDisplayFlags", wintypes.DWORD),
                ("dmDisplayFrequency", wintypes.DWORD),
                ("dmICMMethod", wintypes.DWORD),
                ("dmICMIntent", wintypes.DWORD),
                ("dmMediaType", wintypes.DWORD),
                ("dmDitherType", wintypes.DWORD),
                ("dmReserved1", wintypes.DWORD),
                ("dmReserved2", wintypes.DWORD),
                ("dmPanningWidth", wintypes.DWORD),
                ("dmPanningHeight", wintypes.DWORD),
            ]

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        monitors: list[tuple[int, int]] = []

        monitor_enum_proc = ctypes.WINFUNCTYPE(
            wintypes.BOOL,
            wintypes.HMONITOR,
            wintypes.HDC,
            ctypes.POINTER(wintypes.RECT),
            wintypes.LPARAM,
        )

        def _callback(hmonitor, hdc, lprect, lparam):
            monitor_info = MONITORINFOEXW()
            monitor_info.cbSize = ctypes.sizeof(MONITORINFOEXW)
            if user32.GetMonitorInfoW(hmonitor, ctypes.byref(monitor_info)):
                devmode = DEVMODEW()
                devmode.dmSize = ctypes.sizeof(DEVMODEW)
                if user32.EnumDisplaySettingsW(monitor_info.szDevice, ENUM_CURRENT_SETTINGS, ctypes.byref(devmode)):
                    width = int(devmode.dmPelsWidth or 0)
                    height = int(devmode.dmPelsHeight or 0)
                    if width > 0 and height > 0:
                        monitors.append((width, height))
                        return True
            return True

        user32.EnumDisplayMonitors(0, 0, monitor_enum_proc(_callback), 0)
        return monitors
    except Exception as e:
        logger.debug(f"Résolution physique Win32 indisponible: {e}")
        return []

def sync_display_metrics(screen=None):
    """Synchronise les dimensions globales et l'overlay avec la fenêtre courante."""
    global OVERLAY

    if screen is None:
        screen = pygame.display.get_surface()
    if screen is None:
        return None

    screen_width, screen_height = screen.get_size()
    config.screen_width = screen_width
    config.screen_height = screen_height

    OVERLAY = pygame.Surface((screen_width, screen_height), pygame.SRCALPHA)
    OVERLAY.fill((5, 10, 20, 160))
    return screen

def init_display():
    """Initialise l'écran et les ressources globales.
    Supporte la sélection de moniteur en plein écran.
    Compatible Windows et Linux (Batocera).
    """
    global OVERLAY
    
    
    # Charger les paramètres d'affichage
    settings = load_rgsx_settings()
    logger.debug(f"Settings chargés: display={settings.get('display', {})}")
    target_monitor = settings.get("display", {}).get("monitor", 0)
    is_fullscreen = get_display_fullscreen(settings)
    
    
    # Vérifier les variables d'environnement (priorité sur les settings)
    env_display = os.environ.get("RGSX_DISPLAY")
    if env_display is not None:
        try:
            target_monitor = int(env_display)
            logger.debug(f"Override par RGSX_DISPLAY: monitor={target_monitor}")
        except ValueError:
            pass
    
    
    # Configurer SDL pour utiliser le bon moniteur
    # Cette variable d'environnement doit être définie AVANT la création de la fenêtre
    os.environ["SDL_VIDEO_FULLSCREEN_HEAD"] = str(target_monitor)
    
    # Obtenir les informations d'affichage
    num_displays = 1
    try:
        num_displays = pygame.display.get_num_displays()
    except Exception:
        pass
    
    # S'assurer que le moniteur cible existe
    if target_monitor >= num_displays:
        logger.warning(f"Monitor {target_monitor} not available, using monitor 0")
        target_monitor = 0
    
    # Obtenir la résolution du moniteur cible
    try:
        win32_sizes = _get_windows_monitor_physical_sizes()
        if target_monitor < len(win32_sizes):
            screen_width, screen_height = win32_sizes[target_monitor]
            logger.debug(f"Résolution moniteur via Win32: {screen_width}x{screen_height} (monitor={target_monitor})")
        elif hasattr(pygame.display, 'get_desktop_sizes') and num_displays > 1:
            desktop_sizes = pygame.display.get_desktop_sizes()
            if target_monitor < len(desktop_sizes):
                screen_width, screen_height = desktop_sizes[target_monitor]
            else:
                display_info = pygame.display.Info()
                screen_width = display_info.current_w
                screen_height = display_info.current_h
        else:
            display_info = pygame.display.Info()
            screen_width = display_info.current_w
            screen_height = display_info.current_h
    except Exception as e:
        logger.error(f"Error getting display info: {e}")
        display_info = pygame.display.Info()
        screen_width = display_info.current_w
        screen_height = display_info.current_h
    
    # Créer la fenêtre selon le mode d'affichage configuré.
    if is_fullscreen:
        flags = pygame.FULLSCREEN
        # Sur Linux/Batocera, utiliser SCALED pour respecter la résolution forcée d'EmulationStation
        if platform.system() == "Linux":
            flags |= pygame.SCALED
        # Sur certains systèmes Windows, NOFRAME aide pour le multi-écran
        elif platform.system() == "Windows":
            flags |= pygame.NOFRAME
    else:
        flags = pygame.RESIZABLE
        if platform.system() == "Windows":
            os.environ["SDL_VIDEO_CENTERED"] = "1"

        desktop_width = screen_width
        desktop_height = screen_height
        screen_width = min(desktop_width, max(960, int(desktop_width * 0.9)))
        screen_height = min(desktop_height, max(540, int(desktop_height * 0.9)))
    
    try:
        screen = pygame.display.set_mode((screen_width, screen_height), flags, display=target_monitor)
    except TypeError:
        # Anciennes versions de pygame ne supportent pas le paramètre display=
        screen = pygame.display.set_mode((screen_width, screen_height), flags)
    except Exception as e:
        logger.error(f"Error creating display on monitor {target_monitor}: {e}")
        screen = pygame.display.set_mode((screen_width, screen_height), flags)

    screen = sync_display_metrics(screen)
    screen_width, screen_height = screen.get_size()

    config.current_monitor = target_monitor

    logger.debug(
        f"Écran initialisé: {screen_width}x{screen_height} sur moniteur {target_monitor} "
        f"({'fullscreen' if is_fullscreen else 'windowed'})"
    )
    return screen
