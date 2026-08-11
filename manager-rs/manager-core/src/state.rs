//! State machine: enum + match ile compiler-enforced geçişler.
//!
//! TASK-002a — Python 1:1 durum kümesi (codegraph ile çıkarıldı):
//! - ManagerState: `ports/RGSX/watchdog.py:14-21`
//! - BackendState: `ports/RGSX/watchdog.py:22-26`
//! - DownloadState: `ports/RGSX/network/download_state.py:30-42`
//!
//! Varyant adları UPPER_SNAKE olduğundan serde default serialize değeri
//! Python sabitleriyle ("INIT", "RUNNING", ...) birebir aynıdır.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Bilinmeyen durum string'inden parse başarısız oldu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseStateError(pub String);

/// Manager durum makinesi (roadmap Faz 4 / watchdog.py):
/// `INIT → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagerState {
    Init,
    Running,
    Degraded,
    Unresponsive,
    Restarting,
    Crashed,
}

/// qBittorrent backend durumları (watchdog.py:22-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BackendState {
    Stopped,
    Starting,
    PortResolving,
    WebuiAuthWait,
}

/// Anlık indirme durumu (Faz 8 referans kümesi, download_state.py:30-42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Verifying,
    Extracting,
    RetryScheduled,
    FailedTransient,
    FailedPermanent,
    Completed,
    Canceled,
}

/// Durum değişimini tetikleyen olay (download_state.py:45-58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownloadEvent {
    Started,
    Progress,
    PauseRequested,
    ResumeRequested,
    TransientFailure,
    PermanentFailure,
    RetryTriggered,
    RetryExhausted,
    Transitioned,
    Completed,
    CancelRequested,
}

macro_rules! impl_status_str {
    ($enum:ident { $($variant:ident => $repr:literal),+ $(,)? }) => {
        impl fmt::Display for $enum {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let repr = match self {
                    $($enum::$variant => $repr,)+
                };
                f.write_str(repr)
            }
        }

        impl FromStr for $enum {
            type Err = ParseStateError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($repr => Ok($enum::$variant),)+
                    _ => Err(ParseStateError(s.to_string())),
                }
            }
        }
    };
}

impl_status_str!(ManagerState {
    Init => "INIT",
    Running => "RUNNING",
    Degraded => "DEGRADED",
    Unresponsive => "UNRESPONSIVE",
    Restarting => "RESTARTING",
    Crashed => "CRASHED",
});

impl_status_str!(BackendState {
    Stopped => "STOPPED",
    Starting => "STARTING",
    PortResolving => "PORT_RESOLVING",
    WebuiAuthWait => "WEBUI_AUTH_WAIT",
});

impl_status_str!(DownloadState {
    Queued => "QUEUED",
    Downloading => "DOWNLOADING",
    Paused => "PAUSED",
    Verifying => "VERIFYING",
    Extracting => "EXTRACTING",
    RetryScheduled => "RETRY_SCHEDULED",
    FailedTransient => "FAILED_TRANSIENT",
    FailedPermanent => "FAILED_PERMANENT",
    Completed => "COMPLETED",
    Canceled => "CANCELED",
});

impl_status_str!(DownloadEvent {
    Started => "STARTED",
    Progress => "PROGRESS",
    PauseRequested => "PAUSE_REQUESTED",
    ResumeRequested => "RESUME_REQUESTED",
    TransientFailure => "TRANSIENT_FAILURE",
    PermanentFailure => "PERMANENT_FAILURE",
    RetryTriggered => "RETRY_TRIGGERED",
    RetryExhausted => "RETRY_EXHAUSTED",
    Transitioned => "TRANSITIONED",
    Completed => "COMPLETED",
    CancelRequested => "CANCEL_REQUESTED",
});

/// Tüm varyantlar — exhaustive geçiş testleri / UI enum'ları için.
pub const ALL_MANAGER_STATES: [ManagerState; 6] = [
    ManagerState::Init,
    ManagerState::Running,
    ManagerState::Degraded,
    ManagerState::Unresponsive,
    ManagerState::Restarting,
    ManagerState::Crashed,
];

/// Tüm varyantlar — exhaustive geçiş testleri / UI enum'ları için.
pub const ALL_BACKEND_STATES: [BackendState; 4] = [
    BackendState::Stopped,
    BackendState::Starting,
    BackendState::PortResolving,
    BackendState::WebuiAuthWait,
];

/// Tüm varyantlar — exhaustive geçiş testleri / UI enum'ları için.
pub const ALL_DOWNLOAD_STATES: [DownloadState; 10] = [
    DownloadState::Queued,
    DownloadState::Downloading,
    DownloadState::Paused,
    DownloadState::Verifying,
    DownloadState::Extracting,
    DownloadState::RetryScheduled,
    DownloadState::FailedTransient,
    DownloadState::FailedPermanent,
    DownloadState::Completed,
    DownloadState::Canceled,
];

/// Tüm varyantlar — exhaustive geçiş testleri / UI enum'ları için.
pub const ALL_DOWNLOAD_EVENTS: [DownloadEvent; 11] = [
    DownloadEvent::Started,
    DownloadEvent::Progress,
    DownloadEvent::PauseRequested,
    DownloadEvent::ResumeRequested,
    DownloadEvent::TransientFailure,
    DownloadEvent::PermanentFailure,
    DownloadEvent::RetryTriggered,
    DownloadEvent::RetryExhausted,
    DownloadEvent::Transitioned,
    DownloadEvent::Completed,
    DownloadEvent::CancelRequested,
];

/// Eski history status string'inden enum state'ine; bilinmeyen -> DOWNLOADING
/// (download_state.py:122-128 ile birebir).
pub fn state_from_legacy(status: &str) -> DownloadState {
    if status.is_empty() {
        return DownloadState::Downloading;
    }
    if status.starts_with("Try ") {
        return DownloadState::Downloading;
    }
    legacy_status_to_state()
        .iter()
        .find(|(k, _)| *k == status)
        .map(|(_, v)| *v)
        .unwrap_or(DownloadState::Downloading)
}

/// Enum state'inden TVUI/WebUI'nin anladığı legacy status string'ine
/// (download_state.py:131-144 ile birebir).
pub fn legacy_history_status(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "Queued",
        DownloadState::Downloading => "Téléchargement",
        DownloadState::Paused => "Paused",
        DownloadState::Verifying => "Downloading",
        DownloadState::Extracting => "Extracting",
        DownloadState::RetryScheduled => "Téléchargement",
        DownloadState::FailedTransient => "Téléchargement",
        DownloadState::FailedPermanent => "Erreur",
        DownloadState::Completed => "Download_OK",
        DownloadState::Canceled => "Canceled",
    }
}

/// `_LEGACY_STATUS_TO_STATE` (download_state.py:101-119).
pub fn legacy_status_to_state() -> &'static [(&'static str, DownloadState)] {
    &[
        ("Queued", DownloadState::Queued),
        ("Downloading", DownloadState::Downloading),
        ("downloading", DownloadState::Downloading),
        ("Téléchargement", DownloadState::Downloading),
        ("Connecting", DownloadState::Downloading),
        ("Paused", DownloadState::Paused),
        ("Extracting", DownloadState::Extracting),
        ("Converting", DownloadState::Extracting),
        ("Download_OK", DownloadState::Completed),
        ("Completed", DownloadState::Completed),
        ("Seeding", DownloadState::Completed),
        ("Canceled", DownloadState::Canceled),
        ("Cancelled", DownloadState::Canceled),
        ("Annulé", DownloadState::Canceled),
        ("Annule", DownloadState::Canceled),
        ("Erreur", DownloadState::FailedPermanent),
        ("Error", DownloadState::FailedPermanent),
    ]
}

/// İzin verilmeyen (state, event) kombinasyonu (download_state.py:61-63).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransitionError {
    pub state: DownloadState,
    pub event: DownloadEvent,
}

impl fmt::Display for IllegalTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Illegal transition: {} + {}", self.state, self.event)
    }
}

impl std::error::Error for IllegalTransitionError {}

/// Saf durum geçişi — `_TRANSITIONS` (download_state.py:66-94) ile birebir.
///
/// Geçerli (state, event) kombinasyonunda yeni state, geçersizde `Err` döner.
/// Yan etkiler (persist/emit) yüksek katmanın işidir (TASK-002b).
pub fn transition(
    state: DownloadState,
    event: DownloadEvent,
) -> Result<DownloadState, IllegalTransitionError> {
    use DownloadEvent::{
        CancelRequested, Completed as CompletedEv, PauseRequested, PermanentFailure,
        ResumeRequested, RetryExhausted, RetryTriggered, Started, TransientFailure, Transitioned,
    };
    use DownloadState::{
        Canceled, Completed as CompletedSt, Downloading, Extracting, FailedPermanent,
        FailedTransient, Paused, Queued, RetryScheduled, Verifying,
    };
    match (state, event) {
        (Queued, Started) => Ok(Downloading),

        (Downloading, PauseRequested) => Ok(Paused),
        (Paused, ResumeRequested) => Ok(Downloading),
        (Paused, CancelRequested) => Ok(Canceled),

        (Downloading, Transitioned) => Ok(Verifying),
        (Verifying, Transitioned) => Ok(Extracting),
        (Verifying, CompletedEv) => Ok(CompletedSt),
        (Extracting, CompletedEv) => Ok(CompletedSt),
        (Downloading, CompletedEv) => Ok(CompletedSt),

        (Downloading, TransientFailure) => Ok(FailedTransient),
        (FailedTransient, RetryTriggered) => Ok(RetryScheduled),
        (RetryScheduled, Started) => Ok(Downloading),
        (FailedTransient, PermanentFailure) => Ok(FailedPermanent),
        (FailedTransient, RetryExhausted) => Ok(FailedPermanent),
        (RetryScheduled, PermanentFailure) => Ok(FailedPermanent),
        (RetryScheduled, CancelRequested) => Ok(Canceled),
        (Downloading, PermanentFailure) => Ok(FailedPermanent),

        (Downloading, CancelRequested) => Ok(Canceled),
        (Verifying, CancelRequested) => Ok(Canceled),
        (Extracting, CancelRequested) => Ok(Canceled),
        (FailedTransient, CancelRequested) => Ok(Canceled),

        _ => Err(IllegalTransitionError { state, event }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_python_constants() {
        assert_eq!(ManagerState::Init.to_string(), "INIT");
        assert_eq!(ManagerState::Running.to_string(), "RUNNING");
        assert_eq!(ManagerState::Degraded.to_string(), "DEGRADED");
        assert_eq!(ManagerState::Unresponsive.to_string(), "UNRESPONSIVE");
        assert_eq!(ManagerState::Restarting.to_string(), "RESTARTING");
        assert_eq!(ManagerState::Crashed.to_string(), "CRASHED");

        assert_eq!(BackendState::Stopped.to_string(), "STOPPED");
        assert_eq!(BackendState::Starting.to_string(), "STARTING");
        assert_eq!(BackendState::PortResolving.to_string(), "PORT_RESOLVING");
        assert_eq!(BackendState::WebuiAuthWait.to_string(), "WEBUI_AUTH_WAIT");
    }

    #[test]
    fn display_matches_download_constants() {
        assert_eq!(DownloadState::Queued.to_string(), "QUEUED");
        assert_eq!(DownloadState::Downloading.to_string(), "DOWNLOADING");
        assert_eq!(DownloadState::Paused.to_string(), "PAUSED");
        assert_eq!(DownloadState::Verifying.to_string(), "VERIFYING");
        assert_eq!(DownloadState::Extracting.to_string(), "EXTRACTING");
        assert_eq!(DownloadState::RetryScheduled.to_string(), "RETRY_SCHEDULED");
        assert_eq!(DownloadState::FailedTransient.to_string(), "FAILED_TRANSIENT");
        assert_eq!(DownloadState::FailedPermanent.to_string(), "FAILED_PERMANENT");
        assert_eq!(DownloadState::Completed.to_string(), "COMPLETED");
        assert_eq!(DownloadState::Canceled.to_string(), "CANCELED");
    }

    #[test]
    fn from_str_roundtrip() {
        let manager = ["INIT", "RUNNING", "DEGRADED", "UNRESPONSIVE", "RESTARTING", "CRASHED"];
        for repr in manager {
            assert_eq!(ManagerState::from_str(repr).unwrap().to_string(), repr);
        }
        let backend = ["STOPPED", "STARTING", "PORT_RESOLVING", "WEBUI_AUTH_WAIT"];
        for repr in backend {
            assert_eq!(BackendState::from_str(repr).unwrap().to_string(), repr);
        }
        let download = [
            "QUEUED",
            "DOWNLOADING",
            "PAUSED",
            "VERIFYING",
            "EXTRACTING",
            "RETRY_SCHEDULED",
            "FAILED_TRANSIENT",
            "FAILED_PERMANENT",
            "COMPLETED",
            "CANCELED",
        ];
        for repr in download {
            assert_eq!(DownloadState::from_str(repr).unwrap().to_string(), repr);
        }
    }

    #[test]
    fn from_str_unknown_rejects() {
        assert!(ManagerState::from_str("UNKNOWN").is_err());
        assert!(DownloadState::from_str("").is_err());
    }

    #[test]
    fn legacy_map_matches_python() {
        assert_eq!(state_from_legacy("Queued"), DownloadState::Queued);
        assert_eq!(state_from_legacy("Downloading"), DownloadState::Downloading);
        assert_eq!(state_from_legacy("Téléchargement"), DownloadState::Downloading);
        assert_eq!(state_from_legacy("Connecting"), DownloadState::Downloading);
        assert_eq!(state_from_legacy("Paused"), DownloadState::Paused);
        assert_eq!(state_from_legacy("Converting"), DownloadState::Extracting);
        assert_eq!(state_from_legacy("Download_OK"), DownloadState::Completed);
        assert_eq!(state_from_legacy("Seeding"), DownloadState::Completed);
        assert_eq!(state_from_legacy("Annulé"), DownloadState::Canceled);
        assert_eq!(state_from_legacy("Erreur"), DownloadState::FailedPermanent);
        assert_eq!(state_from_legacy("Error"), DownloadState::FailedPermanent);
    }

    #[test]
    fn legacy_unknown_and_empty_default_to_downloading() {
        assert_eq!(state_from_legacy(""), DownloadState::Downloading);
        assert_eq!(state_from_legacy("Try again later"), DownloadState::Downloading);
        assert_eq!(state_from_legacy("SomeUnknownStatus"), DownloadState::Downloading);
        assert_eq!(state_from_legacy("Try Slow.serv"), DownloadState::Downloading);
    }

    #[test]
    fn legacy_history_status_matches_python() {
        assert_eq!(legacy_history_status(DownloadState::Queued), "Queued");
        assert_eq!(legacy_history_status(DownloadState::Downloading), "Téléchargement");
        assert_eq!(legacy_history_status(DownloadState::Verifying), "Downloading");
        assert_eq!(legacy_history_status(DownloadState::RetryScheduled), "Téléchargement");
        assert_eq!(legacy_history_status(DownloadState::FailedTransient), "Téléchargement");
        assert_eq!(legacy_history_status(DownloadState::FailedPermanent), "Erreur");
        assert_eq!(legacy_history_status(DownloadState::Completed), "Download_OK");
        assert_eq!(legacy_history_status(DownloadState::Canceled), "Canceled");
    }

    #[test]
    fn transition_valid_table_matches_python() {
        let valid: &[((DownloadState, DownloadEvent), DownloadState)] = &[
            ((DownloadState::Queued, DownloadEvent::Started), DownloadState::Downloading),
            ((DownloadState::Downloading, DownloadEvent::PauseRequested), DownloadState::Paused),
            ((DownloadState::Paused, DownloadEvent::ResumeRequested), DownloadState::Downloading),
            ((DownloadState::Paused, DownloadEvent::CancelRequested), DownloadState::Canceled),
            ((DownloadState::Downloading, DownloadEvent::Transitioned), DownloadState::Verifying),
            ((DownloadState::Verifying, DownloadEvent::Transitioned), DownloadState::Extracting),
            ((DownloadState::Verifying, DownloadEvent::Completed), DownloadState::Completed),
            ((DownloadState::Extracting, DownloadEvent::Completed), DownloadState::Completed),
            ((DownloadState::Downloading, DownloadEvent::Completed), DownloadState::Completed),
            ((DownloadState::Downloading, DownloadEvent::TransientFailure), DownloadState::FailedTransient),
            ((DownloadState::FailedTransient, DownloadEvent::RetryTriggered), DownloadState::RetryScheduled),
            ((DownloadState::RetryScheduled, DownloadEvent::Started), DownloadState::Downloading),
            ((DownloadState::FailedTransient, DownloadEvent::PermanentFailure), DownloadState::FailedPermanent),
            ((DownloadState::FailedTransient, DownloadEvent::RetryExhausted), DownloadState::FailedPermanent),
            ((DownloadState::RetryScheduled, DownloadEvent::PermanentFailure), DownloadState::FailedPermanent),
            ((DownloadState::RetryScheduled, DownloadEvent::CancelRequested), DownloadState::Canceled),
            ((DownloadState::Downloading, DownloadEvent::PermanentFailure), DownloadState::FailedPermanent),
            ((DownloadState::Downloading, DownloadEvent::CancelRequested), DownloadState::Canceled),
            ((DownloadState::Verifying, DownloadEvent::CancelRequested), DownloadState::Canceled),
            ((DownloadState::Extracting, DownloadEvent::CancelRequested), DownloadState::Canceled),
            ((DownloadState::FailedTransient, DownloadEvent::CancelRequested), DownloadState::Canceled),
        ];
        assert_eq!(valid.len(), 21, "Python _TRANSITIONS 21 satır");
        for ((s, e), expected) in valid {
            assert_eq!(
                transition(*s, *e),
                Ok(*expected),
                "{s} + {e} bekleniyordu: {expected}"
            );
        }
    }

    #[test]
    fn transition_covers_exactly_python_table() {
        let mut allowed = 0usize;
        for s in ALL_DOWNLOAD_STATES {
            for e in ALL_DOWNLOAD_EVENTS {
                match transition(s, e) {
                    Ok(_) => allowed += 1,
                    Err(err) => {
                        assert_eq!(err.state, s);
                        assert_eq!(err.event, e);
                    }
                }
            }
        }
        assert_eq!(allowed, 21, "yalnızca Python _TRANSITIONS'deki 21 kombinasyon geçerli");
    }

    #[test]
    fn transition_illegal_examples() {
        for (s, e) in [
            (DownloadState::Queued, DownloadEvent::PauseRequested),
            (DownloadState::Paused, DownloadEvent::Started),
            (DownloadState::Completed, DownloadEvent::Completed),
            (DownloadState::Canceled, DownloadEvent::ResumeRequested),
            (DownloadState::FailedPermanent, DownloadEvent::RetryTriggered),
        ] {
            assert!(transition(s, e).is_err(), "{s} + {e} yasak olmali");
        }
    }

    #[test]
    fn transition_error_display() {
        let err = transition(DownloadState::Queued, DownloadEvent::PauseRequested).unwrap_err();
        assert_eq!(err.to_string(), "Illegal transition: QUEUED + PAUSE_REQUESTED");
        assert!(err.to_string().contains("QUEUED"));
    }
}