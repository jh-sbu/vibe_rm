# Scroll casting movement hold

Surveyed 2026-11-02 using `vrm-tool magic <Data> --scrolls`, reading game
records at their external Steam Data path. No game assets are stored here.

The default installed load order contains Skyrim, Update, Dawnguard,
HearthFires, Dragonborn, Fishing, Survival Mode, Rare Curios, Saints & Seducers,
and `_ResourcePack`. Of 104 SCRL records, 102 have SPIT casting type 3 and two
have type 2: `TGDetectLifeScroll` (001019AE) and
`DLC1dunRedwaterDenTelekinesisScroll` (02014042). The five Saints & Seducers
scrolls all have type 3; excluding those leaves 99 records, including the same
two concentration records. These are record counts, not a claim that every
record is obtainable or usable by the player.

Stored SPIT charge times range from zero to three seconds. Seven records have
three seconds: Bane of the Undead, Mass Paralysis, Harmony, Blizzard, and the
three elemental Thralls. This is stored data, not a measured full animation
length or a bound on how long the player may keep the button held.

[UESP's SCRL format](https://en.uesp.net/w/index.php?oldid=2334437)
describes automatic charge-time calculation from effects and identifies type 3
as scroll-specific, while leaving its exact meaning uncertain. The survey
reports raw SPIT values; it does not establish whether every stored charge time
is the effective in-game time. The existing runtime uses SPIT charge time.

[UESP's Scrolls page](https://en.uesp.net/w/index.php?oldid=3543120),
“Using Scrolls”, establishes the walking/running restriction during casting,
release to cast, and preservation on interruption. It does not specify turning,
jumping, sneaking, swimming, airborne behavior, or the precise animation frame
at which movement becomes available. The implementation choices for those
boundaries are recorded in `known_ambiguities/scrolls.md`; manual SE observation
remains in `plans/scroll-casting-hold.md`.

## SE observations supplied by the user (2026-11-02)

The user ran the numbered manual checks in this conversation in Skyrim SE.
These observations supersede the initial broad reading of the Scrolls article:

- Fireball: WASD movement, aiming/turning and sneak toggling remain available
  while charging; jump and sprint do nothing. Casting cannot start while jumping
  or swimming, but works while wading. The charge feels like a standard spell.
- Blizzard: movement stops as soon as casting starts. Early release restores
  running after a short unwind, less than half a second. Sheathing is refused;
  replacing the hand contents cancels casting. Cancellation retains the scroll.
  The full ritual charge takes approximately three seconds.
- Detect Life: does not channel continuously; releasing casts a life detection
  effect lasting approximately 10–15 seconds.

No further precision measurement or game instrumentation is requested. These
qualitative observations are enough to correct ordinary versus ritual movement,
start restrictions, and Detect Life's single release cast.

### Asset cross-check

`vrm-tool magic <Data> FireballScroll` reports ETYP 00013F44 (either hand),
SPIT charge zero, and effect FireDamageFFAimedArea with DATA casting time 0.5s.
BlizzardScroll has ETYP 00013F45 (both hands), SPIT charge 3s, and effects with
casting times 0s and 3s. TGDetectLifeScroll has SPIT cast type 2 but four
fire-and-forget effects with EFIT duration 15s. Thus its apparent failure to
channel agrees with its effects and is not by itself evidence of a game bug.

[UESP's MGEF format](https://en.uesp.net/w/index.php?oldid=3507756) identifies
DATA 0x30 as CastingTime and 0x50 as CastType. Together with the SCRL format's
longest-effect calculation, this supports using effect casting times for
zero/default scroll charge times, without precise manual timing.

The humanoid graph asset has a distinct RitualSpell state, entered through
RitualSpellStart (and an aimed concentration variant). It charges, accepts the
usual hand SpellReady/SpellRelease events, and emits both hand SpellFire events
0.57s into RitualSpell_Release. RitualSpellOut leaves the ritual state at the
release clip's end. Inspected with `hkb-vars` and `hkb-tree <Data>
meshes/actors/character 0_master.hkx RitualSpell 5`. No engine binary was inspected.

Using a both-hand scroll equip slot to select ritual behavior is an inference
from these assets and the observed Fireball/Blizzard difference; it is not an
exhaustive observation of every scroll. Precise cancellation recovery, mixed
casting types and untested concentration scrolls remain documented separately.
