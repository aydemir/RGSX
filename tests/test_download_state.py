"""Faz 8 tests — download state machine, job model, retry integration.

Kapsananlar:
- Transition tablosu: gecerli/gecersiz gecisler, effects callback.
- Error classifier: transient/permanent ayrimi (HTTP kodlari + marker'lar).
- Backoff: ustel buyume + tavan.
- DownloadJob: history entry round-trip (geriye donuk uyum — eski format).
- Entegrasyon: _finalize_download_result (completed/retry_scheduled/failed)
  + _schedule_download_retry yeniden baslatma + history eski format regression.
"""

import json
import sys
import threading
import time
import types

import pytest

sys.path.insert(0, "ports/RGSX")

import config
from network.download_state import (
    DEFAULT_MAX_RETRIES,
    DownloadEvent,
    DownloadJob,
    DownloadState,
    IllegalTransitionError,
    classify_error,
    emit_state_event,
    legacy_history_status,
    retry_backoff_seconds,
    set_state_emitter,
    state_from_legacy,
    transition,
)


class TestTransitions:
    def test_happy_path_download_to_completed(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.QUEUED)
        assert transition(job, DownloadEvent.STARTED) is DownloadState.DOWNLOADING
        assert transition(job, DownloadEvent.COMPLETED) is DownloadState.COMPLETED

    def test_retry_flow(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.DOWNLOADING)
        transition(job, DownloadEvent.TRANSIENT_FAILURE)
        assert job.state is DownloadState.FAILED_TRANSIENT
        transition(job, DownloadEvent.RETRY_TRIGGERED)
        assert job.state is DownloadState.RETRY_SCHEDULED
        transition(job, DownloadEvent.STARTED)
        assert job.state is DownloadState.DOWNLOADING

    def test_transient_then_permanent(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.DOWNLOADING)
        transition(job, DownloadEvent.TRANSIENT_FAILURE)
        transition(job, DownloadEvent.PERMANENT_FAILURE)
        assert job.state is DownloadState.FAILED_PERMANENT

    def test_retry_exhausted(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.FAILED_TRANSIENT)
        transition(job, DownloadEvent.RETRY_EXHAUSTED)
        assert job.state is DownloadState.FAILED_PERMANENT

    def test_pause_resume(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.DOWNLOADING)
        transition(job, DownloadEvent.PAUSE_REQUESTED)
        assert job.state is DownloadState.PAUSED
        transition(job, DownloadEvent.RESUME_REQUESTED)
        assert job.state is DownloadState.DOWNLOADING

    def test_cancel_from_paused(self):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.PAUSED)
        transition(job, DownloadEvent.CANCEL_REQUESTED)
        assert job.state is DownloadState.CANCELED

    @pytest.mark.parametrize("event", [
        DownloadEvent.PAUSE_REQUESTED,
        DownloadEvent.PERMANENT_FAILURE,
        DownloadEvent.COMPLETED,
        DownloadEvent.CANCEL_REQUESTED,
    ])
    def test_illegal_transitions_raise(self, event):
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.COMPLETED)
        with pytest.raises(IllegalTransitionError):
            transition(job, event)
        assert job.state is DownloadState.COMPLETED  # state bozulmadi

    def test_effects_callback_receives_transition(self):
        seen = []
        job = DownloadJob(id="t1", url="http://x", state=DownloadState.DOWNLOADING)

        def effects(j, old, new, event):
            seen.append((old, new, event))

        transition(job, DownloadEvent.COMPLETED, effects=effects)
        assert seen == [(DownloadState.DOWNLOADING, DownloadState.COMPLETED, DownloadEvent.COMPLETED)]
        assert job.state is DownloadState.COMPLETED


class TestClassifier:
    @pytest.mark.parametrize("msg", [
        "Connection failed after 4 attempts - HTTP error 500",
        "429 Too Many Requests",
        "Too Many Requests: 429",
        "Read timed out",
        "Connection aborted.",
        "Network connection error",
        "Temporarily unavailable",
        "Vimm.net limits downloads to one at a time per IP. Wait for the other download to finish and retry.",
        "1F: Erreur serveur (503)",
        "DL: Server error (502)",
        "server error (HTTP 504)",
        "Connection failed after 2 attempts - timeout",
    ])
    def test_transient(self, msg):
        assert classify_error(msg) is True, msg

    @pytest.mark.parametrize("msg", [
        "Access denied (HTTP 403)",
        "Authentication required (HTTP 401)",
        "HTTP error 404",
        "Vimm returned an HTML page instead of an archive (content-type=text/html)",
        "Downloaded payload is not a valid archive",
        "Item archive.org restreint (is_dark=true): xyz",
        "Fichiers disponibles exemples: ['a.zip']",
        "File not found",
        "Download blocked: low disk space",
        "Pas assez d'espace disque pour telecharger",
        "Downloaded empty response from source (content-type=unknown)",
        "Access blocked by a browser challenge on the source host.",
    ])
    def test_permanent(self, msg):
        assert classify_error(msg) is False, msg

    def test_transient_exception_types(self):
        import requests
        assert classify_error("", error_type=requests.Timeout()) is True
        assert classify_error("", error_type=requests.ConnectionError()) is True
        assert classify_error("read timeout", error_type=requests.HTTPError("read timeout")) is True

    def test_insufficient_disk_is_permanent(self):
        from network.helpers import InsufficientDiskSpaceError
        assert classify_error("", error_type=InsufficientDiskSpaceError("manque d'espace")) is False

    def test_empty_or_unknown_defaults_permanent(self):
        assert classify_error("") is False
        assert classify_error("Some totally unknown internal failure") is False


class TestBackoff:
    def test_exponential_growth(self):
        assert retry_backoff_seconds(1) == 5.0
        assert retry_backoff_seconds(2) == 10.0
        assert retry_backoff_seconds(3) == 20.0
        assert retry_backoff_seconds(4) == 40.0

    def test_capped(self):
        assert retry_backoff_seconds(100) == 300.0

    def test_zero_and_negative(self):
        assert retry_backoff_seconds(0) == 0.0
        assert retry_backoff_seconds(-3) == 0.0

    def test_custom_base_max(self):
        assert retry_backoff_seconds(2, base=2.0, max_wait=10.0) == 4.0
        assert retry_backoff_seconds(10, base=2.0, max_wait=10.0) == 10.0


class TestLegacyMapping:
    def test_state_from_legacy(self):
        assert state_from_legacy("Queued") is DownloadState.QUEUED
        assert state_from_legacy("Téléchargement") is DownloadState.DOWNLOADING
        assert state_from_legacy("Downloading") is DownloadState.DOWNLOADING
        assert state_from_legacy("Paused") is DownloadState.PAUSED
        assert state_from_legacy("Extracting") is DownloadState.EXTRACTING
        assert state_from_legacy("Download_OK") is DownloadState.COMPLETED
        assert state_from_legacy("Erreur") is DownloadState.FAILED_PERMANENT
        assert state_from_legacy("Canceled") is DownloadState.CANCELED
        assert state_from_legacy("Try 2/4") is DownloadState.DOWNLOADING
        assert state_from_legacy("") is DownloadState.DOWNLOADING
        assert state_from_legacy("Weird") is DownloadState.DOWNLOADING

    def test_legacy_history_status_roundtrip_active(self):
        for st in (DownloadState.DOWNLOADING, DownloadState.RETRY_SCHEDULED, DownloadState.FAILED_TRANSIENT):
            assert legacy_history_status(st) == "Téléchargement"
        assert legacy_history_status(DownloadState.FAILED_PERMANENT) == "Erreur"
        assert legacy_history_status(DownloadState.COMPLETED) == "Download_OK"
        assert legacy_history_status(DownloadState.PAUSED) == "Paused"
        assert legacy_history_status(DownloadState.QUEUED) == "Queued"


class TestJobRoundTrip:
    OLD_FORMAT_ENTRY = {
        "platform": "NES",
        "game_name": "Super Mario.nes",
        "status": "Downloading",
        "url": "https://example.com/mario.nes",
        "progress": 0,
        "timestamp": "2026-08-11 02:42:48",
    }

    def test_from_old_format_entry(self):
        job = DownloadJob.from_history_entry(dict(self.OLD_FORMAT_ENTRY))
        assert job.url == "https://example.com/mario.nes"
        assert job.game_name == "Super Mario.nes"
        assert job.state is DownloadState.DOWNLOADING
        assert job.retry_count == 0
        assert job.max_retries == DEFAULT_MAX_RETRIES

    def test_apply_preserves_legacy_fields_and_adds_new(self):
        entry = dict(self.OLD_FORMAT_ENTRY)
        job = DownloadJob.from_history_entry(entry)
        job.state = DownloadState.RETRY_SCHEDULED
        job.retry_count = 2
        job.max_retries = 3
        job.error = "HTTP error 500"
        job.retry_at = 123.45
        job.progress = 33.0
        job.message = "Retry ..."
        job.apply_to_history_entry(entry)

        # mevcut alan adlari korunur
        assert entry["platform"] == "NES"
        assert entry["game_name"] == "Super Mario.nes"
        assert entry["url"] == "https://example.com/mario.nes"
        assert entry["timestamp"] == "2026-08-11 02:42:48"
        # ek alanlar eklenir
        assert entry["status"] == "Téléchargement"
        assert entry["entity_state"] == "RETRY_SCHEDULED"
        assert entry["retry_count"] == 2
        assert entry["max_retries"] == 3
        assert entry["error"] == "HTTP error 500"
        assert entry["retry_at"] == 123.45
        assert entry["progress"] == 33

    def test_json_serializable(self):
        entry = dict(self.OLD_FORMAT_ENTRY)
        job = DownloadJob.from_history_entry(entry)
        job.state = DownloadState.FAILED_PERMANENT
        job.retry_count = 3
        job.error = "boom"
        job.apply_to_history_entry(entry)
        # history.json'a yazilabilir olmali (json round-trip)
        dumped = json.dumps(entry, ensure_ascii=False)
        loaded = json.loads(dumped)
        assert loaded["entity_state"] == "FAILED_PERMANENT"
        assert loaded["retry_count"] == 3


class TestStateEmitter:
    def test_emitter_receives_download_state_event(self):
        received = []
        set_state_emitter(lambda event_type, data: received.append((event_type, data)))
        try:
            emit_state_event("retry_scheduled", url="u", retry_count=1)
            assert received[0][0] == "download_state"
            assert received[0][1]["type"] == "retry_scheduled"
            assert received[0][1]["url"] == "u"
        finally:
            set_state_emitter(None)

    def test_no_emitter_is_noop(self):
        set_state_emitter(None)
        emit_state_event("x", url="u")  # hata vermemeli


class TestFinalizeIntegration:
    """network.queue._finalize_download_result + _schedule_download_retry."""

    @pytest.fixture(autouse=True)
    def _stub_side_effects(self, monkeypatch):
        from network import queue as nq
        # disk'e yazma yok; config stubs
        monkeypatch.setattr(nq, "_save_history_with_feedback", lambda *a, **k: True)
        monkeypatch.setattr(nq.config, "needs_redraw", False)
        monkeypatch.setattr(nq.config, "history", [])
        monkeypatch.setattr(nq.config, "DOWNLOAD_MAX_RETRIES", 3)
        monkeypatch.setattr(nq, "_retry_backoff", lambda n: 0.0)
        # emit_state_event pasif (emitter None)
        set_state_emitter(None)
        yield nq

    def _make_entry(self, url="http://x/f.nes", status="Downloading", **kw):
        entry = {
            "platform": "NES",
            "game_name": "f.nes",
            "status": status,
            "url": url,
            "progress": 0,
            "timestamp": "2026-08-11 02:42:48",
        }
        entry.update(kw)
        return entry

    def test_success_marks_completed(self, _stub_side_effects):
        nq = _stub_side_effects
        entry = self._make_entry()
        outcome = nq._finalize_download_result("t1", "http://x/f.nes", True, "OK", "NES", "f.nes", entry)
        assert outcome == "completed"
        assert entry["status"] == "Download_OK"
        assert entry["entity_state"] == DownloadState.COMPLETED.value
        assert entry["progress"] == 100

    def test_transient_failure_schedules_retry(self, _stub_side_effects, monkeypatch):
        nq = _stub_side_effects
        scheduled = {}
        monkeypatch.setattr(nq, "_schedule_download_retry",
                            lambda job, delay: scheduled.update({"job": job, "delay": delay}))

        entry = self._make_entry()
        outcome = nq._finalize_download_result("t1", "http://x/f.nes", False,
                                               "HTTP error 500", "NES", "f.nes", entry)
        assert outcome == "retry_scheduled"
        assert entry["status"] == "Téléchargement"          # aktif görünüm
        assert entry["entity_state"] == DownloadState.RETRY_SCHEDULED.value
        assert entry["retry_count"] == 1
        assert entry["max_retries"] == 3
        assert "error" in entry
        assert scheduled["job"].retry_count == 1
        assert scheduled["job"].url == "http://x/f.nes"

    def test_retry_count_increments_on_second_failure(self, _stub_side_effects, monkeypatch):
        nq = _stub_side_effects
        scheduled = []
        monkeypatch.setattr(nq, "_schedule_download_retry",
                            lambda job, delay: scheduled.append((job, delay)))

        entry = self._make_entry(retry_count=1, max_retries=3, entity_state="RETRY_SCHEDULED")
        outcome = nq._finalize_download_result("t1", "http://x/f.nes", False,
                                               "Connection aborted.", "NES", "f.nes", entry)
        assert outcome == "retry_scheduled"
        assert entry["retry_count"] == 2
        assert scheduled[0][0].retry_count == 2

    def test_retries_exhausted_becomes_permanent(self, _stub_side_effects):
        nq = _stub_side_effects
        entry = self._make_entry(retry_count=3, max_retries=3, entity_state="RETRY_SCHEDULED")
        outcome = nq._finalize_download_result("t1", "http://x/f.nes", False,
                                               "HTTP error 500", "NES", "f.nes", entry)
        assert outcome == "failed"
        assert entry["status"] == "Erreur"
        assert entry["entity_state"] == DownloadState.FAILED_PERMANENT.value
        assert entry["retry_count"] == 3

    def test_permanent_error_no_retry(self, _stub_side_effects, monkeypatch):
        nq = _stub_side_effects
        scheduled = []
        monkeypatch.setattr(nq, "_schedule_download_retry",
                            lambda job, delay: scheduled.append(job))
        outcome = nq._finalize_download_result("t1", "http://x/f.nes", False,
                                               "Access denied (HTTP 403)", "NES", "f.nes", self._make_entry())
        assert outcome == "failed"
        assert scheduled == []  # kalıcı hatada retry planlanmaz

    def test_schedule_retry_relaunches_download_rom(self, monkeypatch):
        from network import queue as nq
        monkeypatch.setattr(nq, "_save_history_with_feedback", lambda *a, **k: True)
        calls = []

        def fake_download_rom(url, platform, game_name, is_zip_non_supported, task_id):
            calls.append((url, platform, game_name, is_zip_non_supported, task_id))
            return None

        monkeypatch.setattr(nq, "download_rom", fake_download_rom)
        monkeypatch.setattr(nq, "asyncio", types.SimpleNamespace(run=lambda c: None))
        monkeypatch.setattr(nq, "_retry_backoff", lambda n: 0.0)
        monkeypatch.setattr(nq.config, "active_download_count", 0)
        monkeypatch.setattr(nq.config, "max_simultaneous_downloads", 5)
        monkeypatch.setattr(nq.config, "download_active", False)
        monkeypatch.setattr(nq, "network", types.SimpleNamespace(_app_shutting_down=False))

        job = DownloadJob(
            id="t1", url="http://x/f.nes", task_id="t1",
            platform="NES", game_name="f.nes",
            state=DownloadState.RETRY_SCHEDULED, retry_count=1, max_retries=3,
        )
        nq._schedule_download_retry(job, 0.0)

        deadline = time.time() + 3.0
        while not calls and time.time() < deadline:
            time.sleep(0.05)
        assert calls, "retry thread download_rom'u cagirmadi"
        assert calls[0][0] == "http://x/f.nes"
        assert calls[0][4].startswith("retry_")

    def test_schedule_retry_aborts_on_shutdown(self, monkeypatch):
        from network import queue as nq
        monkeypatch.setattr(nq, "network", types.SimpleNamespace(_app_shutting_down=True))
        monkeypatch.setattr(nq, "_save_history_with_feedback", lambda *a, **k: True)

        def fake_download_rom(*a, **k):
            raise AssertionError("should not run")

        monkeypatch.setattr(nq, "download_rom", fake_download_rom)
        job = DownloadJob(id="t1", url="http://x", task_id="t1",
                          state=DownloadState.RETRY_SCHEDULED, retry_count=1, max_retries=3)
        nq._schedule_download_retry(job, 0.0)
        time.sleep(0.3)
        # hata firlatmadan sessizce donmeli (assertion yoksa gecti)


class TestHistoryBackwardCompat:
    """history.json eski format regression — yeni alanlar yoksa da okunur."""

    def test_load_old_format_and_save_with_new_fields(self, tmp_path, monkeypatch):
        old_entry = {
            "platform": "NES",
            "game_name": "mario.nes",
            "status": "Download_OK",
            "url": "http://x/mario.nes",
            "progress": 100,
            "timestamp": "2026-08-11 02:42:48",
        }
        p = tmp_path / "history.json"
        p.write_text(json.dumps([old_entry]), encoding="utf-8")

        import history
        monkeypatch.setattr(config, "HISTORY_PATH", str(p))
        loaded = history.load_history()
        assert len(loaded) == 1
        assert loaded[0]["status"] == "Download_OK"

        # yeni alanlarla guncelle -> kaydet -> tekrar yukle
        job = DownloadJob.from_history_entry(loaded[0])
        job.state = DownloadState.FAILED_PERMANENT
        job.retry_count = 1
        job.error = "HTTP error 500"
        job.apply_to_history_entry(loaded[0])
        history.save_history(loaded, force=True)

        # async batched writer thread'in diske yazmasini bekle
        reloaded = []
        deadline = time.time() + 2.0
        while time.time() < deadline:
            reloaded = history.load_history()
            if reloaded and reloaded[0].get("retry_count") == 1:
                break
            time.sleep(0.05)
        assert reloaded and reloaded[0]["status"] == "Erreur"
        assert reloaded[0]["entity_state"] == DownloadState.FAILED_PERMANENT.value
        assert reloaded[0]["retry_count"] == 1
        assert reloaded[0]["error"] == "HTTP error 500"
