# -*- coding: utf-8 -*-
"""Faz 8 — Download item state machine & job model.

Somut durumlar (`DownloadState`), tetikleyiciler (`DownloadEvent`) ve yan etkili
geçişler (`transition()`) resmî bir modeldir; serbest sözlük kullanımının yerine
`DownloadJob` @dataclass'ı geçer.

history.json'a yazım geriye dönük uyumludur: mevcut alan adları
(status/progress/message/timestamp/...) korunur, sadece ek alanlar
(entity_state/retry_count/error/retry_at) eklenir. Eski format veriyi okumak
için özel bir şey gerekmez — `DownloadJob.from_history_entry` eksik alanları
varsayılanlarla doldurur.

Modül bağımsızdır (saf util): config/network eyleme bağımlılığı yoktur.
Retry/SSE yan etkileri `queue.py` ve `rgsx_manager.py` tarafından bağlanır.
"""
from __future__ import annotations

import logging
import re
import threading
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Callable, Optional

logger = logging.getLogger("network")


class DownloadState(str, Enum):
    """Anlık indirme durumu (SNAKE_CASE). Faz 8 referans kümesi."""

    QUEUED = "QUEUED"
    DOWNLOADING = "DOWNLOADING"
    PAUSED = "PAUSED"
    VERIFYING = "VERIFYING"
    EXTRACTING = "EXTRACTING"
    RETRY_SCHEDULED = "RETRY_SCHEDULED"
    FAILED_TRANSIENT = "FAILED_TRANSIENT"
    FAILED_PERMANENT = "FAILED_PERMANENT"
    COMPLETED = "COMPLETED"
    CANCELED = "CANCELED"


class DownloadEvent(str, Enum):
    """Durum değişimini tetikleyen olay."""

    STARTED = "STARTED"
    PROGRESS = "PROGRESS"
    PAUSE_REQUESTED = "PAUSE_REQUESTED"
    RESUME_REQUESTED = "RESUME_REQUESTED"
    TRANSIENT_FAILURE = "TRANSIENT_FAILURE"
    PERMANENT_FAILURE = "PERMANENT_FAILURE"
    RETRY_TRIGGERED = "RETRY_TRIGGERED"
    RETRY_EXHAUSTED = "RETRY_EXHAUSTED"
    TRANSITIONED = "TRANSITIONED"
    COMPLETED = "COMPLETED"
    CANCEL_REQUESTED = "CANCEL_REQUESTED"


class IllegalTransitionError(Exception):
    """İzin verilmeyen (state, event) kombinasyonu."""


# --- geçiş tablosu: (state, event) -> state --------------------------------
_TRANSITIONS: dict[tuple[DownloadState, DownloadEvent], DownloadState] = {
    (DownloadState.QUEUED, DownloadEvent.STARTED): DownloadState.DOWNLOADING,

    (DownloadState.DOWNLOADING, DownloadEvent.PAUSE_REQUESTED): DownloadState.PAUSED,
    (DownloadState.PAUSED, DownloadEvent.RESUME_REQUESTED): DownloadState.DOWNLOADING,
    (DownloadState.PAUSED, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,

    (DownloadState.DOWNLOADING, DownloadEvent.TRANSITIONED): DownloadState.VERIFYING,
    (DownloadState.VERIFYING, DownloadEvent.TRANSITIONED): DownloadState.EXTRACTING,
    (DownloadState.VERIFYING, DownloadEvent.COMPLETED): DownloadState.COMPLETED,
    (DownloadState.EXTRACTING, DownloadEvent.COMPLETED): DownloadState.COMPLETED,
    (DownloadState.DOWNLOADING, DownloadEvent.COMPLETED): DownloadState.COMPLETED,

    # Hata akışı
    (DownloadState.DOWNLOADING, DownloadEvent.TRANSIENT_FAILURE): DownloadState.FAILED_TRANSIENT,
    (DownloadState.FAILED_TRANSIENT, DownloadEvent.RETRY_TRIGGERED): DownloadState.RETRY_SCHEDULED,
    (DownloadState.RETRY_SCHEDULED, DownloadEvent.STARTED): DownloadState.DOWNLOADING,
    (DownloadState.FAILED_TRANSIENT, DownloadEvent.PERMANENT_FAILURE): DownloadState.FAILED_PERMANENT,
    (DownloadState.FAILED_TRANSIENT, DownloadEvent.RETRY_EXHAUSTED): DownloadState.FAILED_PERMANENT,
    (DownloadState.RETRY_SCHEDULED, DownloadEvent.PERMANENT_FAILURE): DownloadState.FAILED_PERMANENT,
    (DownloadState.RETRY_SCHEDULED, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,
    (DownloadState.DOWNLOADING, DownloadEvent.PERMANENT_FAILURE): DownloadState.FAILED_PERMANENT,

    # İptal her durumdan çıkışa izinli (aşağıdakiler açıkça listelenir)
    (DownloadState.DOWNLOADING, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,
    (DownloadState.VERIFYING, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,
    (DownloadState.EXTRACTING, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,
    (DownloadState.FAILED_TRANSIENT, DownloadEvent.CANCEL_REQUESTED): DownloadState.CANCELED,
}

# Zombi state'lere SET değil de ulaşılamaz tutulur: FAILED_TRANSIENT dışındaki
# başka bir durumdan TRANSIENT_FAILURE geçişi zaten yasak — DOWNLOADING'den
# gelebilirler (VERIFYING/EXTRACTING ayrı tetiklerde).

# --- legacy history status eşlemesi ----------------------------------------
_LEGACY_STATUS_TO_STATE: dict[str, DownloadState] = {
    "Queued": DownloadState.QUEUED,
    "Downloading": DownloadState.DOWNLOADING,
    "downloading": DownloadState.DOWNLOADING,
    "Téléchargement": DownloadState.DOWNLOADING,
    "Connecting": DownloadState.DOWNLOADING,
    "Paused": DownloadState.PAUSED,
    "Extracting": DownloadState.EXTRACTING,
    "Converting": DownloadState.EXTRACTING,
    "Download_OK": DownloadState.COMPLETED,
    "Completed": DownloadState.COMPLETED,
    "Seeding": DownloadState.COMPLETED,
    "Canceled": DownloadState.CANCELED,
    "Cancelled": DownloadState.CANCELED,
    "Annulé": DownloadState.CANCELED,
    "Annule": DownloadState.CANCELED,
    "Erreur": DownloadState.FAILED_PERMANENT,
    "Error": DownloadState.FAILED_PERMANENT,
}


def state_from_legacy(status: str) -> DownloadState:
    """Eski history status string'inden enum state'ine; bilinmeyen -> DOWNLOADING."""
    if not status:
        return DownloadState.DOWNLOADING
    if status.startswith("Try "):
        return DownloadState.DOWNLOADING
    return _LEGACY_STATUS_TO_STATE.get(status, DownloadState.DOWNLOADING)


def legacy_history_status(state: DownloadState) -> str:
    """Enum state'inden TVUI/WebUI'nin anladığı legacy status string'ine."""
    return {
        DownloadState.QUEUED: "Queued",
        DownloadState.DOWNLOADING: "Téléchargement",
        DownloadState.PAUSED: "Paused",
        DownloadState.VERIFYING: "Downloading",
        DownloadState.EXTRACTING: "Extracting",
        DownloadState.RETRY_SCHEDULED: "Téléchargement",
        DownloadState.FAILED_TRANSIENT: "Téléchargement",
        DownloadState.FAILED_PERMANENT: "Erreur",
        DownloadState.COMPLETED: "Download_OK",
        DownloadState.CANCELED: "Canceled",
    }[state]


# --- hata sınıflandırıcı ----------------------------------------------------
# Geçici (transient): tekrar denendiğinde başarı şansı yüksek (rate limit,
# geçici ağ kesintisi, 5xx). Kalıcı (permanent): tekrarın faydası yok
# (401/403/404, browser challenge, bozuk payload, disk alanı).
_TRANSIENT_HTTP_STATUS = frozenset({408, 409, 425, 429, 500, 502, 503, 504,
                                    520, 521, 522, 523, 524, 525, 526, 527})
_PERMANENT_HTTP_STATUS = frozenset({400, 401, 402, 403, 404, 405, 406, 410,
                                    411, 412, 413, 414, 415, 416, 417, 418,
                                    422, 423, 424, 426, 428, 431, 451})

_PERMANENT_MARKERS = (
    "access denied", "accès refusé", "access refused",
    "authentication required", "auth required", "unauthorized", "forbidden",
    "browser challenge", "interactive browser session",
    "payload is not a valid archive", "not a valid archive", "valid archive signature",
    "html/challenge content", "downloaded html",
    "empty response",
    "restricted (is_dark", "is_dark=true",
    "file not found", "introuvable", "not found", "has been removed",
    "removed for abuse", "piracy domain",
    "password incorrect", "invalid password", "mot de passe",
    "pas assez d'espace", "insufficient disk space", "low disk space",
    "manque d'espace",
)

_TRANSIENT_MARKERS = (
    "timeout", "timed out", "timed-out", "read timed",
    "connection error", "connexion", "connection aborted", "connection reset",
    "connection refused", "connection timed", "unable to connect", "cannot connect",
    "max retries exceeded", "retries exceeded",
    "rate limit", "too many requests", "temporarily unavailable",
    "server error", "erreur serveur", "service unavailable", "bad gateway",
    "gateway time-out", "limits downloads to one", "limite les téléchargements",
    "link appears down", "temporary failure", "ressayer", "réessayez",
    "essayez plus tard", "slow down", "n'existait pas", "temporairement",
)


def _extract_http_status_codes(text: str) -> set[int]:
    """Serbest 3 haneli HTTP kodlarını toplar (HTTP 429, 429, (500), ...)."""
    codes: set[int] = set()
    for m in re.finditer(r"(?<![\d.])(\d{3})(?![\d.])", text):
        val = int(m.group(1))
        if 400 <= val <= 599:
            codes.add(val)
    return codes


def classify_error(message, error_type=None) -> bool:
    """True ise hata geçici (retry mantıklı), False ise kalıcı.

    - error_type bir exception objesi verilirse tip bilgisinden yararlanır.
    - message yalnızca string ise marker/code tarama yapılır.
    - Belirsiz hatalar varsayılan olarak kalıcı sayılır (sonsuz retry döngüsü
      oluşturmamak için).
    """
    if error_type is not None:
        if _is_insufficient_disk(error_type):
            return False
        # requests.Timeout / ConnectionError -> geçici
        tname = type(error_type).__name__
        if "Timeout" in tname or "Connection" in tname:
            return True

    text = str(message or "").lower()
    if not text:
        return False

    # Kalıcı marker'lar her zaman önceliklidir (bir 5xx süsü bile olsa).
    if any(marker in text for marker in _PERMANENT_MARKERS):
        return False

    codes = _extract_http_status_codes(text)
    if codes:
        if codes & _TRANSIENT_HTTP_STATUS:
            return True
        if codes & _PERMANENT_HTTP_STATUS:
            return False

    if any(marker in text for marker in _TRANSIENT_MARKERS):
        return True

    return False


def _is_insufficient_disk(error_type) -> bool:
    try:
        from network.helpers import InsufficientDiskSpaceError
        return isinstance(error_type, InsufficientDiskSpaceError) or (
            isinstance(error_type, type)
            and issubclass(error_type, InsufficientDiskSpaceError)
        )
    except Exception:
        name = type(error_type).__name__
        return "InsufficientDiskSpace" in name or "insufficientdisk" in str(error_type).lower()


# --- retry backoff ----------------------------------------------------------
DEFAULT_MAX_RETRIES = 3
DEFAULT_BACKOFF_BASE_SEC = 5.0
DEFAULT_BACKOFF_MAX_SEC = 300.0


def retry_backoff_seconds(retry_count: int, base: float = DEFAULT_BACKOFF_BASE_SEC,
                          max_wait: float = DEFAULT_BACKOFF_MAX_SEC) -> float:
    """1. deneme -> base, 2. -> 2*base, 3. -> 4*base ... max_wait ile tavanlanır."""
    if retry_count <= 0:
        return 0.0
    return min(base * (2 ** (retry_count - 1)), max_wait)


# --- job modeli -------------------------------------------------------------
@dataclass
class DownloadJob:
    """Serbest sözlüğün yerine geçen açık indirme modeli.

    history.json'a yazılırken base entry üzerine overlay edilir:
    `apply_to_history_entry(entry)` mevcut alan adlarını korur, ek alanları ekler.
    """
    id: str
    url: str
    destination: str = ""
    state: DownloadState = DownloadState.QUEUED
    progress: float = 0.0
    retry_count: int = 0
    error: str = ""
    task_id: str = ""
    platform: str = ""
    game_name: str = ""
    message: str = ""
    timestamp: str = ""
    is_zip_non_supported: bool = False
    max_retries: int = DEFAULT_MAX_RETRIES
    retry_at: float = 0.0
    metadata: dict = field(default_factory=dict)

    # -- dönüşüm ------------------------------------------------------------
    @classmethod
    def from_history_entry(cls, entry: dict) -> "DownloadJob":
        dest = str(entry.get("local_filename") or entry.get("local_path") or "")
        raw_state = str(entry.get("entity_state") or "")
        try:
            state = DownloadState(raw_state) if raw_state else None
        except ValueError:
            state = None
        if state is None:
            state = state_from_legacy(str(entry.get("status") or ""))
        return cls(
            id=str(entry.get("task_id") or entry.get("url") or ""),
            url=str(entry.get("url") or ""),
            destination=dest,
            state=state,
            progress=float(entry.get("progress", 0) or 0),
            retry_count=int(entry.get("retry_count", 0) or 0),
            error=str(entry.get("error") or ""),
            task_id=str(entry.get("task_id") or ""),
            platform=str(entry.get("platform") or ""),
            game_name=str(entry.get("game_name") or ""),
            message=str(entry.get("message") or ""),
            timestamp=str(entry.get("timestamp") or ""),
            is_zip_non_supported=bool(entry.get("is_zip_non_supported", False)),
            max_retries=int(entry.get("max_retries", DEFAULT_MAX_RETRIES) or DEFAULT_MAX_RETRIES),
            retry_at=float(entry.get("retry_at", 0) or 0),
        )

    def apply_to_history_entry(self, entry: dict) -> dict:
        """Mevcut entry üzerine state modelini overlay eder (geriye dönük uyumlu)."""
        entry["status"] = legacy_history_status(self.state)
        entry["entity_state"] = self.state.value
        entry["retry_count"] = self.retry_count
        entry["max_retries"] = self.max_retries
        if self.error:
            entry["error"] = self.error
        if self.retry_at:
            entry["retry_at"] = round(self.retry_at, 3)
        if self.progress >= 0:
            entry["progress"] = int(round(self.progress))
        if self.message:
            entry["message"] = self.message
        return entry

    def as_dict(self) -> dict:
        base = {
            "id": self.id,
            "url": self.url,
            "destination": self.destination,
            "state": self.state.value,
            "progress": round(self.progress, 2),
            "retry_count": self.retry_count,
            "error": self.error,
            "task_id": self.task_id,
            "platform": self.platform,
            "game_name": self.game_name,
            "message": self.message,
            "timestamp": self.timestamp,
        }
        base.update(self.metadata or {})
        return base


# --- yan etkili geçiş ---------------------------------------------------------
EffectsFn = Callable[[DownloadJob, DownloadState, DownloadState, DownloadEvent], None]


def transition(job: DownloadJob, event: DownloadEvent, effects: Optional[EffectsFn] = None) -> DownloadState:
    """Yan etkili geçiş: job.state güncellenir, geçersiz kombinasyonda hata verir.

    effects(job, old_state, new_state, event) geri çağrısı persist/emit gibi
    yan etkiler için kullanıcıya bırakılır (roadmap: DOWNLOADING+PAUSE_REQUESTED
    -> PAUSED -> downloader.pause() + persist_state() + emit_event()).
    """
    key = (job.state, event)
    nxt = _TRANSITIONS.get(key)
    if nxt is None:
        raise IllegalTransitionError(f"Illegal transition: {job.state.value} + {event.value}")
    old = job.state
    job.state = nxt
    if effects is not None:
        try:
            effects(job, old, nxt, event)
        except Exception as e:
            logger.debug(f"transition effects error ({old.value}+{event.value}): {e}")
    return nxt


def is_active_state(state: DownloadState) -> bool:
    """İşlem hâlâ canlı/ilerliyor mu (kullanıcı akışlarının 'aktif' kabul ettiği küme)."""
    return state in (
        DownloadState.DOWNLOADING,
        DownloadState.PAUSED,
        DownloadState.VERIFYING,
        DownloadState.EXTRACTING,
        DownloadState.RETRY_SCHEDULED,
        DownloadState.FAILED_TRANSIENT,
    )


def retryable(state: DownloadState) -> bool:
    return state in (DownloadState.FAILED_TRANSIENT, DownloadState.RETRY_SCHEDULED)


# --- isteğe bağlı SSE emitter kaydı ------------------------------------------
_STATE_EMITTER: Optional[Callable[[str, dict], None]] = None
_STATE_EMITTER_LOCK = threading.Lock()


def set_state_emitter(fn: Optional[Callable[[str, dict], None]]) -> None:
    """SSE/broadcast emitörünü kaydeder (rgsx_manager._broadcast gibi).

    fn(event_type: str, data: dict) — Faz 8: 'download_state' event tipi.
    """
    global _STATE_EMITTER
    with _STATE_EMITTER_LOCK:
        _STATE_EMITTER = fn


def get_state_emitter():
    with _STATE_EMITTER_LOCK:
        return _STATE_EMITTER


def emit_state_event(event_type: str, **data):
    """Kayıtlı emitör varsa yayınlar; yoksa no-op (headless/management'siz ortam)."""
    fn = get_state_emitter()
    if fn is None:
        return
    try:
        fn("download_state", {"type": event_type, **data})
    except Exception as e:
        logger.debug(f"emit_state_event {event_type} kaybı: {e}")