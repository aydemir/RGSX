//! Watchdog / supervisor saf karar mantığı — `ports/RGSX/watchdog.py` ile 1:1.
//!
//! HTTP/process/thread yok; yalnızca durum kararı. Böylece in-process manager
//! watchdog'u ve dış supervisor (TV UI / Task Scheduler) aynı hysteresis +
//! restart sınırlama mantığını paylaşır. Zaman bağımlı fonksiyonlar `now`
//! parametresi alır — testler deterministik yürür.

use crate::state::ManagerState;

/// Ardışık sağlık sonuçlarına göre RUNNING/DEGRADED/UNRESPONSIVE geçişi
/// (watchdog.py:29-64).
///
/// - ardışık başarısızlık >= `degrade_threshold`      → DEGRADED
/// - ardışık başarısızlık >= `unresponsive_threshold` → UNRESPONSIVE
/// - herhangi bir başarı sayaçları sıfırlar ve durumu RUNNING'e döndürür
///   (hysteresis: seyrek hatalar kalıcı state değişikliği üretmez).
pub struct HysteresisMonitor {
    degrade_threshold: u32,
    unresponsive_threshold: u32,
    consecutive_failures: u32,
    state: ManagerState,
}

impl HysteresisMonitor {
    /// Bozuk yapılandırma (unresponsive < degrade veya degrade < 1) Python'daki
    /// `ValueError` ile aynı şekilde `panic!` yapar.
    pub fn new(degrade_threshold: u32, unresponsive_threshold: u32) -> Self {
        assert!(
            unresponsive_threshold >= degrade_threshold,
            "unresponsive_threshold must be >= degrade_threshold"
        );
        assert!(degrade_threshold >= 1, "degrade_threshold must be >= 1");
        Self {
            degrade_threshold,
            unresponsive_threshold,
            consecutive_failures: 0,
            state: ManagerState::Running,
        }
    }

    /// Yeni bir sağlık sonucu işler, güncel state'i döndürür.
    pub fn report(&mut self, healthy: bool) -> ManagerState {
        if healthy {
            self.consecutive_failures = 0;
            self.state = ManagerState::Running;
        } else {
            self.consecutive_failures += 1;
            if self.consecutive_failures >= self.unresponsive_threshold {
                self.state = ManagerState::Unresponsive;
            } else if self.consecutive_failures >= self.degrade_threshold {
                self.state = ManagerState::Degraded;
            }
        }
        self.state
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.state = ManagerState::Running;
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}

/// Aşırı restart döngüsünü (crash-loop) sınırlar (watchdog.py:66-97).
///
/// Kayan pencerede en fazla `max_restarts` restart'a izin verir; limit dolunca
/// `can_restart()` false döner ve çağıran CRASHED'e geçip durmalı.
pub struct RestartLimiter {
    max_restarts: u32,
    window_seconds: f64,
    timestamps: Vec<f64>,
}

impl RestartLimiter {
    /// `max_restarts` Python'daki `max(1, ...)` ile aynı şekilde en az 1'e kıstırılır.
    pub fn new(max_restarts: u32, window_seconds: f64) -> Self {
        Self {
            max_restarts: max_restarts.max(1),
            window_seconds,
            timestamps: Vec::new(),
        }
    }

    /// Pencere dışındaki zaman damgalarını atar (cutoff = now - window_seconds).
    fn prune(&mut self, now: f64) {
        let cutoff = now - self.window_seconds;
        self.timestamps.retain(|t| *t > cutoff);
    }

    pub fn can_restart(&mut self, now: f64) -> bool {
        self.prune(now);
        (self.timestamps.len() as u32) < self.max_restarts
    }

    /// Bir restart kaydeder; limit doluysa false (içeride artış olmaz).
    pub fn record_restart(&mut self, now: f64) -> bool {
        if !self.can_restart(now) {
            return false;
        }
        self.timestamps.push(now);
        true
    }

    pub fn restart_count(&self) -> usize {
        self.timestamps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_initial_state_is_running() {
        let m = HysteresisMonitor::new(3, 6);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn hysteresis_sparse_failures_stay_running() {
        let mut m = HysteresisMonitor::new(3, 6);
        assert_eq!(m.report(true), ManagerState::Running);
        assert_eq!(m.report(false), ManagerState::Running);
        assert_eq!(m.report(true), ManagerState::Running);
        assert_eq!(m.report(false), ManagerState::Running);
        assert_eq!(m.consecutive_failures(), 1);
    }

    #[test]
    fn hysteresis_degrade_at_threshold() {
        let mut m = HysteresisMonitor::new(3, 6);
        m.report(false);
        m.report(false);
        assert_eq!(m.report(false), ManagerState::Degraded);
        assert_eq!(m.consecutive_failures(), 3);
    }

    #[test]
    fn hysteresis_unresponsive_at_threshold() {
        let mut m = HysteresisMonitor::new(3, 6);
        for _ in 0..5 {
            m.report(false);
        }
        assert_eq!(m.report(false), ManagerState::Unresponsive);
        assert_eq!(m.consecutive_failures(), 6);
    }

    #[test]
    fn hysteresis_remains_unresponsive_past_threshold() {
        let mut m = HysteresisMonitor::new(3, 6);
        for _ in 0..6 {
            m.report(false);
        }
        assert_eq!(m.report(false), ManagerState::Unresponsive);
        assert_eq!(m.consecutive_failures(), 7);
    }

    #[test]
    fn hysteresis_healthy_resets_to_running() {
        let mut m = HysteresisMonitor::new(3, 6);
        for _ in 0..5 {
            m.report(false);
        }
        assert_eq!(m.report(true), ManagerState::Running);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn hysteresis_reset_clears_all() {
        let mut m = HysteresisMonitor::new(3, 6);
        for _ in 0..7 {
            m.report(false);
        }
        m.reset();
        assert_eq!(m.consecutive_failures(), 0);
        assert_eq!(m.report(false), ManagerState::Running);
    }

    #[test]
    #[should_panic(expected = "unresponsive_threshold must be >= degrade_threshold")]
    fn hysteresis_invalid_ordering_panics() {
        let _ = HysteresisMonitor::new(4, 2);
    }

    #[test]
    #[should_panic(expected = "degrade_threshold must be >= 1")]
    fn hysteresis_zero_degrade_panics() {
        let _ = HysteresisMonitor::new(0, 5);
    }

    #[test]
    fn restart_limiter_defaults_full_at_3() {
        let mut l = RestartLimiter::new(3, 3600.0);
        let t0 = 1000.0;
        assert_eq!(l.record_restart(t0), true);
        assert_eq!(l.record_restart(t0 + 1.0), true);
        assert_eq!(l.record_restart(t0 + 2.0), true);
        assert_eq!(
            l.record_restart(t0 + 3.0),
            false,
            "limit dolu — 4. restart yasak"
        );
        assert_eq!(l.restart_count(), 3);
    }

    #[test]
    fn restart_limiter_sliding_window_prunes() {
        let mut l = RestartLimiter::new(3, 3600.0);
        let t0 = 5000.0;
        l.record_restart(t0); // [5000]
        l.record_restart(t0 + 60.0); // [5000, 5060]
        l.record_restart(t0 + 120.0); // [5000, 5060, 5120]
        assert_eq!(
            l.record_restart(t0 + 180.0),
            false,
            "pencere doluyken limit"
        );
        // Pencere taştı: ilk restart düşer, tekrar izin açılır
        assert_eq!(l.can_restart(t0 + 3600.0 + 1.0), true); // cutoff 5001 → [5060, 5120]
        assert_eq!(l.restart_count(), 2);
        // Yeni record da kendi now'uyla prune eder: cutoff 5100 → [5120] + 8700
        assert_eq!(l.record_restart(t0 + 3700.0), true);
        assert_eq!(l.restart_count(), 2);
        // Sonraki record: cutoff 5200 → [8700] + 8800
        assert_eq!(l.record_restart(t0 + 3800.0), true);
        assert_eq!(l.restart_count(), 2);
        assert_eq!(l.can_restart(t0 + 9000.0), true);
    }

    #[test]
    fn restart_limiter_exact_window_boundary_evicts() {
        let mut l = RestartLimiter::new(1, 10.0);
        assert_eq!(l.record_restart(0.0), true);
        // cutoff = now - 10; t=0, now=10 → t > 0 eşitsizliği sağlanmaz → atılır.
        assert_eq!(l.can_restart(10.0), true);
        assert_eq!(l.restart_count(), 0);
    }

    #[test]
    fn restart_limiter_clamps_max_to_1() {
        let mut l = RestartLimiter::new(0, 3600.0);
        assert_eq!(l.record_restart(1.0), true);
        assert_eq!(l.record_restart(2.0), false);
    }
}
