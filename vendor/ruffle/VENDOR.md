# Vendored Ruffle

A subset of [Ruffle](https://github.com/ruffle-rs/ruffle), the Rust Flash Player,
used to run Skyrim's Interface `.swf` menus (Scaleform, ActionScript 2).
Licensed MIT OR Apache-2.0 (`LICENSE.md`).

- Upstream: <https://github.com/ruffle-rs/ruffle>, tag `v0.7.1`, commit
  `89a7049965f3090e706265670df309a5128233ac`.
- Vendored: the crates `ruffle_core` and `ruffle_render_wgpu` build from
  (`core` with `common`, `macros` and `build_playerglobal`; `render` with
  `wgpu`, `naga-agal`, `naga-pixelbender` and `pixel_bender`; `swf`, `wstr`,
  `video`, `flv`, `tools/asc`), the workspace manifest and lock file, the
  license, the upstream README and `rustfmt.toml`.
- Left out: everything else (desktop and web players, the other renderers,
  tests, docs), the crates' `tests/` directories and the
  `render/pixel_bender/assembly_tests` crate, so Ruffle's own tests don't run
  from here. The workspace's member list names the vendored crates only.

The first commit adding this directory is upstream as is; the commits after it
carry the changes, one each. Building needs Java: `ruffle_core`'s build script
compiles the AVM2 `playerglobal` library with `tools/asc/asc.jar`.

The vendored crates are a workspace of their own (`Cargo.toml` here), not
members of the engine's.
