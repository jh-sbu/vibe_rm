#!/usr/bin/env bash
# Plays a menu in a window in real time: ./view.sh <menu> [ops...]
#   <menu>: a file in data/interface without .swf (hudmenu, messagebox, ...)
# The mouse goes to the movie. hudmenu and messagebox also have keys and a "game"
# answering them (printed at start); other menus get the mouse and the ops given.
#   QUALITY=low|medium|high|best   starting quality (Q cycles it)
#   SPIKE_TICK_DUE=1      tick the player only when a movie frame is due (24 fps)
#   SPIKE_NOVSYNC=1       present uncapped, to see the render cost
#   SPIKE_INLINE_BLENDS=1 / SPIKE_NOFILTERS=1   measurement switches, as in run-hud.sh
# Over SSH, run with WAYLAND_DISPLAY=wayland-0: the window opens on the machine's monitor.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
menu="${1:?usage: view.sh <menu> [ops...]}"
shift
exec "$here/swfspike/target/release/swfspike" view "$here/data/interface/$menu.swf" "$@" \
    2> >(grep --line-buffered -v "Fallback font\|Unknown device font" >&2)
