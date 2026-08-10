"""network.one_fichier — 1fichier (gratuit) indirme + regex/yardimcilar.

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import asyncio
import os
import queue
import re
import threading
import time
import unicodedata
import urllib.parse
import logging
import datetime
from datetime import datetime
import requests  # type: ignore
import html as html_module
from urllib.parse import urljoin, unquote
import config
from history import load_history
from display import show_toast
from language import _  # Import de la fonction de traduction
from utils import (
    sanitize_filename,
    extract_zip,
    extract_rar,
    extract_7z,
    load_api_keys,
    resolve_platform_folder,
    get_clean_display_name,
)
from network import (
    progress_queues,
    cancel_events,
    pause_events,
    download_threads,
    urls_in_progress,
    urls_lock,
    url_results,
    url_done_events,
)
from network.helpers import (
    InsufficientDiskSpaceError,
    _check_history_access_before_download,
    _ensure_sufficient_disk_space,
    _is_ps3_redump_target,
    _postprocess_downloaded_file,
    _save_history_with_feedback,
    _update_history_local_target,
)
from network.http_download import (
    _build_browser_download_headers,
    _http_parse_content_range,
    _http_part_path,
    _http_resume_offset,
    _redact_headers,
)
from network.queue import notify_download_finished

logger = logging.getLogger("network")

WAIT_REGEXES_1F = [
    # Patterns avec multiplication par 60 (minutes -> secondes)
    r'var\s+ct\s*=\s*(\d+)\s*\*\s*60',  # var ct = X * 60;
    r'var\s+ct\s*=\s*(\d+)\s*\*60',     # var ct = X*60;
    # Patterns avec temps en minutes explicite
    r'(?:veuillez\s+)?patiente[rz]\s*(\d+)\s*(?:min|minute)s?\b',
    r'please\s+wait\s*(\d+)\s*(?:min|minute)s?\b',
    # Patterns avec temps en secondes
    r'(?:veuillez\s+)?patiente[rz]\s*(\d+)\s*(?:sec|secondes?|s)\b',
    r'please\s+wait\s*(\d+)\s*(?:sec|seconds?)\b',
    r'var\s+ct\s*=\s*(\d+)\s*;',  # var ct = X;
]
def extract_wait_seconds_1f(html_text):
    """Extrait le temps d'attente depuis le HTML 1fichier"""
    for i, pattern in enumerate(WAIT_REGEXES_1F):
        match = re.search(pattern, html_text, re.IGNORECASE)
        if match:
            value = int(match.group(1))
            # Les deux premiers patterns sont en minutes (avec *60)
            if i < 2 or 'min' in pattern.lower():
                seconds = value * 60
            else:
                seconds = value
            logger.debug(f"1fichier wait time detected: {value} ({'minutes' if i < 2 or 'min' in pattern.lower() else 'seconds'}) = {seconds}s total")
            return seconds
    return 0
def _extract_visible_text_from_html(html_text: str) -> str:
    if not html_text:
        return ""
    text = re.sub(r'(?is)<script[^>]*>.*?</script>', ' ', html_text)
    text = re.sub(r'(?is)<style[^>]*>.*?</style>', ' ', text)
    text = re.sub(r'(?is)<[^>]+>', ' ', text)
    text = html_module.unescape(text).replace('\xa0', ' ')
    return re.sub(r'\s+', ' ', text).strip()
def _normalize_1fichier_text(text: str) -> str:
    if not text:
        return ""
    normalized = unicodedata.normalize("NFKD", text)
    normalized = normalized.encode("ascii", "ignore").decode("ascii")
    return re.sub(r'\s+', ' ', normalized).strip().lower()
def _translate_free_mode_message(key: str, fallback: str) -> str:
    try:
        translated = _(key)
        if translated and translated != key:
            return translated
    except Exception:
        pass
    return fallback
def _append_1fichier_upgrade_advice(message: str) -> str:
    advice = _translate_free_mode_message(
        "free_mode_premium_advice",
        "For unlimited, on-demand, full-speed downloads, you need a premium account or debrid service and must enter its API key in RGSX.",
    )
    base_message = (message or "").strip()
    if not base_message:
        return advice
    return f"{base_message}\n{advice}"
def _extract_1fichier_free_mode_block_reason(html_text: str) -> str | None:
    visible_text = _extract_visible_text_from_html(html_text)
    normalized = _normalize_1fichier_text(visible_text)
    if not normalized:
        return None

    if (
        "telechargement gratuit est temporairement limite" in normalized
        and "identifiez-vous immediatement" in normalized
    ):
        return _append_1fichier_upgrade_advice(
            _translate_free_mode_message(
                "free_mode_guest_slots_unavailable",
                "1fichier: free guest download is temporarily unavailable (all slots are currently in use). Please try again later.",
            )
        )

    if (
        "free download is temporarily limited" in normalized
        and "all free slots for guests are currently used" in normalized
    ):
        return _append_1fichier_upgrade_advice(
            _translate_free_mode_message(
                "free_mode_guest_slots_unavailable",
                "1fichier: free guest download is temporarily unavailable (all slots are currently in use). Please try again later.",
            )
        )

    if "identifiez-vous immediatement pour continuer votre telechargement" in normalized:
        return _append_1fichier_upgrade_advice(
            _translate_free_mode_message(
                "free_mode_unavailable_in_app",
                "1fichier: this download is not available in the application right now. Please try again later.",
            )
        )

    if "sign in immediately to continue your download" in normalized:
        return _append_1fichier_upgrade_advice(
            _translate_free_mode_message(
                "free_mode_unavailable_in_app",
                "1fichier: this download is not available in the application right now. Please try again later.",
            )
        )

    return None
def download_1fichier_free_mode(url, dest_dir, session, log_callback=None, progress_callback=None, wait_callback=None, cancel_event=None):
    """
    Télécharge un fichier depuis 1fichier.com en mode gratuit (sans API key).
    Compatible RGSX - Sans dépendances HTML externes ni httpx.
    
    Args:
        url: URL 1fichier
        dest_dir: Dossier de destination
        session: Session requests
        log_callback: Fonction appelée avec les messages de log
        progress_callback: Fonction appelée avec (filename, downloaded, total, percent)
        wait_callback: Fonction appelée avec (remaining_seconds, total_seconds)
        cancel_event: threading.Event pour annuler le téléchargement
        
    Returns:
        (success: bool, filepath: str|None, error_message: str|None)
    """
    
    def _log(msg):
        if log_callback:
            try:
                log_callback(msg)
            except Exception:
                pass
        logger.info(msg)
    
    def _progress(filename, downloaded, total, pct):
        if progress_callback:
            try:
                progress_callback(filename, downloaded, total, pct)
            except Exception:
                pass
    
    def _wait(remaining, total_wait):
        if wait_callback:
            try:
                wait_callback(remaining, total_wait)
            except Exception:
                pass
    
    try:
        os.makedirs(dest_dir, exist_ok=True)
        _log(_("free_mode_download").format(url))
        
        # 1. GET page initiale
        if cancel_event and cancel_event.is_set():
            return (False, None, "Annulé")
        
        r = session.get(url, allow_redirects=True, timeout=30)
        r.raise_for_status()
        html = r.text
        page_url = str(r.url)
        
        # 2. Détection compte à rebours
        wait_s = extract_wait_seconds_1f(html)
        
        if wait_s > 0:
            _log(f"{wait_s}s...")
            for remaining in range(wait_s, 0, -1):
                if cancel_event and cancel_event.is_set():
                    return (False, None, "Annulé")
                _wait(remaining, wait_s)
                time.sleep(1)
        
        # 3. Chercher formulaire et soumettre
        if cancel_event and cancel_event.is_set():
            return (False, None, "Annulé")
            
        form_match = re.search(r'<form[^>]*id=[\"\']f1[\"\'][^>]*>(.*?)</form>', html, re.DOTALL | re.IGNORECASE)
        
        if form_match:
            form_html = form_match.group(1)
            
            # Extraire les champs
            data = {}
            for inp_match in re.finditer(r'<input[^>]+>', form_html, re.IGNORECASE):
                inp = inp_match.group(0)
                
                name_m = re.search(r'name=[\"\']([^\"\']+)', inp)
                value_m = re.search(r'value=[\"\']([^\"\']*)', inp)
                type_m = re.search(r'type=["\']([^"\']+)', inp, re.IGNORECASE)
                
                if name_m:
                    input_type = type_m.group(1).strip().lower() if type_m else 'text'
                    if input_type in {'checkbox', 'radio'} and 'checked' not in inp.lower():
                        continue
                    name = name_m.group(1)
                    value = value_m.group(1) if value_m else ''
                    data[name] = html_module.unescape(value)
            
            # POST formulaire
            _log(_("free_mode_submitting"))
            html = None
            # Parfois la soumission renvoie une page demandant d'attendre encore (rate-limit) --
            # on retry jusqu'à 3 fois en respectant le temps indiqué dans la page de réponse.
            max_post_attempts = 3
            post_attempt = 0
            while post_attempt < max_post_attempts:
                post_attempt += 1
                try:
                    parsed_page = urllib.parse.urlparse(page_url)
                    post_headers = {
                        'Referer': page_url,
                        'Origin': f"{parsed_page.scheme}://{parsed_page.netloc}" if parsed_page.scheme and parsed_page.netloc else page_url,
                    }
                    r2 = session.post(page_url, data=data, headers=post_headers, allow_redirects=True, timeout=30)
                    r2.raise_for_status()
                    html = r2.text
                    page_url = str(r2.url)
                except Exception as pe:
                    logger.debug(f"1fichier: POST attempt {post_attempt} failed: {pe}")
                    if post_attempt >= max_post_attempts:
                        raise
                    time.sleep(1)
                    continue

                # Vérifier si la page de réponse contient un nouveau compteur d'attente
                extra_wait = extract_wait_seconds_1f(html)
                if extra_wait and extra_wait > 0:
                    logger.info(f"1fichier: Response requests extra wait: {extra_wait}s (attempt {post_attempt})")
                    # Attendre proprement en appelant le callback si fourni
                    for remaining in range(extra_wait, 0, -1):
                        if cancel_event and cancel_event.is_set():
                            return (False, None, "Annulé")
                        _wait(remaining, extra_wait)
                        time.sleep(1)
                    # essayer de soumettre à nouveau après la temporisation
                    continue
                # Pas d'attente supplémentaire demandée, on peut continuer
                break

            if html is None:
                return (False, None, "Erreur lors de la soumission du formulaire")

            blocked_reason = _extract_1fichier_free_mode_block_reason(html)
            if blocked_reason:
                logger.warning(f"1fichier: free mode blocked after form submit: {blocked_reason}")
                return (False, None, blocked_reason)
        
        # 4. Chercher lien de téléchargement
        if cancel_event and cancel_event.is_set():
            return (False, None, "Annulé")
            
        patterns = [
            r'href=[\"\']([^\"\']+)[\"\'][^>]*>(?:cliquer|click|télécharger|download)',
            r'href=[\"\']([^\"\']*/dl/[^\"\']+)',
            r'(https?://[a-z0-9.-]*1fichier\.com/[A-Za-z0-9]{8,})'
        ]
        
        direct_link = None
        candidate_entries: list[tuple[int, str]] = []
        seen_candidates: set[str] = set()

        for anchor_match in re.finditer(r'<a[^>]+href=[\"\']([^\"\']+)[\"\'][^>]*>(.*?)</a>', html, re.IGNORECASE | re.DOTALL):
            href = html_module.unescape(anchor_match.group(1).strip())
            anchor_text = re.sub(r'<[^>]+>', ' ', anchor_match.group(2))
            normalized_anchor_text = _normalize_1fichier_text(anchor_text)
            if not href or not normalized_anchor_text:
                continue
            if not any(token in normalized_anchor_text for token in ('download', 'telecharg', 'tlcharg', 'click', 'cliquer')):
                continue
            candidate = href if href.startswith(('http://', 'https://')) else urljoin(page_url, href)
            if candidate in seen_candidates:
                continue
            seen_candidates.add(candidate)
            candidate_entries.append((0, candidate))

        for idx, pattern in enumerate(patterns):
            for match in re.finditer(pattern, html, re.IGNORECASE):
                try:
                    captured_link = html_module.unescape(match.group(1).strip())
                except (IndexError, AttributeError):
                    logger.warning(f"1fichier: Pattern {idx} matched but no usable capture group(1)")
                    continue
                if not captured_link:
                    continue
                candidate = captured_link if captured_link.startswith(('http://', 'https://')) else urljoin(page_url, captured_link)
                if candidate in seen_candidates:
                    continue
                seen_candidates.add(candidate)
                candidate_entries.append((idx, candidate))

        # Examine each pattern and validate the candidate link via HEAD/GET to avoid landing pages (/register, /login)
        for idx, candidate in candidate_entries:
            logger.debug(f"1fichier: Pattern {idx} matched, candidate link: {candidate}")

            # Quick heuristic: skip known non-download endpoints
            lower = candidate.lower()
            if any(x in lower for x in ['/register', '/login', '/inscription', '/compte', '/subscribe']):
                logger.debug(f"1fichier: Skipping candidate because it looks like a landing page: {candidate}")
                continue

            # Validate with HEAD first to check content-type and status
            try:
                head = session.head(candidate, allow_redirects=True, timeout=10)
                if head.status_code >= 400:
                    logger.debug(f"1fichier: HEAD returned status {head.status_code} for {candidate}, skipping")
                    continue
                ctype = head.headers.get('content-type', '')
                if 'text/html' in ctype.lower():
                    logger.debug(f"1fichier: HEAD content-type is HTML for {candidate}, skipping")
                    # as fallback we'll try a quick GET below
                    raise ValueError('HTML content')
                # Looks like a direct file
                direct_link = candidate
                logger.debug(f"1fichier: Direct link validated via HEAD: {direct_link}")
                break
            except Exception as he:
                # HEAD may be blocked; try a quick GET without streaming
                try:
                    logger.debug(f"1fichier: HEAD failed ({he}), trying quick GET for candidate {candidate}")
                    rtest = session.get(candidate, allow_redirects=True, timeout=10)
                    if rtest.status_code >= 400:
                        logger.debug(f"1fichier: quick GET returned status {rtest.status_code} for {candidate}, skipping")
                        continue
                    ctype = rtest.headers.get('content-type', '')
                    if 'text/html' in ctype.lower() or '<html' in (rtest.text or '').lower():
                        logger.debug(f"1fichier: quick GET appears to be HTML/landing for {candidate}, skipping")
                        continue
                    direct_link = candidate
                    logger.debug(f"1fichier: Direct link validated via quick GET: {direct_link}")
                    break
                except Exception as ge:
                    logger.debug(f"1fichier: quick GET also failed for {candidate}: {ge}")
                    continue

        if not direct_link:
            blocked_reason = _extract_1fichier_free_mode_block_reason(html)
            if blocked_reason:
                logger.warning(f"1fichier: no direct link because free mode is blocked: {blocked_reason}")
                return (False, None, blocked_reason)
            logger.error(f"1fichier: No valid download link found. HTML preview (first 700 chars): {html[:700]}")
            return (False, None, "Lien de téléchargement introuvable")
        
        _log(_("free_mode_link_found").format(direct_link[:60]))
        
        # 5. HEAD pour infos fichier
        if cancel_event and cancel_event.is_set():
            return (False, None, "Annulé")
            
        head = session.head(direct_link, allow_redirects=True, timeout=30)
        
        # Nom fichier
        filename = 'downloaded_file'
        cd = head.headers.get('content-disposition', '')
        if cd:
            fn_match = re.search(r'filename\*?=[\"\']?([^\"\';]+)', cd, re.IGNORECASE)
            if fn_match:
                filename = unquote(fn_match.group(1))
        
        filename = sanitize_filename(filename)
        filepath = os.path.join(dest_dir, filename)
        
        # 6. Téléchargement
        _log(_("free_mode_download").format(filename))
        
        with session.get(direct_link, stream=True, allow_redirects=True, timeout=30) as resp:
            resp.raise_for_status()
            total = int(resp.headers.get('content-length', 0))

            has_space, low_space_message = _ensure_sufficient_disk_space(dest_dir, total)
            if not has_space:
                return (False, None, low_space_message)
            
            with open(filepath, 'wb') as f:
                downloaded = 0
                for chunk in resp.iter_content(chunk_size=128*1024):
                    if cancel_event and cancel_event.is_set():
                        return (False, None, "Annulé")
                    
                    f.write(chunk)
                    downloaded += len(chunk)
                    
                    if total:
                        pct = downloaded / total * 100
                        _progress(filename, downloaded, total, pct)
        
        _log(_("free_mode_completed").format(filepath))
        return (True, filepath, None)
        
    except Exception as e:
        error_msg = f"Error Downloading with free mode: {str(e)}"
        _log(error_msg)
        logger.error(error_msg, exc_info=True)
        return (False, None, error_msg)
async def download_from_1fichier(url, platform, game_name, is_zip_non_supported=False, task_id=None):
    # Charger/rafraîchir les clés API (mtime aware)
    keys_info = load_api_keys()
    config.API_KEY_1FICHIER = keys_info.get('1fichier', '')
    config.API_KEY_ALLDEBRID = keys_info.get('alldebrid', '')
    config.API_KEY_DEBRIDLINK = keys_info.get('debridlink', '')
    config.API_KEY_REALDEBRID = keys_info.get('realdebrid', '')
    config.API_KEY_TORBOX = keys_info.get('torbox', '')
    if not config.API_KEY_1FICHIER and config.API_KEY_ALLDEBRID:
        logger.debug("Clé 1fichier absente, utilisation fallback AllDebrid")
    if not config.API_KEY_1FICHIER and not config.API_KEY_ALLDEBRID and config.API_KEY_DEBRIDLINK:
        logger.debug("Clé 1fichier & AllDebrid absentes, utilisation fallback Debrid-Link")
    if not config.API_KEY_1FICHIER and not config.API_KEY_ALLDEBRID and not config.API_KEY_DEBRIDLINK and config.API_KEY_REALDEBRID:
        logger.debug("Clé 1fichier, AllDebrid & Debrid-Link absentes, utilisation fallback RealDebrid")
    if not config.API_KEY_1FICHIER and not config.API_KEY_ALLDEBRID and not config.API_KEY_DEBRIDLINK and not config.API_KEY_REALDEBRID and config.API_KEY_TORBOX:
        logger.debug("Clé 1fichier, AllDebrid, Debrid-Link & RealDebrid absentes, utilisation fallback TorBox")
    elif not config.API_KEY_1FICHIER and not config.API_KEY_ALLDEBRID and not config.API_KEY_DEBRIDLINK and not config.API_KEY_REALDEBRID and not config.API_KEY_TORBOX:
        logger.debug("Aucune clé API disponible (1fichier, AllDebrid, Debrid-Link, RealDebrid, TorBox)")
    logger.debug(f"Début téléchargement 1fichier: {game_name} depuis {url}, is_zip_non_supported={is_zip_non_supported}, task_id={task_id}")
    logger.debug(
        f"Clé API 1fichier: {'présente' if config.API_KEY_1FICHIER else 'absente'} / "
        f"AllDebrid: {'présente' if config.API_KEY_ALLDEBRID else 'absente'} / "
        f"Debrid-Link: {'présente' if config.API_KEY_DEBRIDLINK else 'absente'} / "
        f"RealDebrid: {'présente' if config.API_KEY_REALDEBRID else 'absente'} / "
        f"TorBox: {'présente' if config.API_KEY_TORBOX else 'absente'} (reloaded={keys_info.get('reloaded')})"
    )
    result = [None, None]
    
    # Vérifier si cette URL est déjà en cours de téléchargement (prévenir les doublons)
    with urls_lock:
        if url in urls_in_progress:
            logger.warning(f"⚠️ Un téléchargement pour cette URL est déjà en cours, attente du résultat: {url}")
            # Créer un événement d'attente si ce n'est pas déjà fait
            if url not in url_done_events:
                url_done_events[url] = threading.Event()
            done_event = url_done_events[url]
        else:
            # Ajouter l'URL au set en cours
            urls_in_progress.add(url)
            done_event = None
    
    # Si on attendait un doublon, on attend ici
    if done_event is not None:
        logger.debug(f"Attente de la fin du téléchargement en doublon pour {url}")
        # Attendre de manière asynchrone l'événement (timeout de 30 minutes pour les gros fichiers)
        start_wait = time.time()
        while not done_event.is_set():
            if time.time() - start_wait > 1800:  # 30 minutes timeout
                logger.warning(f"Timeout d'attente pour le doublon de {url}")
                break
            await asyncio.sleep(0.1)
        # Vérifier si on a un résultat en cache
        if url in url_results:
            logger.info(f"Résultat en cache pour {url}: {url_results[url]}")
            try:
                notify_download_finished()
            except Exception:
                pass
            return url_results[url]
        else:
            # Fallback: retourner un message de succès (le premier téléchargement a probablement réussi)
            try:
                notify_download_finished()
            except Exception:
                pass
            return (True, _("network_download_ok").format(game_name))

        # Ajouter l'URL au set en cours
        urls_in_progress.add(url)

    # Créer une queue spécifique pour cette tâche
    logger.debug(f"Création queue pour task_id={task_id}")
    if task_id not in progress_queues:
        progress_queues[task_id] = queue.Queue()
    if task_id not in cancel_events:
        cancel_events[task_id] = threading.Event()

    provider_used = None  # '1F', 'AD', 'DL', 'RD', 'TB'

    def _set_provider_in_history(pfx: str):
        try:
            if not pfx:
                return
            if isinstance(config.history, list):
                for entry in config.history:
                    if entry.get("url") == url:
                        entry["provider"] = pfx
                        entry["provider_prefix"] = f"{pfx}:"
                        _save_history_with_feedback("download_1fichier:set_provider")
                        config.needs_redraw = True
                        break
        except Exception:
            pass

    def download_thread():
        logger.debug(f"Thread téléchargement 1fichier démarré pour {url}, task_id={task_id}")
        # Assurer l'accès à provider_used dans cette closure (lecture/écriture)
        nonlocal provider_used

        def _refresh_alldebrid_final_url(current_link):
            """Request a fresh AllDebrid unlock URL after transient download failures."""
            ad_key = getattr(config, 'API_KEY_ALLDEBRID', '')
            if not ad_key:
                return None, None
            params = {'agent': 'RGSX', 'apikey': ad_key, 'link': current_link}
            refresh_resp = requests.get("https://api.alldebrid.com/v4/link/unlock", params=params, timeout=30)
            refresh_resp.raise_for_status()
            refresh_json = refresh_resp.json()
            if refresh_json.get('status') != 'success':
                logger.warning(f"AllDebrid refresh status != success: {refresh_json}")
                return None, None
            refresh_data = refresh_json.get('data', {})
            return (
                refresh_data.get('link') or refresh_data.get('download') or refresh_data.get('streamingLink'),
                refresh_data.get('filename'),
            )

        try:
            _check_history_access_before_download("download_1fichier")
            cancel_ev = cancel_events.get(task_id)
            link = url.split('&af=')[0]
            logger.debug(f"URL nettoyée: {link}")
            
            # IMPORTANT: Créer l'entrée dans config.history dès le début avec status "Downloading"
            # pour que l'interface web puisse afficher le téléchargement en cours

            # Charger l'historique existant depuis le fichier
            if not isinstance(config.history, list):
                config.history = load_history()
            
            # Vérifier si l'entrée existe déjà
            entry_exists = False
            for entry in config.history:
                if entry.get("url") == url:
                    entry_exists = True
                    # Réinitialiser le status à "Downloading"
                    entry["status"] = "Downloading"
                    entry["progress"] = 0
                    entry["downloaded_size"] = 0
                    entry["platform"] = platform
                    entry["game_name"] = game_name
                    entry["display_name"] = get_clean_display_name(game_name, platform)
                    entry["timestamp"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                    entry["task_id"] = task_id
                    break
            
            # Si l'entrée n'existe pas, la créer
            if not entry_exists:
                config.history.append({
                    "platform": platform,
                    "game_name": game_name,
                    "display_name": get_clean_display_name(game_name, platform),
                    "url": url,
                    "status": "Downloading",
                    "progress": 0,
                    "downloaded_size": 0,
                    "total_size": 0,
                    "speed": 0,
                    "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                    "message": f"Téléchargement 1fichier de {game_name}",
                    "task_id": task_id
                })
            
            # Sauvegarder history.json immédiatement
            _save_history_with_feedback("download_1fichier:init")
            
            # Use symlink path if enabled
            from rgsx_settings import apply_symlink_path, get_platform_custom_path
            
            # Vérifier si un dossier personnalisé est configuré pour cette plateforme
            custom_path = get_platform_custom_path(platform)
            if custom_path and os.path.isdir(custom_path):
                dest_dir = custom_path
                logger.debug(f"Utilisation du dossier personnalisé pour {platform}: {dest_dir}")
                platform_folder = os.path.basename(dest_dir)
            else:
                dest_dir = None
                platform_folder = None
                for platform_dict in config.platform_dicts:
                    if platform_dict.get("platform_name") == platform:
                        platform_folder = platform_dict.get("folder") or platform_dict.get("dossier") or resolve_platform_folder(platform)
                        dest_dir = apply_symlink_path(config.ROMS_FOLDER, platform_folder)
                        break
                if not dest_dir:
                    logger.warning(f"Aucun dossier 'folder'/'dossier' trouvé pour la plateforme {platform}")
                    platform_folder = resolve_platform_folder(platform)
                    dest_dir = apply_symlink_path(config.ROMS_FOLDER, platform_folder)
            logger.debug(f"Répertoire destination déterminé: {dest_dir}")

            # Spécifique: si le système est "- BIOS by TMCTV -" on force le dossier BIOS
            if platform_folder == "bios" or platform == "BIOS" or platform == "- BIOS by TMCTV -":
                dest_dir = config.USERDATA_FOLDER
                logger.debug(f"Plateforme '- BIOS by TMCTV -' détectée, destination forcée vers USERDATA_FOLDER: {dest_dir}")

            logger.debug(f"Vérification répertoire destination: {dest_dir}")
            os.makedirs(dest_dir, exist_ok=True)
            logger.debug(f"Répertoire créé ou existant: {dest_dir}")
            if not os.access(dest_dir, os.W_OK):
                logger.error(f"Pas de permission d'écriture dans {dest_dir}")
                raise PermissionError(f"Pas de permission d'écriture dans {dest_dir}")

            final_url = None
            filename = game_name
            onefichier_error_message = None
            provider_download_session = requests.Session()
            provider_download_headers = _build_browser_download_headers()
            provider_download_session.headers.update(provider_download_headers)

            # Choisir la stratégie d'accès: 1fichier direct via API, sinon AllDebrid pour débrider
            if config.API_KEY_1FICHIER:
                logger.debug("Mode téléchargement sélectionné: 1fichier (API directe)")
                headers = {
                    "Authorization": f"Bearer {config.API_KEY_1FICHIER}",
                    "Content-Type": "application/json"
                }
                payload = {
                    "url": link,
                    "pretty": 1
                }
                logger.debug(f"Préparation requête 1fichier file/info pour {link}")
                response = requests.post("https://api.1fichier.com/v1/file/info.cgi", headers=headers, json=payload, timeout=30)
                logger.debug(f"Réponse file/info reçue, code: {response.status_code}")
                file_info = None
                raw_fileinfo_text = None
                try:
                    raw_fileinfo_text = response.text
                except Exception:
                    pass
                try:
                    file_info = response.json()
                except Exception:
                    file_info = None
                if response.status_code != 200:
                    # 403 souvent = clé invalide ou accès interdit
                    friendly = None
                    raw_err = None
                    if isinstance(file_info, dict):
                        raw_err = file_info.get('message') or file_info.get('error') or file_info.get('status')
                        if raw_err == 'Bad token':
                            friendly = "1F: Clé API 1fichier invalide"
                        elif raw_err:
                            friendly = f"1F: {raw_err}"
                    if not friendly:
                        if response.status_code == 403:
                            friendly = "1F: Accès refusé (403)"
                        elif response.status_code == 401:
                            friendly = "1F: Non autorisé (401)"
                        else:
                            friendly = f"1F: Erreur HTTP {response.status_code}"
                    result[0] = False
                    result[1] = friendly
                    try:
                        result.append({"raw_error_1fichier_fileinfo": raw_err or raw_fileinfo_text})
                    except Exception:
                        pass
                    onefichier_error_message = friendly
                    logger.warning(f"Échec API 1fichier file/info, fallback providers activé: {friendly}")
                if response.status_code == 200:
                    file_info = file_info if isinstance(file_info, dict) else {}
                    if "error" in file_info and file_info["error"] == "Resource not found":
                        logger.error(f"Le fichier {game_name} n'existe pas sur 1fichier")
                        result[0] = False
                        try:
                            if _:
                                not_found_tpl = _("network_file_not_found")
                                msg_nf = not_found_tpl.format(game_name) if "{" in not_found_tpl else f"{not_found_tpl} {game_name}"
                                result[1] = f"1F: {msg_nf}"
                            else:
                                result[1] = f"1F: File not found {game_name}"
                        except Exception:
                            result[1] = f"1F: File not found {game_name}"
                        onefichier_error_message = result[1]
                        logger.warning("Ressource introuvable via API 1fichier, fallback providers activé")
                    else:
                        filename = file_info.get("filename", "").strip()
                        if not filename:
                            logger.error("Impossible de récupérer le nom du fichier")
                            result[0] = False
                            result[1] = _("network_cannot_get_filename")
                            onefichier_error_message = result[1]
                            logger.warning("Nom de fichier 1fichier introuvable, fallback providers activé")
                        else:
                            sanitized_filename = sanitize_filename(filename)
                            dest_path = os.path.join(dest_dir, sanitized_filename)
                            logger.info(f"Chemin destination: {dest_path}")
                            _update_history_local_target(url, task_id, dest_path)

                            remote_size = None
                            try:
                                remote_size = file_info.get("size")
                                if isinstance(remote_size, str):
                                    remote_size = int(remote_size)
                                logger.debug(f"Taille du fichier 1fichier: {remote_size} octets")
                            except Exception as e:
                                logger.debug(f"Impossible de récupérer la taille 1fichier: {e}")

                            file_found = False
                            if os.path.exists(dest_path):
                                logger.info(f"Le fichier {dest_path} existe déjà, vérification de la taille...")
                                local_size = os.path.getsize(dest_path)
                                logger.debug(f"Taille du fichier local: {local_size} octets")
                                if remote_size is not None and local_size != remote_size:
                                    logger.warning(f"Taille mismatch! Local: {local_size}, Remote: {remote_size} - le fichier sera re-téléchargé")
                                    try:
                                        if os.path.exists(dest_path):
                                            os.remove(dest_path)
                                            logger.info(f"Fichier incomplet supprimé: {dest_path}")
                                        else:
                                            logger.debug(f"Fichier déjà supprimé par un autre thread: {dest_path}")
                                    except FileNotFoundError:
                                        logger.debug(f"Fichier déjà supprimé (ou n'existe plus): {dest_path}")
                                    except Exception as e:
                                        logger.error(f"Impossible de supprimer le fichier incomplet: {e}")
                                        result[0] = False
                                        result[1] = f"Erreur suppression fichier incomplet: {str(e)}"
                                        return
                                else:
                                    logger.info(f"Le fichier {dest_path} existe déjà et la taille est correcte, téléchargement ignoré")
                                    result[0] = True
                                    result[1] = _("network_download_ok").format(game_name) + _("download_already_present")
                                    try:
                                        show_toast(result[1])
                                    except Exception as e:
                                        logger.debug(f"Impossible d'afficher le toast: {e}")
                                    with urls_lock:
                                        urls_in_progress.discard(url)
                                        logger.debug(f"URL supprimée du set des téléchargements en cours: {url} (URLs restantes: {len(urls_in_progress)})")
                                    return result[0], result[1]
                                file_found = True

                            if not file_found:
                                base_name_no_ext = os.path.splitext(sanitized_filename)[0]
                                if base_name_no_ext != sanitized_filename:
                                    try:
                                        if os.path.exists(dest_dir):
                                            for existing_file in os.listdir(dest_dir):
                                                existing_base = os.path.splitext(existing_file)[0]
                                                if existing_base == base_name_no_ext:
                                                    existing_path = os.path.join(dest_dir, existing_file)
                                                    logger.info(f"Un fichier avec le même nom de base existe: {existing_path}, vérification de la taille...")
                                                    local_size = os.path.getsize(existing_path)
                                                    logger.debug(f"Taille du fichier local (extension différente): {local_size} octets")
                                                    if remote_size is not None and local_size != remote_size:
                                                        logger.warning(f"Taille mismatch (extension différente)! Local: {local_size}, Remote: {remote_size} - re-téléchargement")
                                                        break
                                                    else:
                                                        logger.info(f"Un fichier avec le même nom de base existe déjà: {existing_path}, téléchargement ignoré")
                                                        result[0] = True
                                                        result[1] = _("network_download_ok").format(game_name) + _("download_already_extracted")
                                                        try:
                                                            show_toast(result[1])
                                                        except Exception as e:
                                                            logger.debug(f"Impossible d'afficher le toast: {e}")
                                                        with urls_lock:
                                                            urls_in_progress.discard(url)
                                                            logger.debug(f"URL supprimée du set des téléchargements en cours: {url} (URLs restantes: {len(urls_in_progress)})")
                                                        return result[0], result[1]
                                    except Exception as e:
                                        logger.debug(f"Erreur lors de la vérification des fichiers existants: {e}")

                            logger.debug(f"Envoi requête 1fichier get_token pour {link}")
                            response = requests.post("https://api.1fichier.com/v1/download/get_token.cgi", headers=headers, json=payload, timeout=30)
                            status_1f = response.status_code
                            raw_text_1f = None
                            try:
                                raw_text_1f = response.text
                            except Exception:
                                pass
                            logger.debug(f"Réponse get_token reçue, code: {status_1f} body_snippet={(raw_text_1f[:120] + '...') if raw_text_1f and len(raw_text_1f) > 120 else raw_text_1f}")
                            download_info = None
                            try:
                                download_info = response.json()
                            except Exception:
                                download_info = None
                            if status_1f != 200:
                                friendly_1f = None
                                raw_error_1f = None
                                if isinstance(download_info, dict):
                                    raw_error_1f = download_info.get('message') or download_info.get('status')
                                    ONEFICHIER_ERROR_MAP = {
                                        "Bad token": "1F: Clé API invalide",
                                        "Must be a customer (Premium, Access) #236": "1F: Compte Premium requis",
                                    }
                                    if raw_error_1f:
                                        friendly_1f = ONEFICHIER_ERROR_MAP.get(raw_error_1f)
                                if not friendly_1f:
                                    if status_1f == 403:
                                        friendly_1f = "1F: Accès refusé (403)"
                                    elif status_1f == 401:
                                        friendly_1f = "1F: Non autorisé (401)"
                                    elif status_1f >= 500:
                                        friendly_1f = f"1F: Erreur serveur ({status_1f})"
                                    else:
                                        friendly_1f = f"1F: Erreur ({status_1f})"
                                result[0] = False
                                result[1] = friendly_1f
                                try:
                                    result.append({"raw_error_1fichier": raw_error_1f or raw_text_1f})
                                except Exception:
                                    pass
                                onefichier_error_message = friendly_1f
                                logger.warning(f"Échec API 1fichier get_token, fallback providers activé: {friendly_1f}")
                            else:
                                response.raise_for_status()
                                if not isinstance(download_info, dict):
                                    logger.error("Réponse 1fichier inattendue (pas un JSON) pour get_token")
                                    result[0] = False
                                    result[1] = _("network_api_error").format("1fichier invalid JSON") if _ else "1fichier invalid JSON"
                                    onefichier_error_message = result[1]
                                else:
                                    final_url = download_info.get("url")
                                    if not final_url:
                                        logger.error("Impossible de récupérer l'URL de téléchargement")
                                        result[0] = False
                                        result[1] = _("network_cannot_get_download_url")
                                        onefichier_error_message = result[1]
                                    else:
                                        logger.debug(f"URL de téléchargement obtenue via 1fichier: {final_url}")
                                        provider_used = '1F'
                                        _set_provider_in_history(provider_used)

            if not final_url:
                # Tentative AllDebrid
                if getattr(config, 'API_KEY_ALLDEBRID', ''):
                    logger.debug("Mode téléchargement sélectionné: AllDebrid (fallback 1)")
                    try:
                        ad_key = config.API_KEY_ALLDEBRID
                        params = {'agent': 'RGSX', 'apikey': ad_key, 'link': link}
                        logger.debug("Requête AllDebrid link/unlock en cours")
                        response = requests.get("https://api.alldebrid.com/v4/link/unlock", params=params, timeout=30)
                        logger.debug(f"Réponse AllDebrid reçue, code: {response.status_code}")
                        response.raise_for_status()
                        ad_json = response.json()
                        if ad_json.get('status') == 'success':
                            data = ad_json.get('data', {})
                            filename = data.get('filename') or game_name
                            final_url = data.get('link') or data.get('download') or data.get('streamingLink')
                            if final_url:
                                logger.debug("Débridage réussi via AllDebrid")
                                provider_used = 'AD'
                                _set_provider_in_history(provider_used)
                        else:
                            logger.warning(f"AllDebrid status != success: {ad_json}")
                    except Exception as e:
                        logger.error(f"Erreur AllDebrid fallback: {e}")
                # Tentative Debrid-Link si pas de final_url
                if not final_url and getattr(config, 'API_KEY_DEBRIDLINK', ''):
                    logger.debug("Tentative fallback Debrid-Link (downloader/add)")
                    try:
                        dl_key = config.API_KEY_DEBRIDLINK
                        headers_dl = {
                            "Authorization": f"Bearer {dl_key}",
                            "Content-Type": "application/json",
                        }
                        payload_dl = {"url": link}
                        dl_resp = requests.post(
                            "https://debrid-link.com/api/v2/downloader/add",
                            json=payload_dl,
                            headers=headers_dl,
                            timeout=30
                        )
                        dl_status = dl_resp.status_code
                        raw_text_dl = None
                        dl_json = None
                        try:
                            raw_text_dl = dl_resp.text
                        except Exception:
                            pass
                        try:
                            dl_json = dl_resp.json()
                        except Exception:
                            dl_json = None
                        logger.debug(f"Réponse Debrid-Link code={dl_status} body_snippet={(raw_text_dl[:120] + '...') if raw_text_dl and len(raw_text_dl) > 120 else raw_text_dl}")

                        DEBRIDLINK_ERROR_MAP = {
                            "badToken": "DL: Invalid API key",
                            "notDebrid": "DL: Host unavailable",
                            "hostNotValid": "DL: Unsupported host",
                            "fileNotFound": "DL: File not found",
                            "fileNotAvailable": "DL: File temporarily unavailable",
                            "badFileUrl": "DL: Invalid link",
                            "badFilePassword": "DL: Invalid file password",
                            "notFreeHost": "DL: Premium account only",
                            "maintenanceHost": "DL: Host in maintenance",
                            "noServerHost": "DL: No server available",
                            "maxLink": "DL: Daily link limit reached",
                            "maxLinkHost": "DL: Daily host limit reached",
                            "maxData": "DL: Daily data limit reached",
                            "maxDataHost": "DL: Daily host data limit reached",
                            "disabledServerHost": "DL: Server or VPN not allowed",
                            "floodDetected": "DL: Rate limit reached",
                        }

                        error_message = None
                        error_message_raw = None
                        if dl_json and isinstance(dl_json, dict):
                            if dl_json.get('success') is True:
                                value = dl_json.get('value') or {}
                                if isinstance(value, dict):
                                    final_url = value.get('downloadUrl') or value.get('downloadURL') or value.get('link') or value.get('url')
                                    filename = value.get('name') or value.get('filename') or filename or game_name
                            else:
                                error_code = dl_json.get('error')
                                if error_code:
                                    error_message = DEBRIDLINK_ERROR_MAP.get(error_code, f"DL: {error_code}")
                                    error_message_raw = str(error_code)
                        if dl_status in (200, 201) and final_url:
                            logger.debug("Débridage réussi via Debrid-Link")
                            provider_used = 'DL'
                            _set_provider_in_history(provider_used)
                        elif not final_url:
                            if not error_message:
                                if dl_status == 401:
                                    error_message = "DL: Unauthorized (401)"
                                elif dl_status == 429:
                                    error_message = "DL: Rate limited (429)"
                                elif dl_status >= 500:
                                    error_message = f"DL: Server error ({dl_status})"
                                else:
                                    error_message = f"DL: Unexpected status ({dl_status})"
                                error_message_raw = raw_text_dl or error_message
                            logger.warning(f"Debrid-Link fallback échec: {error_message}")
                            result[0] = False
                            result[1] = error_message
                            try:
                                if isinstance(result, list):
                                    result.append({"raw_error_debridlink": error_message_raw})
                            except Exception:
                                pass
                    except Exception as e:
                        logger.error(f"Exception Debrid-Link fallback: {e}")
                # Tentative RealDebrid si pas de final_url
                if not final_url and getattr(config, 'API_KEY_REALDEBRID', ''):
                    logger.debug("Tentative fallback RealDebrid (unlock)")
                    try:
                        rd_key = config.API_KEY_REALDEBRID
                        headers_rd = {"Authorization": f"Bearer {rd_key}"}
                        rd_resp = requests.post(
                            "https://api.real-debrid.com/rest/1.0/unrestrict/link",
                            data={"link": link},
                            headers=headers_rd,
                            timeout=30
                        )
                        status = rd_resp.status_code
                        raw_text = None
                        rd_json = None
                        try:
                            raw_text = rd_resp.text
                        except Exception:
                            pass
                        # Tenter JSON même si statut != 200
                        try:
                            rd_json = rd_resp.json()
                        except Exception:
                            rd_json = None
                        logger.debug(f"Réponse RealDebrid code={status} body_snippet={(raw_text[:120] + '...') if raw_text and len(raw_text) > 120 else raw_text}")

                        # Mapping erreurs RD (liste partielle, extensible)
                        REALDEBRID_ERROR_MAP = {
                            # Values intentionally WITHOUT prefix; we'll add 'RD:' dynamically
                            1: "Bad request",
                            2: "Unsupported hoster",
                            3: "Temporarily unavailable",
                            4: "File not found",
                            5: "Too many requests",
                            6: "Access denied",
                            8: "Not premium account",
                            9: "No traffic left",
                            11: "Internal error",
                            20: "Premium account only",  # normalisation wording
                        }

                        error_code = None
                        error_message = None            # Friendly / mapped message (to display in history)
                        error_message_raw = None        # Raw provider message ('error') kept for debugging if needed
                        if rd_json and isinstance(rd_json, dict):
                            # Format attendu quand erreur: {'error_code': int, 'error': 'message'}
                            error_code = rd_json.get('error_code') or rd_json.get('error') if isinstance(rd_json.get('error'), int) else rd_json.get('error_code')
                            if isinstance(error_code, str) and error_code.isdigit():
                                error_code = int(error_code)
                            api_error_text = rd_json.get('error') if isinstance(rd_json.get('error'), str) else None
                            if error_code is not None:
                                mapped = REALDEBRID_ERROR_MAP.get(error_code)
                                # Raw API error sometimes returns 'hoster_not_free' while code=20
                                if api_error_text and api_error_text.strip().lower() == 'hoster_not_free':
                                    api_error_text = 'Premium account only'
                                if mapped and not mapped.lower().startswith('rd:'):
                                    mapped = f"RD: {mapped}"
                                if not mapped and api_error_text and not api_error_text.lower().startswith('rd:'):
                                    api_error_text = f"RD: {api_error_text}"
                                error_message = mapped or api_error_text or f"RD: error {error_code}"
                                # Conserver la version brute séparément
                                error_message_raw = api_error_text if api_error_text and api_error_text != error_message else None
                        # Succès si 200 et presence 'download'
                        if status == 200 and rd_json and rd_json.get('download'):
                            final_url = rd_json.get('download')
                            filename = rd_json.get('filename') or filename or game_name
                            logger.debug("Débridage réussi via RealDebrid")
                            provider_used = 'RD'
                            _set_provider_in_history(provider_used)
                        else:
                            if error_message:
                                logger.warning(f"RealDebrid a renvoyé une erreur (code interne {error_code}): {error_message}")
                            else:
                                # Pas d'erreur structurée -> traiter statut HTTP
                                if status == 503:
                                    error_message = "RD: service unavailable (503)"
                                elif status >= 500:
                                    error_message = f"RD: server error ({status})"
                                elif status == 429:
                                    error_message = "RD: rate limited (429)"
                                else:
                                    error_message = f"RD: unexpected status ({status})"
                                logger.warning(f"RealDebrid fallback échec: {error_message}")
                                # Pas de détail JSON -> utiliser friendly comme raw aussi
                                error_message_raw = error_message
                            # Conserver message dans result si aucun autre provider ne réussit
                            if not final_url:
                                # Marquer le provider même en cas d'erreur pour affichage du préfixe dans l'historique
                                if provider_used is None:
                                    provider_used = 'RD'
                                    _set_provider_in_history(provider_used)
                                result[0] = False
                                # Pour l'interface: stocker le message friendly en priorité
                                result[1] = error_message or error_message_raw
                                # Stocker la version brute pour éventuel usage avancé
                                try:
                                    if isinstance(result, list):
                                        # Ajouter un dict auxiliaire pour meta erreurs
                                        result.append({"raw_error_realdebrid": error_message_raw})
                                except Exception:
                                    pass
                    except Exception as e:
                        logger.error(f"Exception RealDebrid fallback: {e}")
                # Tentative TorBox si pas de final_url
                if not final_url and getattr(config, 'API_KEY_TORBOX', ''):
                    logger.debug("Tentative fallback TorBox (webdl/createwebdownload)")
                    try:
                        import hashlib as _hashlib
                        tb_key = config.API_KEY_TORBOX
                        headers_tb = {"Authorization": f"Bearer {tb_key}"}

                        TORBOX_ERROR_MAP = {
                            "BAD_TOKEN": "TB: Invalid API key",
                            "AUTH_ERROR": "TB: Authentication error",
                            "NO_AUTH": "TB: No credentials provided",
                            "PLAN_RESTRICTED_FEATURE": "TB: Plan upgrade required",
                            "DOWNLOAD_TOO_LARGE": "TB: Download too large for plan",
                            "MONTHLY_LIMIT": "TB: Monthly limit reached",
                            "COOLDOWN_LIMIT": "TB: Download cooldown active",
                            "ACTIVE_LIMIT": "TB: Max active downloads reached",
                            "LINK_OFFLINE": "TB: Link offline or inaccessible",
                            "ITEM_NOT_FOUND": "TB: Item not found",
                            "NO_SERVERS_AVAILABLE_ERROR": "TB: No servers available",
                            "DOWNLOAD_SERVER_ERROR": "TB: Download server error",
                        }

                        error_message = None
                        error_message_raw = None
                        tb_webdl_id = None

                        # Étape 0: Vérifier le cache (lien déjà disponible instantanément)
                        link_hash = _hashlib.md5(link.encode()).hexdigest()
                        try:
                            tb_cache_resp = requests.get(
                                "https://api.torbox.app/v1/api/webdl/checkcached",
                                params={"hash": link_hash, "format": "list"},
                                headers=headers_tb,
                                timeout=15
                            )
                            tb_cache_json = tb_cache_resp.json()
                            if tb_cache_json.get('success') and tb_cache_json.get('data'):
                                logger.debug("TorBox: lien trouvé en cache, téléchargement immédiat possible")
                        except Exception as cache_e:
                            logger.debug(f"TorBox checkcached error (non-fatal): {cache_e}")

                        # Étape 1: Créer le web download
                        tb_create_resp = requests.post(
                            "https://api.torbox.app/v1/api/webdl/createwebdownload",
                            data={"link": link},
                            headers=headers_tb,
                            timeout=30
                        )
                        tb_create_status = tb_create_resp.status_code
                        raw_text_tb = None
                        tb_create_json = None
                        try:
                            raw_text_tb = tb_create_resp.text
                        except Exception:
                            pass
                        try:
                            tb_create_json = tb_create_resp.json()
                        except Exception:
                            tb_create_json = None
                        logger.debug(f"Réponse TorBox createwebdownload code={tb_create_status} body_snippet={(raw_text_tb[:120] + '...') if raw_text_tb and len(raw_text_tb) > 120 else raw_text_tb}")

                        if tb_create_json and isinstance(tb_create_json, dict):
                            if tb_create_json.get('success') is True:
                                tb_data = tb_create_json.get('data', {})
                                if isinstance(tb_data, dict):
                                    tb_webdl_id = tb_data.get('webdl_id') or tb_data.get('id')
                                    tb_hash = tb_data.get('hash', '')
                            elif tb_create_json.get('error') == 'DUPLICATE_ITEM':
                                # Le lien a déjà été soumis, récupérer l'ID existant
                                logger.debug("TorBox: DUPLICATE_ITEM - récupération du download existant")
                                tb_dup_data = tb_create_json.get('data', {})
                                if isinstance(tb_dup_data, dict):
                                    tb_webdl_id = tb_dup_data.get('webdl_id') or tb_dup_data.get('id')
                                if not tb_webdl_id:
                                    # Chercher dans la liste par hash
                                    try:
                                        tb_find_resp = requests.get(
                                            "https://api.torbox.app/v1/api/webdl/mylist",
                                            headers=headers_tb,
                                            timeout=30
                                        )
                                        tb_find_json = tb_find_resp.json()
                                        if tb_find_json.get('success') and tb_find_json.get('data'):
                                            tb_find_list = tb_find_json['data']
                                            if isinstance(tb_find_list, list):
                                                for item in tb_find_list:
                                                    if item.get('hash') == link_hash or item.get('original_url') == link:
                                                        tb_webdl_id = item.get('id')
                                                        break
                                    except Exception as find_e:
                                        logger.debug(f"TorBox find existing error: {find_e}")
                            else:
                                tb_err_code = tb_create_json.get('error', '')
                                error_message = TORBOX_ERROR_MAP.get(tb_err_code, f"TB: {tb_create_json.get('detail', tb_err_code)}")
                                error_message_raw = str(tb_err_code)

                        if tb_webdl_id is not None and not error_message:
                            # Étape 2: Attendre que le téléchargement soit prêt (polling)
                            max_wait = 120  # secondes max d'attente
                            poll_interval = 3  # secondes entre chaque vérification
                            start_wait = time.time()
                            tb_ready = False

                            while time.time() - start_wait < max_wait:
                                try:
                                    tb_list_resp = requests.get(
                                        "https://api.torbox.app/v1/api/webdl/mylist",
                                        params={"id": tb_webdl_id},
                                        headers=headers_tb,
                                        timeout=30
                                    )
                                    tb_list_json = tb_list_resp.json()
                                    if tb_list_json.get('success') and tb_list_json.get('data'):
                                        tb_item = tb_list_json['data']
                                        if isinstance(tb_item, list):
                                            tb_item = tb_item[0] if tb_item else {}
                                        tb_dl_state = tb_item.get('download_state', '')
                                        tb_dl_finished = tb_item.get('download_finished', False)
                                        logger.debug(f"TorBox webdl status: download_state={tb_dl_state}, finished={tb_dl_finished}")
                                        if tb_dl_state in ('cached', 'completed', 'uploading', 'done') or tb_dl_finished:
                                            tb_ready = True
                                            filename = tb_item.get('name') or tb_item.get('original_name') or filename or game_name
                                            break
                                        elif tb_dl_state in ('error', 'failed', 'stalled'):
                                            error_message = f"TB: Download failed ({tb_dl_state})"
                                            break
                                except Exception as poll_e:
                                    logger.debug(f"TorBox poll error: {poll_e}")
                                time.sleep(poll_interval)

                            if tb_ready:
                                # Étape 3: Demander le lien de téléchargement
                                try:
                                    tb_dl_resp = requests.get(
                                        "https://api.torbox.app/v1/api/webdl/requestdl",
                                        params={"token": tb_key, "web_id": tb_webdl_id, "file_id": 0},
                                        timeout=30
                                    )
                                    tb_dl_json = tb_dl_resp.json()
                                    if tb_dl_json.get('success') and tb_dl_json.get('data'):
                                        final_url = tb_dl_json['data']
                                        logger.debug("Débridage réussi via TorBox")
                                        provider_used = 'TB'
                                        _set_provider_in_history(provider_used)
                                    else:
                                        tb_err = tb_dl_json.get('error', '')
                                        error_message = TORBOX_ERROR_MAP.get(tb_err, f"TB: {tb_dl_json.get('detail', tb_err)}")
                                        error_message_raw = str(tb_err)
                                except Exception as dl_e:
                                    logger.error(f"TorBox requestdl error: {dl_e}")
                            elif not error_message:
                                error_message = "TB: Download not ready (timeout)"
                        elif not error_message:
                            if tb_webdl_id is None:
                                if tb_create_status == 403:
                                    error_message = "TB: Authentication failed (403)"
                                elif tb_create_status == 429:
                                    error_message = "TB: Rate limited (429)"
                                elif tb_create_status >= 500:
                                    error_message = f"TB: Server error ({tb_create_status})"
                                elif tb_create_status != 200:
                                    error_message = f"TB: Unexpected status ({tb_create_status})"
                                else:
                                    error_message = "TB: No webdl_id returned"

                        if not final_url and error_message:
                            logger.warning(f"TorBox fallback échec: {error_message}")
                            if provider_used is None:
                                provider_used = 'TB'
                                _set_provider_in_history(provider_used)
                            result[0] = False
                            result[1] = error_message
                            try:
                                if isinstance(result, list):
                                    result.append({"raw_error_torbox": error_message_raw or error_message})
                            except Exception:
                                pass
                    except Exception as e:
                        logger.error(f"Exception TorBox fallback: {e}")
                if not final_url:
                    # NOUVEAU: Fallback mode gratuit 1fichier si aucune clé API disponible
                    logger.warning("Aucune URL directe obtenue via API - Tentative mode gratuit 1fichier")
                    
                    # Créer un lock pour ce téléchargement
                    free_lock = threading.Lock()
                    last_free_saved_percent = {"value": -1}
                    last_free_wait_save_ts = {"value": 0.0}
                    
                    try:
                        # Créer une session requests pour le mode gratuit
                        free_session = requests.Session()
                        free_session.headers.update(
                            _build_browser_download_headers(
                                referer=link,
                                accept='text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8'
                            )
                        )
                        
                        # Callbacks pour le mode gratuit
                        def log_cb(msg):
                            logger.info(msg)
                            if isinstance(config.history, list):
                                for entry in config.history:
                                    if "url" in entry and entry["url"] == url:
                                        entry["message"] = msg
                                        config.needs_redraw = True
                                        break
                        
                        def progress_cb(filename, downloaded, total, pct):
                            with free_lock:
                                if isinstance(config.history, list):
                                    for entry in config.history:
                                        if "url" in entry and entry["url"] == url and entry["status"] == "Downloading":
                                            progress_percent = int(pct) if pct else 0
                                            entry["progress"] = progress_percent
                                            entry["downloaded_size"] = downloaded
                                            entry["total_size"] = total
                                            # Effacer le message personnalisé pour afficher le pourcentage
                                            entry["message"] = ""
                                            config.needs_redraw = True
                                            should_save = (
                                                (progress_percent % 5 == 0 or progress_percent >= 99)
                                                and progress_percent != last_free_saved_percent["value"]
                                            )
                                            if should_save and _save_history_with_feedback("download_1fichier:free_progress"):
                                                last_free_saved_percent["value"] = progress_percent
                                            break
                                progress_queues[task_id].put((task_id, downloaded, total))
                        
                        def wait_cb(remaining, total_wait):
                            if isinstance(config.history, list):
                                for entry in config.history:
                                    if "url" in entry and entry["url"] == url:
                                        entry["message"] = _("free_mode_waiting").format(remaining, total_wait)
                                        config.needs_redraw = True
                                        now_wait = time.time()
                                        if now_wait - last_free_wait_save_ts["value"] >= 2.0:
                                            if _save_history_with_feedback("download_1fichier:free_wait"):
                                                last_free_wait_save_ts["value"] = now_wait
                                        break
                        
                        # Lancer le téléchargement gratuit
                        success, filepath, error_msg = download_1fichier_free_mode(
                            url=link,
                            dest_dir=dest_dir,
                            session=free_session,
                            log_callback=log_cb,
                            progress_callback=progress_cb,
                            wait_callback=wait_cb,
                            cancel_event=cancel_ev
                        )
                        
                        if success:
                            logger.info(f"Téléchargement gratuit réussi: {filepath}")
                            result[0] = True
                            result[1] = _("network_download_ok").format(game_name) if _ else f"Download successful: {game_name}"
                            provider_used = 'FREE'
                            _set_provider_in_history(provider_used)
                            
                            # Mettre à jour l'historique
                            if isinstance(config.history, list):
                                for entry in config.history:
                                    if "url" in entry and entry["url"] == url:
                                        entry["status"] = "Completed"
                                        entry["progress"] = 100
                                        entry["message"] = result[1]
                                        entry["provider"] = "FREE"
                                        entry["provider_prefix"] = "FREE:"
                                        _save_history_with_feedback("download_1fichier:free_success")
                                        config.needs_redraw = True
                                        break
                            
                            # Traiter le fichier (extraction si nécessaire)
                            if not is_zip_non_supported:
                                try:
                                    if filepath.lower().endswith('.zip'):
                                        logger.info(f"Extraction ZIP: {filepath}")
                                        extract_zip(filepath, dest_dir, url)
                                        logger.info("ZIP extrait")
                                    elif filepath.lower().endswith('.rar'):
                                        logger.info(f"Extraction RAR: {filepath}")
                                        extract_rar(filepath, dest_dir, url)
                                        logger.info("RAR extrait")
                                    elif filepath.lower().endswith('.7z'):
                                        logger.info(f"Extraction 7z: {filepath}")
                                        extract_7z(filepath, dest_dir, url)
                                        logger.info("7z extrait")
                                except Exception as e:
                                    logger.error(f"Erreur extraction: {e}")
                            
                            return
                        else:
                            logger.error(f"Échec téléchargement gratuit: {error_msg}")
                            result[0] = False
                            if isinstance(error_msg, str) and error_msg.startswith("1fichier:"):
                                result[1] = error_msg
                            else:
                                result[1] = f"Error Downloading with free mode: {error_msg}"
                            return
                    
                    except Exception as e:
                        logger.error(f"Exception mode gratuit: {e}", exc_info=True)
                    
                    # Si le mode gratuit a échoué aussi
                    logger.error("Échec de tous les providers (API + mode gratuit)")
                    result[0] = False
                    if result[1] is None:
                        result[1] = _("network_api_error").format("No provider available") if _ else "No provider available"
                    return
                if not filename:
                    filename = game_name
                sanitized_filename = sanitize_filename(filename)
                dest_path = os.path.join(dest_dir, sanitized_filename)

                # Essayer de récupérer la taille du serveur via HEAD request
                remote_size = None
                try:
                    if final_url and provider_used not in {'AD', 'DL', 'RD'}:
                        head_response = provider_download_session.head(final_url, timeout=10, allow_redirects=True)
                        if head_response.status_code == 200:
                            content_length = head_response.headers.get('content-length')
                            if content_length:
                                remote_size = int(content_length)
                                logger.debug(f"Taille du fichier serveur (AllDebrid/Debrid-Link/RealDebrid): {remote_size} octets")
                    elif final_url:
                        logger.debug(f"Saut du HEAD préliminaire pour provider {provider_used}: URL temporaire potentiellement sensible ({final_url})")
                except Exception as e:
                    logger.debug(f"Impossible de vérifier la taille serveur (AllDebrid/Debrid-Link/RealDebrid): {e}")
                
                # Vérifier si le fichier existe déjà (exact ou avec autre extension)
                file_found = False
                if os.path.exists(dest_path):
                    logger.info(f"Le fichier {dest_path} existe déjà, vérification de la taille...")
                    
                    # Vérifier la taille du fichier local
                    local_size = os.path.getsize(dest_path)
                    logger.debug(f"Taille du fichier local: {local_size} octets")
                    
                    # Comparer les tailles si on a obtenu la taille distante
                    if remote_size is not None and local_size != remote_size:
                        logger.warning(f"Taille mismatch! Local: {local_size}, Remote: {remote_size} - le fichier sera re-téléchargé")
                        # Les tailles ne correspondent pas, il faut re-télécharger
                        try:
                            if os.path.exists(dest_path):
                                os.remove(dest_path)
                                logger.info(f"Fichier incomplet supprimé: {dest_path}")
                            else:
                                logger.debug(f"Fichier déjà supprimé par un autre thread: {dest_path}")
                        except FileNotFoundError:
                            logger.debug(f"Fichier déjà supprimé (ou n'existe plus): {dest_path}")
                        except Exception as e:
                            logger.error(f"Impossible de supprimer le fichier incomplet: {e}")
                            result[0] = False
                            result[1] = f"Erreur suppression fichier incomplet: {str(e)}"
                            return
                        # Continuer le téléchargement normal (ne pas faire return)
                    else:
                        # Les tailles correspondent ou on ne peut pas vérifier, considérer comme déjà téléchargé
                        logger.info(f"Le fichier {dest_path} existe déjà et la taille est correcte, téléchargement ignoré")
                        result[0] = True
                        result[1] = _("network_download_ok").format(game_name) + _("download_already_present")
                        # Afficher un toast au lieu d'ouvrir l'historique
                        try:
                            show_toast(result[1])
                        except Exception as e:
                            logger.debug(f"Impossible d'afficher le toast: {e}")
                        with urls_lock:
                            urls_in_progress.discard(url)
                            logger.debug(f"URL supprimée du set des téléchargements en cours: {url} (URLs restantes: {len(urls_in_progress)})")
                        return result[0], result[1]
                    file_found = True
                
                # Vérifier si un fichier avec le même nom de base mais extension différente existe (SEULEMENT si fichier exact non trouvé)
                if not file_found:
                    base_name_no_ext = os.path.splitext(sanitized_filename)[0]
                    if base_name_no_ext != sanitized_filename:  # Seulement si une extension était présente
                        try:
                            # Lister tous les fichiers dans le répertoire de destination
                            if os.path.exists(dest_dir):
                                for existing_file in os.listdir(dest_dir):
                                    existing_base = os.path.splitext(existing_file)[0]
                                    if existing_base == base_name_no_ext:
                                        existing_path = os.path.join(dest_dir, existing_file)
                                        logger.info(f"Un fichier avec le même nom de base existe: {existing_path}, vérification de la taille...")
                                        
                                        # Vérifier la taille du fichier local
                                        local_size = os.path.getsize(existing_path)
                                        logger.debug(f"Taille du fichier local (extension différente): {local_size} octets")
                                        
                                        # Comparer les tailles si on a obtenu la taille distante
                                        if remote_size is not None and local_size != remote_size:
                                            logger.warning(f"Taille mismatch (extension différente)! Local: {local_size}, Remote: {remote_size} - re-téléchargement")
                                            # Continuer le téléchargement normal
                                            break
                                        else:
                                            # Les tailles correspondent, fichier complet
                                            logger.info(f"Un fichier avec le même nom de base existe déjà: {existing_path}, téléchargement ignoré")
                                            result[0] = True
                                            result[1] = _("network_download_ok").format(game_name) + _("download_already_extracted")
                                            # Afficher un toast au lieu d'ouvrir l'historique
                                            try:
                                                show_toast(result[1])
                                            except Exception as e:
                                                logger.debug(f"Impossible d'afficher le toast: {e}")
                                            with urls_lock:
                                                urls_in_progress.discard(url)
                                                logger.debug(f"URL supprimée du set des téléchargements en cours: {url} (URLs restantes: {len(urls_in_progress)})")
                                            return result[0], result[1]
                        except Exception as e:
                            logger.debug(f"Erreur lors de la vérification des fichiers existants: {e}")
            lock = threading.Lock()
            retries = 10
            retry_delay = 10
            download_header_variants = [
                provider_download_headers,
                _build_browser_download_headers(accept='*/*'),
                {
                    'User-Agent': 'curl/8.4.0',
                    'Accept': '*/*',
                    'Accept-Encoding': 'identity',
                    'Connection': 'keep-alive',
                },
            ]
            logger.debug(f"Initialisation progression avec taille inconnue pour task_id={task_id}")
            progress_queues[task_id].put((task_id, 0, 0))  # Taille initiale inconnue
            for attempt in range(retries):
                logger.debug(f"Début tentative {attempt + 1} pour télécharger {final_url}")
                try:
                    attempt_headers = download_header_variants[min(attempt, len(download_header_variants) - 1)]
                    resume_offset = _http_resume_offset(dest_path)
                    if resume_offset > 0:
                        attempt_headers = dict(attempt_headers)
                        attempt_headers['Range'] = f'bytes={resume_offset}-'
                    logger.debug(f"Headers tentative {attempt + 1}: {_redact_headers(attempt_headers)}")
                    with provider_download_session.get(final_url, stream=True, headers=attempt_headers, timeout=(30, 120), allow_redirects=True) as response:
                        logger.debug(f"Réponse GET reçue, code: {response.status_code}")
                        if response.status_code == 503 and provider_used == 'AD' and attempt < retries - 1:
                            logger.warning("AllDebrid a renvoyé 503 sur l'URL débridée, tentative de régénération du lien")
                            try:
                                refreshed_url, refreshed_filename = _refresh_alldebrid_final_url(link)
                                if refreshed_url:
                                    if refreshed_url != final_url:
                                        logger.debug(f"Nouvelle URL AllDebrid obtenue: {refreshed_url}")
                                    final_url = refreshed_url
                                if refreshed_filename:
                                    filename = refreshed_filename
                                    sanitized_filename = sanitize_filename(filename)
                                    dest_path = os.path.join(dest_dir, sanitized_filename)
                            except Exception as refresh_error:
                                logger.warning(f"Impossible de régénérer le lien AllDebrid après 503: {refresh_error}")
                        response.raise_for_status()
                        content_range_total = _http_parse_content_range(response.headers.get('content-range'))
                        is_range = bool(resume_offset > 0 and response.status_code == 206)
                        if content_range_total and content_range_total > 0:
                            total_size = content_range_total
                        else:
                            content_length = int(response.headers.get('content-length', 0))
                            total_size = content_length + (resume_offset if is_range else 0)
                        logger.debug(f"Taille totale: {total_size} octets")

                        has_space, low_space_message = _ensure_sufficient_disk_space(dest_dir, total_size)
                        if not has_space:
                            raise InsufficientDiskSpaceError(low_space_message)

                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == url:
                                    entry["total_size"] = total_size  # Ajouter la taille totale
                                    _save_history_with_feedback("download_1fichier:total_size")
                                    break
                        with lock:
                            if isinstance(config.history, list):
                                for entry in config.history:
                                    if "url" in entry and entry["url"] == url and entry["status"] == "Downloading":
                                        entry["total_size"] = total_size
                                        config.needs_redraw = True
                                        break
                            progress_queues[task_id].put((task_id, resume_offset if is_range else 0, total_size))  # Mettre à jour la taille totale

                        downloaded = resume_offset if is_range else 0
                        chunk_size = 8192
                        last_update_time = time.time()
                        last_downloaded = 0
                        update_interval = 0.1  # Mettre à jour toutes les 0,1 secondes
                        download_canceled = False
                        part_path = _http_part_path(dest_path)
                        logger.debug(f"Ouverture fichier: {part_path}")
                        with open(part_path, 'ab' if is_range else 'wb') as f:
                            for chunk in response.iter_content(chunk_size=chunk_size):
                                # Vérifier la pause (dynamiquement car l'événement peut être créé après le début)
                                while True:
                                    pause_ev = pause_events.get(task_id)
                                    if pause_ev is None or not pause_ev.is_set():
                                        break  # Pas en pause, continuer le téléchargement
                                    if cancel_ev is not None and cancel_ev.is_set():
                                        break  # Sortir de la boucle de pause si annulation demandée
                                    time.sleep(0.1)  # Attendre en pause
                                
                                if cancel_ev is not None and cancel_ev.is_set():
                                    logger.debug(f"Annulation détectée, arrêt du téléchargement 1fichier pour task_id={task_id}")
                                    result[0] = False
                                    result[1] = _("download_canceled") if _ else "Download canceled"
                                    download_canceled = True
                                    try:
                                        f.close()
                                    except Exception:
                                        pass
                                    try:
                                        if os.path.exists(part_path):
                                            os.remove(part_path)
                                    except Exception:
                                        pass
                                    break
                                if chunk:
                                    f.write(chunk)
                                    downloaded += len(chunk)
                                    current_time = time.time()
                                    if current_time - last_update_time >= update_interval:
                                        with lock:
                                            if isinstance(config.history, list):
                                                for entry in config.history:
                                                    if "url" in entry and entry["url"] == url and entry["status"] == "Downloading":
                                                        progress_percent = int(downloaded / total_size * 100) if total_size > 0 else 0
                                                        progress_percent = max(0, min(100, progress_percent))
                                                        entry["progress"] = progress_percent
                                                        entry["status"] = "Téléchargement"
                                                        entry["downloaded_size"] = downloaded
                                                        entry["total_size"] = total_size
                                                        config.needs_redraw = True
                                                        break
                                        # Calcul de la vitesse en Mo/s
                                        delta = downloaded - last_downloaded
                                        speed = (delta / (current_time - last_update_time) / (1024 * 1024)) if (current_time - last_update_time) > 0 else 0.0
                                        last_downloaded = downloaded
                                        last_update_time = current_time
                                        progress_queues[task_id].put((task_id, downloaded, total_size, speed))

                    if download_canceled:
                        try:
                            if os.path.exists(part_path):
                                os.remove(part_path)
                        except Exception:
                            pass
                    elif downloaded > 0:
                        try:
                            os.replace(part_path, dest_path)
                            logger.debug(f"Fichier partiel finalisé: {part_path} -> {dest_path}")
                        except Exception as e:
                            logger.warning(f"Impossible de finaliser le fichier partiel {part_path}: {e}")
                    else:
                        try:
                            if os.path.exists(part_path):
                                os.remove(part_path)
                        except Exception:
                            pass

                    if downloaded <= 0:
                        try:
                            if os.path.exists(dest_path):
                                os.remove(dest_path)
                        except Exception:
                            pass
                        content_type = response.headers.get('content-type', '') if response is not None else ''
                        raise requests.HTTPError(
                            f"Downloaded empty response from source (content-type={content_type or 'unknown'})"
                        )

                    # Si annulé, ne pas continuer avec extraction
                    if download_canceled:
                        return
                    
                    # Vérifier si l'extraction automatique est activée dans les paramètres
                    from rgsx_settings import get_auto_extract
                    auto_extract_enabled = get_auto_extract()
                    
                    # Déterminer si extraction est nécessaire
                    force_extract = is_zip_non_supported and auto_extract_enabled
                    is_ps3_target = _is_ps3_redump_target(platform_folder, platform)
                    if not force_extract and auto_extract_enabled and is_ps3_target:
                        force_extract = True
                        logger.debug("Extraction forcée activée pour PS3 Redump (déchiffrement ISO)")
                    
                    if force_extract:
                        with lock:
                            if url in config.download_progress:
                                config.download_progress[url]["status"] = "Extracting"
                                config.download_progress[url]["progress_percent"] = 0
                                config.download_progress[url]["downloaded_size"] = 0
                                config.needs_redraw = True
                            if isinstance(config.history, list):
                                for entry in config.history:
                                    if "url" in entry and entry["url"] == url and entry["status"] in ["Téléchargement", "Downloading"]:
                                        entry["progress"] = 0
                                        entry["status"] = "Extracting"
                                        entry["message"] = "Préparation de l'extraction..."
                                        _save_history_with_feedback("download_1fichier:extracting")
                                        config.needs_redraw = True
                                        break
                        logger.debug(f"Début post-traitement du téléchargement: {os.path.splitext(dest_path)[1].lower()}")
                        try:
                            result[0], result[1] = _postprocess_downloaded_file(dest_path, dest_dir, url, game_name, is_ps3_target)
                        except Exception as e:
                            logger.error(f"Exception lors du post-traitement: {str(e)}")
                            result[0] = False
                            result[1] = f"Erreur téléchargement {game_name}: {str(e)}"
                    else:
                        logger.debug(f"Application des permissions sur {dest_path}")
                        os.chmod(dest_path, 0o644)
                        logger.info(f"Téléchargement terminé: {dest_path}")
                        result[0] = True
                        result[1] = _("network_download_ok").format(game_name)
                    return

                except requests.exceptions.RequestException as e:
                    logger.error(f"Tentative {attempt + 1} échouée: {e}")
                    if attempt < retries - 1:
                        logger.debug(f"Attente de {retry_delay} secondes avant nouvelle tentative")
                        time.sleep(retry_delay)
                    else:
                        logger.error(f"Nombre maximum de tentatives atteint")
                        result[0] = False
                        result[1] = _("network_download_failed").format(retries)
                        return

        except requests.exceptions.RequestException as e:
            logger.error(f"Erreur API 1fichier: {e}")
            result[0] = False
            result[1] = _("network_api_error").format(str(e))
        except InsufficientDiskSpaceError as e:
            logger.warning(f"Téléchargement 1fichier annulé par manque d'espace disque pour {url}: {e}")
            result[0] = False
            result[1] = str(e)
        except Exception as e:
            logger.error(f"Erreur inattendue téléchargement 1fichier: {e}", exc_info=True)
            result[0] = False
            result[1] = _("network_download_error").format(game_name, str(e))

        finally:
            logger.debug(f"Thread téléchargement 1fichier terminé pour {url}, task_id={task_id}")
            progress_queues[task_id].put((task_id, result[0], result[1]))
            logger.debug(f"Résultat final envoyé à la queue: success={result[0]}, message={result[1]}, task_id={task_id}")
            # Nettoyer l'URL du set en cours de téléchargement
            with urls_lock:
                urls_in_progress.discard(url)
                logger.debug(f"URL supprimée du set des téléchargements en cours (finally): {url} (URLs restantes: {len(urls_in_progress)})")

    logger.debug(f"Démarrage thread pour {url}, task_id={task_id}")
    thread = threading.Thread(target=download_thread, daemon=True)
    download_threads[task_id] = thread
    thread.start()

    # Boucle principale pour mettre à jour la progression
    logger.debug(f"Début boucle de progression pour task_id={task_id}")
    while thread.is_alive():
        try:
            task_queue = progress_queues.get(task_id)
            if task_queue:
                while not task_queue.empty():
                    data = task_queue.get()
                    #logger.debug(f"Données queue progression reçues: {data}")
                    if isinstance(data[1], bool):  # Fin du téléchargement
                        success, message = data[1], data[2]
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == url and entry["status"] in ["Downloading", "Téléchargement", "Extracting", "Converting"]:
                                    entry["status"] = "Download_OK" if success else "Erreur"
                                    entry["progress"] = 100 if success else 0
                                    entry["message"] = message
                                    _save_history_with_feedback("download_1fichier:final")
                                    # Marquer le jeu comme téléchargé si succès
                                    if success:
                                        logger.debug(f"[1F_WHILE_LOOP] Marking game as downloaded: platform={platform}, game={game_name}")
                                        from history import mark_game_as_downloaded
                                        file_size = entry.get("size", "N/A")
                                        mark_game_as_downloaded(platform, game_name, file_size)
                                    config.needs_redraw = True
                                    logger.debug(f"Mise à jour finale historique: status={entry['status']}, progress={entry['progress']}%, message={message}, task_id={task_id}")
                                    break
                    else:
                        if len(data) >= 4:
                            downloaded, total_size, speed = data[1], data[2], data[3]
                        else:
                            downloaded, total_size = data[1], data[2]
                            speed = 0.0
                        progress_percent = int(downloaded / total_size * 100) if total_size > 0 else 0
                        progress_percent = max(0, min(100, progress_percent))
                        
                        if isinstance(config.history, list):
                            for entry in config.history:
                                if "url" in entry and entry["url"] == url and entry["status"] in ["Downloading", "Téléchargement"]:
                                    entry["progress"] = progress_percent
                                    entry["status"] = "Téléchargement"
                                    entry["downloaded_size"] = downloaded
                                    entry["total_size"] = total_size
                                    entry["speed"] = speed  # Ajout de la vitesse
                                    config.needs_redraw = True
                                    break
            try:
                await asyncio.sleep(0.1)
            except asyncio.CancelledError:
                # Cf. download_rom : on ne propage pas l'annulation ici pour garantir
                # que thread.join()/notify_download_finished() s'exécutent toujours et
                # libèrent le slot de la file d'attente (sinon active_download_count
                # reste bloqué et la queue ne redémarre plus après un Cancel Download).
                logger.debug(f"Boucle de progression annulée (Cancel Download) pour task_id={task_id}")
                break
        except Exception as e:
            logger.error(f"Erreur mise à jour progression: {str(e)}")

    logger.debug(f"Fin boucle de progression, attente fin thread pour task_id={task_id}")
    thread.join()
    try:
        download_threads.pop(task_id, None)
    except Exception:
        pass
    logger.debug(f"Thread terminé, nettoyage queue pour task_id={task_id}")
    # Drain any remaining final message to ensure history is saved
    try:
        task_queue = progress_queues.get(task_id)
        if task_queue:
            while not task_queue.empty():
                data = task_queue.get()
                if isinstance(data[1], bool):
                    success, message = data[1], data[2]
                    logger.debug(f"[1F_DRAIN_QUEUE] Processing final message: success={success}, message={message[:100] if message else 'None'}")
                    if isinstance(config.history, list):
                        for entry in config.history:
                            if "url" in entry and entry["url"] == url and entry["status"] in ["Downloading", "Téléchargement", "Extracting", "Converting"]:
                                entry["status"] = "Download_OK" if success else "Erreur"
                                entry["progress"] = 100 if success else 0
                                entry["message"] = message
                                _save_history_with_feedback("download_1fichier:drain")
                                # Marquer le jeu comme téléchargé si succès
                                if success:
                                    logger.debug(f"[1F_DRAIN_QUEUE] Marking game as downloaded: platform={platform}, game={game_name}")
                                    from history import mark_game_as_downloaded
                                    file_size = entry.get("size", "N/A")
                                    mark_game_as_downloaded(platform, game_name, file_size)
                                break
    except Exception as e:
        logger.error(f"[1F_DRAIN_QUEUE] Error processing final message: {e}")
    # Nettoyer la queue
    if task_id in progress_queues:
        del progress_queues[task_id]
    cancel_events.pop(task_id, None)
    logger.debug(f"Fin download_from_1fichier, résultat: success={result[0]}, message={result[1]}")
    
    # Sauvegarder le résultat AVANT de retirer l'URL du set (pour les doublons)
    with urls_lock:
        url_results[url] = (result[0], result[1])
        urls_in_progress.discard(url)
        logger.debug(f"URL supprimée du set des téléchargements en cours: {url} (URLs restantes: {len(urls_in_progress)})")
        # Signaler l'événement pour les appels doublons en attente
        if url in url_done_events:
            url_done_events[url].set()
    
    try:
        notify_download_finished()
    except Exception:
        pass
    return result[0], result[1]
def is_1fichier_url(url):
    """Détecte si l'URL est un lien 1fichier."""
    return isinstance(url, str) and "1fichier.com" in url
