"""Tests for thread_safety.py locking primitives."""

import sys
import threading
import types

import pytest

import thread_safety


class TestContextManagers:
    @pytest.mark.parametrize("lock", [
        thread_safety.network_lock,
        thread_safety.pause_events_lock,
        thread_safety.download_threads_lock,
        thread_safety.cancel_events_lock,
        thread_safety.progress_queues_lock,
        thread_safety.torrent_temp_roots_lock,
        thread_safety.url_done_events_lock,
        thread_safety.url_results_lock,
        thread_safety.download_tasks_lock,
        thread_safety.download_progress_lock,
        thread_safety.download_queue_lock,
        thread_safety.history_lock,
    ])
    def test_lock_enters_and_exits(self, lock):
        inside = []
        with lock():
            inside.append(True)
        assert inside == [True]

    def test_lock_releases_after_error(self):
        with pytest.raises(RuntimeError):
            with thread_safety.network_lock():
                raise RuntimeError("boom")

    def test_network_lock_is_rlock(self):
        with thread_safety.network_lock():
            with thread_safety.network_lock():
                pass  # reentrant, must not deadlock


class TestDecorator:
    def test_with_network_lock_decorator(self):
        @thread_safety.with_network_lock
        def double(x):
            return x * 2

        assert double(21) == 42

    def test_decorator_preserves_arguments(self):
        @thread_safety.with_network_lock
        def add(a, b):
            return a + b

        assert add(2, 3) == 5


class TestConvenienceFunctions:
    @pytest.fixture
    def fake_network(self, monkeypatch):
        fake = types.SimpleNamespace(
            pause_events={},
            download_threads={},
            cancel_events={},
        )
        monkeypatch.setitem(sys.modules, "network", fake)
        return fake

    def test_pause_event_lifecycle(self, fake_network):
        ev = thread_safety.get_pause_event("t1")
        assert isinstance(ev, threading.Event)
        assert fake_network.pause_events["t1"] is ev

        thread_safety.set_pause_event("t1")
        assert ev.is_set()

        thread_safety.clear_pause_event("t1")
        assert not ev.is_set()

    def test_pause_event_reuses_existing(self, fake_network):
        existing = threading.Event()
        fake_network.pause_events["t1"] = existing
        assert thread_safety.get_pause_event("t1") is existing

    def test_register_unregister_download_thread(self, fake_network):
        t = threading.Thread()
        thread_safety.register_download_thread("dl1", t)
        assert fake_network.download_threads["dl1"] is t
        thread_safety.unregister_download_thread("dl1")
        assert "dl1" not in fake_network.download_threads

    def test_unregister_missing_is_noop(self, fake_network):
        thread_safety.unregister_download_thread("nope")  # must not raise

    def test_cancel_event_lifecycle(self, fake_network):
        ev = thread_safety.get_cancel_event("c1")
        assert isinstance(ev, threading.Event)
        assert fake_network.cancel_events["c1"] is ev
        assert thread_safety.request_cancel_task("c1") is True
        assert ev.is_set()

    def test_request_cancel_missing_returns_false(self, fake_network):
        assert thread_safety.request_cancel_task("missing") is False

    def test_register_cancel_event(self, fake_network):
        ev = thread_safety.register_cancel_event("c2")
        assert isinstance(ev, threading.Event)
        assert fake_network.cancel_events["c2"] is ev


def test_all_exports_present():
    for name in thread_safety.__all__:
        assert hasattr(thread_safety, name), f"missing export: {name}"
