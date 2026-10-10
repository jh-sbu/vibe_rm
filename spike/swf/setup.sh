#!/usr/bin/env bash
# Builds the harness against the vendored Ruffle (vendor/ruffle, with its
# `spike` feature). Needs Java: ruffle_core's build compiles its AVM2 library
# with asc.jar.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cargo build --release --manifest-path "$here/swfspike/Cargo.toml"
