"""Faz 3 — qBittorrent WebUI port fallback testleri.

18572 doluysa backend'in 18572+N aralığında serbest porta geçmesi gerekiyor
(Windows + Linux aynı davranış). Burada yalnızca saf port seçim mantığı test
edilir — gerçek qBittorrent süreci başlatılmaz.
"""

import os
import socket

import pytest

import qbittorrent_backend as qbt


class TestIsPortFree:
    def test_free_port_returns_true(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("0.0.0.0", 0))
        port = s.getsockname()[1]
        s.close()
        assert qbt._is_port_free(port) is True

    def test_occupied_port_returns_false(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.bind(("0.0.0.0", 0))
            port = s.getsockname()[1]
            s.listen(1)
            assert qbt._is_port_free(port) is False
        finally:
            s.close()

    def test_double_bind_without_reuseaddr_fails(self):
        s1 = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s2 = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s1.bind(("0.0.0.0", 0))
            port = s1.getsockname()[1]
            s1.listen(1)
            # SO_REUSEADDR kullanılmadan ikinci bind başarısız olmalı (Windows uyumu)
            with pytest.raises(OSError):
                s2.bind(("0.0.0.0", port))
        finally:
            s1.close()
            s2.close()


class TestFindFreeWebuiPort:
    def test_returns_target_when_free(self):
        assert qbt._find_free_webui_port() == qbt._TARGET_PORT

    def test_falls_back_when_target_occupied(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind(("0.0.0.0", qbt._TARGET_PORT))
            s.listen(1)
            chosen = qbt._find_free_webui_port()
            assert chosen != qbt._TARGET_PORT
            assert qbt._TARGET_PORT < chosen <= qbt._TARGET_PORT + qbt._PORT_MAX_ATTEMPTS
            assert qbt._is_port_free(chosen) is True
        finally:
            s.close()

    def test_returns_zero_when_range_exhausted(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind(("0.0.0.0", qbt._TARGET_PORT))
            s.listen(1)
            assert qbt._find_free_webui_port(max_attempts=0) == 0
        finally:
            s.close()


class TestFindAvailablePort:
    def test_preferred_used_when_free(self):
        assert qbt._find_available_port(40001) == 40001

    def test_fallback_scan(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind(("0.0.0.0", 40002))
            s.listen(1)
            chosen = qbt._find_available_port(40002)
            assert chosen == 40003  # 40003 serbest olmalı
        finally:
            s.close()


class TestWebuiPortCandidates:
    def test_target_first_then_range(self):
        candidates = list(qbt._webui_port_candidates(max_attempts=3))
        assert candidates[0] == qbt._TARGET_PORT
        assert candidates == [
            qbt._TARGET_PORT,
            qbt._TARGET_PORT + 1,
            qbt._TARGET_PORT + 2,
            qbt._TARGET_PORT + 3,
        ]


class TestWebuiUrl:
    def test_default_port(self, monkeypatch):
        monkeypatch.setattr(qbt, "_base_url", f"http://127.0.0.1:{qbt._TARGET_PORT}")
        assert qbt.get_webui_url() == f"http://localhost:{qbt._TARGET_PORT}/"

    def test_reflects_fallback_port(self, monkeypatch):
        monkeypatch.setattr(qbt, "_base_url", "http://127.0.0.1:18573")
        assert qbt.get_webui_url() == "http://localhost:18573/"

    def test_current_webui_port_fallback_on_garbage(self, monkeypatch):
        monkeypatch.setattr(qbt, "_base_url", "pas une url")
        assert qbt._current_webui_port() == qbt._TARGET_PORT


class TestProbeExistingWebuiSession:
    def test_closed_port_fast_skip(self):
        # Kapalı portta TCP pre-check anında None döndürmeli (3 sn'lik
        # _wait_for_webui döngüsüne girmeden) — 101 adaylı fallback taraması bu
        # yüzden kabul edilebilir sürede kalır.
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
        s.close()
        assert qbt._probe_existing_webui_session(port) is None


class TestPreseedWindowsProfile:
    def test_writes_chosen_port_to_ini(self, tmp_path, monkeypatch):
        monkeypatch.setattr(qbt, "_extract_dir", str(tmp_path))
        qbt._preseed_windows_profile(webui_port=18573)
        ini_path = os.path.join(
            tmp_path, "data", "profile", "qBittorrent", "config", "qBittorrent.ini"
        )
        assert os.path.isfile(ini_path)
        content = open(ini_path, encoding="utf-8").read()
        assert "WebUI\\Port=18573" in content
