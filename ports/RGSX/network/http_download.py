"""network.http_download — HTTP indirme hatti (headers/challenge/resume/vimm/browser).

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import os
import re
import time
import urllib.parse
import logging
import requests  # type: ignore
import html as html_module
from urllib.parse import urljoin, unquote
from network import pause_events
from network.helpers import _parse_known_size_to_bytes

logger = logging.getLogger("network")

def _build_browser_download_headers(referer: str | None = None, accept: str = 'application/octet-stream,*/*;q=0.8') -> dict:
    """Build browser-like headers for file downloads that reject minimal clients."""
    headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
        'Accept': accept,
        'Accept-Language': 'en-US,en;q=0.9,fr;q=0.8',
        'Accept-Encoding': 'identity',
        'Connection': 'keep-alive',
        'DNT': '1',
    }
    if referer:
        headers['Referer'] = referer
    return headers
def _default_referer_for_url(url: str) -> str | None:
    """Return a sensible same-site referer instead of stale provider-specific values."""
    try:
        parsed = urllib.parse.urlsplit(url)
        if not parsed.scheme or not parsed.netloc:
            return None
        host = (parsed.netloc or '').lower()
        if 'archive.org' in host and '/download/' in (parsed.path or ''):
            identifier = parsed.path.split('/download/', 1)[1].split('/', 1)[0]
            if identifier:
                return f"https://archive.org/details/{identifier}"
            return 'https://archive.org/'
        return f"{parsed.scheme}://{parsed.netloc}/"
    except Exception:
        return None
def _is_browser_challenge_response(response) -> bool:
    """Detect browser-only challenge pages (e.g. Cloudflare) to fail fast with a clear message."""
    if response is None:
        return False
    try:
        status_code = int(getattr(response, 'status_code', 0) or 0)
    except Exception:
        status_code = 0
    if status_code not in (403, 429, 503):
        return False
    try:
        text = (getattr(response, 'text', '') or '')[:4000].lower()
    except Exception:
        text = ''
    markers = (
        'just a moment',
        'cf_chl_opt',
        'challenge-platform',
        'enable javascript and cookies to continue',
        'checking your browser before accessing',
    )
    return any(marker in text for marker in markers)
def _redact_headers(headers: dict) -> dict:
    """Return a copy of headers with sensitive fields redacted for logs."""
    if not isinstance(headers, dict):
        return {}
    safe = headers.copy()
    if 'Cookie' in safe and safe['Cookie']:
        safe['Cookie'] = '<redacted>'
    return safe
def _extract_vimm_download_info(html_text: str, page_url: str) -> dict[str, str | int] | None:
    """Extract Vimm download form data using the regex parser path."""
    if not html_text:
        return None

    action = ''
    media_id = ''
    size_hint = 0

    form_tag_match = re.search(r'<form\b(?=[^>]*\bid\s*=\s*(["\'])dl_form\1)[^>]*>', html_text, re.IGNORECASE | re.DOTALL)
    if form_tag_match:
        form_tag = form_tag_match.group(0)
        action_match = re.search(r'\baction\s*=\s*(["\'])(.*?)\1', form_tag, re.IGNORECASE | re.DOTALL)
        if action_match:
            action = html_module.unescape(action_match.group(2)).strip()

    form_block_match = re.search(r'<form\b(?=[^>]*\bid\s*=\s*(["\'])dl_form\1)[^>]*>(.*?)</form>', html_text, re.IGNORECASE | re.DOTALL)
    form_block = form_block_match.group(2) if form_block_match else html_text

    media_match = re.search(r'<input\b[^>]*\bname\s*=\s*(["\'])mediaId\1[^>]*\bvalue\s*=\s*(["\'])(.*?)\2', form_block, re.IGNORECASE | re.DOTALL)
    if media_match:
        media_id = html_module.unescape(media_match.group(3)).strip()
    else:
        media_match = re.search(r'<input\b[^>]*\bvalue\s*=\s*(["\'])(\d+)\1[^>]*\bname\s*=\s*(["\'])mediaId\3', form_block, re.IGNORECASE | re.DOTALL)
        if media_match:
            media_id = html_module.unescape(media_match.group(2)).strip()

    if size_hint <= 0:
        size_match = re.search(r'\bid\s*=\s*(["\'])dl_size\1[^>]*>\s*([^<]+?)\s*<', html_text, re.IGNORECASE | re.DOTALL)
        if size_match:
            size_hint = _parse_known_size_to_bytes(html_module.unescape(size_match.group(2)).strip())

    if not media_id:
        js_media_match = re.search(r'\blet\s+media\s*=\s*\[\{"ID":(\d+)', html_text)
        if js_media_match:
            media_id = js_media_match.group(1)

    if size_hint <= 0:
        js_size_match = re.search(r'"ZippedText":"([^"]+)"', html_text)
        if js_size_match:
            size_hint = _parse_known_size_to_bytes(html_module.unescape(js_size_match.group(1)).strip())

    if not action or not media_id:
        return None

    base_download_url = urljoin(page_url, action)
    separator = '&' if '?' in base_download_url else '?'
    download_url = base_download_url + separator + urllib.parse.urlencode({'mediaId': media_id})
    return {
        'media_id': media_id,
        'base_download_url': base_download_url,
        'download_url': download_url,
        'size_hint': max(0, int(size_hint or 0)),
    }
def _fetch_vimm_download_info(url: str, session: requests.Session) -> dict[str, str | int] | None:
    try:
        if 'vimm.net' not in url:
            return None
        resp = session.get(url, timeout=30)
        resp.raise_for_status()
        info = _extract_vimm_download_info(resp.text, url)
        if info:
            logger.debug("Analyse vimm.net via parser regex")
        return info
    except Exception as e:
        logger.debug(f"Erreur lors de la récupération des informations vimm.net: {e}")
        return None
def _get_vimm_file_size(url: str, session: requests.Session, download_info: dict[str, str | int] | None = None) -> int:
    """Récupère la taille du fichier pour les URLs vimm.net avant téléchargement."""
    try:
        if 'vimm.net' not in url:
            return 0
            
        logger.debug("Récupération de la taille du fichier vimm.net...")

        if download_info is None:
            download_info = _fetch_vimm_download_info(url, session)
        if not download_info:
            logger.debug("Informations de téléchargement introuvables pour vimm.net")
            return 0

        media_id = str(download_info.get('media_id') or '').strip()
        download_url = str(download_info.get('download_url') or '').strip()
        if not media_id or not download_url:
            logger.debug("mediaId ou URL de téléchargement vimm.net introuvable")
            return 0

        logger.debug(f"mediaId trouvé pour taille: {media_id}")

        # Faire un HEAD request pour récupérer la taille
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36',
            'Referer': url
        }
        
        head_resp = session.head(download_url, timeout=10, headers=headers, allow_redirects=True)
        if head_resp.status_code == 200:
            # Extraire le nom réel depuis Content-Disposition
            cd = head_resp.headers.get('content-disposition', '')
            if cd and isinstance(download_info, dict):
                fn_match = re.search(r'filename\*?=\"([^\"]+)\"|filename\*?=\'([^\']+)\'|filename\*?=([^\s;]+)', cd, re.IGNORECASE)
                if fn_match:
                    real_filename = unquote((fn_match.group(1) or fn_match.group(2) or fn_match.group(3)).strip())
                    if real_filename:
                        download_info['real_filename'] = real_filename
                        logger.debug(f"Nom de fichier réel Vimm: {real_filename}")
            content_length = head_resp.headers.get('content-length')
            if content_length:
                size = int(content_length)
                logger.debug(f"Taille du fichier vimm.net récupérée: {size} octets")
                return size

        size_hint = int(download_info.get('size_hint') or 0)
        if size_hint > 0:
            logger.debug(f"Taille du fichier vimm.net récupérée depuis la page: {size_hint} octets")
            return size_hint

        logger.debug("Impossible de récupérer la taille du fichier vimm.net")
        return 0
        
    except Exception as e:
        logger.debug(f"Erreur lors de la récupération de la taille vimm.net: {e}")
        return 0
def _http_part_path(dest_path: str) -> str:
    """Chemin du fichier partiel (.part) utilisé pour la reprise HTTP."""
    return f"{dest_path}.part"
def _http_resume_offset(dest_path: str) -> int:
    """Taille (octets) du fichier .part existant, sinon 0 (aucune reprise)."""
    try:
        part_path = _http_part_path(dest_path)
        if os.path.isfile(part_path):
            size = os.path.getsize(part_path)
            return size if size > 0 else 0
    except Exception:
        pass
    return 0
def _http_parse_content_range(header: str | None) -> int | None:
    """Extrait la taille totale depuis Content-Range ('bytes 0-99/1000' -> 1000)."""
    if not header:
        return None
    try:
        match = re.match(r'bytes\s+(\d+)-(\d+)/(\d+)', str(header))
        if match:
            return int(match.group(3))
    except Exception:
        pass
    return None
def _stream_response_to_path(response, dest_path: str, task_id: str | None, cancel_ev, progress_queue_obj, fallback_total_size: int = 0, resume_offset: int = 0) -> dict[str, int | float | bool]:
    part_path = _http_part_path(dest_path)
    resume_offset = max(0, int(resume_offset or 0))
    is_range = bool(resume_offset > 0 and getattr(response, 'status_code', 0) == 206)

    content_range_total = _http_parse_content_range(response.headers.get('content-range'))
    if content_range_total and content_range_total > 0:
        total_size = content_range_total
    else:
        content_length = int(response.headers.get('content-length', 0) or 0)
        if content_length > 0:
            total_size = content_length + (resume_offset if is_range else 0)
        else:
            total_size = max(0, int(fallback_total_size or 0))

    downloaded = resume_offset if is_range else 0
    chunk_size = 4096
    last_update_time = time.time()
    last_downloaded = 0
    update_interval = 0.1
    download_canceled = False

    if progress_queue_obj is not None and task_id is not None:
        progress_queue_obj.put((task_id, downloaded, total_size))

    try:
        with open(part_path, 'ab' if is_range else 'wb') as f:
            for chunk in response.iter_content(chunk_size=chunk_size):
                while True:
                    pause_ev = pause_events.get(task_id)
                    if pause_ev is None or not pause_ev.is_set():
                        break
                    if cancel_ev is not None and cancel_ev.is_set():
                        break
                    time.sleep(0.1)

                if cancel_ev is not None and cancel_ev.is_set():
                    logger.debug(f"Annulation détectée, arrêt du téléchargement pour task_id={task_id}")
                    download_canceled = True
                    try:
                        f.close()
                    except Exception:
                        pass
                    break

                if chunk:
                    size_received = len(chunk)
                    f.write(chunk)
                    downloaded += size_received
                    current_time = time.time()

                    current_percent = int(downloaded / total_size * 100) if total_size > 0 else 0
                    last_percent = int(last_downloaded / total_size * 100) if total_size > 0 else 0

                    should_update = (progress_queue_obj is not None and task_id is not None and
                                   (current_time - last_update_time >= update_interval or
                                    current_percent != last_percent or
                                    total_size == 0))

                    if should_update:
                        delta = downloaded - last_downloaded
                        speed = delta / (current_time - last_update_time) / (1024 * 1024) if current_time > last_update_time else 0
                        last_downloaded = downloaded
                        last_update_time = current_time
                        progress_queue_obj.put((task_id, downloaded, total_size, speed))
    finally:
        try:
            if response is not None:
                response.close()
        except Exception:
            pass

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

    return {
        'total_size': total_size,
        'downloaded': downloaded,
        'last_downloaded': last_downloaded,
        'last_update_time': last_update_time,
        'download_canceled': download_canceled,
    }
