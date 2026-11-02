#!/usr/bin/env bash
# Extracts the menus, font libraries and text files from Skyrim - Interface.bsa
# into $SPIKE_SWF_HOME/interface, outside the repository (game files: never put
# them in the repository; see paths.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
. "$here/paths.sh"
data="${SKYRIM_DATA:-$HOME/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data}"
bsa="$data/Skyrim - Interface.bsa"
cargo build -q --release -p vrm-tools --manifest-path "$repo/Cargo.toml"
tool="$repo/target/release/vrm-tool"
"$tool" bsa-list "$bsa" | while IFS= read -r p; do
    case "$p" in
        interface/*.swf | interface/*.txt | interface/*.gfx)
            mkdir -p "$SPIKE_SWF_HOME/$(dirname "$p")"
            "$tool" bsa-extract "$bsa" "$p" "$SPIKE_SWF_HOME/$p"
            ;;
    esac
done
# The menus load each other by mixed-case names; the archive's are lower case.
# Copies, not symlinks (AGENTS.md forbids symlinks to game assets).
cd "$SPIKE_INTERFACE"
mkdir -p "Inventory components"
for f in BottomBar InventoryLists InvertedInventoryLists ItemCard; do
    lower="$(echo "$f" | tr '[:upper:]' '[:lower:]').swf"
    cp "inventory components/$lower" "inventory components/$f.swf"
    cp "inventory components/$lower" "Inventory components/$lower"
    cp "inventory components/$lower" "Inventory components/$f.swf"
done
for f in TextEntry BethesdaNetLogin; do
    cp "$(echo "$f" | tr '[:upper:]' '[:lower:]').swf" "$f.swf"
done
echo "extracted to $SPIKE_INTERFACE"
