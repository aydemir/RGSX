"""network.lolroms — LOLROMs icin ozel indirme (external tool, probe, archive dogrulama).

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
import zipfile
import urllib.parse
import logging
import config
from language import _  # Import de la fonction de traduction
from network.updates import _safe_remove_file

logger = logging.getLogger("network")

def _is_lolroms_url(url: str | None) -> bool:
    if not url or not isinstance(url, str):
        return False
    try:
        host = (urllib.parse.urlsplit(url).netloc or '').lower()
        return host.endswith('lolroms.com')
    except Exception:
        return False
def _normalize_lolroms_url(url: str) -> str:
    parsed = urllib.parse.urlsplit(url)
    normalized_path = urllib.parse.quote(
        urllib.parse.unquote(parsed.path or '/'),
        safe="/@:$&'()*+,;=-._~",
    )
    normalized_query = urllib.parse.quote_plus(
        urllib.parse.unquote_plus(parsed.query),
        safe="=&:$,;+-._~!*'()",
    ) if parsed.query else ''
    return urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, normalized_path, normalized_query, parsed.fragment))
def _extract_reported_total_size(output_text: str) -> int:
    if not output_text:
        return 0
    patterns = [
        r'(?im)^content-length:\s*(\d+)\s*$',
        r'(?im)^length:\s*(\d+)(?:\s|$)',
        r'(?im)\bcontent-length:\s*(\d+)\b',
        r'(?im)\blength:\s*(\d+)\b',
    ]
    for pattern in patterns:
        matches = re.findall(pattern, output_text)
        if matches:
            try:
                return int(matches[-1])
            except Exception:
                continue
    return 0
def _build_lolroms_parent_url(url: str) -> str:
    parsed = urllib.parse.urlsplit(_normalize_lolroms_url(url))
    path = parsed.path or '/'
    parent_path = path.rsplit('/', 1)[0].rstrip('/') + '/'
    return urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, parent_path, '', ''))
def _looks_like_html_or_challenge(file_path: str) -> bool:
    try:
        with open(file_path, 'rb') as handle:
            head = handle.read(2048).lower()
        markers = (b'<html', b'<!doctype html', b'cloudflare', b'just a moment', b'cf-chl', b'challenge-platform')
        return any(marker in head for marker in markers)
    except Exception:
        return True
def _should_accept_partial_archive(downloaded: int, total_size: int, file_path: str) -> tuple[bool, str]:
    if total_size <= 0 or downloaded >= total_size:
        return True, "archive complete"

    difference = max(0, total_size - downloaded)
    if difference <= 0:
        return True, "archive complete"

    extension = os.path.splitext(file_path)[1].lower()
    if extension not in {'.7z', '.zip', '.rar'}:
        return True, "non-archive payload"

    if not _matches_expected_archive_signature(file_path):
        return False, "invalid archive signature"

    if difference <= 16:
        return True, f"small size mismatch tolerated ({downloaded}/{total_size} bytes)"

    ratio = difference / total_size if total_size > 0 else 0.0
    if ratio <= 0.0005 and difference <= 64:
        return True, f"tiny size mismatch tolerated ({downloaded}/{total_size} bytes)"

    if extension == '.zip':
        try:
            with zipfile.ZipFile(file_path) as archive:
                archive.testzip()
                return True, f"archive validates despite partial size mismatch ({downloaded}/{total_size} bytes)"
        except Exception:
            pass

    return False, f"incomplete archive payload downloaded ({downloaded}/{total_size} bytes)"
def _matches_expected_archive_signature(file_path: str) -> bool:
    extension = os.path.splitext(file_path)[1].lower()
    if extension not in {'.7z', '.zip', '.rar'}:
        return True
    try:
        with open(file_path, 'rb') as handle:
            head = handle.read(8)
    except Exception:
        return False
    if extension == '.7z':
        return head.startswith(bytes.fromhex('377abcaf271c'))
    if extension == '.zip':
        return head.startswith(b'PK\x03\x04') or head.startswith(b'PK\x05\x06') or head.startswith(b'PK\x07\x08')
    if extension == '.rar':
        return head.startswith(b'Rar!\x1a\x07\x00') or head.startswith(b'Rar!\x1a\x07\x01\x00')
    return True
def _resolve_lolroms_external_command() -> tuple[str | None, str | None]:
    if config.OPERATING_SYSTEM == 'Windows':
        candidates = [r'C:\Windows\System32\curl.exe', 'curl.exe', 'curl']
        for candidate in candidates:
            resolved = candidate if os.path.isabs(candidate) and os.path.exists(candidate) else shutil.which(candidate)
            if resolved:
                return 'curl', resolved
        return None, None

    for candidate in ('wget',):
        resolved = shutil.which(candidate)
        if resolved:
            return 'wget', resolved
    return None, None
def _probe_lolroms_remote_size(url: str) -> int:
    tool_kind, tool_cmd = _resolve_lolroms_external_command()
    if not tool_kind or not tool_cmd:
        return 0

    url = _normalize_lolroms_url(url)
    parent_url = _build_lolroms_parent_url(url)
    browser_ua = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36'
    cookie_fd, cookie_path = tempfile.mkstemp(prefix='rgsx_lolroms_probe_', suffix='.cookies')
    os.close(cookie_fd)
    parent_fd, parent_output = tempfile.mkstemp(prefix='rgsx_lolroms_probe_parent_', suffix='.html')
    os.close(parent_fd)

    try:
        if tool_kind == 'curl':
            parent_cmd = [
                tool_cmd,
                '-L',
                '--connect-timeout', '20',
                '--silent',
                '--show-error',
                '--insecure',
                '-A', browser_ua,
                '-H', 'Accept: application/octet-stream,*/*',
                '-H', 'Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                '-H', 'Referer: https://lolroms.com/',
                '-c', cookie_path,
                '-b', cookie_path,
                '-o', parent_output,
                parent_url,
            ]
            probe_cmd = [
                tool_cmd,
                '-I',
                '-L',
                '--connect-timeout', '20',
                '--silent',
                '--show-error',
                '--insecure',
                '-A', browser_ua,
                '-H', 'Accept: application/octet-stream,*/*',
                '-H', 'Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                '-H', f'Referer: {parent_url}',
                '-c', cookie_path,
                '-b', cookie_path,
                url,
            ]
        else:
            parent_cmd = [
                tool_cmd,
                '--timeout=60',
                '--tries=2',
                '--max-redirect=10',
                '--no-verbose',
                '--no-hsts',
                f'--user-agent={browser_ua}',
                '--referer=https://lolroms.com/',
                '--header=Accept: application/octet-stream,*/*',
                '--header=Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                '--no-check-certificate',
                '--save-cookies', cookie_path,
                '--load-cookies', cookie_path,
                '--keep-session-cookies',
                '-O', parent_output,
                parent_url,
                '--quiet',
            ]
            probe_cmd = [
                tool_cmd,
                '--spider',
                '--server-response',
                '--timeout=60',
                '--tries=1',
                '--max-redirect=10',
                '--no-verbose',
                '--no-hsts',
                f'--user-agent={browser_ua}',
                f'--referer={parent_url}',
                '--header=Accept: application/octet-stream,*/*',
                '--header=Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                '--no-check-certificate',
                '--save-cookies', cookie_path,
                '--load-cookies', cookie_path,
                '--keep-session-cookies',
                url,
            ]

        try:
            subprocess.run(parent_cmd, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=65)
        except Exception:
            pass

        probe = subprocess.run(
            probe_cmd,
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace',
            timeout=65,
        )
        probe_text = '\n'.join(part for part in (probe.stdout, probe.stderr) if part)
        return _extract_reported_total_size(probe_text)
    except Exception as exc:
        logger.debug(f"Impossible de sonder la taille lolroms via {tool_kind}: {exc}")
        return 0
    finally:
        _safe_remove_file(parent_output)
        _safe_remove_file(cookie_path)
def _download_lolroms_with_external_tool(url: str, dest_path: str, task_id: str | None, cancel_ev=None, progress_queue=None) -> tuple[bool | None, str | None]:
    tool_kind, tool_cmd = _resolve_lolroms_external_command()
    if not tool_kind or not tool_cmd:
        return None, None

    url = _normalize_lolroms_url(url)
    parent_url = _build_lolroms_parent_url(url)
    browser_ua = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36'
    cookie_fd, cookie_path = tempfile.mkstemp(prefix='rgsx_lolroms_', suffix='.cookies')
    os.close(cookie_fd)
    parent_fd, parent_output = tempfile.mkstemp(prefix='rgsx_lolroms_parent_', suffix='.html')
    os.close(parent_fd)
    parent_fetch_attempted = False

    def _build_cmd(target_url: str, output_path: str, referer: str, quiet_parent: bool = False, resume: bool = False):
        if tool_kind == 'curl':
            cmd = [
                tool_cmd,
                '-L',
                '--connect-timeout', '20',
                '--speed-time', '120',
                '--speed-limit', '1024',
                '--silent',
                '--show-error',
                '--insecure',
                '-A', browser_ua,
                '-H', 'Accept: application/octet-stream,*/*',
                '-H', 'Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                '-H', f'Referer: {referer}',
                '-c', cookie_path,
                '-b', cookie_path,
            ]
            if resume:
                cmd.extend(['-C', '-'])
            cmd.extend([
                '-o', output_path,
                target_url,
            ])
            return cmd
        cmd = [
            tool_cmd,
            '--timeout=60',
            '--tries=3',
            '--max-redirect=10',
            '--no-verbose',
            '--no-hsts',
            f'--user-agent={browser_ua}',
            f'--referer={referer}',
            '--header=Accept: application/octet-stream,*/*',
            '--header=Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
            '--no-check-certificate',
            '--save-cookies', cookie_path,
            '--load-cookies', cookie_path,
            '--keep-session-cookies',
        ]
        if resume:
            cmd.append('--continue')
        cmd.extend([
            '-O', output_path,
            target_url,
        ])
        if quiet_parent:
            cmd.append('--quiet')
        return cmd

    def _should_retry_partial_download(return_code: int, stderr_text: str, stdout_text: str, current_size: int, expected_size: int) -> bool:
        if current_size <= 0:
            return False
        if expected_size > 0 and current_size >= expected_size:
            return False
        combined = f"{stderr_text or ''}\n{stdout_text or ''}".lower()
        if tool_kind == 'curl':
            return return_code in {18, 28, 56} or 'end of response' in combined or 'transfer closed' in combined
        return return_code in {4} or 'connection closed' in combined or 'read error' in combined or 'timed out' in combined

    def _probe_total_size(target_url: str, referer: str) -> int:
        try:
            if tool_kind == 'curl':
                probe_cmd = [
                    tool_cmd,
                    '-I',
                    '-L',
                    '--connect-timeout', '20',
                    '--silent',
                    '--show-error',
                    '--insecure',
                    '-A', browser_ua,
                    '-H', 'Accept: application/octet-stream,*/*',
                    '-H', 'Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                    '-H', f'Referer: {referer}',
                    '-c', cookie_path,
                    '-b', cookie_path,
                    target_url,
                ]
            else:
                probe_cmd = [
                    tool_cmd,
                    '--spider',
                    '--server-response',
                    '--timeout=60',
                    '--tries=1',
                    '--max-redirect=10',
                    '--no-verbose',
                    '--no-hsts',
                    f'--user-agent={browser_ua}',
                    f'--referer={referer}',
                    '--header=Accept: application/octet-stream,*/*',
                    '--header=Accept-Language: fr-FR,fr;q=0.9,en;q=0.8',
                    '--no-check-certificate',
                    '--save-cookies', cookie_path,
                    '--load-cookies', cookie_path,
                    '--keep-session-cookies',
                    target_url,
                ]
            logger.debug(f"lolroms size probe via {tool_kind}: {' '.join(probe_cmd)}")
            probe = subprocess.run(
                probe_cmd,
                capture_output=True,
                text=True,
                encoding='utf-8',
                errors='replace',
                timeout=65,
            )
            probe_text = '\n'.join(part for part in (probe.stdout, probe.stderr) if part)
            reported_total = _extract_reported_total_size(probe_text)
            if reported_total > 0:
                return reported_total
        except Exception as exc:
            logger.debug(f"lolroms size probe failed via {tool_kind}: {exc}")
        return 0

    def _run_command(cmd, watch_output: str | None = None, known_total_size: int = 0):
        logger.debug(f"lolroms external fallback via {tool_kind}: {' '.join(cmd)}")
        stdout_lines: list[str] = []
        stderr_lines: list[str] = []
        total_size_state = {"value": known_total_size}

        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding='utf-8',
            errors='replace',
        )

        def _drain_stream(stream, collector: list[str]):
            if stream is None:
                return
            try:
                for line in iter(stream.readline, ''):
                    if not line:
                        break
                    collector.append(line)
                    if total_size_state["value"] <= 0:
                        reported_total = _extract_reported_total_size(line)
                        if reported_total > 0:
                            total_size_state["value"] = reported_total
            finally:
                try:
                    stream.close()
                except Exception:
                    pass

        stdout_thread = threading.Thread(target=_drain_stream, args=(process.stdout, stdout_lines), daemon=True)
        stderr_thread = threading.Thread(target=_drain_stream, args=(process.stderr, stderr_lines), daemon=True)
        stdout_thread.start()
        stderr_thread.start()

        last_size = 0
        last_time = time.time()
        while process.poll() is None:
            if cancel_ev is not None and cancel_ev.is_set():
                try:
                    process.terminate()
                    process.wait(timeout=5)
                except Exception:
                    try:
                        process.kill()
                    except Exception:
                        pass
                raise RuntimeError(_("download_canceled") if _ else 'Download canceled')
            if watch_output and progress_queue is not None and task_id:
                current_size = os.path.getsize(watch_output) if os.path.exists(watch_output) else 0
                current_time = time.time()
                elapsed = max(current_time - last_time, 0.001)
                speed = max(0.0, (current_size - last_size) / elapsed / (1024 * 1024))
                last_size = current_size
                last_time = current_time
                progress_queue.put((task_id, current_size, total_size_state["value"], speed))
            time.sleep(0.2)
        process.wait()
        stdout_thread.join(timeout=2)
        stderr_thread.join(timeout=2)
        stdout = ''.join(stdout_lines)
        stderr = ''.join(stderr_lines)
        return process.returncode, stdout, stderr

    def _fetch_parent_page_if_needed(force: bool = False) -> bool:
        nonlocal parent_fetch_attempted
        if parent_fetch_attempted and not force:
            return False

        parent_fetch_attempted = True
        parent_cmd = _build_cmd(parent_url, parent_output, 'https://lolroms.com/', quiet_parent=True)
        parent_code, parent_stdout, parent_stderr = _run_command(parent_cmd)
        if parent_code != 0:
            logger.debug(f"lolroms parent fetch fallback failed via {tool_kind}: {parent_stderr[:300]}")
            return False

        logger.debug(f"lolroms parent fetch fallback succeeded via {tool_kind}")
        return True

    try:
        total_size = _probe_total_size(url, parent_url)
        if total_size <= 0:
            logger.debug(f"lolroms size probe returned 0 via {tool_kind}, trying parent fetch fallback")
            if _fetch_parent_page_if_needed():
                total_size = _probe_total_size(url, parent_url)

        if total_size > 0 and progress_queue is not None and task_id:
            progress_queue.put((task_id, 0, total_size, 0.0))

        file_code = 1
        file_stdout = ''
        file_stderr = ''
        max_attempts = 3
        for attempt_index in range(max_attempts):
            resume_download = attempt_index > 0 and os.path.exists(dest_path) and os.path.getsize(dest_path) > 0
            if resume_download:
                current_size = os.path.getsize(dest_path)
                logger.debug(f"lolroms external retry via {tool_kind}: reprise tentative {attempt_index + 1}/{max_attempts} a {current_size} octets")
            file_cmd = _build_cmd(url, dest_path, parent_url, resume=resume_download)
            file_code, file_stdout, file_stderr = _run_command(file_cmd, watch_output=dest_path, known_total_size=total_size)
            current_size = os.path.getsize(dest_path) if os.path.exists(dest_path) else 0
            if file_code == 0:
                break
            if current_size <= 0 and not parent_fetch_attempted:
                logger.debug(f"lolroms direct download failed via {tool_kind} without parent fetch, trying parent fetch fallback before retry")
                _fetch_parent_page_if_needed()
            if _should_retry_partial_download(file_code, file_stderr, file_stdout, current_size, total_size) and attempt_index < max_attempts - 1:
                logger.debug(
                    f"lolroms external partial transfer via {tool_kind}: tentative {attempt_index + 1} interrompue a {current_size}/{total_size or 0} octets, reprise"
                )
                continue
            break

        if file_code != 0:
            if os.path.exists(dest_path):
                current_size = os.path.getsize(dest_path)
                if total_size > 0 and current_size >= total_size:
                    logger.debug(f"lolroms external {tool_kind} returned code {file_code} but file size reached expected total ({current_size})")
                else:
                    _safe_remove_file(dest_path)
            return False, f"lolroms external download failed via {tool_kind}: {file_stderr[:300].strip() or file_stdout[:300].strip() or 'unknown error'}"

        if not os.path.exists(dest_path):
            return False, f"lolroms external download failed via {tool_kind}: output file missing"
        if _looks_like_html_or_challenge(dest_path):
            _safe_remove_file(dest_path)
            return False, f"lolroms external download via {tool_kind} returned HTML/challenge content"
        if not _matches_expected_archive_signature(dest_path):
            _safe_remove_file(dest_path)
            return False, f"lolroms external download via {tool_kind} returned an invalid archive payload"
        if total_size > 0 and progress_queue is not None and task_id:
            final_size = os.path.getsize(dest_path)
            progress_queue.put((task_id, final_size, max(total_size, final_size), 0.0))
        return True, None
    except RuntimeError as exc:
        if cancel_ev is not None and cancel_ev.is_set():
            _safe_remove_file(dest_path)
            logger.debug(f"lolroms external download canceled, partial file removed: {dest_path}")
            return False, str(exc)
        raise
    finally:
        _safe_remove_file(parent_output)
        _safe_remove_file(cookie_path)
