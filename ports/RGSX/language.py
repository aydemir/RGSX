import os
import json
import logging
import warnings
import config
from config import HEADLESS
try:
    if not HEADLESS:
        import pygame  # type: ignore
    else:
        pygame = None  # type: ignore
except Exception:
    pygame = None  # type: ignore
import subprocess 
from rgsx_settings import (
    load_rgsx_settings,
    save_rgsx_settings,
    get_language_fallback_notified,
    set_language_fallback_notified,
)

logger = logging.getLogger(__name__)

# Langue par défaut et variables globales
DEFAULT_LANGUAGE = "en"
current_language = DEFAULT_LANGUAGE
translations = {}
show_language_selector_on_startup = False


# Mapping optionnel pour normaliser les locales Batocera -> codes 2 lettres
BATOCERA_LOCALE_MAP = {
    "en_US": "en",
    "en_GB": "en",
    "fr_FR": "fr",
    "de_DE": "de",
    "es_ES": "es",
    "it_IT": "it",
    "tr_TR": "tr",
    "zh_CN": "zh",
}

# Windows locale() tam adları (ör. "Turkish_Turkey") -> ISO 639-1 kodu.
# `locale.getlocale()` Windows'ta "tr_TR" yerine İngilizce tam ad döndürür.
LOCALE_NAME_TO_CODE = {
    "english": "en",
    "french": "fr",
    "german": "de",
    "spanish": "es",
    "italian": "it",
    "portuguese": "pt",
    "turkish": "tr",
    "russian": "ru",
    "chinese": "zh",
    "japanese": "ja",
    "korean": "ko",
    "dutch": "nl",
    "polish": "pl",
    "swedish": "sv",
    "greek": "el",
    "czech": "cs",
    "hungarian": "hu",
    "romanian": "ro",
    "ukrainian": "uk",
    "hebrew": "he",
    "arabic": "ar",
    "vietnamese": "vi",
    "thai": "th",
    "indonesian": "id",
    "hindi": "hi",
}

def load_language(lang_code=None):
    """Charge les traductions pour la langue spécifiée ou la langue par défaut."""
    global current_language, translations
    
    if lang_code is None:
        lang_code = DEFAULT_LANGUAGE
    
    lang_file = os.path.join(config.APP_FOLDER, "languages", f"{lang_code}.json")
    
    try:
        if not os.path.exists(lang_file):
            if lang_code != DEFAULT_LANGUAGE:
                logger.warning(f"Fichier de langue {lang_code} non trouvé, utilisation de la langue par défaut")
                return load_language(DEFAULT_LANGUAGE)
            else:
                logger.error(f"Fichier de langue par défaut {lang_file} non trouvé")
                return False
        
        
        with open(lang_file, 'r', encoding='utf-8') as f:
            translations = json.load(f)
        
        current_language = lang_code
        #logger.debug(f"Langue {lang_code} chargée avec succès ({len(translations)} traductions)")
        return True
    
    except Exception as e:
        logger.error(f"Erreur lors du chargement de la langue {lang_code}: {str(e)}")
        if lang_code != DEFAULT_LANGUAGE:
            logger.warning(f"Tentative de chargement de la langue par défaut")
            return load_language(DEFAULT_LANGUAGE)
        return False

def get_size_units():
    """Retourne les unités de taille adaptées à la langue courante.
    
    Français utilise l'octet (o, Ko, Mo, Go, To, Po)
    Autres langues utilisent byte (B, KB, MB, GB, TB, PB)
    """
    if current_language == "fr":
        return ['o', 'Ko', 'Mo', 'Go', 'To', 'Po']
    else:
        return ['B', 'KB', 'MB', 'GB', 'TB', 'PB']

def get_speed_unit():
    """Retourne l'unité de vitesse adaptée à la langue courante.
    
    Français utilise Mo/s
    Autres langues utilisent MB/s
    """
    if current_language == "fr":
        return "Mo/s"
    else:
        return "MB/s"

def get_text(key, default=None):
    """Récupère la traduction correspondant à la clé en garantissant une chaîne.

    - Ne retourne jamais None: fallback vers default (si fourni) sinon la clé.
    - Si la valeur traduite n'est pas une chaîne (liste/dict/etc.), fallback similaire.
    """
    try:
        if not translations:
            load_language()
        # Valeur brute potentielle
        val = translations.get(key) if isinstance(translations, dict) else None
        if isinstance(val, str) and val:
            return val
        # Fallback: utiliser default si fourni
        if isinstance(default, str) and default:
            return default
        # Dernier recours: retourner la clé elle-même (stringifiée)
        return str(key)
    except Exception as e:
        try:
            logger.warning(f"get_text fallback for key={key}: {e}")
        except Exception:
            pass
        return str(default) if default is not None else str(key)

def get_available_languages() -> list[str]:
    """Récupère la liste des langues disponibles."""
    
    if not os.path.exists(config.LANGUAGES_FOLDER):
        logger.warning(f"Dossier des langues {config.LANGUAGES_FOLDER} non trouvé")
        return []
    
    languages: list[str] = []
    for file in os.listdir(config.LANGUAGES_FOLDER):
        if file.endswith(".json"):
            lang_code = os.path.splitext(file)[0]
            languages.append(lang_code)
    
    return languages

def set_language(lang_code):
    """Change la langue courante et sauvegarde la préférence (manuel = utilisateur)."""
    if load_language(lang_code):
        config.current_language = lang_code
        save_language_preference(lang_code, manual=True)
        return True
    return False

def save_language_preference(lang_code, manual=False):
    """Sauvegarde la préférence de langue dans rgsx_settings.json.

    `manual=True` = choix explicite de l'utilisateur (mode "manual" → plus jamais
    auto-détecté). `manual=False` = langue posée par l'auto-détection (mode "auto").
    """
    try:
        settings = load_rgsx_settings()
        settings["language"] = lang_code
        settings["language_mode"] = "manual" if manual else "auto"
        save_rgsx_settings(settings)
        
        logger.debug(f"Préférence de langue sauvegardée: {lang_code} (mode={'manual' if manual else 'auto'})")
        return True
    except Exception as e:
        logger.error(f"Erreur lors de la sauvegarde de la préférence de langue: {str(e)}")
        return False

def load_language_preference():
    """Charge la préférence de langue depuis rgsx_settings.json."""
    global show_language_selector_on_startup
    
    try:
        settings = load_rgsx_settings()
        lang_code = settings.get("language", DEFAULT_LANGUAGE)
        return lang_code
    except Exception as e:
        logger.error(f"Erreur lors du chargement de la préférence de langue: {str(e)}")
        # Recréer le fichier avec le français par défaut
        save_language_preference(DEFAULT_LANGUAGE)
        return DEFAULT_LANGUAGE

def get_language_name(lang_code):
    """Retourne le nom de la langue à partir du code."""
    language_names = {
        "fr": "Français",
        "en": "English",
        "es": "Español",
        "de": "Deutsch",
        "it": "Italiano",
        "pt": "Português",
        "ja": "日本語",
        "zh": "中文",
        "ru": "Русский",
        "tr": "Türkçe"
    }
    return language_names.get(lang_code, lang_code)

def draw_language_selector(screen, selected_language_index):
    """Affiche le sélecteur de langue."""
    from display import THEME_COLORS, get_overlay
    
    # Obtenir les langues disponibles
    available_languages = get_available_languages()
    
    if not available_languages:
        logger.error("Aucune langue disponible")
        return
    
    # Afficher l'overlay
    screen.blit(get_overlay(), (0, 0))
    
    # Titre
    title_text = _("language_select_title")
    title_surface = config.font.render(title_text, True, THEME_COLORS["text"])
    title_rect = title_surface.get_rect(center=(config.screen_width // 2, config.screen_height // 4))
    
    # Fond du titre
    title_bg_rect = title_rect.inflate(40, 20)
    pygame.draw.rect(screen, THEME_COLORS["button_idle"], title_bg_rect, border_radius=10)
    pygame.draw.rect(screen, THEME_COLORS["border"], title_bg_rect, 2, border_radius=10)
    screen.blit(title_surface, title_rect)
    
    # Options de langue
    button_height = 60
    button_width = 300
    button_spacing = 20
    
    total_height = len(available_languages) * (button_height + button_spacing) - button_spacing
    start_y = (config.screen_height - total_height) // 2
    
    for i, lang_code in enumerate(available_languages):
        # Obtenir le nom de la langue
        lang_name = get_language_name(lang_code)
        
        # Position du bouton
        button_x = (config.screen_width - button_width) // 2
        button_y = start_y + i * (button_height + button_spacing)
        
        # Dessiner le bouton
        button_color = THEME_COLORS["button_hover"] if i == selected_language_index else THEME_COLORS["button_idle"]
        pygame.draw.rect(screen, button_color, (button_x, button_y, button_width, button_height), border_radius=10)
        pygame.draw.rect(screen, THEME_COLORS["border"], (button_x, button_y, button_width, button_height), 2, border_radius=10)
        
        # Texte du bouton
        text_surface = config.font.render(lang_name, True, THEME_COLORS["text"])
        text_rect = text_surface.get_rect(center=(button_x + button_width // 2, button_y + button_height // 2))
        screen.blit(text_surface, text_rect)
    
    # Instructions
    instruction_text = _("language_select_instruction")
    instruction_surface = config.small_font.render(instruction_text, True, THEME_COLORS["text"])
    instruction_rect = instruction_surface.get_rect(center=(config.screen_width // 2, config.screen_height - 50))
    screen.blit(instruction_surface, instruction_rect)

def handle_language_menu_events(event, screen):
    """Gère les événements du menu de sélection de langue avec support clavier et manette."""
    available_languages = get_available_languages()
    
    if not available_languages:
        logger.error("Aucune langue disponible")
        config.menu_state = "platform"  # Toujours revenir à platform en cas d'erreur
        config.needs_redraw = True
        return
    
    # Navigation avec les touches du clavier
    if event.type == pygame.KEYDOWN:
        # Navigation vers le haut
        if event.key == pygame.K_UP:
            config.selected_language_index = (config.selected_language_index - 1) % len(available_languages)
            config.needs_redraw = True
            logger.debug(f"Navigation vers le haut dans le sélecteur de langue: {config.selected_language_index}")
        
        # Navigation vers le bas
        elif event.key == pygame.K_DOWN:
            config.selected_language_index = (config.selected_language_index + 1) % len(available_languages)
            config.needs_redraw = True
            logger.debug(f"Navigation vers le bas dans le sélecteur de langue: {config.selected_language_index}")
        
        # Sélection de la langue
        elif event.key == pygame.K_RETURN:
            lang_code = available_languages[config.selected_language_index]
            if set_language(lang_code):
                logger.info(f"Langue changée pour {lang_code}")
                config.current_language = lang_code
                
                # Déterminer l'état suivant en fonction du contexte
                if config.previous_menu_state is None:
                    # Premier démarrage - passer à l'état loading pour charger les plateformes
                    config.menu_state = "loading"
                    logger.debug("Premier démarrage: passage à l'état loading après sélection de la langue")
                elif config.previous_menu_state == "pause_menu":
                    # Si on vient du menu pause, retourner au menu pause avec un message
                    config.menu_state = "restart_popup"
                    config.popup_message = _("language_changed").format(lang_code)
                    config.popup_timer = 2000  # 2 secondes
                    config.previous_menu_state = "platform"  # Pour revenir à l'écran principal après le popup
                    logger.debug("Message de confirmation de changement de langue affiché, retour au menu pause")
                else:
                    # Autre cas, retourner à l'état précédent avec un message
                    config.menu_state = "platform"  # Toujours revenir à platform pour éviter les problèmes
                    logger.debug(f"Retour à l'écran principal après sélection de la langue")
            else:
                # Retour au menu pause en cas d'erreur
                config.menu_state = "platform"  # Toujours revenir à platform en cas d'erreur
            
            config.needs_redraw = True
            logger.debug(f"Sélection de la langue: {lang_code}")
        
        # Annulation (seulement si on n'est pas au démarrage)
        elif event.key == pygame.K_ESCAPE and config.previous_menu_state is not None:
            config.menu_state = "pause_menu"
            config.needs_redraw = True
            logger.debug("Annulation de la sélection de langue, retour au menu pause")
    
    # Support de la manette
    elif event.type == pygame.JOYBUTTONDOWN:
        # Sélection avec le bouton A (généralement 0)
        if event.button == 0:  # Bouton A
            lang_code = available_languages[config.selected_language_index]
            if set_language(lang_code):
                logger.info(f"Langue changée pour {lang_code} (manette)")
                config.current_language = lang_code
                
                # Déterminer l'état suivant en fonction du contexte
                if config.previous_menu_state is None:
                    # Premier démarrage - passer à l'état loading pour charger les plateformes
                    config.menu_state = "loading"
                    logger.debug("Premier démarrage: passage à l'état loading après sélection de la langue (manette)")
                else:
                    config.menu_state = "platform"
            else:
                config.menu_state = "platform"
            config.needs_redraw = True
        
        # Annulation avec le bouton B (généralement 1)
        elif event.button == 1 and config.previous_menu_state is not None:  # Bouton B
            config.menu_state = "pause_menu"
            config.needs_redraw = True
            logger.debug("Annulation de la sélection de langue (manette), retour au menu pause")
    
    # Navigation avec le D-pad
    elif event.type == pygame.JOYHATMOTION:
        if event.value == (0, 1):  # Haut
            config.selected_language_index = (config.selected_language_index - 1) % len(available_languages)
            config.needs_redraw = True
            logger.debug(f"Navigation vers le haut dans le sélecteur de langue (D-pad): {config.selected_language_index}")
        elif event.value == (0, -1):  # Bas
            config.selected_language_index = (config.selected_language_index + 1) % len(available_languages)
            config.needs_redraw = True
            logger.debug(f"Navigation vers le bas dans le sélecteur de langue (D-pad): {config.selected_language_index}")
    
    # Navigation avec les joysticks analogiques
    elif event.type == pygame.JOYAXISMOTION:
        # Joystick gauche vertical (généralement axe 1)
        if event.axis == 1 and abs(event.value) > 0.5:
            if event.value < -0.5:  # Haut
                config.selected_language_index = (config.selected_language_index - 1) % len(available_languages)
                config.needs_redraw = True
                logger.debug(f"Navigation vers le haut dans le sélecteur de langue (joystick): {config.selected_language_index}")
            elif event.value > 0.5:  # Bas
                config.selected_language_index = (config.selected_language_index + 1) % len(available_languages)
                config.needs_redraw = True
                logger.debug(f"Navigation vers le bas dans le sélecteur de langue (joystick): {config.selected_language_index}")


def update_valid_states():
    """Ajoute l'état language_select à la liste des états valides."""
    from controls import VALID_STATES
    if "language_select" not in VALID_STATES:
        VALID_STATES.append("language_select")
        logger.debug("État language_select ajouté aux états valides")

def _classify_environment() -> str:
    """Ortam sınıflandırıcısı: "batocera" | "termux" | "retrobat" | "desktop".

    Termux/RetroBat'ta host'tan miras env "gerçek sistem dili" sayılmaz.
    """
    try:
        if os.path.exists("/userdata/system/batocera.conf"):
            return "batocera"
    except Exception:
        pass
    if os.environ.get("TERMUX_VERSION") or os.environ.get("PREFIX", "").startswith("/data/data/com.termux"):
        return "termux"
    if os.environ.get("RETROBAT") or os.environ.get("RETROBAT_LAUNCH"):
        return "retrobat"
    return "desktop"


def _normalize_lang_code(code) -> str | None:
    """Locate/code normalizasyonu: `tr_TR.UTF-8` → "tr", `pt_BR` → "pt". None ise kullanılamaz."""
    if not code:
        return None
    s = str(code).strip()
    if "." in s:
        s = s.split(".", 1)[0]
    s = s.replace("-", "_")
    parts = [p for p in s.split("_") if p]
    if not parts:
        return None
    base = parts[0].lower()
    if base in ("c", "posix"):
        return None
    mapped = LOCALE_NAME_TO_CODE.get(base)
    if mapped:
        return mapped
    if len(base) == 2:
        return base
    return None


def _translation_exists(lang_code) -> bool:
    """Belirtilen kod için çeviri dosyası var mı?"""
    if not lang_code:
        return False
    return os.path.exists(os.path.join(config.LANGUAGES_FOLDER, f"{lang_code}.json"))


def _detect_os_locale():
    """Genel OS dil algılama: locale.getlocale() → env LANG/LC_ALL/LC_MESSAGES → getdefaultlocale()."""
    try:
        import locale as _locale
        try:
            lang, _enc = _locale.getlocale()
            if lang:
                norm = _normalize_lang_code(lang)
                if norm:
                    logger.info(f"Locale système détectée (getlocale): {lang} -> {norm}")
                    return norm
        except Exception:
            pass

        for var in ("LANG", "LC_ALL", "LC_MESSAGES"):
            val = os.environ.get(var, "").strip()
            if val and val.lower() not in ("c", "posix"):
                norm = _normalize_lang_code(val)
                if norm:
                    logger.info(f"Locale système détectée (env {var}): {val} -> {norm}")
                    return norm

        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", DeprecationWarning)
                lang, _enc = _locale.getdefaultlocale()
            if lang:
                norm = _normalize_lang_code(lang)
                if norm:
                    logger.info(f"Locale système détectée (getdefaultlocale): {lang} -> {norm}")
                    return norm
        except Exception:
            pass
    except Exception:
        pass
    return None


def detect_system_language():
    """Sistemin gerçek dilini algılar, 2 harfli kod döndürür (yoksa None).

    Öncelik: Batocera (gerçek locale) > genel OS (locale/env) > en (yalnızca bellek).
    """
    env = _classify_environment()
    if env == "batocera":
        detected = detect_batocera_language()
        if detected:
            return detected
    elif env in ("termux", "retrobat"):
        logger.info(
            f"Environnement {env}: env hérité ignoré comme 'vraie langue système' "
            "(détection à priorité minimale)"
        )

    return _detect_os_locale()


def detect_batocera_language():
    """Tente de lire la langue système de Batocera et retourne un code à 2 lettres, sinon None."""
    try:
        batocera_conf = "/userdata/system/batocera.conf"
        if not os.path.exists(batocera_conf):
            logger.debug("batocera.conf introuvable, détection Batocera ignorée")
            return None

        # batocera-settings-get system.language -> ex: en_US, fr_FR, ...
        res = subprocess.run(
            ["batocera-settings-get", "system.language"],
            capture_output=True,
            text=True,
            timeout=3,
        )
        if res.returncode != 0:
            logger.warning(f"Lecture batocera-settings-get échouée (code {res.returncode}): {res.stderr.strip()}")
            return None

        locale_val = res.stdout.strip()
        if not locale_val:
            logger.warning("Langue Batocera vide")
            return None

        lang2 = BATOCERA_LOCALE_MAP.get(locale_val, locale_val.split("_")[0].lower())
        logger.info(f"Langue Batocera détectée: {locale_val} -> {lang2}")
        return lang2
    except FileNotFoundError:
        logger.debug("Commande batocera-settings-get introuvable")
        return None
    except Exception as e:
        logger.error(f"Erreur lors de la détection de la langue Batocera: {e}")
        return None

def _raw_settings():
    """rgsx_settings.json ham içeriği — default merge OLMADAN. Yoksa None."""
    try:
        if os.path.exists(config.RGSX_SETTINGS_PATH):
            with open(config.RGSX_SETTINGS_PATH, "r", encoding="utf-8") as f:
                return json.load(f)
    except Exception:
        pass
    return None


def initialize_language():
    """Initialise la langue au démarrage (Faz 11: auto-détection système).

    - `language_mode == "manual"` → préférence utilisateur, aucune détection.
    - Sinon → auto-détection; langue supportée écrite (mode auto), langue non
      supportée → key supprimée + en en mémoire (WebUI/TVUI cohérents).
    - Migration (legacy dosyalar): `language=="en"` (veya key yok) → auto;
      `language!="en"` → manual (kullanıcı tercihi korunur). Legacy-en → algılanan
      desteklenen dil FARKLIYSA tek seferlik bildirim gösterilir.
    - Fallback auto→en: `language_fallback_notified` marker'ı ile tek seferlik
      bildirim (display init sonrası `config.language_fallback_notify` ile).
    """
    global show_language_selector_on_startup
    show_language_selector_on_startup = False

    raw = _raw_settings()
    settings = load_rgsx_settings()
    chosen = DEFAULT_LANGUAGE
    notify_kind = None  # None | "fallback" | ("auto_detected", lang_display)
    migrated_from_legacy_en = False

    # Mod belirleme: yeni kurulum vs. legacy dosya migrasyonu
    if raw is None:
        settings["language_mode"] = "auto"
    elif "language_mode" not in settings:
        if raw.get("language") not in (None, DEFAULT_LANGUAGE):
            settings["language_mode"] = "manual"
        else:
            settings["language_mode"] = "auto"
            migrated_from_legacy_en = True
        save_rgsx_settings(settings)
        logger.info(f"Migration language_mode: {settings['language_mode']}")

    if settings["language_mode"] == "manual" and settings.get("language"):
        chosen = settings["language"]
        if not _translation_exists(chosen):
            logger.warning(f"Manuel dil {chosen} bulunamadı, onarım: auto-detect")
            settings.pop("language", None)
            settings["language_mode"] = "auto"
            save_rgsx_settings(settings)
            chosen = None  # auto-detect'e düş

    if chosen is None or settings["language_mode"] == "auto":
        detected = detect_system_language()
        if detected == DEFAULT_LANGUAGE:
            chosen = DEFAULT_LANGUAGE
        elif detected and _translation_exists(detected):
            settings["language"] = detected
            chosen = detected
            logger.info(f"Auto-détection: {detected}")
            if migrated_from_legacy_en:
                notify_kind = ("auto_detected", get_language_name(detected))
        else:
            if detected:
                notify_kind = "fallback"
                logger.warning(f"Algılanan dil {detected} desteklenmiyor, en fallback")
            settings.pop("language", None)
            chosen = DEFAULT_LANGUAGE
        settings["language_mode"] = "auto"
        save_rgsx_settings(settings)

    if not load_language(chosen):
        logger.warning(f"Impossible de charger la langue {chosen}, utilisation de la langue par défaut")
        load_language(DEFAULT_LANGUAGE)
    else:
        logger.info(f"Langue chargée au démarrage: {chosen}")

    config.current_language = current_language

    if notify_kind is not None:
        if get_language_fallback_notified():
            config.language_fallback_notify = False
        else:
            set_language_fallback_notified(True)
            config.language_fallback_notify = True
            if notify_kind == "fallback":
                config.language_notify_message = _("language_fallback_notice")
            else:
                config.language_notify_message = _("language_auto_detected").format(lang=notify_kind[1])
    else:
        config.language_fallback_notify = False

    return True

# Alias pour faciliter l'utilisation
_ = get_text