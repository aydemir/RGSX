# -*- coding: utf-8 -*-
"""RGSX Manager - Sunucu Ayarları penceresi (Tkinter).

Systray'dan açılan küçük bir dialog: port, host, auto-start ve kaydet+restart.
WebUI ayarlar sayfasına ("Ayarlar") göre servis seviyesindeki ayarlar içindir.
Tkinter stdlib'de mevcuttur; pythonw ile ek pencere yoktur, bu modül pencereyi açar.
"""

import logging
import os
import socket
import sys
import threading

logger = logging.getLogger("rgsx_manager")


def _validate_port(port_str: str) -> int:
    port = int(port_str)
    if not (0 < port < 65536):
        raise ValueError("Port 1-65535 arasında olmalı")
    return port


def _validate_host(host_str: str) -> str:
    host = host_str.strip()
    if not host:
        raise ValueError("Host boş olamaz")
    return host


def _is_port_free(port: int, host: str = "0.0.0.0") -> bool:
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.bind((host, port))
            return True
        finally:
            s.close()
    except OSError:
        return False


class ServerSettingsDialog:
    """Modal Tkinter dialog - Sunucu Ayarları."""

    def __init__(self, on_save, get_current, app_dir=None):
        self.on_save = on_save
        self.get_current = get_current
        self.app_dir = app_dir or os.path.dirname(os.path.abspath(__file__))
        self.root = None
        self.result = None

    def open(self):
        import tkinter as tk
        from tkinter import messagebox, ttk

        current = self.get_current() or {}
        cur_port = int(current.get("port", 5000))
        cur_host = str(current.get("host", "0.0.0.0"))
        cur_autostart = bool(current.get("autostart", True))

        self.root = tk.Tk()
        self.root.title("RGSX - Sunucu Ayarları")
        self.root.resizable(False, False)

        frm = ttk.Frame(self.root, padding=14)
        frm.grid(row=0, column=0, sticky="nsew")

        ttk.Label(frm, text="Port (1-65535):").grid(row=0, column=0, sticky="w", pady=3)
        port_var = tk.StringVar(value=str(cur_port))
        port_entry = ttk.Entry(frm, textvariable=port_var, width=12)
        port_entry.grid(row=0, column=1, sticky="w", pady=3, padx=(8, 0))

        ttk.Label(frm, text="Host (0.0.0.0 = herkes):").grid(row=1, column=0, sticky="w", pady=3)
        host_var = tk.StringVar(value=cur_host)
        host_entry = ttk.Entry(frm, textvariable=host_var, width=20)
        host_entry.grid(row=1, column=1, sticky="w", pady=3, padx=(8, 0))

        autostart_var = tk.BooleanVar(value=cur_autostart)
        ttk.Checkbutton(frm, text="Bilgisayar açılışında otomatik başlat",
                        variable=autostart_var).grid(row=2, column=0, columnspan=2, sticky="w", pady=6)

        status_var = tk.StringVar(value="")
        ttk.Label(frm, textvariable=status_var, foreground="#777").grid(
            row=3, column=0, columnspan=2, sticky="w", pady=2)

        btns = ttk.Frame(frm)
        btns.grid(row=4, column=0, columnspan=2, sticky="ew", pady=(10, 0))

        def _save_and_restart():
            try:
                port = _validate_port(port_var.get())
                host = _validate_host(host_var.get())
            except ValueError as e:
                messagebox.showerror("RGSX - Ayarlar", str(e), parent=self.root)
                return

            if port != cur_port and not _is_port_free(port, host):
                status_var.set(f"Port {port} dolu — farklı bir port seç")
                return

            restart = (port != cur_port or host != cur_host)
            self.result = {
                "port": port,
                "host": host,
                "autostart": autostart_var.get(),
                "restart": restart,
            }
            save_btn.state(["disabled"])
            status_var.set("Kaydediliyor...")
            # Dialog'u kapatıp kaydetme+restart'ı callback'te yap (UI donmasın)
            self.root.after(50, self._finish)

        def _cancel():
            self.result = None
            self.root.destroy()

        save_btn = ttk.Button(btns, text="Kaydet ve Yeniden Başlat", command=_save_and_restart)
        save_btn.grid(row=0, column=0, sticky="ew", padx=(0, 6))
        ttk.Button(btns, text="İptal", command=_cancel).grid(row=0, column=1, sticky="ew", padx=(6, 0))
        btns.columnconfigure(0, weight=1)
        btns.columnconfigure(1, weight=1)

        port_entry.focus_set()
        self.root.eval('tk::PlaceWindow . center')
        self.root.mainloop()
        return self.result

    def _finish(self):
        try:
            self.root.destroy()
        except Exception:
            pass
        if self.on_save:
            self.on_save(self.result)


def open_server_settings_dialog(on_save, get_current, app_dir=None):
    """Systray callback'i: dialog'u ayrı bir thread'de aç (tkinter ana thread'i bloklamaz)."""
    dialog = ServerSettingsDialog(on_save, get_current, app_dir)

    def _run():
        try:
            dialog.open()
        except Exception as e:
            logger.error(f"[MANAGER] Settings dialog hatası: {e}")
            try:
                import tkinter
                from tkinter import messagebox
                tk = tkinter.Tk()
                tk.withdraw()
                messagebox.showerror("RGSX - Ayarlar", f"Ayarlar penceresi açılamadı:\n{e}")
                tk.destroy()
            except Exception:
                pass

    threading.Thread(target=_run, daemon=True).start()
