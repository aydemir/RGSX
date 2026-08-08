"""Shared pytest fixtures for RGSX.

SDL dummy isolation: SDL_VIDEODRIVER/SDL_AUDIODRIVER are set to "dummy" BEFORE
pygame is imported anywhere, so display drawing code runs headless.
"""

import os
import sys

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

_PKG_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "ports", "RGSX"))
if _PKG_DIR not in sys.path:
    sys.path.insert(0, _PKG_DIR)

import pygame
import pytest


@pytest.fixture(scope="session", autouse=True)
def _pygame_init():
    if not pygame.get_init():
        pygame.init()
    yield
    if pygame.get_init():
        pygame.quit()


class FakeFont:
    """Minimal pygame font stand-in: render() returns a plain surface."""

    def __init__(self, width: int = 12, height: int = 12):
        self._width = width
        self._height = height

    def render(self, text, antialias, color=None):
        return pygame.Surface((self._width, self._height))

    def size(self, text):
        return (self._width, self._height)

    def get_height(self):
        return self._height


@pytest.fixture
def display_env():
    """Configure minimal display/config stubs so draw_* functions run headless."""
    import config
    from display import core

    config.screen_width = 1280
    config.screen_height = 720
    config.font = FakeFont()
    config.title_font = FakeFont()
    config.small_font = FakeFont()
    config.tiny_font = FakeFont()
    config.screen = pygame.display.set_mode((1280, 720))
    core.OVERLAY = pygame.Surface((1280, 720))
    yield config
    config.game_filter_obj = None
    config.selected_filter_option = 0
