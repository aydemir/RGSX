import os
import platform
import warnings
import logging


def _enable_windows_dpi_awareness_early():
    """Enable DPI awareness before importing pygame so SDL sees physical monitor sizes."""
    if platform.system() != "Windows":
        return

    try:
        os.environ.setdefault("SDL_WINDOWS_DPI_AWARENESS", "permonitorv2")
    except Exception:
        pass

    try:
        import ctypes

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        if hasattr(user32, "SetProcessDpiAwarenessContext"):
            for awareness in (-4, -3):
                try:
                    if user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(awareness)):
                        return
                except Exception:
                    continue
    except Exception:
        pass

    try:
        import ctypes

        shcore = ctypes.WinDLL("shcore", use_last_error=True)
        if hasattr(shcore, "SetProcessDpiAwareness"):
            shcore.SetProcessDpiAwareness(2)
            return
    except Exception:
        pass

    try:
        import ctypes

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        if hasattr(user32, "SetProcessDPIAware"):
            user32.SetProcessDPIAware()
    except Exception:
        pass


_enable_windows_dpi_awareness_early()

# Ignorer le warning de deprecation de pkg_resources dans pygame
warnings.filterwarnings("ignore", category=UserWarning, module="pygame.pkgdata")
warnings.filterwarnings("ignore", message="pkg_resources is deprecated")

# NOTE: config modül-seviyesinde pygame import eder; bu yüzden DPI çağrısı
# (config'ten önce) korunmalıdır — SDL, pygame import edilmeden önce
# SDL_WINDOWS_DPI_AWARENESS'i görmelidir.
import config


# Configuration du logging
# RotatingFileHandler : 20 MB max par fichier, 2 backups → 60 MB total maximum.
# Évite les fichiers RGSX.log de 1.5 GB causés par un flood de logs torrent, la rotation
# servant de garde-fou.
try:
    os.makedirs(config.log_dir, exist_ok=True)
    from logging.handlers import RotatingFileHandler as _RotatingFileHandler
    _log_handler = _RotatingFileHandler(
        config.log_file,
        maxBytes=20 * 1024 * 1024,  # 20 MB
        backupCount=2,
        encoding='utf-8',
    )
    _log_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))
    logging.root.setLevel(logging.DEBUG)
    logging.root.addHandler(_log_handler)
except Exception as e:
    logging.basicConfig(
        level=logging.DEBUG,
        format='%(asctime)s - %(levelname)s - %(message)s'
    )
    logging.error(f"Échec de la configuration du logging dans {config.log_file}: {str(e)}")

# Handler crash : ne retient que les erreurs/critiques (diagnostic ciblé)
try:
    from logging.handlers import RotatingFileHandler as _CrashRotatingFileHandler
    _crash_handler = _CrashRotatingFileHandler(
        config.log_file_crash,
        maxBytes=5 * 1024 * 1024,  # 5 MB
        backupCount=1,
        encoding='utf-8',
    )
    _crash_handler.setLevel(logging.ERROR)
    _crash_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))
    logging.root.addHandler(_crash_handler)
except Exception as e:
    logging.warning(f"Impossible de configurer le crash log dans {config.log_file_crash}: {e}")

# Faz 6-5: boot akışı + ana döngü tvui.py'ye taşındı; manager spawn/supervisor
# manager_launcher.py'de. Bu dosya yalnızca giriş noktasıdır (python __main__.py).
import asyncio

from tvui import main  # noqa: E402

if config.OPERATING_SYSTEM == "Emscripten":
    asyncio.ensure_future(main())
else:
    if __name__ == "__main__":
        asyncio.run(main())
