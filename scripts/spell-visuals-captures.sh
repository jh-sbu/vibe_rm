#!/usr/bin/env bash
# vibe_rm's reference captures for `test-mod spell-visuals`: the same plugin
# the user runs in Skyrim SE, loaded offscreen, each scene the plugin's
# message box lists rendered as a few frames for side-by-side comparison
# with the SE screenshots in tests/results/spell-visuals/.
#
#   scripts/spell-visuals-captures.sh [out dir]   (default tests/captures/spell-visuals)
#
# The game's Data directory is never written to: the plugin and its script
# are linked into an overlay under tests/assets/overlay/Data beside links to
# the game's own files. Saves the plugin requests go to tests/assets/capture-saves.
# Everything written is under the ignored tests/. A manifest beside the
# captures records the commit and the commands.
set -euo pipefail
cd "$(dirname "$0")/.."

DATA="${SKYRIM_DATA:-$HOME/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data}"
OUT="${1:-tests/captures/spell-visuals}"
OVERLAY=tests/assets/overlay
PLUGIN=VrmSpellVisuals.esp
SCRIPT=VrmSpellVisualsProbe.pex

cargo build -p vrm-tools
cargo build --release
target/debug/vrm-tool test-mod spell-visuals "$DATA" "tests/assets/$PLUGIN"

rm -rf "$OVERLAY"
mkdir -p "$OVERLAY/Data/Scripts"
# The engine reads Skyrim.ccc beside the Data directory (Creation Club plugins).
[ -e "$DATA/../Skyrim.ccc" ] && ln -s "$(cd "$DATA/.." && pwd)/Skyrim.ccc" "$OVERLAY/Skyrim.ccc"
for e in "$DATA"/*; do
    [ "$(basename "$e")" = Scripts ] || ln -s "$e" "$OVERLAY/Data/"
done
if [ -d "$DATA/Scripts" ]; then
    for e in "$DATA"/Scripts/*; do ln -s "$e" "$OVERLAY/Data/Scripts/"; done
fi
ln -s "$PWD/tests/assets/$PLUGIN" "$OVERLAY/Data/"
ln -s "$PWD/tests/assets/Scripts/$SCRIPT" "$OVERLAY/Data/Scripts/"

mkdir -p "$OUT" tests/assets/capture-saves
MANIFEST="$OUT/manifest.txt"
{
    echo "vibe_rm spell visual captures, $(date -u +%Y-%m-%dT%H:%MZ)"
    echo "commit $(git rev-parse HEAD)$(git diff --quiet || echo ' (with uncommitted changes)')"
    echo
} > "$MANIFEST"

# The quest's first update, its message box and Survival Mode's are over by
# frame 650; the plugin's spots and spells are in place by then.
SETTLED=650
FLAMES=00012FCD
FROSTBITE=0002B96B
FIREBALL=0001C789

# capture <name> <yaw> <hour> <wait> <burst from> <burst step> <console commands...>
# --wait holds the camera where the run starts: $POS (x,y) starts it at a
# spot the plugin moves the player to, on the ground there.
capture() {
    local name=$1 yaw=$2 hour=$3 wait=$4 from=$5 step=$6
    shift 6
    local args=(--data "$OVERLAY/Data" --plugin "$PLUGIN" --yaw "$yaw" --hour "$hour"
        --weather SkyrimClear ${POS:+--pos "$POS,-100000"}
        --wait "$wait" --burst "$step" --burst-from "$from"
        --console "@5 pthird" --console "@$SETTLED cqf VrmFx StopVisual")
    for c in "$@"; do args+=(--console "$c"); done
    mkdir -p "$OUT/$name"
    echo "$name: target/release/vibe_rm ${args[*]} --screenshot $OUT/$name/$name.png" >> "$MANIFEST"
    VRM_SAVES=tests/assets/capture-saves target/release/vibe_rm "${args[@]}" \
        --screenshot "$OUT/$name/$name.png" > "$OUT/$name/log.txt" 2>&1
    echo "$name: $(ls "$OUT/$name"/*.png | wc -l) frames"
}

# own <name> <yaw> <hour> <spot function> <spell> [release frame]
# Held spells: frames from 10 to 140 frames into the hold. Fireball, let go
# at the release frame: every 3 frames from just after it, while it flies.
own() {
    local name=$1 yaw=$2 hour=$3 spot=$4 spell=$5 release=${6:-}
    local cmds=("@655 cqf VrmFx $spot" "@660 player.equipspell $spell right" "@670 pdraw"
        "@820 pcast right")
    if [ -n "$release" ]; then
        cmds+=("@$release pcast right off")
        capture "$name" "$yaw" "$hour" $((release + 60)) $((release + 9)) 3 "${cmds[@]}"
    else
        capture "$name" "$yaw" "$hour" 960 830 15 "${cmds[@]}"
    fi
}

# npc <name> <yaw> <hour> <spot function> <caster function> [step]
npc() {
    capture "$1" "$2" "$3" 960 760 "${6:-25}" "@655 cqf VrmFx $4" "@660 cqf VrmFx $5"
}

# Oakflesh as the quest plays it, before StopVisual.
capture oakflesh 140 12 600 540 30
own own-flames-rock 140 12 Rock "$FLAMES"
own own-flames-air 50 12 Air "$FLAMES"
own own-frost-rock 140 12 Rock "$FROSTBITE"
own own-frost-air 50 12 Air "$FROSTBITE"
own own-fireball-rock 140 12 Rock "$FIREBALL" 880
npc npc-flames-side 104 12 Side NpcFlames
# AirSide is the plugin's open-air side spot (spell_visuals.rs AIR_SIDE).
POS=19110.4,-47056.9 npc npc-flames-air-side 320 12 AirSide NpcFlamesAir
npc npc-frost-side 104 12 Side NpcFrost
POS=19110.4,-47056.9 npc npc-frost-air-side 320 12 AirSide NpcFrostAir
# Its fireballs cross in a few frames: every 4th frame, 760 to 860.
capture npc-fireball-side 104 12 860 760 4 "@655 cqf VrmFx Side" "@660 cqf VrmFx NpcFireball"
own own-flames-rock-night 140 23 Rock "$FLAMES"
npc npc-flames-side-night 104 23 Side NpcFlames
echo "captures in $OUT ($MANIFEST)"
