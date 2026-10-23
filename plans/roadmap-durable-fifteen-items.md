# Fifteen native continuity fixes

Advances Roadmap Next 9 (saves), with one independent commit per item.
Native policies do not establish Skyrim SE or original `.ess` parity.
Synthetic fixtures only; no game assets or engine binaries are needed.

All fifteen items are complete. Each uses an optional section, replaces the
previous session's state, supplies an explicit legacy default and validates
before live state changes. Each has a synthetic capture/read/apply regression
and a malformed-payload regression. Existing layouts are unchanged.

| Item | Completed behavior | Section |
| --- | --- | --- |
| 1 | Trespass warning level and next escalation deadline | `TRSP` |
| 2 | Package-owned private homes | `PHOM` |
| 3 | Owners asking the player to leave and their next dialogue deadline | `HWAR` |
| 4 | Guards approaching for arrest and remaining bark cooldown | `GPRS` |
| 5 | Resisted factions, one-yield gate and sheathed-weapon timer | `ARRY` |
| 6 | Jail sentences, owed bounty and queued prison transport | `JSEN` |
| 7 | Accumulated jail days read by GetDaysInJail | `JDYS` |
| 8 | Ordered delayed parent activations and absolute deadlines | `ACTQ` |
| 9 | Gameplay and condition random streams resume their exact sequences | `RNGS` |
| 10 | Last-hit critical condition memory; validation keeps malformed clocks visible | `LHCR` |
| 11 | Player sneak toggle, with crouch settled and stale locomotion flags cleared | `SNEK` |
| 12 | Player attack cooldown, discarding unsaved swing/input/graph phases | `ATRC` |
| 13 | Map camera preference, including the open map view at capture | `MCAM` |
| 14 | Ordered image modifiers, strength, elapsed time and cross-fade progress | `IMOD` |
| 15 | Script black/white screen fades, delays and elapsed clocks; transient shakes clear | `GFAD` |

Additional validation covers duplicate/reordered collection keys, inflated
counts, every truncated payload prefix, duplicate sections and trailing bytes.
Invalid direct snapshots leave the running engine untouched. An actual interior
load checks sneak stance and both random continuations after cell attachment.
Image tests check cross-fade rendering and expiry; jail tests check escape bounty;
activation tests check dispatch deadlines and queue order. Console map preferences
are bounded as on normal map opening.

Known SE questions and native policy details are in `known_ambiguities/saves.md`.
Public research used the local UESP index and permanent revision links. No game
assets, leaked documents, engine executables or engine libraries were inspected
or committed. Tests use generated records only.

## Final validation

- Host GPU `cargo test --workspace`: 583 passed, 195 ignored, no failures.
- Host GPU `cargo test --bin vibe_rm save:: -- --include-ignored`:
  128 passed, no failures or ignored tests, including all 33 added tests.
- Changed Rust files pass rustfmt; `git diff --check` is clean.
- The sandbox GPU fails the existing terrain sampled-texture binding limit;
  host GPU runs passed without renderer changes.

The final validation also reinstates saved random streams after cell attachment,
preventing reconstruction from consuming the continuation. Map capture uses the
same camera bounds as map opening. All fifteen items are independently committed.
