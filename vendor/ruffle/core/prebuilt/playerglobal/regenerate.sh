#!/usr/bin/env bash
# Rebuilds the prebuilt AVM2 playerglobal here from core/src/avm2/globals
# (needs Java: asc.jar compiles it) and records its inputs' hash in inputs.fnv.
# Needed after changing those sources, asc.jar or the code that builds them
# (core/build.rs warns, and builds with Java, while the copy is out of date).
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
cargo run --quiet --release --manifest-path "$root/core/build_playerglobal/Cargo.toml" -- prebuild
