//! manager-core: platform-bağımsız state machine ve contract tipleri.
//!
//! TASK-002a: state.rs (enum + transition), watchdog.rs (hysteresis/restart).

pub mod contract;
pub mod retry;
pub mod state;
pub mod watchdog;
pub mod settings;