#!/bin/sh
# RGSX native launcher (Batocera/Knulli) — TASK-012-gap-02.
# Eski Python tvui.py launcher'inin (python-skeleton-final tag) Rust karşılığı:
# manager-bin'i başlatır; WebUI SPA + SDL2 TVUI aynı süreçte servis edilir.
DIR="$(dirname "$0")"

export RGSX_TVUI="${RGSX_TVUI:-1}"

exec "$DIR/manager-bin"
