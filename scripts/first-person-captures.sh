#!/usr/bin/env bash
# Reproducible offscreen smoke checks; writes captures/logs outside the repo.
# Run cargo build first. These are engine captures, not vanilla SE evidence.
set -euo pipefail
cd "$(dirname "$0")/.."
out_dir=${1:-/tmp/vrm-first-person-captures}
data_dir=${SKYRIM_DATA:-$HOME/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data}
mkdir -p "$out_dir"

capture() {
    local name=$1
    shift
    VRM_SEED=1 VRM_SAVES="$out_dir/saves" target/debug/vibe_rm \
        --data "$data_dir" --cell WhiterunBanneredMare --no-scripts \
        --player-at-camera --wait 210 --size 960x540 \
        --screenshot "$out_dir/$name.png" "$@" > "$out_dir/$name.log" 2>&1
}

sword=(--console 'player.additem 00012eb7 1' --console 'player.equipitem 00012eb7' --console '@30 pdraw')
capture sheathed
capture sword "${sword[@]}"
capture bow --console 'player.additem 0003b562 1' --console 'player.additem 0001397d 20' \
    --console 'player.equipitem 0003b562' --console '@30 pdraw' --console '@140 ppress'
capture flames --console 'player.equipspell 00012fcd right' --console 'player.equipspell 00012fcd left' \
    --console '@30 pdraw' --console '@140 pcast right' --console '@140 pcast left'
capture torch --console 'player.additem 0001d4ec 1' --console 'player.equipitem 0001d4ec' --console '@30 pdraw'
capture shield "${sword[@]}" --console 'player.additem 00012eb6 1' --console 'player.equipitem 00012eb6'
capture sneak "${sword[@]}" --console '@100 psneak'
capture down "${sword[@]}" --pitch -80
