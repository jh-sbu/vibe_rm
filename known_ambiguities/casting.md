# Casting

Implemented in `src/casting.rs` (the player's hands, powers, magicka,
NPCs' combat spells) and `src/ai/casters.rs` (casters keeping their
distance and casting), with spell projectiles flying as arrows do in
`src/ai/archery.rs` and their impacts in `src/impacts.rs`. Sources: UESP's Magic Overview, Magicka and
Spells pages; the records (`SPEL` `SPIT`, `MGEF` `DATA`, `PROJ` `DATA`,
`EXPL`); the game settings in Skyrim.esm; the character behaviour graphs
(`magicbehavior.hkx`, read with `vrm-tool hkb-tree` / `hkb-run`) for the
hands' casting events.

## Settled

- **Charge times** are the spell's `SPIT` charge time (Firebolt 0.5 s,
  Flames 0); fire-and-forget spells let go sooner aren't cast.
- **Cost** is `crate::spells::spell_cost` (paid when cast; concentration
  spells by the second while held), including the school's Fortify cost
  reduction (`known_ambiguities/magic-menu.md`).
- **Magicka comes back** at the race's `MagickaRate` (percent of the most a
  second) times `MagickaRateMult` (base 100, as `SpeedMult`), times
  `fCombatMagickaRegenRateMult` (0.33) while fighting.
- **Projectiles**: speed, gravity and range from the effect's `PROJ`
  (Firebolt: a missile, 2500 units/s, range 10000). Flames are a flame-type
  projectile with a range of 768.
- **Area**: an effect's area is in feet; Fireball's 15 feet match its
  explosion's radius of 320 units (21.3 units a foot).
- **Messages**: `sMagicCastInsufficientMagicka`, `sMagicCastPowerUsed`.
- **Casters' combat styles**: `CSGD` holds the melee and magic score
  multipliers after the offensive, defensive and group offensive ones
  (csWarlock: melee 0.25, magic 2.15; csHumanMeleeLvl1: 1 and 1).
- **NPCs' magicka** doesn't come back while they cast
  (`bRegenNPCMagickaDuringCast` false).
- **The hands in the graph** (`magicbehavior.hkx`): `BeginCastRight` /
  `BeginCastLeft` take the graph from `MagicReadyState` to `CastingState`
  and `CastStop` back. There each hand's state machine
  (`MRh_ReadyCast_Behavior`, `MLh_...`) waits in its ready state for the
  kind of cast: `..._SpellAimedStart`, `..._SpellSelfStart`,
  `..._SpellAimedConcentrationStart`, `..._SpellSelfConcentrationStart`,
  `..._WardStart`, `..._SpellTelekinesisStart`. A fire-and-forget hand
  pre-charges and loops its charge until `..._SpellReady_Event`, holds it
  ready until `..._SpellRelease_Event`, and its release clip raises
  `..._SpellFire_Event` (Firebolt's `MRh_Release`: at 0.17 s) and, at its
  end, `..._Equipped_Event`, the hand's wildcard back to ready. Concentration
  and ward hands go from their charge loop to their casting loop on
  `..._SpellRelease_Event`. The engine listens for `MRh_SpellFire_Event` /
  `MLh_SpellFire_Event` (CommonLibSSE's `FixedStrings`). The hand types are
  9 with a spell in hand (`test-mod autotest2`).
- **Dual casting** needs the school's dual casting perk: `AlterationDualCasting`
  .. `RestorationDualCasting` set entry point 75 (`CanDualCastSpell`) to 1
  for spells of their skill (`EPMagic_SpellHasSkill`). A dual cast costs
  `fMagicDualCastingCostMult` (2.8) times as much and charges
  `fMagicDualCastingTimeMult` (1) times as long.
- **Dual casting's power** (`test-mod dualcast`, 2026-10-18): a flat
  `fMagicDualCastingEffectivenessBase` (2.2), whatever the effect's dual
  casting scale (`MGEF` `DATA` 0x70: scales 0, 1, 1.5 and 3 all dual cast
  50 points to 110), so the scale isn't about power. It multiplies the
  duration of effects flagged 0x400000 (`kPowerAffectsDuration`: 4 s to
  8.3 s, the magnitude unchanged) and the magnitude of every other effect,
  flagged 0x200000 (`kPowerAffectsMagnitude`) or not (an effect with
  neither flag: 50 to 110). A spell flagged `SPIT` 0x800000
  (`kNoDualCastMods`) dual cast lands twice its single magnitude (50 to
  100): two casts' worth without the bonus. In vanilla that flag is on the
  two-handed master spells (Fire Storm, Blizzard, Mayhem, the thralls).
  Alteration's spells mostly gain duration this way: the flesh spells'
  `ArmorFFSelf...` effects are flagged for duration (UESP: Oakflesh's 60 s
  become 132 s dual cast, 198 s with Stability), Transmute's for neither
  (UESP: no effect), Detect Life and Telekinesis for magnitude. Of player
  spells' effects, Alteration's are 29 duration / 9 magnitude / 1 neither,
  Conjuration's 38 / 10, Destruction's 67 magnitude, Illusion's 32 / 21,
  Restoration's 15 duration / 23 magnitude / 44 neither / 1 both.
- **Dual casting in the graph** (`magicbehavior.hkx`): the left hand's
  state machine takes `DualMagic_SpellAimedStart`, `..._SpellSelfStart`,
  `..._SpellAimedConcentrationStart`, `..._SpellSelfConcentrationStart` and
  `..._WardStart` from any state into `Dual_Cast` (there is no telekinesis
  one) and from there runs on the left hand's events (`MLh_SpellReady_Event`,
  `MLh_SpellRelease_Event`); its release clips raise `MLh_SpellFire_Event`
  (`DMagAimRelease`: at 0.17 s) and `MLh_Equipped_Event`. The right hand's
  machine stays in its ready state.

## Open (choices made here)

- **Regeneration delay**: magicka waits `fDamagedMagickaRegenDelay` (0.5 s)
  after being spent, and doesn't come back while a spell is held ready or
  flowing (`bRegenNPCMagickaDuringCast` is false for NPCs; assumed for the
  player too).
- **Starting a spell**: a fire-and-forget spell needs its whole cost to start
  charging and when cast; a concentration spell starts with 1 magicka or
  more and stops when a frame's cost can't be paid. Whether the game checks
  the cost at the press or the release isn't published.
- **Concentration landings**: a concentration spell lands on what it reaches
  every 0.25 s. Its effects (Flames: 8 a second for 1 s) are replaced each
  time, so the damage runs on at the effect's rate; how often the game
  applies them isn't known.
- **Target actor** spells reach the spell's `SPIT` range (4096 without one);
  contact spells 160 units.
- **Target location** spells without a projectile land where the view meets
  the world (within 4096 units), on the actors in their area.
- **Area** (`test-mod autotest4`, `autotest5`): Fireball cast by an NPC at
  an NPC struck that one and, of NPCs measured before the blast, one 279
  units off to the side, not those 95, 152, 221 and 253 off behind it (the
  struck one's body between), nor any 335 to 490 off, nor the caster. In
  `autotest4`'s line of NPCs, each behind the last, the third was spared.
  The blast pushed the sheltered ones all the same.
- **Area** spells strike every live actor whose body's middle is within the
  radius (the player too when someone else casts it) and in sight of where
  it went off, nothing solid and no body (the struck one's too) between;
  never the caster. A hostile spell delivered on oneself with an area
  (Battle Cry) lands on the others around but not on the caster.
- **Skill XP**: the spell's base cost (`SPIT`) times its school's effect's
  skill usage multiplier, through the skill's `AVSK`; hostile spells give
  it for each actor they land on, the rest when cast; concentration spells a
  quarter of it each landing (a second's worth a second). UESP says the
  base cost decides; whether it is the cost before or after the record's
  auto-calculation isn't clear.
- **Greater powers** come back a game day after they were used (a day of
  game time, not the next day); lesser powers whenever.
- **Who casts in combat**: an actor whose combat style's magic multiplier
  is above its melee one (or equal to it, holding no weapon: a flame
  atronach's are both 1), with a hostile aimed or target actor spell it has
  the magicka for (a second's worth of a concentration spell), fights with
  the costliest of them; otherwise as everyone else. The game weighs every
  weapon and spell by its scores; that isn't public.
- **How casters fight**: they close to 85% of the spell's range (at most
  2000 units, as archers) with a clear line, stand facing the target and
  throw a spell every 1.5 to 3 seconds, or channel a concentration spell
  for 1.5 to 3 seconds, aimed at the middle of the target's body. Humanoids
  charge a thrown spell for its charge time in their graph; creatures
  (atronachs, whose graphs have no hands' casting) throw at once.
- **The player is struck** by flames, beams and target actor spells that
  pass within their capsule (they aren't in the physics world).
- **The Story Manager's spell cast event** (`CAST`) is sent for the player's
  spells with the caster as `R1` and the spell as `F1`; which members the
  game fills isn't published.
- **Which kind of cast** (`cast_start_event`): self-delivered spells cast
  as self, everything else (contact, target actor and target location too)
  as aimed, fire-and-forget or concentration by the spell's casting type;
  wards by their first effect's archetype (Accumulate Magnitude, on
  `WardPower`) and telekinesis by its archetype, both self-delivered
  concentration spells in the records. The records carry no casting
  animation, and how the game picks isn't published.
- **An empty hand beside a spell** is a spell's (9) to the graph, as both
  hands' types: only the left hand's spell and staff states
  (`1HM_Readied_BehaviorGraph`, its start bound to `iLeftHandType`) reach
  the magic behaviour, so a spell in the right hand alone would otherwise
  have none. `test-mod autotest2` read 9 with spells in both hands; a spell
  in one hand alone wasn't measured.
- **Hands the graph has no casting for**: a spell beside a weapon, shield or
  torch (`iLeftHandType` 1 to 7, 10 to 12) leaves the graph outside the
  magic behaviour; the graph refusing the start, the spell is cast as with
  no graph, without animation. How the game animates these isn't known.
- **When the hands are told** (the player): `BeginCast...` then the start
  at the press, both only once the draw is over (`WeapEquip_Out`). `..._SpellReady_Event` the
  update after the charge time is reached (or at the release, if that
  comes first), and a concentration spell's `..._SpellRelease_Event` the
  update after it starts, when the hand's state machine runs. A
  concentration spell lands from the press, not from the end of its
  pre-loop clip.
- **Cast at `SpellFire`**: a charged fire-and-forget spell let go is cast,
  and paid for, at its hand's `..._SpellFire_Event`; with no graph (or
  before it has one) at once, as before. The graph refusing the release, or
  no `SpellFire` within 1.5 s, casts nothing. Death in between casts
  nothing.
- **A hand stopping**: let go before it charged, a concentration spell
  ended, magicka run out, the spell unequipped or the hands put away, the
  engine sends the hand's own `..._Equipped_Event` to bring it back to
  ready (the graph has no cancelling event); when neither hand is casting
  or playing out a release (its clip's `..._Equipped_Event`, or 1.5 s),
  `CastStop`.
- **Dual casting's power, the rest**: effects flagged for both magnitude
  and duration (Circle of Protection, the Dragonborn teleport FX) gain only
  duration here, as the measured rule reads; not measured. With
  `kNoDualCastMods` the power is taken as 2 for duration effects too, and
  as one effect of twice the magnitude (the game may instead land each
  hand's cast, stacking). What the dual casting scale does (perhaps the
  dual casting art's size, which the `DUAL` record's "inherit scale" flags
  suggest) isn't known; it isn't read. The power goes before the caster's
  and target's perks, and dual cast calm, fear and frenzy reach higher
  levels. Skill XP is a single cast's; whether a dual cast teaches more
  isn't published.
- **Starting a dual cast** (the player): the second hand pressed within 0.2 s
  of the first, holding the same spell, with the perk and the magicka for
  the dual cost; otherwise each hand casts its own. The first hand's cast
  becomes the dual cast as it stands (what a concentration spell landed
  in that moment stays). Letting go of either button lets it go; a hand
  losing the spell ends it. The window isn't published.
- **Casters' hands** (`ActorRuntime::set_hand_vars`): while an NPC fights
  with a spell its graph's hand types say a spell is in hand (it holds none
  in its inventory): the left hand beside a one-handed weapon in the right
  (`BeginCastLeft`), else both hands, casting with the right. The graph
  follows the types while drawn (`RootBoneSwitch_MSG` re-selects), so no
  redraw. Which hand the game's NPCs ready their spells in isn't published.
- **Casters' events** follow the player's: the start once the hands are out
  (`weaponDraw`; refused, again 0.5 s later), `SpellReady` after the charge
  time, `SpellRelease` the update after, the spell cast at `SpellFire`
  (none within 1.5 s: nothing cast), `CastStop` when the release clip ends
  (or 1.5 s); a concentration spell flowing the update after it starts and
  stopped by its hand's `Equipped` and `CastStop`. A thrown spell keeps the
  recipient it was started at.
- **Casters dual casting**: a caster with both hands free (no one-handed
  weapon) whose perks let it dual cast its spell (`CanDualCastSpell`; in
  vanilla only Destruction Dual Casting, kept by some named mages and the
  Forsworn magic templates: most warlock bosses list it but their template's
  spell list replaces it) dual casts half its casts (each start, and each
  retry after the graph refused one, rolled again), as the player does on
  the left hand's state machine; the graph refusing `DualMagic_...Start`,
  it casts with one hand. Paid 2.8 times, or cast singly when it can't pay
  that. The game's choice (`CombatBehaviorPrepareDualCast`) isn't
  published; the half is a guess. Its charge time isn't lengthened
  (`fMagicDualCastingTimeMult` is 1).
- **`EffectWasDualCast`** holds for an effect of a dual cast spell, when it
  lands and when its conditions are looked at again. Impact's stagger
  (`PerkImpactStaggerPushFFAimed` on Destruction's aimed spells) lands only
  so, and with the caster's `Impact` perk (both conditions run on the
  caster); dual cast, its magnitude (0.25 on Firebolt) gets the 2.2 too, as
  any effect without the duration flag.
