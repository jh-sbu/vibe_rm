#!/usr/bin/env bash
# Builds the harness against the vendored Ruffle (vendor/ruffle, with its
# `spike` feature).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cargo build --release --manifest-path "$here/swfspike/Cargo.toml"
