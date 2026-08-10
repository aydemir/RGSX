"""network.archive_org — Archive.org URL/indirme yardimcilari.

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import os
import urllib.parse
import logging
from network.http_download import _stream_response_to_path

logger = logging.getLogger("network")

def _split_archive_org_path(url: str):
    """Parse archive.org download URL and return (identifier, archive_name, inner_path)."""
    try:
        parsed = urllib.parse.urlsplit(url)
        parts = parsed.path.split('/download/', 1)
        if len(parts) != 2:
            return None, None, None
        after = parts[1]
        identifier = after.split('/', 1)[0]
        rest = after[len(identifier):]
        if rest.startswith('/'):
            rest = rest[1:]
        rest_decoded = urllib.parse.unquote(rest)
        if '/' not in rest_decoded:
            return identifier, None, None
        first_seg, remainder = rest_decoded.split('/', 1)
        if first_seg.lower().endswith(('.zip', '.rar', '.7z')):
            return identifier, first_seg, remainder
        return identifier, None, None
    except Exception:
        return None, None, None
def _normalize_archive_org_download_path(identifier: str, rest: str) -> str:
    """Normalize archive.org download paths while preserving encoded inner archive members."""
    if not rest:
        return f"/download/{identifier}"

    rest = rest.lstrip('/')
    first_sep = rest.find('/')
    if first_sep == -1:
        encoded_rest = urllib.parse.quote(urllib.parse.unquote(rest), safe="%:@$&'()*+,;=-._~")
        return f"/download/{identifier}/{encoded_rest}"

    archive_name_raw = rest[:first_sep]
    member_raw = rest[first_sep + 1:]
    archive_name = urllib.parse.unquote(archive_name_raw)

    if archive_name.lower().endswith(('.zip', '.rar', '.7z')):
        encoded_archive_name = urllib.parse.quote(archive_name, safe="%:@$&'()*+,;=-._~")
        encoded_member = urllib.parse.quote(urllib.parse.unquote(member_raw), safe="%:@$&'()*+,;=-._~")
        return f"/download/{identifier}/{encoded_archive_name}/{encoded_member}"

    normalized_rest = urllib.parse.quote(
        urllib.parse.unquote(rest),
        safe="/@:$&'()*+,;=-._~",
    )
    return f"/download/{identifier}/{normalized_rest}"
def _try_archive_org_alternate_urls(session, archive_alt_urls: list[str], active_url: str, download_headers: dict, dest_path: str, task_id: str | None, cancel_ev, progress_queue_obj, fallback_total_size: int = 0, resume_offset: int = 0):
    seen_urls = {active_url}
    for alt_url in archive_alt_urls:
        if not alt_url or alt_url in seen_urls:
            continue
        seen_urls.add(alt_url)
        alt_response = None
        try:
            logger.debug(f"Réponse vide, tentative Archive.org alternative: {alt_url}")
            alt_headers = download_headers.copy()
            alt_host = urllib.parse.urlsplit(alt_url).netloc
            if alt_host.startswith('ia') and alt_host.endswith('.archive.org'):
                alt_headers['Referer'] = f"https://{alt_host}/"
                alt_headers['Origin'] = 'https://archive.org'

            alt_response = session.get(alt_url, stream=True, timeout=(45, 90), allow_redirects=True, headers=alt_headers)
            alt_response.raise_for_status()
            transfer = _stream_response_to_path(alt_response, dest_path, task_id, cancel_ev, progress_queue_obj, fallback_total_size=fallback_total_size, resume_offset=resume_offset)
            if transfer['downloaded'] > 0:
                return alt_response, alt_url, transfer

            try:
                if os.path.exists(dest_path):
                    os.remove(dest_path)
            except Exception:
                pass
        except Exception as exc:
            logger.debug(f"Fallback Archive.org vide échoué pour {alt_url}: {exc}")
            try:
                if os.path.exists(dest_path):
                    os.remove(dest_path)
            except Exception:
                pass
        finally:
            try:
                if alt_response is not None:
                    alt_response.close()
            except Exception:
                pass

    return None, active_url, None
