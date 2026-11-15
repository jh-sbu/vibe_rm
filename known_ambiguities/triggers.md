# Trigger volumes

Implemented in `src/triggers.rs`; `vrm-tool triggers <data>` counts primitives,
their bases and scripts.

## What is known

- `XPRM` (UESP): half extents x, y, z, colour, a float, type (1 box, 2 sphere,
  3 portal box, 4 plane). Of 20077 references with a primitive, 5286 have an
  activator base; most of the rest are room bounds and occlusion (statics,
  portal boxes) with no script. Common scripts: `defaultsetStageTrigSCRIPT`
  (453), `WICommentTriggerScript`, `defaultAddMusicSCRIPT`, `WETriggerScript`,
  `defaultStartSceneTrigScript`, enable / disable linked ref triggers.
- Phantoms (`bhkSimpleShapePhantom`, collision layer 12, trigger): 185 in
  the vanilla meshes; a scripted reference with one is a trigger volume of
  its shapes. Pressure plates, oil pools, trip wires.
- Activate parents (`XAPR`: parent form, delay in seconds; `XAPD` flags, 1
  "Parent Activate Only", per UESP): the CK's Activate Parents. Trap trigger
  scripts block their own activation and unblock it around `Activate(self)`,
  so passing activation on to children is taken as default processing,
  which blocking stops; children get the parent as their activator (the trap
  scripts read `TriggerType` off it).
- Scripted box and sphere volumes send `OnTriggerEnter` / `OnTriggerLeave`
  (with the actor as `akActionRef`) to the reference's scripts and the
  aliases it fills; `GetTriggerObjectCount`.

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

A box of half extent 128 with a script logging its events:

- An NPC entered it standing with its centre 20 and 10 units outside a side
  (and at 0), not 40: about the 20 units of reach used here.
- An NPC killed inside left it; an iron sword placed in it entered nothing;
  the player entered and left; NPCs deleted inside left it.
- Disabled, it sent `OnTriggerLeave` for the NPC inside, and
  `OnTriggerEnter` again when enabled.

## Choices made without a source

- Phantom volumes reach a body within 20 units of one of its three points,
  as primitives do; a phantom on an animated node doesn't move with it.
- Who sets them off: the player and loaded, living actors, tested at three
  heights over their feet with 20 units of reach (a capsule, roughly).
  Projectiles don't.
- Actors already inside when the cell loads enter on the first update.
- A script moving a trigger doesn't move its volume.
- `OnTrigger` (41 scripts: `TrapPoisonGas` and `DLC1LD_LavaTriggerScript`
  place an explosion on each one inside that hasn't the effect yet, the
  fire plates and pressure plates count who stays): the CK wiki's "about
  twice a second" read as every 0.5 s of real time, the clock starting when
  the first one steps in, one event for each one inside, the first an
  interval after entering (not at once). The wiki gives no exact rate, and
  whether `OnTriggerEnter`'s frame also sends one is not said. Who sets it
  off is the same as for entering.

- **Placed hazards** (`PHZD`, `src/hazards.rs`): the radius (`HAZD`
  `DATA` 0x04) read in feet, as spells' areas are (3 for
  `FireLgPlacedHazard`, 40 for the Throat of the World's blizzard), and
  scaled by the reference's scale (the Ratway fire's 0.68 makes 44
  units); reached from three heights over a body's feet, as triggers'.
  Each interval (0x10) gives the spell again, replacing the last, quietly
  (no `OnHit`, no fight), with the hazard as the caster. Unused: the limit
  (`FireLgPlacedHazard`'s is the float 1.0), lifetime (placed ones never
  end), image space radius, light, impact data set, sound and model (the
  placed fires' flames are separate statics: whether the hazard's
  `FXFire01New.nif` is drawn too is unknown), drop to ground, align to
  normal. Disabled hazards do nothing.
- **Spawned hazards** (Spawn Hazard effects, archetype 40: Blizzard,
  Circle of Protection, Guardian Circle; `Engine::spawn_effect_hazard`).
  Sources: UESP *Skyrim Mod:Mod File Format/HAZD* (revision 2308972: the
  flags "Inherit Duration from Spawn Spell" 0x2, "Inherit Radius from
  Spawn Spell" 0x8, "Drop to Ground" 0x10); CommonLibSSE's `Hazard`
  (an owner actor, lifetime and magnitude) and `SpawnHazardEffect` (a
  hazard handle, overriding start and finish but no update). Choices:
  - It is left where the spell landed, else at its target's feet, and
    stays there (no update to follow its caster). It lasts the effect's
    duration with 0x2, else its record's lifetime; reaches the effect's
    area with 0x8. It outlasts the effect if the record says so: whether
    the effect's end removes it isn't known.
  - Its touches are its owner's (the caster): conditions run against the
    owner, kills are theirs. Still quiet (no fight from a touch). A
    hostile hazard spell spares its owner, as area spells spare their
    caster: UESP's *Skyrim:Blizzard* (revision 3426842, notes) warns it
    hurts the caster's friends and that it harms only hostile targets;
    neither says the caster is hurt. The friends rule is left to the
    spell (nothing spares them here).
  - Its spell's magnitudes are scaled by what the spawning effect gained
    over its authored magnitude (perks, dual casting). UESP's *Skyrim:Circle
    of Protection* (revision 3483831) gives dual casting 220% of the levels turned and
    Necromage a larger cap; Blizzard's Augmented Frost raises its 20
    damage to 25 or 30. That the gain carries this way (CommonLibSSE's
    hazard `magnitude`) is inferred.
  - Drop to ground and align to impact normal aren't used.
- **Hazards placed by script** (`PlaceAtMe` of a `HAZD`: Guardian
  Circle's `MagicPlaceHazard`): live at once, owned by the actor placed at
  (or its actor cause), until disabled or deleted; no lifetime.
- **Water and lava**: water "damage" is the `WATR` `FNAM` flag 0x1
  (CommonLibSSE `kCauseDamage`, `GetDangerous`), which the
  `IsInDangerousWater` condition (332) reads; the water does no damage of
  its own (`DATA` is 0 everywhere, no `XNAM` contact spell). Vanilla sets
  it only on `DLC2ApocryphaWater` and `DLC2ApocryphaWaterSmall`, whose harm
  is an ability: `DLC2abApoWaterDamage` (in Apocrypha) gives
  `DLC2abApoWaterDamageActual`, with separate effects conditioned on
  swimming (70 magnitude) and dangerous water (25 magnitude). Mods use the flag as a water
  detector: Wade In Water and needs mods (SunHelm's waterskins and
  drinking) want every water type flagged, through xEdit / zEdit patchers
  or an SKSE utility setting it at load. Loaded water types and the
  dangerous-water condition are now modelled in `src/water.rs`; swimming,
  unloaded actors' contact and the contact spell remain open. xEdit marks
  `WATR` `DATA` unused in SSE. See `water.md` for sources and choices.
  The Soul Cairn's lava is `DLC1LD_LavaTriggerScript` on
  trigger volumes, which run.

## Open questions

- Collision layers of trigger volumes (`XTRI`, `BGSCollisionLayer`): which
  objects each kind of trigger reacts to.
