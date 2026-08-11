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
}