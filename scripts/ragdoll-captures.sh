#!/usr/bin/env bash
# Engine-only smoke captures; game assets remain at their external Data path.
set -euo pipefail
cd "$(dirname "$0")/.."
out_dir=${1:-/tmp/vrm-ragdoll-captures}
data_dir=${SKYRIM_DATA:-$HOME/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data}
mkdir -p "$out_dir"
VRM_SEED=1 VRM_SAVES="$out_dir/saves" target/debug/vibe_rm \
  --data "$data_dir" --cell WhiterunBanneredMare --no-scripts \
  --wait 1200 --watch 000813BA --burst 60 --burst-from 60 --size 800x450 \
  --screenshot "$out_dir/sinmir.png" --console 'tai' \
  --console '@60 knock 000813BA 5' --console '@360 gstate 000813BA' \
  --console '@700 gstate 000813BA' --console '@1199 gstate 000813BA' \
  > "$out_dir/sinmir.log" 2>&1
VRM_SEED=1 VRM_SAVES="$out_dir/saves" target/debug/vibe_rm \
  --data "$data_dir" --cell WhiterunBanneredMare --no-scripts --player-at-camera \
  --wait 600 --burst 60 --size 800x450 --screenshot "$out_dir/player.png" \
  --console 'tai' --console '@60 knock player 0' \
  --console '@180 gstate 00000014' --console '@599 gstate 00000014' \
  > "$out_dir/player.log" 2>&1
