# Map menu observations

Direct observations in vanilla Skyrim SE supplied by the user on 2026-11-01:

1. M closes the map.
2. Right-button mouse movement both tilts and turns the camera.
3. The custom destination remains after the player reaches it.
4. The Move It / Leave It / Remove It question opens only when a marker
   already exists and the player clicks again on the map. Move It immediately
   moves it to that new click position. There is no subsequent placement click.
5. Clicking an undiscovered location in combat shows the Place Marker question.
6. Esc from the journal opened over the map returns to the map.
7. Show on Map adds no extra target highlighting.
8. The first click on empty map places the custom destination immediately.

Asset evidence and the public reference revision links are in `plans/map-menu.md`.
The Show on Map opening zoom was not specified by these observations.

## Implementation validation

The application suite passed: 591 tests, zero failures (393 opt-in tests were
not part of that default run). This includes the new regression decoding both
legacy enchanting question tags (8 and 9) alongside custom destination tag 10.
All 26 map tests passed with opt-in integration tests enabled; fast travel's
nine tests and the two custom destination save tests also passed with a GPU.
The final application build, `cargo fmt --check` and `git diff --check` passed.

The installed-asset movie integration test reads the installed Interface
archives at their external path and uses synthetic plugin records. It checks:

- Immediate empty-map placement, the captured replacement click, the modal
  question, and camera input stopping while the question is displayed.
- Existing delegate routing: both RefreshMarkers and SetSelectedMarker
  affect the world-map layer. Production calls use explicit world-layer paths.
- The map's button art, E recentering, right-button tilt and turn, the map
  movement binding following gameplay rebinding, and M closing the map.
- Keeping the map under the journal, Esc returning, refreshing after a quest
  is untracked, and Show on Map still resolving that untracked objective.
- A short click whose press and release precede the next movie tick.

A reproducible run, with no copying of game assets:

```sh
SKYRIM_DATA="<installed Skyrim SE>/Data" \
VRM_SETTINGS=/tmp/vrm-map-menu-captures/test-settings.ini \
VRM_MAP_CAPTURES=/tmp/vrm-map-menu-captures \
cargo test -p vibe_rm map_movie_ -- --ignored
```

`VRM_MAP_CAPTURES` is optional. When set, the test writes these rendered PNGs
outside the repository: `movie-custom-destination.png`,
`movie-move-leave-remove.png`, and `movie-journal-show-on-map.png`.
They were inspected: the question's order is Move It / Leave It / Remove It,
the Set Destination art is P, the journal hides the map's bottom bar, and
Show on Map is fully enabled for the synthetic resolvable objective.

A separate full-terrain offscreen capture uses the ordinary engine path:

```sh
VRM_SETTINGS=/tmp/vrm-map-menu-captures/settings.ini \
VRM_SAVES=/tmp/vrm-map-menu-captures/saves \
target/debug/vibe_rm --cell Riverwood --radius 0 --no-scripts \
  --console map --menu-key click:700,400 --menu-key pause \
  --screenshot /tmp/vrm-map-menu-captures/custom-destination.png
```

The tested world-map scope is complete. Local-map integration follows its own
plan. The precise vanilla Show on Map opening zoom remains unobserved; the
implemented normal-opening zoom choice and multi-target selection policy are
recorded in `known_ambiguities/journal.md`, with the zoom question also in the
ROADMAP. No deliberate deviation was introduced.
