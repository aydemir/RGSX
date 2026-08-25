//! manager-core: platform-bağımsız state machine ve contract tipleri.
//!
//! TASK-002a: state.rs (enum + transition), watchdog.rs (hysteresis/restart).

pub mod contract;
pub mod disk;
pub mod extract;
pub mod game_filters;
pub mod retry;
pub mod secrets;
pub mod settings;
pub mod state;
pub mod watchdog;
