# Fifteen native save-continuity items

Advances Roadmap Next 9, with independent commits and synthetic tests. Optional
sections preserve old layouts and replace destination state; absent sections use
explicit legacy defaults. Research uses the local UESP index and public material,
never engine binaries. No game assets are needed or included.

1. Harvested plants and their regrowth timestamps (`HRVT`).
2. Cleared locations (`CLRD`).
3. Scripted location keyword values (`LKWD`).
4. Scripted actor crime factions, including explicit removal (`CRFC`).
5. Pickpocket victims who caught the player (`PKCT`).
6. Assault victims and player assailants (`ASLT`).
7. Unreported crimes and surviving witness identities (`UWIT`).
8. Last crime per faction for arrest events (`LCRM`).
9. Script-disabled controls, both camera-lock owners and cart HUD (`CTRL`).
10. Voice recovery and HUD denominator (`VREC`).
11. Player stamina regeneration delay (`SREG`).
12. Player magicka regeneration delay and casting reset on load (`MREG`).
13. Journal visibility and ordering/alias-fill counters (`JMEM`).
14. Favorite spell, school and shout usage histories (`MUSE`).
15. Camera perspective and third-person distance (`CAMR`).

Native save policies and outstanding Skyrim SE questions are documented in
`known_ambiguities/saves.md`. Outcomes follow as each item completes.

## 1 completed

Added optional HRVT with sorted nonnull plant identities and finite nonnegative
game-hour timestamps. Restoration precedes cell attachment and clears legacy
memory. Payload validation and production capture/read/apply tests passed;
the runtime test verifies regrowth after elapsed game days and legacy clearing.
Host Vulkan was required because the sandbox terrain pipeline exceeds its
texture-binding limit. Shared adjunct infrastructure preserves existing layouts.

## 2 completed

CLRD restores cleared locations before cell attachment without replaying
clearing banners, music or statistics. Payload and production runtime regressions
cover exact replacement and legacy clearing.

## 3 completed

LKWD retains signed finite location keyword data, including explicit zero.
Sorted pair keys and strict payload decoding preserve deterministic saves.
Payload and host-Vulkan replacement/legacy regressions passed.

## 4 completed

CRFC preserves actor crime-faction overrides, distinguishing an explicit
removal from absence. Payload and host-Vulkan API lookup/replacement/legacy
regressions passed.
