"""Faz 4 — Watchdog / supervisor karar mantığı.

Saf ve bağımlılıksız modül: in-process manager watchdog'u (rgsx_manager) ve dış
supervisor (__main__ / TV UI) aynı hysteresis + restart sınırlama mantığını
kullanır. HTTP/process/thread yok — yalnızca durum kararı; böylece sandbox'ta
saf birim test edilir.

Manager durum makinesi (roadmap Faz 4):
    INIT → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED
"""

import time

# Manager durumları
STATE_INIT = "INIT"
STATE_RUNNING = "RUNNING"
STATE_DEGRADED = "DEGRADED"
STATE_UNRESPONSIVE = "UNRESPONSIVE"
STATE_RESTARTING = "RESTARTING"
STATE_CRASHED = "CRASHED"

# qBittorrent backend durumları
STATE_STOPPED = "STOPPED"
STATE_STARTING = "STARTING"
STATE_PORT_RESOLVING = "PORT_RESOLVING"
STATE_WEBUI_AUTH_WAIT = "WEBUI_AUTH_WAIT"


class HysteresisMonitor:
    """Ardışık sağlık sonuçlarına göre RUNNING/DEGRADED/UNRESPONSIVE geçişi.

    - ardışık başarısızlık >= degrade_threshold      → DEGRADED
    - ardışık başarısızlık >= unresponsive_threshold → UNRESPONSIVE
    - herhangi bir başarı sayaçları sıfırlar ve durumu RUNNING'e döndürür
      (hysteresis: seyrek hatalar kalıcı state değişikliği üretmez).
    """

    def __init__(self, degrade_threshold: int = 3, unresponsive_threshold: int = 6):
        if unresponsive_threshold < degrade_threshold:
            raise ValueError("unresponsive_threshold must be >= degrade_threshold")
        if degrade_threshold < 1:
            raise ValueError("degrade_threshold must be >= 1")
        self.degrade_threshold = int(degrade_threshold)
        self.unresponsive_threshold = int(unresponsive_threshold)
        self.consecutive_failures = 0
        self.state = STATE_RUNNING

    def report(self, healthy: bool) -> str:
        """Yeni bir sağlık sonucu işler, güncel state'i döndürür."""
        if healthy:
            self.consecutive_failures = 0
            self.state = STATE_RUNNING
        else:
            self.consecutive_failures += 1
            if self.consecutive_failures >= self.unresponsive_threshold:
                self.state = STATE_UNRESPONSIVE
            elif self.consecutive_failures >= self.degrade_threshold:
                self.state = STATE_DEGRADED
        return self.state

    def reset(self) -> None:
        self.consecutive_failures = 0
        self.state = STATE_RUNNING


class RestartLimiter:
    """Aşırı restart döngüsünü (crash-loop) sınırlar.

    Kayan pencerede en fazla `max_restarts` restart'a izin verir; limit dolunca
    `can_restart()` False döner ve çağıran CRASHED'e geçip durmalı.
    """

    def __init__(self, max_restarts: int = 3, window_seconds: float = 3600):
        self.max_restarts = max(1, int(max_restarts))
        self.window_seconds = float(window_seconds)
        self._timestamps: list[float] = []

    def _prune(self, now: float) -> None:
        cutoff = now - self.window_seconds
        self._timestamps = [t for t in self._timestamps if t > cutoff]

    def can_restart(self, now: float | None = None) -> bool:
        now = time.time() if now is None else now
        self._prune(now)
        return len(self._timestamps) < self.max_restarts

    def record_restart(self, now: float | None = None) -> bool:
        """Bir restart kaydeder; limit doluysa False (içeride artış yok)."""
        now = time.time() if now is None else now
        if not self.can_restart(now):
            return False
        self._timestamps.append(now)
        return True

    @property
    def restart_count(self) -> int:
        return len(self._timestamps)
