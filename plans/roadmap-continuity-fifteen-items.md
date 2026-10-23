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

## 5 completed

PKCT retains identities of victims who caught a pickpocket attempt, so load
does not erase the current retry gate. Strict payload and host-Vulkan exact
replacement/legacy tests passed.

## 6 completed

ASLT retains player assault victims (used for murder attribution) and
assailants whose continuing hits should not recount assault. Payload and
host-Vulkan capture/apply/legacy tests passed.

## 7 completed

UWIT preserves pending crime order, bounty amount, violence and witness
identities. The runtime regression restores a pending report and kills its last
witness through the existing loaded-actor update, withdrawing its saved bounty.
Payload, runtime and legacy tests passed with host Vulkan.

## 8 completed

LCRM retains each faction's last reported crime for arrest Story Manager
inputs. Decoding rejects unknown crime discriminants. Payload and host-Vulkan
replacement/legacy tests passed.

## 9 completed

CTRL restores all script control gates, independent regular/beast camera
locks and cart HUD mode before cell attachment. The runtime regression releases
each POV owner separately after load. Payload and host-Vulkan tests passed.

## 10 completed

VREC restores remaining voice recovery and its HUD denominator, clearing
held voice input and old meter flashes. The runtime test checks the restored
meter and expiry through update_shouts. Payload and host-Vulkan tests passed.
