"""Faz 4 — Watchdog / supervisor karar mantığı testleri.

Saf modül testi: HysteresisMonitor (DEGRADED/UNRESPONSIVE geçişleri, recovery)
ve RestartLimiter (crash-loop sınırı). HTTP isteği / süreç başlatılmaz.
"""

import pytest

from watchdog import (
    STATE_DEGRADED,
    STATE_RUNNING,
    STATE_UNRESPONSIVE,
    HysteresisMonitor,
    RestartLimiter,
)


class TestHysteresisMonitor:
    def test_initial_state_running(self):
        assert HysteresisMonitor().state == STATE_RUNNING

    def test_single_failure_stays_running(self):
        m = HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)
        assert m.report(False) == STATE_RUNNING

    def test_degraded_after_threshold(self):
        m = HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)
        m.report(False)
        m.report(False)
        assert m.report(False) == STATE_DEGRADED

    def test_unresponsive_after_hard_threshold(self):
        m = HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)
        for _ in range(5):
            m.report(False)
        assert m.report(False) == STATE_UNRESPONSIVE

    def test_recovery_resets_counter(self):
        m = HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)
        for _ in range(3):
            m.report(False)
        assert m.state == STATE_DEGRADED
        assert m.report(True) == STATE_RUNNING
        assert m.consecutive_failures == 0
        assert m.report(False) == STATE_RUNNING

    def test_recovery_from_unresponsive(self):
        m = HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)
        for _ in range(6):
            m.report(False)
        assert m.state == STATE_UNRESPONSIVE
        assert m.report(True) == STATE_RUNNING

    def test_validates_thresholds(self):
        with pytest.raises(ValueError):
            HysteresisMonitor(degrade_threshold=5, unresponsive_threshold=3)
        with pytest.raises(ValueError):
            HysteresisMonitor(degrade_threshold=0)

    def test_reset(self):
        m = HysteresisMonitor(degrade_threshold=2, unresponsive_threshold=3)
        m.report(False)
        m.report(False)
        assert m.state == STATE_DEGRADED
        m.reset()
        assert m.consecutive_failures == 0
        assert m.state == STATE_RUNNING


class TestRestartLimiter:
    def test_allows_up_to_max(self):
        lim = RestartLimiter(max_restarts=3, window_seconds=3600)
        assert lim.can_restart()
        assert lim.record_restart()
        assert lim.record_restart()
        assert lim.record_restart()
        assert lim.restart_count == 3
        assert not lim.can_restart()
        assert not lim.record_restart()

    def test_window_expiry_allows_restart(self):
        lim = RestartLimiter(max_restarts=2, window_seconds=100)
        t0 = 1000.0
        assert lim.record_restart(t0)
        assert lim.record_restart(t0 + 1)
        assert not lim.can_restart(t0 + 50)
        assert lim.can_restart(t0 + 200)

    def test_old_timestamps_pruned(self):
        lim = RestartLimiter(max_restarts=2, window_seconds=100)
        t0 = 1000.0
        lim.record_restart(t0)
        lim.record_restart(t0 + 1)
        assert not lim.can_restart(t0 + 50)
        assert lim.can_restart(t0 + 150)
        assert lim.record_restart(t0 + 150)
        assert lim.restart_count == 1  # t0 ve t0+1 pencereden düştü

    def test_max_restarts_at_least_one(self):
        lim = RestartLimiter(max_restarts=0)
        assert lim.max_restarts >= 1
        assert lim.record_restart()
