"""network.upnp — UPnP + aria2/torrent altyapisi (port, IGD, seeding status).

Faz 6-2: eski network.py'den tasindi. Davranis degismez.
"""

import os
import socket
import threading
import time
import logging
import config
import qbittorrent_backend
from network import pause_events
from network.helpers import (
    _save_history_with_feedback,
    _should_prefer_qbittorrent_backend,
)

logger = logging.getLogger("network")

def _reserve_ephemeral_tcp_port() -> int:
    """Reserve and return a free local TCP port number for aria2 listen-port."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(("", 0))
        return int(sock.getsockname()[1])
    finally:
        try:
            sock.close()
        except Exception:
            pass
def _get_local_ip_for_route(dest_ip: str = "8.8.8.8", dest_port: int = 80) -> str | None:
    """Détermine l'IP locale utilisée pour joindre `dest_ip` (sans envoyer de données),
    afin de forcer la découverte UPnP sur la bonne interface réseau (utile quand
    plusieurs adaptateurs sont actifs : VPN, Docker/Hyper-V, VMware, etc., qui font
    parfois échouer la sélection d'interface multicast par défaut sur Windows)."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect((dest_ip, dest_port))
        return s.getsockname()[0]
    except Exception:
        return None
    finally:
        try:
            s.close()
        except Exception:
            pass
def _upnp_discover_igd(timeout: float = 3.0) -> tuple[str, str] | None:
    """
    Découverte SSDP de l'IGD (Internet Gateway Device) sur le réseau local.
    Retourne (location_url, local_ip) ou None si aucun dispositif trouvé.
    """
    SSDP_ADDR = "239.255.255.250"
    SSDP_PORT = 1900
    SSDP_ST   = "urn:schemas-upnp-org:device:InternetGatewayDevice:1"
    msg = (
        "M-SEARCH * HTTP/1.1\r\n"
        f"HOST: {SSDP_ADDR}:{SSDP_PORT}\r\n"
        "MAN: \"ssdp:discover\"\r\n"
        "MX: 2\r\n"
        f"ST: {SSDP_ST}\r\n"
        "\r\n"
    ).encode()

    # Sur Windows, avec plusieurs adaptateurs réseau actifs (VPN, Docker/Hyper-V,
    # VMware, etc.), le système peut envoyer le paquet multicast sur la mauvaise
    # interface, ce qui fait que la box ne le reçoit jamais et qu'aucune réponse
    # ne revient. On détermine explicitement l'IP locale utilisée pour joindre
    # Internet (donc la vraie interface réseau) et on force la socket à l'utiliser
    # via bind() + IP_MULTICAST_IF.
    preferred_local_ip = _get_local_ip_for_route()

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
    try:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        if preferred_local_ip:
            try:
                sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_IF, socket.inet_aton(preferred_local_ip))
                sock.bind((preferred_local_ip, 0))
                logger.debug(f"UPnP: interface locale forcée pour la découverte SSDP: {preferred_local_ip}")
            except Exception as e:
                logger.debug(f"UPnP: impossible de forcer l'interface locale {preferred_local_ip}: {e}")
        sock.settimeout(timeout)

        deadline = time.time() + timeout
        # Renvoyer la requête M-SEARCH une seconde fois après un court délai :
        # les paquets UDP/multicast peuvent se perdre, un seul essai n'est pas fiable.
        sock.sendto(msg, (SSDP_ADDR, SSDP_PORT))
        resent = False
        while True:
            remaining = deadline - time.time()
            if remaining <= 0:
                break
            if not resent and remaining < timeout - 0.5:
                try:
                    sock.sendto(msg, (SSDP_ADDR, SSDP_PORT))
                except Exception:
                    pass
                resent = True
            sock.settimeout(max(0.1, remaining))
            try:
                data, addr = sock.recvfrom(4096)
            except OSError:
                break
            text = data.decode(errors="replace")
            logger.debug(f"UPnP: réponse SSDP reçue de {addr[0]}")
            for line in text.splitlines():
                if line.upper().startswith("LOCATION:"):
                    location = line.split(":", 1)[1].strip()
                    local_ip = preferred_local_ip or addr[0]
                    if not preferred_local_ip:
                        # addr[0] est l'IP de la box; on veut notre propre IP locale.
                        # On la déduit en ouvrant une socket UDP vers la box.
                        deduced = _get_local_ip_for_route(addr[0], 1900)
                        if deduced:
                            local_ip = deduced
                    return location, local_ip
    finally:
        sock.close()
    logger.debug("UPnP: aucune réponse SSDP reçue (vérifier pare-feu Windows / bonne interface réseau)")
    return None
def _upnp_get_control_url(location: str) -> str | None:
    """
    Récupère l'URL de contrôle WANIPConnection (ou WANPPPConnection) depuis
    le XML de description de l'IGD.
    """
    import xml.etree.ElementTree as _ET
    import urllib.request as _req

    try:
        with _req.urlopen(location, timeout=5) as resp:
            xml_data = resp.read()
    except Exception as e:
        logger.debug("UPnP: impossible de récupérer la description IGD: %s", e)
        return None

    try:
        root = _ET.fromstring(xml_data)
    except Exception as e:
        logger.debug("UPnP: XML IGD invalide: %s", e)
        return None

    # Supprimer les namespaces pour simplifier la recherche
    for el in root.iter():
        if "}" in el.tag:
            el.tag = el.tag.split("}", 1)[1]

    base = location.rsplit("/", 1)[0]
    for service in root.iter("service"):
        st = (service.findtext("serviceType") or "").lower()
        if "wanipconnection" in st or "wanpppconnection" in st:
            ctrl = service.findtext("controlURL") or ""
            if ctrl.startswith("/"):
                from urllib.parse import urlparse as _up
                p = _up(location)
                return f"{p.scheme}://{p.netloc}{ctrl}"
            return f"{base}/{ctrl.lstrip('/')}"
    return None
def _upnp_soap(control_url: str, service_type: str, action: str, args: dict) -> bool:
    """Envoie une requête SOAP UPnP. Retourne True si succès (HTTP 200)."""
    import urllib.request as _req

    args_xml = "".join(f"<{k}>{v}</{k}>" for k, v in args.items())
    body = (
        '<?xml version="1.0"?>'
        '<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
        's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">'
        '<s:Body>'
        f'<u:{action} xmlns:u="{service_type}">'
        f'{args_xml}'
        f'</u:{action}>'
        '</s:Body>'
        '</s:Envelope>'
    ).encode()
    headers = {
        "Content-Type": 'text/xml; charset="utf-8"',
        "SOAPAction": f'"{service_type}#{action}"',
        "Content-Length": str(len(body)),
    }
    try:
        req = _req.Request(control_url, data=body, headers=headers, method="POST")
        with _req.urlopen(req, timeout=5) as resp:
            return resp.status == 200
    except Exception as e:
        logger.debug("UPnP SOAP %s: %s", action, e)
        return False
def _upnp_open_port(port: int, description: str = "RGSX-BT") -> dict | None:
    """
    Ouvre le port TCP+UDP via UPnP (pure stdlib, compatible Batocera/Windows/Linux).
    Retourne un dict de contexte pour la fermeture, ou None si UPnP indisponible.
    """
    result = _upnp_discover_igd()
    if result is None:
        logger.debug("UPnP: aucun IGD trouvé sur le réseau local")
        return None
    location, local_ip = result
    control_url = _upnp_get_control_url(location)
    if control_url is None:
        logger.debug("UPnP: URL de contrôle WANIPConnection introuvable")
        return None

    # Déterminer le serviceType réel depuis l'URL de contrôle
    service_type = "urn:schemas-upnp-org:service:WANIPConnection:1"
    if "ppp" in control_url.lower():
        service_type = "urn:schemas-upnp-org:service:WANPPPConnection:1"

    opened = []
    for proto in ("TCP", "UDP"):
        ok = _upnp_soap(control_url, service_type, "AddPortMapping", {
            "NewRemoteHost": "",
            "NewExternalPort": port,
            "NewProtocol": proto,
            "NewInternalPort": port,
            "NewInternalClient": local_ip,
            "NewEnabled": 1,
            "NewPortMappingDescription": description,
            "NewLeaseDuration": 0,
        })
        if ok:
            opened.append(proto)
            logger.info("UPnP: port %s/%s ouvert → %s:%s", port, proto, local_ip, port)
        else:
            logger.debug("UPnP: échec ouverture port %s/%s", port, proto)

    if not opened:
        return None
    return {"control_url": control_url, "service_type": service_type, "opened": opened}
def _upnp_close_port(ctx: dict | None, port: int) -> None:
    """Ferme le port TCP+UDP précédemment ouvert via UPnP."""
    if not ctx:
        return
    for proto in ctx.get("opened", []):
        ok = _upnp_soap(ctx["control_url"], ctx["service_type"], "DeletePortMapping", {
            "NewRemoteHost": "",
            "NewExternalPort": port,
            "NewProtocol": proto,
        })
        if ok:
            logger.info("UPnP: port %s/%s fermé", port, proto)
        else:
            logger.debug("UPnP: impossible de fermer port %s/%s", port, proto)
def _download_torrent_with_aria2(
    torrent_meta: dict[str, str | int],
    dest_dir: str,
    dest_path: str,
    task_id: str,
    cancel_ev,
    progress_queue,
    original_history_url: str = "",
    allow_resume: bool = True,
    stall_retries_left: int | None = None,
) -> tuple[bool, str]:
    source_url = str(torrent_meta.get("source_url") or "")
    relative_path = str(torrent_meta.get("relative_path") or "").strip() or os.path.basename(dest_path)
    fallback_name = os.path.basename(relative_path) or os.path.basename(dest_path)
    file_index = int(torrent_meta.get("file_index") or 1)
    total_size = int(torrent_meta.get("size_bytes") or 0)
    # Créé ici (pas seulement dans toggle_pause_download) pour que l'objet Event soit déjà
    # en place avant que l'utilisateur ne mette en pause depuis l'UI.
    pause_ev = pause_events.setdefault(task_id, threading.Event())

    if _should_prefer_qbittorrent_backend():
        platform_label = "Windows" if config.OPERATING_SYSTEM == "Windows" else "Linux/Batocera"
        logger.info("Téléchargement torrent via qBittorrent sur %s", platform_label)
        try:
            return qbittorrent_backend.download_torrent_via_qbittorrent(
                torrent_meta, dest_dir, dest_path, task_id, cancel_ev, progress_queue, original_history_url,
                pause_ev=pause_ev,
            )
        except qbittorrent_backend.BackendUnavailableError as exc:
            raise qbittorrent_backend.BackendUnavailableError(str(exc)) from exc

    raise qbittorrent_backend.BackendUnavailableError("qBittorrent introuvable, non démarré ou non disponible")
def _update_seeding_status(original_history_url: str, peers: int, ul_speed: float = 0.0) -> None:
    """Met à jour l'entrée historique avec le statut Seeding, le nombre de peers et la vitesse UL."""
    if not isinstance(config.history, list):
        return
    for entry in config.history:
        if entry.get("url") == original_history_url:
            entry["status"] = "Seeding"
            entry["seeds"] = peers
            entry["ul_speed"] = ul_speed
            config.needs_redraw = True
            break
def _stop_seeding_status(original_history_url: str) -> None:
    """Restaure le statut Download_OK une fois le seed terminé."""
    if not isinstance(config.history, list):
        return
    for entry in config.history:
        if entry.get("url") == original_history_url:
            if entry.get("status") == "Seeding":
                entry["status"] = "Download_OK"
                entry["seeds"] = 0
                config.needs_redraw = True
                _save_history_with_feedback("seeder:done")
            break
def _start_pending_torrent_seed_if_any(task_id: str) -> None:
    return
def _discard_pending_torrent_seed(task_id: str) -> None:
    return
