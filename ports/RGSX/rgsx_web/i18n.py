# -*- coding: utf-8 -*-
"""Traductions web (TRANSLATIONS) + normalisation des tailles."""
import json
import os
import logging

import config
from rgsx_settings import get_language

logger = logging.getLogger("rgsx_web")

# Charger les traductions au démarrage du serveur
def load_translations():
    """Charge les traductions depuis le fichier de langue configuré"""
    language = get_language()  # Lit depuis rgsx_settings.json
    lang_file = os.path.join(config.LANGUAGES_FOLDER, f'{language}.json')

    try:
        with open(lang_file, 'r', encoding='utf-8') as f:
            translations = json.load(f)
            logging.info(f"Traductions chargées : {language} ({len(translations)} clés)")
            return translations
    except FileNotFoundError:
        logging.warning(f"Fichier de langue non trouvé : {lang_file}, utilisation de l'anglais par défaut")
        # Fallback sur l'anglais
        fallback_file = os.path.join(config.LANGUAGES_FOLDER, 'en.json')
        with open(fallback_file, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        logging.error(f"Erreur lors du chargement des traductions : {e}")
        return {}

# Charger les traductions globalement
TRANSLATIONS = load_translations()

# Fonction d'aide pour obtenir une traduction
def get_translation(key, default=None):
    """Obtient une traduction depuis le dictionnaire global TRANSLATIONS"""
    if key in TRANSLATIONS:
        return TRANSLATIONS[key]
    if default is not None:
        return default
    return key

# Fonction pour normaliser les tailles de fichier
def normalize_size(size_str, lang='en'):
    """
    Normalise une taille de fichier dans différents formats (Ko, KiB, Mo, MiB, Go, GiB)
    en un format uniforme selon la langue (MB/GB pour anglais, Mo/Go pour français).
    Exemples: "150 Mo" -> "150 MB" (en), "1.5 Go" -> "1.5 GB" (en), "500 Ko" -> "0.5 MB"
    """
    if not size_str:
        return None

    import re

    # Utiliser regex pour extraire le nombre et l'unité
    match = re.match(r'([0-9.]+)\s*(ko|kio|kib|kb|mo|mio|mib|mb|go|gio|gib|gb)',
                     str(size_str).lower().strip())

    if not match:
        return size_str  # Retourner original si ne correspond pas au format

    try:
        value = float(match.group(1))
        unit = match.group(2).lower()

        # Convertir tout en Mo
        if unit in ['ko', 'kb']:
            value = value / 1024  # Ko en Mo
        elif unit in ['kio', 'kib']:
            value = value / 1024  # KiB en Mo
        elif unit in ['mo', 'mb']:
            pass  # Déjà en Mo
        elif unit in ['mio', 'mib']:
            pass  # MiB ≈ Mo
        elif unit in ['go', 'gb']:
            value = value * 1024  # Go en Mo
        elif unit in ['gio', 'gib']:
            value = value * 1024  # GiB en Mo

        # Déterminer les unités selon la langue
        if lang == 'fr':
            mb_unit = 'Mo'
            gb_unit = 'Go'
        else:
            mb_unit = 'MB'
            gb_unit = 'GB'

        # Afficher en GB/Go si > 1024 Mo, sinon en MB/Mo
        if value >= 1024:
            return f"{value / 1024:.2f} {gb_unit}".replace('.00 ', ' ').rstrip('0').rstrip('.')
        else:
            # Arrondir à 1 décimale pour MB/Mo
            rounded = round(value, 1)
            if rounded == int(rounded):
                return f"{int(rounded)} {mb_unit}"
            else:
                return f"{rounded} {mb_unit}".rstrip('0').rstrip('.')
    except (ValueError, TypeError):
        return size_str  # Retourner original si conversion échoue
