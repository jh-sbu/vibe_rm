# Plan: explosions

**Status: done** (2026-11-11; steps 0–9, with follow-up fixes to step 3's
knockdown rule and to `add_objects`); `test-mod explosions` awaits the
user's run in SE. See "Outcome" at the end. ROADMAP item 5 (Game logic),
"Destructible object damage and lifecycle: ... Explosions (`EXPL` damage
within their radius) and self damage; the stage's ... explosion", and the
call sites other plans left for them: explosion knockdowns through
`Engine::knock_down` (`plans/living-ragdolls.md`), explosions' noises
(item 3, Detection), runes (item 5, magic).

Research: `research/explosions.md` (xEdit, UESP, CommonLibSSE's names, the
census). Choices where the sources are silent:
`known_ambiguities/explosions.md`.

Rules as for all work here (AGENTS.md): public sources and game assets only;
`vrm-tool explosion-census` reads plugin records only.

## What an explosion does here

An explosion goes off at a point, with an owner (the actor it is blamed on,
if any) and a source (the spell, weapon or projectile behind it). At once:

- **Its targets:** living actors (the player too) whose body's middle is
  within its radius and, unless "Ignore LOS Check", in sight of its centre;
  loaded destructible references within it the same way.
- **Damage:** `DATA` damage to each actor's health, and to each
  destructible reference.
- **Enchantment** (`EITM`): applied to each actor reached, cast by the
  owner.
- **Force:** dynamic bodies and corpses within the radius are pushed away
  from the centre; living actors are knocked down when the flags say so.
- **Being noticed:** each actor reached gets `OnHit` (the owner as
  aggressor; the explosion, or the spell behind it, as source) and, if
  hurt by an owner, a fight as from a blow; the owner makes a noise of the
  explosion's sound level.
- **What it shows:** its model (as an effect, one shot), its light, its two
  sounds, its image space modifier for the player within the IS radius, its
  impact data set on the surface beneath, its placed object, its spawned
  projectile.

Explosions are set off by: projectiles with one landing (spells, arrows and
bolts; a dual cast's `DUAL` explosion in place of the projectile's), spells
landing whose effect names one (self-area spells, shouts), destruction stages
entered, `PlaceAtMe` of an `EXPL`, and projectiles with an alternate trigger
(runes: proximity or timer).

## Steps

Each numbered step is one commit, in this order.

0. **Research and census** (this plan, `research/explosions.md`, `vrm-tool
   explosion-census`).
1. **The record** (`src/explosions.rs`, `ExplosionData::load`): every `DATA`
   field and flag, the 52- and 48-byte layouts (the 48-byte one without the
   vertical offset mult); shorter ones keep their model and sounds but no
   force, damage or radius. The enchantment and image space modifier. Unit
   tests on built records.
2. **Going off** (`Engine::explode`): the targets (radius, line of sight,
   the source reference spared), damage to actors and destructibles,
   the enchantment, `OnHit`, fights and assaults as from a blow by the owner,
   the noise. Explosions set off while one is going off (a destruction stage
   entered by its damage) wait for the next frame, so chains can't recurse.
   Engine tests.
3. **Force and knockdowns:** dynamic bodies' impulses, corpses' ragdolls,
   "Knock Down - Always" and "By Formula" through `knock_down`, "Push
   Explosion Source Ref Only" (`fExplosionSourceRefMult`). Tests.
4. **What it shows:** the model as a one-shot effect (`load_spell_model`,
   oriented to the world or to where it landed), its light, sounds 1 and 2,
   the image space modifier within the IS radius, the impact data set on
   the surface beneath. An offscreen capture of one.
5. **What it leaves:** its placed object (hazards through `spawn_hazard`,
   explosions going off in turn, other forms created there), and explosion
   references: `PlaceAtMe` of an `EXPL` sets it off at the reference and
   returns a temporary reference of it, removed once it is over (others get
   one too, for what they place). Tests.
6. **Setting them off:** spell projectiles and arrows landing (the
   projectile's explosion, the `DUAL` one when dual cast), spells landing
   with an effect's explosion (when their projectile has none), destruction
   stages entered (the stage's explosion, blamed on the reference's actor
   cause). Tests.
7. **Runes and other alternate triggers:** projectiles with "Alt. Trigger"
   (`PROJ` flag 0x4) stop where they land and go off when an actor comes
   within the proximity (`DATA` 0x1C) or the timer (0x20) runs out; the
   spell's effects land where they go off. Tests.
8. **`test-mod explosions`:** an SE batch for the choices in
   `known_ambiguities/explosions.md` that a script can read (damage with
   distance and armor, line of sight, `OnHit`'s source, knockdowns by
   formula, the `PlaceAtMe` reference's life).
9. **Close:** ROADMAP Done and Next, the plan's outcome.

## Left out

- Chain Lightning's leaps ("Chain", bit 7): nothing public says how the next
  target is chosen or how often it leaps.
- The spawn projectile: one record has one (`ChainLightningMassExplosion` →
  `ShockBoltAimChain`, flags Chain), and nothing uses that record; what the
  projectile would carry and where it would fly is unknown.
- Exploding and severed limbs (`BPTD` explodable / severable explosions):
  with killmoves and dismemberment.
- Water splashes (`fExplosionSplashRadius`, `fExplosionWaterRadiusRatio`,
  the "In Water" flag) and controller vibration.
- The 44- and 40-byte `DATA` layouts (six records): their order isn't
  known.
- NPCs avoiding explosions and runes (`fCombatThreatProximityExplosionAvoidTime`,
  the combat style's threat avoidance: `plans/ai-depth.md`, blocked).

## Outcome

All steps are implemented, except step 5's spawn projectile (moved to Left
out: its one record is unused). Also found and fixed on the way:
`add_objects` dropped a cell's runtime when the scene had no render cell
for it; explosion models name their one sequence for its use, so the first
plays when there's no `Idle`. Left for the SE batch (`test-mod
explosions`, see its module doc for the readings our engine predicts):
damage by distance and armor, shading, `OnHit`'s source and aggressor, the
"By Formula" rule, the `PlaceAtMe` reference's life. Its dry run in our
engine found two open questions outside explosions (ROADMAP, Test
plugins). Also left out: the rune's decal (`PROJ` `DATA` 0x54), the Light
Foot perk, and runes' crime and re-targeting rules (UESP's Rune (effect)).
