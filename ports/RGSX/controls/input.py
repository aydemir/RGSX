import json
import logging
import os
import re

import pygame

import config
from config import CONTROLS_CONFIG_PATH, REPEAT_DELAY, REPEAT_INTERVAL

logger = logging.getLogger("controls")

key_states = {}

def load_controls_config(path=CONTROLS_CONFIG_PATH):
    """Charge la configuration des contrôles.
    Priorité:
    1) Fichier utilisateur dans SAVE_FOLDER (controls.json)
    2) Préréglage correspondant dans PRECONF_CONTROLS_PATH (sans copie)
    3) Configuration clavier par défaut
    """
    default_config = {
        "confirm": {"type": "key", "key": pygame.K_RETURN},
        "cancel": {"type": "key", "key": pygame.K_ESCAPE},
        "left": {"type": "key", "key": pygame.K_LEFT},
        "right": {"type": "key", "key": pygame.K_RIGHT},
        "up": {"type": "key", "key": pygame.K_UP},
        "down": {"type": "key", "key": pygame.K_DOWN},
        "start": {"type": "key", "key": pygame.K_p},
        "clear_history": {"type": "key", "key": pygame.K_x},
        "history": {"type": "key", "key": pygame.K_h},
        "page_up": {"type": "key", "key": pygame.K_PAGEUP},
        "page_down": {"type": "key", "key": pygame.K_PAGEDOWN},
        "filter": {"type": "key", "key": pygame.K_f},
        "delete": {"type": "key", "key": pygame.K_BACKSPACE},
        "space": {"type": "key", "key": pygame.K_SPACE}
    }

    def _is_keyboard_only_config(data):
        if not isinstance(data, dict) or not data:
            return False
        for action_name, mapping in data.items():
            if action_name == "device":
                continue
            if not isinstance(mapping, dict):
                return False
            if mapping.get("type") != "key":
                return False
        return True
    
    try:
        # 1) Fichier utilisateur
        if os.path.exists(path):
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
                if not isinstance(data, dict):
                    data = {}
            keyboard_mode = (not getattr(config, 'joystick', False)) or getattr(config, 'keyboard', False)
            if keyboard_mode and not _is_keyboard_only_config(data):
                logging.getLogger(__name__).info("Configuration utilisateur manette ignorée en mode clavier")
            else:
            # Compléter les actions manquantes, et sauve seulement si le fichier utilisateur existe
                changed = False
                for k, v in default_config.items():
                    if k not in data:
                        data[k] = v
                        changed = True
                if changed:
                    try:
                        os.makedirs(os.path.dirname(path), exist_ok=True)
                        with open(path, "w", encoding="utf-8") as f:
                            json.dump(data, f, indent=2)
                        logging.getLogger(__name__).debug(f"controls.json complété avec les actions manquantes: {path}")
                    except Exception as e:
                        logging.getLogger(__name__).warning(f"Impossible d'écrire les actions manquantes dans {path}: {e}")
                return data

        # 2) Préréglages sans copie si aucun fichier utilisateur
        try:
            # --- Auto-match par nom de périphérique détecté ---
            def _sanitize(s: str) -> str:
                s = (s or "").strip().lower()
                s = re.sub(r"[^a-z0-9]+", "_", s)
                s = re.sub(r"_+", "_", s).strip("_")
                return s

            def _extract_device_from_comment(val: str) -> str:
                try:
                    if not isinstance(val, str):
                        return ""
                    # Expect formats like "# Device: NAME" or just NAME
                    if "Device:" in val:
                        part = val.split("Device:", 1)[1]
                        return part.strip().lstrip('#').strip()
                    return val.strip().lstrip('#').strip()
                except Exception:
                    return ""

            device_name = getattr(config, 'controller_device_name', '') or ''
            if getattr(config, 'joystick', False) and device_name:
                target_norm = _sanitize(device_name)
                try:
                    for fname in os.listdir(config.PRECONF_CONTROLS_PATH):
                        if not fname.lower().endswith('.json'):
                            continue
                        src = os.path.join(config.PRECONF_CONTROLS_PATH, fname)
                        try:
                            with open(src, 'r', encoding='utf-8') as f:
                                preset = json.load(f)
                        except Exception:
                            continue
                        # Match by explicit device field
                        dev_field = preset.get('device') if isinstance(preset, dict) else None
                        if isinstance(dev_field, str) and _sanitize(dev_field) == target_norm:
                            logging.getLogger(__name__).info(f"Chargement préréglage (device) depuis le fichier: {fname}")
                            print(f"Chargement prereglage (device) depuis le fichier: {fname}")
                            return preset
                except Exception as e:
                    logging.getLogger(__name__).warning(f"Échec scan préréglages par device: {e}")

            # Fallback préréglage explicite clavier si pas de joystick
            if not getattr(config, 'joystick', False) or getattr(config, 'keyboard', False):
                src = os.path.join(config.PRECONF_CONTROLS_PATH, 'keyboard.json')
                if os.path.exists(src):
                    with open(src, 'r', encoding='utf-8') as f:
                        data = json.load(f)
                        if isinstance(data, dict) and data:
                            logging.getLogger(__name__).info("Chargement des contrôles préréglés: keyboard.json")
                            return data
        except Exception as e:
            logging.getLogger(__name__).warning(f"Échec du chargement des contrôles préréglés: {e}")

        # 3) Fallback: si joystick présent mais aucun préréglage trouvé, retourner {} pour déclencher le remap
        if getattr(config, 'joystick', False):
            logging.getLogger(__name__).info("Aucun préréglage trouvé pour le joystick connecté, ouverture du remap")
            return {}
        # Sinon, fallback clavier par défaut
        logging.getLogger(__name__).info("Aucun fichier utilisateur ou préréglage trouvé, utilisation des contrôles clavier par défaut")
        return default_config.copy()
    
    except Exception as e:
        logging.getLogger(__name__).error(f"Erreur load_controls_config: {e}")
        return default_config.copy()

def is_input_matched(event, action_name):
    if not config.controls_config.get(action_name):
        return False
    mapping = config.controls_config[action_name]
    input_type = mapping["type"]
    
    if input_type == "key" and event.type == pygame.KEYDOWN:
        return event.key == mapping.get("key")
    elif input_type == "button" and event.type == pygame.JOYBUTTONDOWN:
        return event.button == mapping.get("button")
    elif input_type == "axis" and event.type == pygame.JOYAXISMOTION:
        axis = mapping.get("axis")
        direction = mapping.get("direction")
        threshold = 0.5
        # Pour les triggers Xbox (axes 4 et 5), la position de repos est -1.0
        # Il faut inverser la détection : direction -1 = trigger appuyé (vers +1.0)
        if axis in [4, 5]:
            # Triggers Xbox: repos à -1.0, appuyé vers +1.0
            # On inverse la direction configurée
            if direction == -1:
                # Direction -1 configurée = détecter quand trigger appuyé (valeur positive)
                return event.axis == axis and event.value > threshold
            else:
                # Direction +1 configurée = détecter aussi quand trigger appuyé
                return event.axis == axis and event.value > threshold
        else:
            # Autres axes: logique normale
            return event.axis == axis and abs(event.value) > threshold and (1 if event.value > 0 else -1) == direction
    elif input_type == "hat" and event.type == pygame.JOYHATMOTION:
        hat_value = mapping.get("value")
        if isinstance(hat_value, list):
            hat_value = tuple(hat_value)
        return event.value == hat_value
    elif input_type == "mouse" and event.type == pygame.MOUSEBUTTONDOWN:
        return event.button == mapping.get("button")
    
    # Fallback clavier pour dépannage (fonctionne toujours même avec manette configurée)
    if event.type == pygame.KEYDOWN:
        keyboard_fallback = {
            "up": pygame.K_UP,
            "down": pygame.K_DOWN,
            "left": pygame.K_LEFT,
            "right": pygame.K_RIGHT,
            "confirm": pygame.K_RETURN,
            "cancel": pygame.K_ESCAPE,
            "start": pygame.K_RALT,
            "filter": pygame.K_f,
            "history": pygame.K_h,
            "clear_history": pygame.K_DELETE,
            "delete": pygame.K_d,
            "space": pygame.K_SPACE,
            "page_up": pygame.K_PAGEUP,
            "page_down": pygame.K_PAGEDOWN,
        }
        if action_name in keyboard_fallback:
            return event.key == keyboard_fallback[action_name]
    
    return False

def is_global_search_input_matched(event, action_name):
    """Fallback robuste pour la recherche globale, independant du preset courant."""
    if is_input_matched(event, action_name):
        return True

    if event.type == pygame.KEYDOWN:
        keyboard_fallback = {
            "up": pygame.K_UP,
            "down": pygame.K_DOWN,
            "left": pygame.K_LEFT,
            "right": pygame.K_RIGHT,
            "confirm": pygame.K_RETURN,
            "cancel": pygame.K_ESCAPE,
            "filter": pygame.K_f,
            "delete": pygame.K_BACKSPACE,
            "space": pygame.K_SPACE,
            "page_up": pygame.K_PAGEUP,
            "page_down": pygame.K_PAGEDOWN,
        }
        if action_name in keyboard_fallback and event.key == keyboard_fallback[action_name]:
            return True

    if event.type == pygame.JOYBUTTONDOWN:
        common_button_fallback = {
            "confirm": {0},
            "cancel": {1},
            "filter": {6},
            "start": {7},
            "delete": {2},
            "space": {5},
            "page_up": {4},
            "page_down": {5},
        }
        if action_name in common_button_fallback and event.button in common_button_fallback[action_name]:
            return True

    if event.type == pygame.JOYHATMOTION:
        hat_fallback = {
            "up": (0, 1),
            "down": (0, -1),
            "left": (-1, 0),
            "right": (1, 0),
        }
        if action_name in hat_fallback and event.value == hat_fallback[action_name]:
            return True

    if event.type == pygame.JOYAXISMOTION:
        axis_fallback = {
            "left": (0, -1),
            "right": (0, 1),
            "up": (1, -1),
            "down": (1, 1),
        }
        if action_name in axis_fallback:
            axis_id, direction = axis_fallback[action_name]
            if event.axis == axis_id and abs(event.value) > 0.5 and (1 if event.value > 0 else -1) == direction:
                return True

    return False

def update_key_state(action, pressed, event_type=None, event_value=None):
    """Met à jour l'état d'une touche pour la répétition automatique."""
    current_time = pygame.time.get_ticks()
    
    if pressed:
        # La touche vient d'être pressée
        if action not in key_states:
            key_states[action] = {
                "pressed": True,
                "first_press_time": current_time,
                "last_repeat_time": current_time,
                "event_type": event_type,
                "event_value": event_value
            }
    else:
        # La touche vient d'être relâchée
        if action in key_states:
            del key_states[action]

def clear_joystick_repeat_states() -> None:
    """Supprime les états de répétition issus de la manette.

    Utile quand une manette Bluetooth se déconnecte sans envoyer tous les
    événements de relâchement, afin d'éviter des événements fantômes en boucle.
    """
    joystick_event_types = {
        pygame.JOYBUTTONDOWN,
        pygame.JOYAXISMOTION,
        pygame.JOYHATMOTION,
    }

    for action, state in list(key_states.items()):
        if state.get("event_type") in joystick_event_types:
            del key_states[action]

def process_key_repeats(sources, joystick, screen):
    """Traite la répétition des touches."""
    from controls.handlers import handle_controls  # lazy: controls.input <-> controls.handlers dongusunu onler
    current_time = pygame.time.get_ticks()

    # Si aucune manette active, purge les états de répétition joystick pour
    # éviter la génération d'événements JOY* synthétiques bloquants.
    if not getattr(config, 'joystick', False) or joystick is None:
        clear_joystick_repeat_states()
    
    for action, state in list(key_states.items()):
        if not state["pressed"]:
            continue

        # En l'absence de manette, ignorer les repeats joystick résiduels.
        if state.get("event_type") in (pygame.JOYBUTTONDOWN, pygame.JOYAXISMOTION, pygame.JOYHATMOTION):
            if not getattr(config, 'joystick', False) or joystick is None:
                del key_states[action]
                continue
            
        time_since_first_press = current_time - state["first_press_time"]
        time_since_last_repeat = current_time - state["last_repeat_time"]
        
        # Vérifier si nous devons déclencher une répétition
        if (time_since_first_press > REPEAT_DELAY and 
            time_since_last_repeat > REPEAT_INTERVAL):
            
            # Créer un événement synthétique selon le type
            event_type = state["event_type"]
            event_value = state["event_value"]
            
            if event_type == pygame.KEYDOWN:
                event = pygame.event.Event(pygame.KEYDOWN, {"key": event_value})
            elif event_type == pygame.JOYBUTTONDOWN:
                event = pygame.event.Event(pygame.JOYBUTTONDOWN, {"button": event_value})
            elif event_type == pygame.JOYAXISMOTION:
                axis, value = event_value
                event = pygame.event.Event(pygame.JOYAXISMOTION, {"axis": axis, "value": value})
            elif event_type == pygame.JOYHATMOTION:
                event = pygame.event.Event(pygame.JOYHATMOTION, {"value": event_value})
            else:
                continue  # Type d'événement non pris en charge
            
            # Traiter l'événement répété
            handle_controls(event, sources, joystick, screen)
            
            # Mettre à jour le temps de la dernière répétition
            state["last_repeat_time"] = current_time
            
            # Forcer le redessinage
            config.needs_redraw = True

def get_emergency_controls():
    """Retourne une configuration de contrôles de secours pour permettre la navigation de base."""
    return {
        "confirm": {"type": "key", "key": pygame.K_RETURN},
        "cancel": {"type": "key", "key": pygame.K_ESCAPE},
        "up": {"type": "key", "key": pygame.K_UP},
        "down": {"type": "key", "key": pygame.K_DOWN},
        "left": {"type": "key", "key": pygame.K_LEFT},
        "right": {"type": "key", "key": pygame.K_RIGHT},
        "start": {"type": "key", "key": pygame.K_p},
        "history": {"type": "key", "key": pygame.K_h},
        "clear_history": {"type": "key", "key": pygame.K_x},
        "page_up": {"type": "key", "key": pygame.K_PAGEUP},
        "page_down": {"type": "key", "key": pygame.K_PAGEDOWN},
        # manette basique
        "confirm_joy": {"type": "button", "button": 0},
        "cancel_joy": {"type": "button", "button": 1},
    }

