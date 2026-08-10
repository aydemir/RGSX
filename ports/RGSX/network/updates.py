"""network.updates — Guncelleme kontrolu + changelog + extract_update.

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import asyncio
import os
import re
import shutil
import subprocess
import sys
import time
import zipfile
import logging
import requests  # type: ignore
import html as html_module
import config
from config import OTA_VERSION_ENDPOINT, APP_FOLDER, UPDATE_FOLDER, OTA_UPDATE_ZIP, OTA_UPDATE_WINDOWS_ZIP
from config import HEADLESS
try:
    if not HEADLESS:
        import pygame  # type: ignore
    else:
        pygame = None  # type: ignore
except Exception:
    pygame = None  # type: ignore
from language import _  # Import de la fonction de traduction

logger = logging.getLogger("network")

cache = {}
CACHE_TTL = 3600  # 1 heure
def test_internet():
    """Teste la connexion Internet de manière complète et portable pour Windows et Linux/Batocera."""
    logger.debug("=== Début test de connexion Internet complet ===")
    
    # Test 1: Ping vers serveurs DNS publics
    ping_option = '-n' if sys.platform.startswith("win") else '-c'
    dns_servers = ['8.8.8.8', '1.1.1.1', '208.67.222.222']  # Google, Cloudflare, OpenDNS
    
    ping_success = False
    for dns_server in dns_servers:
        
        try:
            result = subprocess.run(
                ['ping', ping_option, '2', dns_server],
                capture_output=True,
                text=True,
                timeout=8
            )
            if result.returncode == 0:
                logger.debug(f"[OK] Ping vers {dns_server} réussi")
                ping_success = True
                break
            else:
                logger.debug(f"[FAIL] Ping vers {dns_server} échoué (code: {result.returncode})")
                if result.stderr:
                    logger.debug(f"Erreur ping: {result.stderr.strip()}")
        except subprocess.TimeoutExpired:
            logger.debug(f"[FAIL] Timeout ping vers {dns_server}")
        except Exception as e:
            logger.debug(f"[FAIL] Exception ping vers {dns_server}: {str(e)}")
    
    # Test 2: Tentative de résolution DNS
    dns_success = False
    try:
        import socket
       
        socket.gethostbyname('google.com')
        logger.debug("[OK] Résolution DNS réussie")
        dns_success = True
    except socket.gaierror as e:
        logger.debug(f"[FAIL] Erreur résolution DNS: {str(e)}")
    except Exception as e:
        logger.debug(f"[FAIL] Exception résolution DNS: {str(e)}")
    
    # Test 3: Tentative de connexion HTTP
    http_success = False
    test_urls = [
        'http://www.google.com',
        'http://www.cloudflare.com',
        'https://httpbin.org/get'
    ]
    
    for test_url in test_urls:
        try:
            response = requests.get(test_url, timeout=5, allow_redirects=True)
            if response.status_code == 200:
                logger.debug(f"[OK] Connexion HTTP vers {test_url} réussie (code: {response.status_code})")
                http_success = True
                break
            else:
                logger.debug(f"[FAIL] Connexion HTTP vers {test_url} échouée (code: {response.status_code})")
        except requests.exceptions.Timeout:
            logger.debug(f"[FAIL] Timeout connexion HTTP vers {test_url}")
        except requests.exceptions.ConnectionError as e:
            logger.debug(f"[FAIL] Erreur connexion HTTP vers {test_url}: {str(e)}")
        except Exception as e:
            logger.debug(f"[FAIL] Exception connexion HTTP vers {test_url}: {str(e)}")
    
    # Analyse des résultats
    total_tests = 3
    passed_tests = sum([ping_success, dns_success, http_success])

    
    # Diagnostic et conseils
    if passed_tests == 0:
        logger.error("Aucune connexion Internet détectée. Vérifiez:")
        logger.error("- Câble réseau ou WiFi connecté")
        logger.error("- Configuration proxy/firewall")
        logger.error("- Paramètres réseau système")
        return False
    elif passed_tests < total_tests:
        logger.warning(f"Connexion Internet partielle ({passed_tests}/{total_tests})")
        if not ping_success:
            logger.warning("- Ping échoué: possible blocage ICMP par firewall")
        if not dns_success:
            logger.warning("- DNS échoué: problème serveurs DNS")
        if not http_success:
            logger.warning("- HTTP échoué: possible blocage proxy/firewall")
        return True  # Connexion partielle acceptable
    else:
        logger.debug("[OK] Connexion Internet complète et fonctionnelle")
        return True
def _normalize_release_notes(raw_notes):
    if not raw_notes:
        return ""
    notes = html_module.unescape(str(raw_notes))
    notes = notes.replace("\r\n", "\n").replace("\r", "\n")
    notes = re.sub(r"\n{3,}", "\n\n", notes)
    return notes.strip()
def _extract_changelog_section(raw_text):
    text = _normalize_release_notes(raw_text)
    if not text:
        return ""

    def _is_version_heading(heading_text):
        normalized = str(heading_text or "").strip()
        return re.match(
            r"^(?:release\s+)?(?:v(?:ersion)?\s*)?\d+(?:\.\d+)+(?:\s*[(:-].*)?$",
            normalized,
            re.IGNORECASE,
        ) is not None

    lines = text.split("\n")
    heading_re = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
    changelog_start = None
    changelog_level = None

    for index, line in enumerate(lines):
        match = heading_re.match(line.strip())
        if not match:
            continue
        if "changelog" in match.group(2).lower():
            changelog_start = index + 1
            changelog_level = len(match.group(1))
            break

    if changelog_start is None:
        return text

    extracted = []
    for line in lines[changelog_start:]:
        stripped = line.strip()
        if stripped == "---":
            break
        match = heading_re.match(stripped)
        if match and len(match.group(1)) <= changelog_level and not _is_version_heading(match.group(2)):
            break
        extracted.append(line)

    return _normalize_release_notes("\n".join(extracted))
def _fetch_recent_release_changelogs(limit=5):
    repo = getattr(config, "GITHUB_REPO", "RetroGameSets/RGSX")
    api_url = f"https://api.github.com/repos/{repo}/releases"
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": "RGSX",
    }

    response = requests.get(api_url, headers=headers, timeout=10)
    response.raise_for_status()

    releases = response.json()
    if not isinstance(releases, list):
        return []

    changelogs = []
    for release in releases:
        if not isinstance(release, dict) or release.get("draft"):
            continue

        version_label = (
            release.get("tag_name")
            or release.get("name")
            or release.get("published_at")
            or "Unknown"
        )
        release_body = _extract_changelog_section(release.get("body", ""))
        if not release_body:
            release_body = "Changelog unavailable"

        changelogs.append({
            "version": str(version_label).strip(),
            "body": release_body,
        })

        if len(changelogs) >= limit:
            break

    return changelogs
def _build_recent_changelog_text(latest_version, limit=5):
    changelogs = _fetch_recent_release_changelogs(limit=limit)
    title = _("network_update_available").format(latest_version) if _ else f"Update available: {latest_version}"
    intro = f"{title}\n\nLast {len(changelogs) if changelogs else limit} changelogs:\n"

    if not changelogs:
        return f"{intro}\nChangelog unavailable"

    blocks = []
    for item in changelogs:
        blocks.append(f"=== {item['version']} ===\n{item['body']}")

    return intro + "\n\n".join(blocks) + "\n\nPress Confirm to install the update."
def _format_size(num_bytes):
    value = float(max(0, num_bytes))
    units = ["B", "KB", "MB", "GB"]
    unit_index = 0
    while value >= 1024 and unit_index < len(units) - 1:
        value /= 1024.0
        unit_index += 1
    if unit_index == 0:
        return f"{int(value)} {units[unit_index]}"
    return f"{value:.1f} {units[unit_index]}"
def _set_loading_details(*lines):
    config.loading_detail_lines = [str(line) for line in lines if line]
    config.needs_redraw = True
def _safe_remove_file(file_path, retries=8, delay=0.25):
    if not file_path or not os.path.exists(file_path):
        return True

    last_error = None
    for _ in range(retries):
        try:
            os.remove(file_path)
            return True
        except PermissionError as error:
            last_error = error
            time.sleep(delay)
        except FileNotFoundError:
            return True
        except Exception as error:
            last_error = error
            break

    if last_error is not None:
        logger.warning(f"Impossible de supprimer temporairement {file_path}: {last_error}")
    return False
def _schedule_windows_file_replace_when_unlocked(source_path: str, target_path: str) -> bool:
    """Planifie un remplacement différé d'un fichier verrouillé (ex: .bat en cours d'exécution)."""
    if config.OPERATING_SYSTEM != "Windows":
        return False
    if not source_path or not target_path:
        return False

    try:
        ps_script = (
            "$src = [IO.Path]::GetFullPath($args[0]); "
            "$dst = [IO.Path]::GetFullPath($args[1]); "
            "for ($i = 0; $i -lt 240; $i++) { "
            "  try { "
            "    Copy-Item -LiteralPath $src -Destination $dst -Force -ErrorAction Stop; "
            "    Remove-Item -LiteralPath $src -Force -ErrorAction SilentlyContinue; "
            "    exit 0; "
            "  } catch { Start-Sleep -Milliseconds 500 } "
            "} "
            "exit 1"
        )
        creationflags = 0
        if hasattr(subprocess, "CREATE_NO_WINDOW"):
            creationflags = subprocess.CREATE_NO_WINDOW
        subprocess.Popen(
            [
                "powershell",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-Command",
                ps_script,
                source_path,
                target_path,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=creationflags,
        )
        return True
    except Exception as exc:
        logger.warning(f"Impossible de planifier le remplacement différé de {target_path}: {exc}")
        return False
def _copy_windows_update_tree(source_root: str, target_root: str) -> tuple[int, int, list[str]]:
    """Copie une arborescence windows update; diffère le .bat s'il est verrouillé."""
    updated_files = 0
    deferred_files = 0
    errors: list[str] = []

    for current_root, _dirs, files in os.walk(source_root):
        rel_root = os.path.relpath(current_root, source_root)
        if rel_root == ".":
            rel_root = ""
        dest_root = os.path.join(target_root, rel_root) if rel_root else target_root
        os.makedirs(dest_root, exist_ok=True)

        for filename in files:
            src_file = os.path.join(current_root, filename)
            dst_file = os.path.join(dest_root, filename)
            dst_is_launcher = filename.lower() == "rgsx retrobat.bat"

            try:
                if os.path.exists(dst_file):
                    try:
                        os.chmod(dst_file, 0o666)
                    except Exception:
                        pass

                shutil.copy2(src_file, dst_file)
                updated_files += 1
                continue
            except PermissionError:
                pass
            except OSError:
                pass
            except Exception as exc:
                errors.append(f"{dst_file}: {exc}")
                continue

            if dst_is_launcher:
                pending_path = dst_file + ".pending_update"
                try:
                    shutil.copy2(src_file, pending_path)
                    if _schedule_windows_file_replace_when_unlocked(pending_path, dst_file):
                        deferred_files += 1
                    else:
                        errors.append(f"{dst_file}: impossible de planifier le remplacement différé")
                except Exception as exc:
                    errors.append(f"{dst_file}: {exc}")
            else:
                errors.append(f"{dst_file}: fichier verrouillé ou inaccessible")

    return updated_files, deferred_files, errors
async def _apply_pending_windows_update(latest_version: str) -> tuple[bool, str]:
    """Applique le ZIP update_windows sur Windows sans bloquer en cas de .bat verrouillé."""
    if config.OPERATING_SYSTEM != "Windows":
        return True, "Windows update skipped (non-Windows OS)"

    windows_zip_url = OTA_UPDATE_WINDOWS_ZIP
    windows_zip_path = os.path.join(UPDATE_FOLDER, f"RGSX_update_windows_v{latest_version}.zip")
    extract_root = os.path.join(UPDATE_FOLDER, f"RGSX_update_windows_extract_{latest_version}")

    try:
        logger.debug(f"Téléchargement du ZIP Windows update: {windows_zip_url}")
        with requests.get(windows_zip_url, stream=True, timeout=10) as r:
            if r.status_code == 404:
                logger.info("ZIP update_windows introuvable (404): mise à jour Windows spécifique ignorée")
                return True, "Windows update zip not published"
            r.raise_for_status()
            with open(windows_zip_path, "wb") as f:
                for chunk in r.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)

        if os.path.isdir(extract_root):
            shutil.rmtree(extract_root, ignore_errors=True)
        os.makedirs(extract_root, exist_ok=True)

        ok_extract, msg_extract = await asyncio.to_thread(extract_update, windows_zip_path, extract_root, windows_zip_url)
        if not ok_extract:
            return False, f"Windows update extraction failed: {msg_extract}"

        # The Windows update zip is expected to contain the direct content
        # of roms/windows at archive root.
        source_root = extract_root

        target_root = os.path.join(config.ROMS_FOLDER, "windows")
        os.makedirs(target_root, exist_ok=True)

        updated_count, deferred_count, copy_errors = _copy_windows_update_tree(source_root, target_root)
        for error_text in copy_errors[:8]:
            logger.warning(f"Windows update: {error_text}")
        if len(copy_errors) > 8:
            logger.warning(f"Windows update: {len(copy_errors) - 8} erreurs supplémentaires omises")

        if copy_errors and updated_count == 0 and deferred_count == 0:
            return False, "Windows update copy failed"

        return True, f"Windows update applied ({updated_count} fichiers, {deferred_count} différé(s))"
    except Exception as exc:
        return False, f"Windows update failed: {exc}"
    finally:
        _safe_remove_file(windows_zip_path)
        try:
            if os.path.isdir(extract_root):
                shutil.rmtree(extract_root, ignore_errors=True)
        except Exception:
            pass
async def apply_pending_update(latest_version):
    UPDATE_ZIP = OTA_UPDATE_ZIP
    logger.debug(f"URL de mise à jour : {UPDATE_ZIP} (version {latest_version})")

    config.current_loading_system = _("network_update_available").format(latest_version)
    config.loading_progress = 10.0
    _set_loading_details("Preparing update...")
    logger.debug(f"Téléchargement du ZIP de mise à jour : {UPDATE_ZIP}")

    os.makedirs(UPDATE_FOLDER, exist_ok=True)
    update_zip_path = os.path.join(UPDATE_FOLDER, f"RGSX_update_v{latest_version}.zip")
    logger.debug(f"Téléchargement de {UPDATE_ZIP} vers {update_zip_path}")

    with requests.get(UPDATE_ZIP, stream=True, timeout=10) as r:
        r.raise_for_status()
        total_size = int(r.headers.get('content-length', 0))
        downloaded = 0
        start_time = time.time()
        with open(update_zip_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)
                    downloaded += len(chunk)
                    config.loading_progress = 10.0 + (40.0 * downloaded / total_size) if total_size > 0 else 10.0
                    elapsed = max(time.time() - start_time, 0.001)
                    speed = downloaded / elapsed
                    progress_line = f"Download: {_format_size(downloaded)} / {_format_size(total_size)}" if total_size > 0 else f"Download: {_format_size(downloaded)}"
                    _set_loading_details(progress_line, f"Speed: {_format_size(speed)}/s")
                    await asyncio.sleep(0)
    logger.debug(f"ZIP téléchargé : {update_zip_path}")

    config.current_loading_system = _("network_extracting_update")
    config.loading_progress = 60.0
    _set_loading_details(f"Archive: {_format_size(os.path.getsize(update_zip_path))}")
    success, message = await asyncio.to_thread(extract_update, update_zip_path, APP_FOLDER, UPDATE_ZIP)
    if not success:
        logger.error(f"Échec de l'extraction : {message}")
        return False, _("network_extraction_failed").format(message)

    if _safe_remove_file(update_zip_path):
        logger.debug(f"Fichier ZIP {update_zip_path} supprimé")

    # Mise à jour complémentaire des assets Windows (launcher/scripts), uniquement sous Windows.
    if config.OPERATING_SYSTEM == "Windows":
        config.current_loading_system = "Applying Windows launcher update..."
        config.loading_progress = 85.0
        _set_loading_details("Applying Windows-specific update package")
        win_ok, win_msg = await _apply_pending_windows_update(latest_version)
        if win_ok:
            logger.info(f"Windows update: {win_msg}")
        else:
            # Ne pas annuler l'update principale si le package Windows échoue.
            logger.warning(f"Windows update non bloquant: {win_msg}")

    config.current_loading_system = _("network_update_completed")
    config.loading_progress = 100.0
    _set_loading_details("Update installed successfully")
    logger.debug("Mise à jour terminée avec succès")

    config.menu_state = "restart_popup"
    config.update_result_message = _("network_update_success").format(latest_version)
    config.popup_message = config.update_result_message
    config.popup_timer = 2000
    config.update_result_error = False
    config.update_result_start_time = pygame.time.get_ticks() if pygame is not None else 0
    config.needs_redraw = True
    logger.debug("Affichage de la popup de mise à jour réussie, redémarrage imminent")

    try:
        from utils import restart_application
        restart_application(2000)
    except Exception as e:
        logger.error(f"Erreur lors du redémarrage après mise à jour: {e}")

    return True, _("network_update_success_message")
async def check_for_updates():
    try:
        logger.debug("Vérification de la version disponible sur le serveur")
        config.current_loading_system = _("network_checking_updates")
        config.loading_progress = 5.0
        config.needs_redraw = True

        # Liste des endpoints à essayer (GitHub principal, puis fallback)
        endpoints = [
            OTA_VERSION_ENDPOINT,
            "https://retrogamesets.fr/softs/version.json"
        ]
        
        response = None
        last_error = None
        
        for endpoint_index, endpoint in enumerate(endpoints):
            is_fallback = endpoint_index > 0
            if is_fallback:
                logger.info(f"Tentative sur endpoint de secours : {endpoint}")
            
            # Gestion des erreurs de rate limit GitHub (429) avec retry
            max_retries = 3 if not is_fallback else 1  # Moins de retries sur fallback
            retry_count = 0
            
            while retry_count < max_retries:
                try:
                    response = requests.get(endpoint, timeout=10)
                    
                    # Gestion spécifique des erreurs 429 (Too Many Requests) - surtout pour GitHub
                    if response.status_code == 429:
                        retry_after = response.headers.get('retry-after')
                        x_ratelimit_remaining = response.headers.get('x-ratelimit-remaining', '1')
                        x_ratelimit_reset = response.headers.get('x-ratelimit-reset')
                        
                        if retry_after:
                            # En-tête retry-after présent : attendre le nombre de secondes spécifié
                            wait_time = int(retry_after)
                            logger.warning(f"Rate limit atteint (429) sur {endpoint}. Attente de {wait_time}s (retry-after header)")
                        elif x_ratelimit_remaining == '0' and x_ratelimit_reset:
                            # x-ratelimit-remaining est 0 : attendre jusqu'à x-ratelimit-reset
                            import time
                            reset_time = int(x_ratelimit_reset)
                            current_time = int(time.time())
                            wait_time = max(reset_time - current_time, 60)  # Minimum 60s
                            logger.warning(f"Rate limit atteint (429) sur {endpoint}. Attente de {wait_time}s (x-ratelimit-reset)")
                        else:
                            # Pas d'en-têtes spécifiques : attendre au moins 60s
                            wait_time = 60
                            logger.warning(f"Rate limit atteint (429) sur {endpoint}. Attente de {wait_time}s par défaut")
                        
                        if retry_count < max_retries - 1:
                            logger.info(f"Nouvelle tentative dans {wait_time}s... ({retry_count + 1}/{max_retries})")
                            await asyncio.sleep(wait_time)
                            retry_count += 1
                            continue
                        else:
                            # Si rate limit persistant et qu'on est sur GitHub, essayer le fallback
                            if not is_fallback:
                                logger.warning(f"Rate limit GitHub persistant, passage au serveur de secours")
                                break  # Sortir de la boucle retry pour essayer le prochain endpoint
                            raise requests.exceptions.HTTPError(
                                f"Limite de débit atteinte (429). Veuillez réessayer plus tard."
                            )
                    
                    response.raise_for_status()
                    # Succès, sortir de toutes les boucles
                    logger.debug(f"Version récupérée avec succès depuis : {endpoint}")
                    break
                    
                except requests.exceptions.HTTPError as e:
                    last_error = e
                    if response and response.status_code == 429:
                        # 429 géré au-dessus, continuer la boucle ou passer au fallback
                        retry_count += 1
                        if retry_count >= max_retries:
                            break  # Passer au prochain endpoint
                    else:
                        # Erreur HTTP autre que 429
                        logger.warning(f"Erreur HTTP {response.status_code if response else 'inconnue'} sur {endpoint}")
                        break  # Passer au prochain endpoint
                        
                except requests.exceptions.RequestException as e:
                    last_error = e
                    if retry_count < max_retries - 1:
                        # Erreur réseau, réessayer avec backoff exponentiel
                        wait_time = 2 ** retry_count  # 1s, 2s, 4s
                        logger.warning(f"Erreur réseau sur {endpoint}. Nouvelle tentative dans {wait_time}s...")
                        await asyncio.sleep(wait_time)
                        retry_count += 1
                    else:
                        logger.warning(f"Erreur réseau persistante sur {endpoint} : {e}")
                        break  # Passer au prochain endpoint
            
            # Si on a une réponse valide, sortir de la boucle des endpoints
            if response and response.status_code == 200:
                break
        
        # Si aucun endpoint n'a fonctionné
        if not response or response.status_code != 200:
            raise last_error if last_error else requests.exceptions.RequestException(
                "Impossible de vérifier les mises à jour sur tous les serveurs"
            )
        
        # Accepter différents content-types (application/json, text/plain, text/html)
        content_type = response.headers.get("content-type", "")
        allowed_types = ["application/json", "text/plain", "text/html"]
        if not any(allowed in content_type for allowed in allowed_types):
            raise ValueError(
                f"Le fichier version.json n'est pas un JSON valide (type de contenu : {content_type})"
            )
        
        version_data = response.json()
        latest_version = version_data.get("version")
        logger.debug(f"Version distante : {latest_version}, version locale : {config.app_version}")

        # --- Protection anti-downgrade ---
        def _parse_version(v: str):
            try:
                return [int(p) for p in str(v).strip().split('.') if p.isdigit()]
            except Exception:
                return [0]

        local_parts = _parse_version(getattr(config, 'app_version', '0'))
        remote_parts = _parse_version(latest_version or '0')
        # Normaliser longueur
        max_len = max(len(local_parts), len(remote_parts))
        local_parts += [0] * (max_len - len(local_parts))
        remote_parts += [0] * (max_len - len(remote_parts))
        logger.debug(f"Comparaison versions normalisées local={local_parts} remote={remote_parts}")
        if remote_parts <= local_parts:
            # Pas de mise à jour si version distante identique ou inférieure (empêche downgrade accidentel)
            logger.info("Version distante inférieure ou égale – skip mise à jour (anti-downgrade)")
            return True, _("network_no_update_available") if _ else "No update (local >= remote)"

        if latest_version != config.app_version:
            try:
                changelog_text = _build_recent_changelog_text(latest_version, limit=5)
            except Exception as changelog_error:
                logger.warning(f"Impossible de récupérer les changelogs récents: {changelog_error}")
                changelog_text = "Changelog unavailable"

            config.pending_update_version = latest_version
            config.startup_update_confirmed = False
            config.text_file_name = f"RGSX {latest_version}"
            config.text_file_content = changelog_text
            config.text_file_scroll_offset = 0
            config.text_file_mode = "ota_update"
            config.previous_menu_state = "loading"
            config.menu_state = "text_file_viewer"
            config.update_checked = True
            config.needs_redraw = True
            return True, _("network_update_available").format(latest_version)
        else:
            logger.debug("Aucune mise à jour disponible")
            config.update_checked = True
            return True, _("network_no_update_available")

    except Exception as e:
        logger.error(f"Erreur OTA : {str(e)}")
        config.menu_state = "update_result"
        config.update_result_message = _("network_update_error").format(str(e))
        config.popup_message = config.update_result_message
        config.popup_timer = 5000
        config.update_result_error = True
        config.update_result_start_time = pygame.time.get_ticks() if pygame is not None else 0
        config.needs_redraw = True
        return False, _("network_check_update_error").format(str(e))
def extract_update(zip_path, dest_dir, source_url):
    
    try:
        os.makedirs(dest_dir, exist_ok=True)
        logger.debug(f"Tentative d'ouverture du ZIP : {zip_path}")
        # Extraire le ZIP
        skipped_files = []
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            file_infos = [info for info in zip_ref.infolist() if not info.is_dir()]
            total_bytes = sum(max(0, info.file_size) for info in file_infos)
            extracted_bytes = 0
            for file_info in zip_ref.infolist():
                try:
                    zip_ref.extract(file_info, dest_dir)
                    if not file_info.is_dir():
                        extracted_bytes += max(0, file_info.file_size)
                        if total_bytes > 0:
                            config.loading_progress = 60.0 + (40.0 * extracted_bytes / total_bytes)
                        _set_loading_details(
                            f"Extracting: {_format_size(extracted_bytes)} / {_format_size(total_bytes)}" if total_bytes > 0 else f"Extracting: {file_info.filename}",
                            file_info.filename
                        )
                except PermissionError as e:
                    logger.warning(f"Impossible d'extraire {file_info.filename}: {str(e)}")
                    skipped_files.append(file_info.filename)
                except Exception as e:
                    logger.warning(f"Erreur lors de l'extraction de {file_info.filename}: {str(e)}")
                    skipped_files.append(file_info.filename)

        if skipped_files:
            message = _("network_extraction_partial").format(', '.join(skipped_files))
            logger.warning(message)
            return True, message  # Considérer comme succès si certains fichiers sont extraits
        return True, _("network_extraction_success")

    except Exception as e:
        logger.error(f"Erreur critique lors de l'extraction du ZIP {source_url}: {str(e)}")
        return False, _("network_zip_extraction_error").format(source_url, str(e))
