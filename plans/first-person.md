# Plan: the first-person view

**Status: implementation complete; original-game probe awaiting a user run**
(2026-11-03). Steps 0–7 and 9 are implemented. Step 8's generator and scripts
are built at `tests/assets/VrmFirstPerson.esp` and
`tests/assets/Scripts/VrmFirstPersonScript.pex`; no original-game readings
have been supplied. Each implementation stage has its own formatted commit.

The implemented scope includes both rigs/graphs, record model selection,
view-selected notifications, camera placement, the separate render pass,
held spell/enchantment/torch effects, gameplay origins, first-person graph
footsteps, wheel transitions and camera locks, supported crafting furniture's
FFFP override, and native camera-save continuity. General player chair/bed
entry and exit animations remain the existing furniture work in ROADMAP.
Bow zoom's camera factor/perk mapping remains open, as step 7 permits.

Research and decisions: `research/first-person.md` and
`known_ambiguities/first-person.md`. Captures can be reproduced with
`bash scripts/first-person-captures.sh /tmp/vrm-first-person-captures` after
`cargo build`; they are engine smoke tests, not vanilla parity measurements.

The goal: in first person, draw the player's arms, hands, held weapons,
shields, torches and spells. Use the game's own first-person skeleton,
models and behaviour graph, so draws, swings, bow draws, casts and blocks
look as they do in Skyrim SE. That graph also times the gameplay events
(`HitFrame`, `arrowRelease`, casts, footsteps) in first person.

Rules as for all work here (AGENTS.md): public sources and game assets only;
no engine binaries. Assets are examined, never copied into the repository.
Choices made without a source go in `known_ambiguities/player-body.md` (or a
new `known_ambiguities/first-person.md` if that file grows too long). Open
questions stay in this plan.

## Starting point (before this implementation)

- `src/player_body.rs`: `PlayerBody` holds the race's third-person skeleton
  and one `GraphAnim` (the race's `defaultmale/female.hkx` project). It is
  built by `Engine::build_player_body` and animated by `update_player_body`.
  The graph is told `IsNPC` 0, and `i1stPerson` and `IsFirstPerson` 0 always.
- In first person, `scene.player` (the third-person body) is
  `shadow_only`. Nothing else is drawn: no arms, weapons or spell hands.
- The camera is `Engine::view_camera`: the eye (`Player::eye`, 56 units
  above the capsule's centre, dropping when sneaking) in first person. In
  third person it is pulled back over the shoulder. `fov_y` is 1.0 rad, and
  the near plane is 5 units (`render::Camera::proj`, reverse-Z infinite).
- F toggles the view (`app.rs`), and the wheel zooms the third-person
  camera only. `Game.ForceFirstPerson` / `ForceThirdPerson`, the console's
  view toggle, and `IsPC1stPerson` (condition 391) exist. So does the save
  continuity of `third_person` / distance. `engine.rs:738` already picks
  `POPM` / `P3OM` (the player's output model, 1st / 3rd person) for sounds.
- Gameplay timing: blows land at the third-person graph's `HitFrame`, and
  arrows fly at its `arrowRelease`, in both views (`player-body.md`, "Blows
  at `HitFrame`" and "Bow release timing" say this is a stand-in for the
  first-person graph). Player footsteps in first person are by distance
  walked (`footsteps.rs`, `stride`), not by graph events.
- Spell visuals hang from `NPC L/R MagicNode` of the third-person skeleton
  (`spell_fx`).
- Armor models: `world::actor::armor_models` reads ARMA `MOD2` / `MOD3`
  only. Rigid equipment (`ai::equipment::rigid_equipment`) hangs weapon
  `MODL`s from skeleton nodes by name.

## The evidence

Game assets (examined with `vrm-tool`; nothing copied):

- `Skyrim - Animations.bsa` holds a whole first-person character:
  - the project `meshes/actors/character/_1stperson/firstperson.hkx`;
  - its character `characters/firstperson.hkx` and Havok skeleton
    `characterassets/skeletonfirst.hkx`;
  - 15 behaviour files (`0_master`, `1hm_behavior`, `bow_direction_behavior`,
    `magicbehavior`, `weapequip`, `blockbehavior`, `bashbehavior`,
    `sprintbehavior`, `shout_behavior`, `horsebehavior`...);
  - 751 clips under `_1stperson/animations`;
  - `meshes/animationdata/firstperson.txt` (clip triggers) and
    `boundanims/anims_firstperson.txt` (root motion);
  - `meshes/animationsetdata/firstpersondata/*.txt`, including
    `thirdpersononlyevents.txt`, which lists two furniture idles
    (`IdleSharpeningWheelStart`, `IdleTanningEnter`).
- `Skyrim - Meshes0.bsa` holds `meshes/actors/character/_1stperson/skeleton.nif`
  (90 nodes). It has the same named attachment nodes as the third-person
  skeleton (`WEAPON`, `SHIELD`, `NPC R/L MagicNode`, `NPC Head MagicNode`,
  `WeaponSword`, `WeaponBack`, `WeaponBow`, `QUIVER`...). It also has two
  extra nodes under `NPC Root`: `Camera1st [Cam1]` and `Camera Control`.
  `Camera1st` and `NPC LookNode` both sit 120.5 units up. The rest of the
  body (`NPC Translate` at 64.1 and below) hangs under `NPC LookNode`. The
  Havok skeleton has `NPC LookNode` but no camera bone.
- First-person body and hand models live in
  `character assets/1stperson{male,female}{body,hands,handsargonian}_{0,1}.nif`.
  Armor has its own `1stperson*` models (`armor/bandit/1stpersonbody1m_0.nif`,
  `armor/daedric/1stpersondaedricshield.nif`...). Weapons have them too,
  e.g. the Creation Club's `1stpersonamberdagger01.nif`.
- `vrm-tool hkb-classes` on the project lists only classes that the
  third-person runtime already reads. `vrm-tool hkb-run` with
  `IsFirstPerson=1 i1stPerson=1 IsPlayer=1` runs it: `weapEquip` raises
  `weaponDraw` / `WeapEquip_Out`, and `attackStart` enters
  `H2H_1stPerson_AttackRight_State` and raises `preHitFrame`, `weaponSwing`
  and `SoundPlay.WPNSwingUnarmed`. The original uncovered-bone diagnostic
  was corrected during implementation to account for implicit-one mask tails.
- `vrm-tool hkb-vars` lists first-person-only variables: `1stPRot` /
  `1stPRotDamped` (`sind(Direction * 360) * RotMax * clamp(Speed, 0, 1)`),
  `blendMove1stP`, `fIsFirstPerson`, `camerafromx/y/z`, `bowZoom` /
  `bowZoomAmt` (`IsAttacking * ... iWantBlock * bowZoom`), and the aim
  variables (`AimPitchCurrent`, `AimHeadingCurrent`, `BowAimOffsetPitch`,
  `MagicAimOffsetPitch`).

Records (xEdit's `wbDefinitionsTES5.pas` in `refs/xedit`; UESP
[Mod File Format/ARMA](https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format/ARMA)):

- ARMA has a "1st Person" model pair, `MOD4` (male) / `MOD5` (female),
  beside the third-person `MOD2` / `MOD3`. UESP: "If MOD4 is not present,
  MOD2/3 are all viewpoints, if MOD4 is present, then MOD2/3 are
  specifically 3rd person while 4/5 are 1st person"; some records have only
  `MOD2`/`MOD4` or `MOD3`/`MOD5`.
- WEAP `WNAM` is the "1st Person Model Object" (a `STAT`, or null).
- DOBJ `FFFP` names the `FurnitureForce1stPerson` keyword (000A56D7). `POPM` / `P3OM` are the
  player's output models.
- CAMS has a "First Person Camera" flag (camera shots; not in this plan).
- RACE names no first-person skeleton or behaviour. Nothing found so far
  ties a race to `_1stperson/`: the path looks fixed (step 0 checks).

UESP [Skyrim:Console](https://en.uesp.net/w/index.php?oldid=3628513):
`s1st` "Shows 1st person model in 3rd person ... It shows your 1st person arms
behind your character". This confirms that the game keeps a separate
first-person model that it can draw while the third-person body is shown.

Already confirmed in SE (`autotest2`): `IsFirstPerson` follows the view in
the player's (third-person) graph.

## Design

- **Two bodies, two graphs.** `PlayerBody` gains a `first: Option<FirstPerson>`:
  - the first-person skeleton;
  - its `GraphAnim` on `_1stperson/firstperson.hkx`;
  - its meshes and rigid equipment.

  Both graphs run in both views, so switching views shows a graph that is
  already in the right state (a draw in progress, a bow held drawn). Every
  event and variable the body sends goes to both graphs. Only the *view's*
  graph supplies gameplay notifications (`HitFrame`, `arrowRelease`, cast
  events, `weaponDraw` / `WeapEquip_Out`, footsteps). The other graph's
  notifications are drained and dropped. Script animation events
  (`RegisterForAnimationEvent`) follow the same rule until the probe
  (step 8) says otherwise.
- **Graph variables.** First-person graph: `IsFirstPerson` 1, `i1stPerson`
  1, `fIsFirstPerson` 1, `IsPlayer` 1, `IsNPC` 0. Hand types, `weapAdj` and
  speed multipliers are as for the third-person graph. `Speed`, `Direction`
  and `IsSneaking`... are as they are now. Aim: `AimPitchCurrent` /
  `AimHeadingCurrent` from the view (see step 4). Third-person graph:
  `IsFirstPerson` follows the view, as `autotest2` saw. `i1stPerson` remains
  the rig identity (0 third, 1 first): changing it with the view resets a
  pending bow release. This correction to the draft is recorded in the
  decision note; the SE probe records the variable too.
- **What is worn in first person.**
  - Each worn ARMA's `MOD4` / `MOD5` (falling back across sex as `MOD2` /
    `MOD3` do), and the skin's (the race skin's `1stperson*hands`).
  - Shield ARMA without either fall back to MOD2/MOD3: IronShieldAA and
    many vanilla shields have only shared models, consistent with UESP.
  - Other ARMA without either are left out, rather than drawing their
    third-person model over the eye. This goes against UESP's "all
    viewpoints" sentence for helmets, hair and circlets. Step 0 measures
    which addons this drops, and the choice is recorded.
  - Weapons: `WNAM`'s `STAT` model, else the weapon's own `MODL`.
  - Shields and torches through their ARMA / `MODL` the same way.
- **Placement.**
  - The first-person skeleton stands at the player's feet, facing the
    camera's yaw, scaled as the body is.
  - It is moved so that `Camera1st` sits at the eye.
  - The view's pitch turns `NPC LookNode` about its local right axis. Its
    origin is the eye, and everything else hangs under it, so the arms tip
    with the view about the eye.

  The assets suggest this; no source states it (recorded as a choice).
- **Drawing.**
  - The first-person instance is drawn only in first person, and casts no
    shadow (the third-person body keeps casting it).
  - It is drawn after the world in its own pass, with a cleared (or
    range-split) depth, a near plane of about 1 unit, and its own vertical
    FOV, so weapons don't poke into walls or get cut by the near plane.
  - The FOV is 65 degrees vertically (STEP's published default number);
    SE's angular convention is unmeasured. The near plane is one unit;
    see the decision note for this projection/depth choice.
- **Gameplay origins.** In first person, spell hands, projectiles and the
  torch's light come from the first-person skeleton's `NPC L/R MagicNode` /
  `SHIELD` / `WEAPON` nodes. In third person they stay as they are.

## Steps

0. **Survey and research** (no engine change). Add `vrm-tool first-person`
   (or flags on existing commands) to report:
   - how many ARMA records have `MOD4` / `MOD5`, and which biped slots the
     ones without them cover. This settles which worn addons the
     first-person view leaves out;
   - how many WEAP have `WNAM` and whether every `WNAM` `STAT`'s `MODL`
     exists;
   - whether any RACE subrecord or other record names a `_1stperson` path.

   Run `hkb-run` through draw / swing / bow / cast / block / sheathe
   sequences, and list the notification events per clip from
   `animationdata/firstperson.txt` (`hkb-annotations`). Search UESP and the
   public INI listings for `fDefault1stPersonFOV` and other first-person
   settings (`[Display]`, `[Camera]`: `f1st...`). Write the findings to
   `research/first-person.md` with revision links. Update this plan's
   design where the survey changes it.
1. **First-person models from records.** `world::actor`:
   - `armor_models` takes a viewpoint;
   - `dress_first_person` gives the skin's and worn addons' `MOD4` / `MOD5`
     models, left out as step 0 decided;
   - `weapon_first_person_model` resolves `WNAM` → `STAT` → `MODL`.

   Ignored (asset) tests: an Imperial male's hands are
   `1stpersonmalehands_*.nif`; an Argonian female's are
   `1stpersonfemalehandsargonian_*.nif`; Banded Iron Armor or a bandit body
   resolves its `1stperson*` model; a weapon with `WNAM` gives the STAT's
   model.
2. **The first-person body.** Build the skeleton (`_1stperson/skeleton.nif`)
   and graph (`_1stperson/firstperson.hkx`, with the variables above) in
   `build_player_body`. It is reused under the same rules as the
   third-person body (`can_reuse`, equipment rebuilds keeping the running
   graph and drawn state). `FirstPerson` holds its own `world_from_model`,
   pose and rigid equipment. Rigid equipment moves between sheath and hand
   on its own graph's `weaponDraw` / `weaponSheathe`. Not drawn yet. Tests:
   - the body builds, and the graph starts in its default states with
     `IsFirstPerson` 1;
   - an equipment rebuild while drawn keeps it drawn;
   - unit tests on a synthetic graph, as `equipment_body()` does now.
3. **Both graphs hear everything.** Make `PlayerBody::event` /
   `set_variable` fan out to both graphs (the turn request and root motion
   stay the third-person graph's). Split notifications:
   `PlayerCombatEvents`, `step_events` and `script_events` come from the
   view's graph. The other graph's are drained. `IsFirstPerson`
   in the third-person graph follows the view; `i1stPerson`
   remains the rig identity. Tests:
   - a swing in first person lands at the first-person graph's `HitFrame`
     and not twice;
   - a bow shot in first person flies at its `arrowRelease`;
   - switching view during a draw neither loses nor duplicates the arrow
     (bow-release and swing state stay in `PlayerBody`, not per graph).
4. **Placement and the camera.**
   - Place the first-person skeleton (feet, yaw, scale, `Camera1st` at the
     eye) and turn `NPC LookNode` by the pitch. Re-check the eye height
     against `Camera1st`'s 120.5 × the race height; `EYE_OFFSET` is now
     chosen.
   - Feed `AimPitchCurrent` / `AimHeadingCurrent` (and the bow / magic aim
     offsets, if `hkb-vars` shows the graph reads them in first person).
   - In first person, `player_aim` stays the eye ray.

   Tests: with a bind pose, `Camera1st` lands at `Player::eye()`; the
   pitched rig keeps `Camera1st` fixed while the hands move.
5. **Drawing it.** Add `scene.first_person: Option<ActorInstance>` (the
   same instance type: skinning, skin tone, weight, lights). It is drawn in
   first person only, after the opaque world, in a pass with its own
   projection (near ≈ 1, the FOV from step 0) and a depth cleared for it. It
   is excluded from shadow casting, culling and `instances_on_ray` picking.
   Exclude `scene.player` from the main pass in first person, as
   `shadow_only` does now. Smoke-test offscreen, using the screenshot hooks
   (memory: remote testing) in WhiterunBanneredMare:
   - sheathed (arms out of view);
   - an iron sword drawn;
   - a bow drawn;
   - flames in both hands;
   - a torch and a shield;
   - sneaking;
   - looking straight down.

   Screenshots stay under the scratchpad.
6. **Gameplay from the first-person body.**
   - In first person, spell visuals (`spell_fx` cast sources), projectile
     origins (spells, arrows), the torch's held light and the shield's
     block use the first-person skeleton's nodes.
   - Player footsteps in first person come from the first-person graph's
     foot events if step 0 finds them. Else the stride stays, and the
     ROADMAP note on "the first-person stride" is updated.
   - Update `player-body.md`: drop "timed by the third-person graph in
     first person too" and the bow release note's stand-in sentence.
7. **View switching.**
   - The wheel scrolling in from `THIRD_PERSON_MIN` goes to first person,
     and scrolling out in first person goes to third person at the minimum
     distance (from memory of the game; checked in step 8).
   - Disabled camera switching blocks both.
   - Furniture with the keyword named by DOBJ `FFFP` forces first person while the
     player uses it, and restores the view after.
   - `bowZoom`: the graph computes `bowZoomAmt` (blocking with a bow
     drawn). Narrow the first-person FOV by it if step 0 finds the zoom
     factor's source (the Eagle Eye perk's entry point, a game setting).
     Otherwise leave it, as an open question.
   - Tests for the toggle rules and save continuity.
8. **Skyrim SE probe** (`vrm-tool test-mod firstperson`; the user runs it;
   result pages ≈ 20 lines each, driven by `set <global> to N` polling, no
   SKSE). Scripted:
   - which `RegisterForAnimationEvent` events (`weaponSwing`, `HitFrame`,
     `FootLeft`, `arrowRelease`, `BeginCastRight`, `weaponDraw`) reach the
     player's script in first vs third person, and the real time from
     `Debug.SendAnimationEvent`-free inputs (the user swings) to each.
     This tells whether scripts hear the first-person graph, the
     third-person graph or both;
   - `IsPC1stPerson` across `Game.ForceFirstPerson` inside an `FFFP`
     furniture.

   Observed by the user, with the probe's message box listing what to look
   for:
   - whether the player's shadow shows in first person;
   - whether the wheel moves between the views as step 7 assumes;
   - whether sheathed arms are visible;
   - whether a helmet or hood shows at the view's edge.

   Record the readings in `research/first-person.md` and resolve the
   matching ambiguities.
9. **Documentation.**
   - ROADMAP: move the first-person part of "The player's body" to Done.
   - `known_ambiguities/player-body.md`: the choices (left-out addons,
     LookNode pitch, FOV, notifications from the view's graph, wheel
     rules).
   - This plan's status.

## Open questions

- Which graph scripts hear in first person, and whether the game sends
  every event to both graphs (step 8). `thirdpersononlyevents.txt` suggests
  a per-event filter for at least two furniture idles. How the game uses
  that list is unknown.
- How the game positions the first-person rig: at the camera, at the feet,
  with pitch at `NPC LookNode` or `Camera Control`. What `Camera Control`
  and the `camerafromx/y/z` variables are for.
- The SE conversion of the published 65-degree first-person FOV default,
  and the original viewmodel depth algorithm.
- The ARMA `MOD2`/`MOD3`-only case: UESP says those models are "all
  viewpoints". The survey established that shields need the shared-model
  fallback; suppression of other addons still needs original-game evidence.
- Bow zoom's factor (`bowZoom` is 1 by default in the graph) and its perk.
- Mounted first person (`horsebehavior.hkx`) and werewolf / vampire lord
  first person (`ForceThirdPerson` on transform): out of this plan. Horses
  and beast forms aren't playable yet.
- Killmoves and other camera-controlled sequences (CAMS "First Person
  Camera"): out of this plan.

## Running the SE probe

Use a throwaway save with `VrmFirstPerson.esp` and its generated script.
The user installs/runs these; the generator never writes to the game's Data.
Set `VrmFirstPersonStep` to 1–6 to arm first/third-person sword, bow and spell
windows. Perform one action with normal controls in each window, including
walking/drawing if testing those events. Each phase keeps at most twenty event
lines; only its first FootLeft is listed and counts toward them, and the phase's
FootLeft total is appended on leaving it. Rearming a phase replaces its earlier
report. Times are real seconds
from arming, with HitFrame also timed relative to weaponSwing. Exact input-key
timestamps are unavailable through the vanilla APIs used here.

Step 7 spawns a chair using Skyrim.esm's actual FFFP keyword. Sit in it
manually, set the step to 9 while seated, stand, then set it to 10; both
readings use only vanilla console commands (no SKSE `cqf`). Step 8 saves/stops. Copy saves to `tests/results/` and read
`vrm-tool test-results tests/results`. Report shadows, wheel transitions,
sheathed arms and helmet/hood visibility separately. No result has yet been
recorded as vanilla SE behavior.

## Validation (engine implementation)

- Engine tests: 624 passed, 417 ignored (the existing asset/GPU fixtures are
  opt-in). Run outside the restricted software-GPU sandbox.
- First-person opt-in checks: five passed, including actual rig/model loading,
  retained equipment state, drawn-bow arm poses and exactly one arrow across
  a view switch, and furniture/control locks.
- Camera-save continuity runtime check: passed.
- Havok tests: 127 passed. Tool tests: 55 passed, three ignored; the final
  probe source also compiled and `test-mod firstperson` generated its ESP/PEX.
- Eight offscreen Bannered Mare cases rendered and were inspected: sheathed,
  sword, held bow, two equipped Flames hands, torch, shield, sneaking and
  looking down. Captures/logs: `/tmp/vrm-first-person-captures-bowfix/`.
  The final bow-release capture/log confirms nineteen of twenty arrows remain
  after a release and immediate switch to third person.
- `cargo fmt --check` and workspace compilation pass. The sandbox's software
  adapter cannot run the existing terrain pipeline (17 sampled textures,
  limit 16); GPU tests/captures passed with hardware access. No renderer
  feature/limit was relaxed to make those checks pass.

Original-game execution and observations remain pending, as documented above.
