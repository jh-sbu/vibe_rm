# The skills menu (statsmenu.swf, levelupmenu.swf)

Implemented in `src/swf_ui/stats_menu.rs` (the movies' contents and keys) and
`src/stats_view.rs` (the sky and the perk trees' stars behind them, as the
map has its own scene).
Sources: the movies' ActionScript (`StatsMenu`, `AnimatedSkillText`,
`LevelUpMenu`: the `SetDescriptionCard`, `SetPlayerInfo`, `UpdateSkillList`,
`SetDirection`, `HideRing`, `SetStatsMode`, `SetPerkCount`,
`ShowLegendaryButtonHint` callbacks, the static `StatsMenu.SkillStatsA` and
`PerkNamesA`, `UpdatePerkText`, the `MoveCamera` / `SetFadedIn` calls, the
level up popup's `addHealth` / `addMagicka` / `addStamina`); CommonLibSSE's
`StatsMenu` (its three models: `skydomeNode`, `starsNode`, `linesNode`; the
camera's sequences; an effect shader and a state for each of the 18 trees)
and `BGSSkillPerkTreeNode` (the tree nodes' grid cells and offsets); the
skydome in `Skyrim - Meshes1.bsa` (`Interface/INTPerkSkydome.nif`: its
constellation images `cImage00` to `cImage17` from
`INTConstellationImages.dds`, its `Point00` to `Point17` nodes, its
`CameraIntro`, `CameraOutro` and `Idle` sequences; `vrm-tool
nif-tree` prints where they stand and their bounds); the star and line models
(`Interface/INTPerkStars01.nif`, `INTPerkLine01.nif`) and their behaviour
graphs in `Skyrim - Animations.bsa` (`InterfaceBehaviors/Behaviors/
StarBehavior.hkx` and `Line/Line.hkx`, named by
`meshes/animationdata/intperkstars01.txt` and `intperkline01.txt`); the game settings
`sLegendaryResetConfirm`, `sLegendaryResetSecondConfirm`,
`sRequirementsText`; `Interface/Controls/PC/controlmap.txt` (its Stats
context binds only `Rotate`, to the right stick).

## Settled (by the movies)

- The movie only draws; the game keeps the selection and input. The ring is
  18 skills, 20 degrees each (`SKILL_ANGLE`); `SetDirection(angle)` turns it
  and the skill at `angle / 20` is the centre one.
- The movie lays the skills out left to right in `SkillStatsA`'s order,
  which the game fills. Its constants (`ALTERATION` 0 to `HEAVY_ARMOR` 17)
  are declared but never read, so they don't give the ring's order.
- **The sky is the skydome, on the ring too.** `StatsMenu` keeps only the
  skydome, the perk stars and the perk lines (no slot for
  `INTStatMenuStars.nif`, a model of twelve constellations, three of whose
  textures aren't shipped). The skydome's 18 constellation images and 18
  `Point` nodes stand around its origin 20 degrees apart (image *n* at *n*
  times 20 degrees clockwise from +Y, seen from above), the ring's 18 skills
  at `SKILL_ANGLE`.
- **The ring's order** is the skydome's pictures, image 0 up, left to right:
  Enchanting (a runed sword), Smithing (an anvil), Heavy Armor (a helmet),
  Block (a shield), Two-handed, One-handed (a sword in a fist), Archery (a
  bow), Light Armor (linked rings), Sneak (a snake), Lockpicking (picks),
  Pickpocket (a hand taking a coin), Speech (two faces), Alchemy, Illusion,
  Conjuration (a gate), Destruction (flames), Restoration (a bird),
  Alteration. Read from the pictures' art, then checked in Skyrim SE
  (2026-10-23): each skill shows its picture, and Enchanting sits between
  Alteration on its left and Smithing on its right.
- Each skill is five values in `SkillStatsA`: level, name, progress percent,
  level colour, legendary count. The description card takes the skill's
  `AVIF` `DESC`.
- Perk labels are HTML names with the fractions of the 1280 x 720 stage they
  sit at; the movie scales them by their height (`y * 165 + 10` percent) and
  skips those whose point is above 115 pixels (`MaxPerkNameHeight`), 175 in
  level up mode (`MaxPerkNameHeightLevelMode`). A label's text box is 47.85
  pixels high at full scale, centred on its point across and above it
  (`PerkNameClipInstance` at y -97.55): the text's middle is 73.6 pixels
  times its scale above the point. `UpdatePerkText(true)` fades them in a
  movie frame a call.
- `SetStatsMode(false, points)` is the movie's level up mode, where "Perks to
  increase: n" shows over the ring; `true` the plain stats mode.
- **Legendary is the ring's.** `SetPerkCount` keeps the Legendary hint up
  only for a skill at 100 with no tree open (`bInPerkMode` false), and only
  `ShowLegendaryButtonHint` puts it up; a tree's card always shows the perk
  count. The hint's key art, baked into the movie, is Space on PC, and the
  control map has no event for it.
- A perk's card shows no name: `SetDescriptionCard` empties the name field in
  perk mode and shows the skill and requirements under the description.
- **The stars' and lines' states** are their behaviour graphs'
  (`vrm-tool hkb-tree`, `hkb-obj ... BGSGamebryoSequenceGenerator`), each
  state playing the model's sequence of its name. A star: `Locked`
  (start), `Unlock` (0.67 s) then `Unlocked`, `Own` (0.67 s) then `Owned`,
  `Select` (4.9 s) then `Selected`, `Deselect` (4.9 s) back to `Owned`;
  events `Unlock`, `Own`, `Select`, `Deselect`, and wildcards
  `UnlockedWild`, `OwnedWild`, `SelectedWild` and `reset` jumping to
  `Unlocked`, `Owned`, `Selected` and `Locked` from anywhere. The resting
  states loop; the transitions end (`End`). So the graph selects and lets go
  of an owned star only. A line: `Locked` (start), `Unlock` (0.83 s) then
  `Unlocked`, and `FadeOut` / `FadeIn` from either, through
  `LockedFadeOut` / `UnlockedFadeOut` and back. The star's sequences scale
  and colour its art (`Plane01`) and two glows; the line's fade its one
  shape.
- The star model lies in its xy plane under a facing billboard
  (`FacingRoot`, mode 2), about 16 units across with a glow of 23; the line
  is 11 units wide and runs 56.2 units along its y from its origin to
  `BeamEnd`, under a billboard turning about one axis (mode 1). The
  skydome's constellation images are about 24 by 42 units at a radius of
  92: the game must scale the stars down or bring the camera close.
- **The skydome's sequences** (`vrm-tool nif-dump`): `CameraIntro` and
  `CameraOutro` (0.633 s each, clamped) and `Idle` (118.67 s, looping),
  each keying the same blocks. `CameraIntro` raises `CameraPosition` from
  (0, 0, -3000) to the origin (0.14), easing out (keys every 1/30 s);
  `CameraOutro` lowers it from -157 to -3000, easing in. Both hold the star
  layers at `Idle`'s first pose and the gases' V offset at 0. `Idle` turns
  `StarsBackground` about z from -6.2 to -1.3 degrees and back,
  `StarsNear01` from 0 to -5.1 and back, `StarsNear05` from 7.5 to 12.9 and
  back (the turn at 57.3 s), scrolls the five gases' V offset (shader
  variable 8) from 0 to 5, and holds `CameraPosition` at -3000. The node's
  rotation is a Gamebryo camera's (looking down its x): it faces +Y (image 0)
  with +Z up. From 3000 below, horizontal, only the outer star tiles show;
  the nebula panoramas come into view in the last few hundred units.
- **The movie's camera hooks.** `StatsMenu.StartCameraAnimation` (static;
  nothing in the movie calls it) calls `MoveCamera(0)`, then every 41 ms
  `UpdateCamera` calls `MoveCamera(alpha)` with
  `CameraMovementInstance.CameraPositionAlpha._alpha / 100` while that
  clip's frame is below 100. The clip plays its 100 frames on its own and
  loops (it has no stop), the alpha rising evenly from 0 to about 99
  (measured in Ruffle). `UpdateCamera` computes the frame plus 8, capped at
  100, but never uses it: no `gotoAndStop` follows. The root timeline's
  `FadeOut` runs frames 2 to 16 (0.625 s at 24 fps, about `CameraOutro`'s
  length); `FadeIn` (frame 17) calls `SetFadedIn`.
- The level up popup is three buttons with the attribute names baked in;
  its calls are `addHealth`, `addMagicka` and `addStamina`, each followed by
  `PlaySound("UIMenuOK")`. Accept reaches the focused button twice: CLIK's
  `Button.handleInput` presses it on Enter, and so does the player's own
  press of the focused clip (Ruffle patch 8).

## Unconfirmed: choices made

- **The menu opens on image 0** (Enchanting), which the camera faces at
  rest.
- **The camera** stands at the skydome's origin and faces the selected
  skill's image (60 degrees of view, level), turning with the ring. In a
  tree it narrows to 40 degrees and looks up 9.5 degrees, to the images'
  middle. The game's camera is its own (`skillsLookAt`, `starCameraOffset`,
  `starLookAt`); the images are drawn as the model has them, all alike.
- **The skydome's sequences**: `CameraIntro` plays as the menu opens, then
  `Idle` loops; `CameraOutro` plays when the movie starts its fade out, and
  the menu closes when the fade ends (15 frames, 0.625 s: the outro's last
  8 ms may not show). The camera follows `CameraPosition` only while the
  intro or outro plays: `Idle` keys it at 3000 units below, which, followed,
  would drop the camera out of the sky the moment the intro ends. During
  the rise it keeps the ring's heading rather than the node's (which faces
  image 0, the menu's opening skill anyway). The dome is moved rather than
  the camera, since it is drawn as sky about the camera. Opening straight
  into a tree (`skills open <skill>`) plays the intro too. The outro starts
  at 157 units below, not at the origin, so the sky jumps by that much as
  it begins; whether the game starts it elsewhere, or blends, is unknown.
  The rise looked right in screenshots (a dark star field below the
  nebulae, rising into them) but hasn't been compared with Skyrim SE.
- **`MoveCamera` isn't used** and `StatsMenu.StartCameraAnimation` is never
  invoked: when the game calls it, and what the alpha it reports drives (the
  intro's progress, a blend between `CameraPosition` and the game's camera,
  or the move between the ring and a tree), is unknown. Read literally, the
  ramp lasts up to 4.1 s and starts wherever the looping clip is; the
  intended 8 frames a tick would make it about 0.5 s, close to the
  sequences' 0.63 s.

- **The level up popup** opens over the skills menu whenever a level up is
  waiting, again for each level up, and the menu then shows the level up
  mode (`SetStatsMode(false, ...)`) for the rest of its time open. When the
  game shows it (on opening the menu, or when the level is earned) is
  unconfirmed. Each press (Accept or a click) that reaches the popup takes
  at most one level, since one Accept calls back twice. No sound of our own:
  the popup plays `UIMenuOK`.
- **Keys**: A / D or the arrows turn the ring; E or Enter (Menu Mode's
  Accept) opens the skill's perks (in a tree: picks a first node, then takes
  the perk); Esc, Tab or K go back from a tree, then close; Space, as the
  hint shows, makes the ring's skill Legendary. The mouse, in the stage's
  coordinates (letterbox and scale taken out): a click in the middle fifth
  opens the skill, left or right of it turns the ring, the wheel turns it.
  In a tree, a click on a label's text (or within 8 pixels of it) picks that
  perk and a second click on it takes it. No confirmation before a perk is
  taken (the previous egui menu had none either). Apart from Space, none of
  these bindings is documented.
- **Legendary** (Space on the ring, for a skill at 100) asks
  `sLegendaryResetConfirm`, then
  `sLegendaryResetSecondConfirm` (Yes / No); the guide the rule comes from
  (`skills.md`) describes only one confirmation.
- **Perk requirements** on the card: "Requires" and what the perk's
  conditions ask that the player lacks, skills (`GetActorValue` /
  `GetBaseActorValue` of a skill, at least or above a value or global) as
  "Alteration 30" and perks (`HasPerk`) by name. Conditions joined by their
  OR flag are one group (the game's (a OR b) AND (c OR d)): a group met is
  left out, one not met is listed with "or" between its parts (in English,
  not a game setting), the groups joined by commas, in the conditions'
  order. The card's format isn't documented. Two groups can wrap past the
  requirement field's second line, which the movie clips.
- **Label colours**: the picked perk white, owned ones gold, available grey,
  locked dark grey; ranks as "name 2/5" once one is owned. Skill levels are
  always white (`#FFFFFF`; the colour string's source is unknown).
- **Where perks sit**: the tree's grid cells plus offsets (`XNAM` + `HNAM`,
  `YNAM` + `VNAM`) fitted to the stage between 8% and 92% across and 27% and
  66% down (the top below level up mode's 175 pixels), stretched to fill
  both ways (cells as wide as tall left the tall trees' labels piled up),
  then labels whose text would overlap pushed apart the shorter way, their
  widths estimated at 12 pixels a character at full scale (about 10 for
  names on the movie's font, more with a rank's digits) for the longest
  label a node can have. The stars stand where those points are on a plane
  facing the tree's camera 86 units out (just in front of the
  constellation images), so a star moves with its label; how the game maps
  the grid into the sky (its cell size, origin and whether a tree lines up
  with its picture) isn't documented. The labels go where the stars
  project, through the window's camera and the movie's stage transform.
  Arrows pick the nearest node in that direction within a slightly wider
  than quarter turn.
- **The stars and lines**: each star faces the camera at 0.16 of its size
  (2.6 units across), each line 0.12 of its width, stretched from its
  perk's star to the next's (the model's billboard is set aside: it turns
  about the world's up, which can't lean a line between two stars). Lines
  are the trees' `CNAM` links between perks; the root's (no perk, no star)
  aren't drawn. A star is `Owned` with a rank taken, `Unlocked` when its
  next rank can be taken now (as the grey label), else `Locked`. A line is
  lit once both its perks are taken. Opening a tree puts each star and line
  straight into its resting sequence (as the graph's wildcards would);
  then taking a perk plays `Own`, a perk coming within reach `Unlock`, a
  line lighting `Unlock`. The picked star plays `Select` then `Selected`
  when it is owned; otherwise, since the graph's `Select` starts only from
  `Owned`, it jumps to `Selected` (`SelectedWild`) and back to its rest.
  The lines' `FadeOut` / `FadeIn` aren't played (when the game uses them,
  perhaps while the camera moves between the ring and a tree, is unknown),
  nor the stars' own `reset`.
- **Sounds**: `UISkillsForwardSD` / `UISkillsBackwardSD` turning the ring,
  `UISkillsTransitionUp` opening a tree, `UISkillsFocusSD` between perks,
  `UISkillsPerkSelectASD` taking one, `UIMenuCancelSD` going back: chosen
  from their names alone.

## Unexplained assets and fields (leads)

- `Interface/INTSkillsToPerkTransitionEffect.nif`: a particle cloud
  (`PCloud03`) with `FadeIn` (3.33 s, clamped), `Loop` (26.6 s) and
  `FadeOut` (3.3 s) sequences, and a `TestCameraAttach` shape about 730
  units across. Its name says it plays between the ring and a tree;
  `StatsMenu` has no named slot for it and nothing says where it stands or
  when each sequence plays. Not shown.
- `DLC01/Interface/INTVampirePerkSkydome.nif`: the skydome's stars with the
  werewolf's sky (`WolfSky01` to `06`, the `Moon`) towards +Y and the
  vampire lord's (`VampSky1` to `04`) towards -Y. UESP's Vampire Lord page
  (https://en.uesp.net/w/index.php?oldid=3576137, Skill Perks) and
  Lycanthropy page (https://en.uesp.net/w/index.php?oldid=3637628, Skill
  Perks) have the character menu showing the beast's tree in beast form.
  CommonLibSSE names a `StatsMenuBaseInstance.BeastSkillHolder` the SE
  `statsmenu.swf` doesn't have, and a `numSelectableTrees`; how the menu
  shows a beast tree (one tree, no ring?) is unknown. Waits on the beast
  forms.
- CommonLibSSE's `horizontalVelocity` and `horizontalAcceleration` suggest
  the ring turns with acceleration; ours turns at a steady 520 degrees a
  second. Its `zoomed` and `scrolling` flags, and the 18 trees' effect
  shaders with resting / entering / exiting states, say there is more to
  entering a tree than the camera's narrower view; what each changes is
  unknown.
