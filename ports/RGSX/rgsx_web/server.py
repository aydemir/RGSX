# -*- coding: utf-8 -*-
"""Serveur HTTP (run_server) + CURRENT_HTTPD pour arrêt propre via rgsx_manager.

NOTE roadmap: FlushFileHandler logging bootstrap'ında kalıyor (__init__.py) —
logging setup'ı loglardan önce çalışır, server.py `from . import logger` ister,
bu da döngüsel import oluştururdu. server.py = run_server + CURRENT_HTTPD.
"""
import logging
import os
import socket
import time

import config

from . import logger
from .handlers import RGSXHandler

# Dernier serveur HTTP démarré (utilisé par rgsx_manager pour l'arrêt propre)
CURRENT_HTTPD = None


def run_server(host='0.0.0.0', port=5000, handler_class=RGSXHandler, kill_conflicts=True):
    """Démarre le serveur HTTP.

    kill_conflicts=True ise port doluysa (Faz 4 öncesi davranış) o process'i öldürür.
    Manager (rgsx_manager) kill_conflicts=False kullanır: zaten alternatif port seçti,
    başka bir uygulamanın process'ini asla öldürmez.
    """
    from http.server import ThreadingHTTPServer

    server_address = (host, port)

    # Créer une classe HTTPServer personnalisée qui réutilise le port
    # (multithread pour supporter les connexions SSE longues)
    class ReuseAddrHTTPServer(ThreadingHTTPServer):
        allow_reuse_address = True

    # Tuer les processus existants utilisant le port (plateforme spécifique)
    if kill_conflicts:
        try:
            import subprocess
            # Windows: utiliser netstat + taskkill
            if os.name == 'nt' or getattr(config, 'OPERATING_SYSTEM', '').lower() == 'windows':
                try:
                    netstat = subprocess.run(['netstat', '-ano'], capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=3)
                    lines = netstat.stdout.splitlines()
                    pids = set()
                    for line in lines:
                        parts = line.split()
                        if len(parts) >= 5:
                            local = parts[1]
                            pid = parts[-1]
                            if local.endswith(f':{port}'):
                                pids.add(pid)
                    for pid in pids:
                        # Safer: ignore PID 0 and non-numeric entries (system / header lines)
                        if not pid or not pid.isdigit():
                            continue
                        pid_int = int(pid)
                        if pid_int <= 0:
                            continue
                        try:
                            subprocess.run(['taskkill', '/PID', pid, '/F'], timeout=3)
                            logger.info(f"Processus {pid} tué (port {port} libéré) [Windows]")
                        except Exception as e:
                            logger.warning(f"Impossible de tuer le processus {pid}: {e}")
                except Exception as e:
                    logger.debug(f"Windows port release check failed: {e}")
            else:
                # Unix-like: utiliser lsof + kill
                result = subprocess.run(['lsof', '-ti', f':{port}'], capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=2)
                pids = result.stdout.strip().split('\n')
                for pid in pids:
                    if pid:
                        try:
                            subprocess.run(['kill', '-9', pid], timeout=2)
                            logger.info(f"Processus {pid} tué (port {port} libéré)")
                        except Exception as e:
                            logger.warning(f"Impossible de tuer le processus {pid}: {e}")
        except Exception as e:
            logger.warning(f"Impossible de libérer le port {port}: {e}")

    # Attendre un peu pour que le port se libère
    time.sleep(1)

    httpd = ReuseAddrHTTPServer(server_address, handler_class)

    global CURRENT_HTTPD
    CURRENT_HTTPD = httpd

    logger.info("=" * 60)
    logger.info("RGSX Web Server démarré !")
    logger.info("=" * 60)
    logger.info(f"Accès local: http://localhost:{port}")

    # Force flush
    for handler in logging.root.handlers:
        handler.flush()

    # Afficher l'IP locale pour accès réseau (éviter les cartes virtuelles)
    try:
        # Méthode 1: Créer une connexion UDP pour trouver l'IP réelle (sans envoyer de données)
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        local_ip = s.getsockname()[0]
        s.close()
        logger.info(f"Accès réseau: http://{local_ip}:{port}")
    except Exception as e:
        # Fallback: méthode classique
        try:
            hostname = socket.gethostname()
            local_ip = socket.gethostbyname(hostname)
            logger.info(f"🌍 Accès réseau: http://{local_ip}:{port}")
        except:
            logger.warning("⚠️ Impossible de déterminer l'IP locale")

    logger.info("=" * 60)
    logger.info("Appuyez sur Ctrl+C pour arrêter le serveur")
    logger.info("=" * 60)

    # Force flush final avant de commencer à servir
    for handler in logging.root.handlers:
        handler.flush()

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        logger.info("\n🛑 Arrêt du serveur...")
        for handler in logging.root.handlers:
            handler.flush()
        httpd.shutdown()
        logger.info("✅ Serveur arrêté proprement")
        for handler in logging.root.handlers:
            handler.flush()
