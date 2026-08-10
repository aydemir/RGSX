from pathlib import Path
import collections
import io
import shutil
import requests  # type: ignore
import re
import json
import os
import logging
import platform
import subprocess
import urllib.parse
import config
from config import HEADLESS, Game
try:
    if not HEADLESS:
        import pygame  # type: ignore
    else:
        pygame = None  # type: ignore
except Exception:
    pygame = None  # type: ignore
import glob
import threading
from rgsx_settings import load_rgsx_settings, save_rgsx_settings, get_allow_unknown_extensions
import zipfile
import time
import random
import config
from history import save_history
from language import _ 
from datetime import datetime
import sys
import tempfile
try:
    from PIL import Image  # type: ignore
except Exception:
    Image = None  # type: ignore

logger = logging.getLogger("utils")



def parse_game_size_to_bytes(value) -> int:
    if isinstance(value, (int, float)):
        return max(0, int(value))
    if not isinstance(value, str):
        return 0

    text = value.strip()
    if not text:
        return 0

    if ',' in text and '.' in text:
        text = text.replace(',', '')
    elif ',' in text:
        numeric_part_match = re.match(r'^([0-9][0-9,]*)', text)
        numeric_part = numeric_part_match.group(1) if numeric_part_match else ''
        comma_groups = numeric_part.split(',')
        if len(comma_groups) > 1 and all(group.isdigit() for group in comma_groups) and all(len(group) == 3 for group in comma_groups[1:]):
            text = text.replace(',', '')
        else:
            text = text.replace(',', '.')

    match = re.match(r'^([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z]+)?$', text)
    if not match:
        return 0

    amount = float(match.group(1))
    unit = (match.group(2) or 'B').strip().lower()
    multipliers = {
        'b': 1,
        'byte': 1,
        'bytes': 1,
        'octet': 1,
        'octets': 1,
        'k': 1024,
        'kb': 1024,
        'kib': 1024,
        'ko': 1024,
        'm': 1024 ** 2,
        'mb': 1024 ** 2,
        'mib': 1024 ** 2,
        'mo': 1024 ** 2,
        'g': 1024 ** 3,
        'gb': 1024 ** 3,
        'gib': 1024 ** 3,
        'go': 1024 ** 3,
        't': 1024 ** 4,
        'tb': 1024 ** 4,
        'tib': 1024 ** 4,
        'to': 1024 ** 4,
        'p': 1024 ** 5,
        'pb': 1024 ** 5,
        'pib': 1024 ** 5,
        'po': 1024 ** 5,
    }
    return int(amount * multipliers.get(unit, 0)) if unit in multipliers else 0



def sort_games_list(items: list[Game], option: str = 'name_asc') -> list[Game]:
    reverse = option in ('name_desc', 'size_desc')

    if option.startswith('size_'):
        return sorted(
            items,
            key=lambda game: (
                parse_game_size_to_bytes(game.size),
                str(game.display_name or game.name or '').lower(),
            ),
            reverse=reverse,
        )

    return sorted(
        items,
        key=lambda game: (
            str(game.display_name or game.name or '').lower(),
            parse_game_size_to_bytes(game.size),
        ),
        reverse=reverse,
    )



def sort_games_list_from_settings(items: list[Game]) -> list[Game]:
    try:
        from rgsx_settings import get_global_sort_option

        option = get_global_sort_option()
    except Exception:
        option = 'name_asc'
    return sort_games_list(items, option)
