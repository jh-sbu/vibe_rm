#!/usr/bin/env bash
# Extracts the menus, font libraries and text files from Skyrim - Interface.bsa
# into data/interface (game files: never commit them).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
data="${SKYRIM_DATA:-$HOME/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data}"
bsa="$data/Skyrim - Interface.bsa"
cargo build -q --release -p vrm-tools --manifest-path "$repo/Cargo.toml"
tool="$repo/target/release/vrm-tool"
"$tool" bsa-list "$bsa" | while IFS= read -r p; do
    case "$p" in
        interface/*.swf | interface/*.txt | interface/*.gfx)
            mkdir -p "$here/data/$(dirname "$p")"
            "$tool" bsa-extract "$bsa" "$p" "$here/data/$p"
            ;;
    esac
done
# The menus load each other by mixed-case names; the archive's are lower case.
cd "$here/data/interface"
ln -sfn "inventory components" "Inventory components"
for f in BottomBar InventoryLists InvertedInventoryLists ItemCard; do
    ln -sfn "$(echo "$f" | tr '[:upper:]' '[:lower:]').swf" "inventory components/$f.swf"
done
for f in TextEntry BethesdaNetLogin; do
    ln -sfn "$(echo "$f" | tr '[:upper:]' '[:lower:]').swf" "$f.swf"
done
echo "extracted to $here/data/interface"
