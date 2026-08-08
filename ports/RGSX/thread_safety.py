#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Thread Safety Module - Centralized locking for shared data structures.

This module provides a centralized locking mechanism for shared mutable
data structures accessed from multiple threads (download threads, SSE thread,
UI thread, etc.).

All shared mutable state in network.py and config.py should be protected
by the locks defined here.
"""

import threading
from contextlib import contextmanager
from typing import Any, Generator

# ============================================================
# Global Lock for Network Module Shared State
# ============================================================

# Master lock for all network module shared dictionaries
# Using a single lock for simplicity and to avoid deadlocks
# (fine-grained locking adds complexity without much benefit here)
_network_lock = threading.RLock()


@contextmanager
def network_lock() -> Generator[None, None, None]:
    """
    Context manager for network module shared state lock.
    
    Usage:
        with network_lock():
            pause_events[task_id] = threading.Event()
            download_threads[task_id] = thread
    """
    _network_lock.acquire()
    try:
        yield
    finally:
        _network_lock.release()


def with_network_lock(func):
    """
    Decorator to automatically acquire network lock for a function.
    
    Usage:
        @with_network_lock
        def my_function(task_id: str):
            pause_events[task_id] = threading.Event()
    """
    def wrapper(*args, **kwargs):
        with network_lock():
            return func(*args, **kwargs)
    return wrapper


# ============================================================
# Individual Locks for Fine-Grained Control (if needed)
# ============================================================

# Lock for pause_events dict
_pause_events_lock = threading.RLock()

# Lock for download_threads dict
_download_threads_lock = threading.RLock()

# Lock for cancel_events dict
_cancel_events_lock = threading.RLock()

# Lock for progress_queues dict
_progress_queues_lock = threading.RLock()

# Lock for torrent_temp_roots dict
_torrent_temp_roots_lock = threading.RLock()

# Lock for url_done_events dict
_url_done_events_lock = threading.RLock()

# Lock for url_results dict
_url_results_lock = threading.RLock()


# ============================================================
# Config Module Shared State Locks
# ============================================================

# Lock for config.download_tasks dict
_download_tasks_lock = threading.RLock()

# Lock for config.download_progress dict
_download_progress_lock = threading.RLock()

# Lock for config.download_queue list
_download_queue_lock = threading.RLock()

# Lock for config.history list
_history_lock = threading.RLock()


# ============================================================
# Context Managers for Individual Locks
# ============================================================

@contextmanager
def pause_events_lock() -> Generator[None, None, None]:
    """Lock for pause_events dict."""
    _pause_events_lock.acquire()
    try:
        yield
    finally:
        _pause_events_lock.release()


@contextmanager
def download_threads_lock() -> Generator[None, None, None]:
    """Lock for download_threads dict."""
    _download_threads_lock.acquire()
    try:
        yield
    finally:
        _download_threads_lock.release()


@contextmanager
def cancel_events_lock() -> Generator[None, None, None]:
    """Lock for cancel_events dict."""
    _cancel_events_lock.acquire()
    try:
        yield
    finally:
        _cancel_events_lock.release()


@contextmanager
def progress_queues_lock() -> Generator[None, None, None]:
    """Lock for progress_queues dict."""
    _progress_queues_lock.acquire()
    try:
        yield
    finally:
        _progress_queues_lock.release()


@contextmanager
def torrent_temp_roots_lock() -> Generator[None, None, None]:
    """Lock for torrent_temp_roots dict."""
    _torrent_temp_roots_lock.acquire()
    try:
        yield
    finally:
        _torrent_temp_roots_lock.release()


@contextmanager
def url_done_events_lock() -> Generator[None, None, None]:
    """Lock for url_done_events dict."""
    _url_done_events_lock.acquire()
    try:
        yield
    finally:
        _url_done_events_lock.release()


@contextmanager
def url_results_lock() -> Generator[None, None, None]:
    """Lock for url_results dict."""
    _url_results_lock.acquire()
    try:
        yield
    finally:
        _url_results_lock.release()


# Config locks
@contextmanager
def download_tasks_lock() -> Generator[None, None, None]:
    """Lock for config.download_tasks dict."""
    _download_tasks_lock.acquire()
    try:
        yield
    finally:
        _download_tasks_lock.release()


@contextmanager
def download_progress_lock() -> Generator[None, None, None]:
    """Lock for config.download_progress dict."""
    _download_progress_lock.acquire()
    try:
        yield
    finally:
        _download_progress_lock.release()


@contextmanager
def download_queue_lock() -> Generator[None, None, None]:
    """Lock for config.download_queue list."""
    _download_queue_lock.acquire()
    try:
        yield
    finally:
        _download_queue_lock.release()


@contextmanager
def history_lock() -> Generator[None, None, None]:
    """Lock for config.history list."""
    _history_lock.acquire()
    try:
        yield
    finally:
        _history_lock.release()


# ============================================================
# Convenience Functions for Common Operations
# ============================================================

def get_pause_event(task_id: str) -> threading.Event:
    """Get or create pause event for a task (thread-safe)."""
    from network import pause_events
    with pause_events_lock():
        if task_id not in pause_events:
            pause_events[task_id] = threading.Event()
        return pause_events[task_id]


def set_pause_event(task_id: str) -> None:
    """Set pause event for a task (thread-safe)."""
    from network import pause_events
    with pause_events_lock():
        if task_id in pause_events:
            pause_events[task_id].set()


def clear_pause_event(task_id: str) -> None:
    """Clear pause event for a task (thread-safe)."""
    from network import pause_events
    with pause_events_lock():
        if task_id in pause_events:
            pause_events[task_id].clear()


def register_download_thread(task_id: str, thread: threading.Thread) -> None:
    """Register a download thread (thread-safe)."""
    from network import download_threads
    with download_threads_lock():
        download_threads[task_id] = thread


def unregister_download_thread(task_id: str) -> None:
    """Unregister a download thread (thread-safe)."""
    from network import download_threads
    with download_threads_lock():
        download_threads.pop(task_id, None)


def get_cancel_event(task_id: str) -> threading.Event:
    """Get or create cancel event for a task (thread-safe)."""
    from network import cancel_events
    with cancel_events_lock():
        if task_id not in cancel_events:
            cancel_events[task_id] = threading.Event()
        return cancel_events[task_id]


def request_cancel_task(task_id: str) -> bool:
    """Request cancellation for a task (thread-safe)."""
    from network import cancel_events
    with cancel_events_lock():
        ev = cancel_events.get(task_id)
        if ev is not None:
            ev.set()
            return True
    return False


def register_cancel_event(task_id: str) -> threading.Event:
    """Register cancel event for a task (thread-safe)."""
    from network import cancel_events
    with cancel_events_lock():
        if task_id not in cancel_events:
            cancel_events[task_id] = threading.Event()
        return cancel_events[task_id]


# Export all public symbols
__all__ = [
    # Master lock
    'network_lock', 'with_network_lock',
    # Individual locks
    'pause_events_lock', 'download_threads_lock', 'cancel_events_lock',
    'progress_queues_lock', 'torrent_temp_roots_lock',
    'url_done_events_lock', 'url_results_lock',
    # Config locks
    'download_tasks_lock', 'download_progress_lock', 'download_queue_lock', 'history_lock',
    # Convenience functions
    'get_pause_event', 'set_pause_event', 'clear_pause_event',
    'register_download_thread', 'unregister_download_thread',
    'get_cancel_event', 'request_cancel_task', 'register_cancel_event',
]