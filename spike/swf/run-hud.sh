#!/usr/bin/env bash
# Drives hudmenu.swf the way the game would and prints timings. out/hud.png is
# taken 2.5 s in; out/hud-end.png after the timing run, once the meters, location
# and notification have faded out as they do in the game.
#   QUALITY=low|medium|high|best (default high)
#   SPIKE_INLINE_BLENDS=1  draw Add-blend objects inline (measurement only, not correct)
#   SPIKE_NOFILTERS=1      drop filters (measurement only)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/out"
H=_root.HUDMovieBaseInstance
"$here/swfspike/target/release/swfspike" run "$here/data/interface/hudmenu.swf" "$here/out/hud-end.png" \
    "quality:${QUALITY:-high}" \
    "call:$H.ShowElements|All|true" \
    "call:$H.SetHealthMeterPercent|60|false" \
    "call:$H.SetMagickaMeterPercent|35|false" \
    "call:$H.SetStaminaMeterPercent|85|false" \
    "call:$H.SetCompassAngle|30|30|true" \
    "call:$H.SetLocationName|Riverwood" \
    "call:$H.ShowMessage|You have entered Riverwood" \
    "call:$H.ShowSubtitle|Hey, you. You're finally awake." \
    "call:$H.SetCrosshairTarget|true|Iron Sword|true|false|false|true|9|25|7|" \
    "frames:60" \
    "png:$here/out/hud.png" \
    "blends:" \
    "split:600" \
    2>&1 | grep -v "Fallback font\|Unknown device font"
