# The skills menu (statsmenu.swf, levelupmenu.swf)

Implemented in `src/swf_ui/stats_menu.rs` (the movies' contents and keys) and
`src/stats_view.rs` (the sky behind them, as the map has its own scene).
Sources: the movies' ActionScript (`StatsMenu`, `AnimatedSkillText`,
`LevelUpMenu`: the `SetDescriptionCard`, `SetPlayerInfo`, `UpdateSkillList`,
`SetDirection`, `HideRing`, `SetStatsMode`, `SetPerkCount`,
`ShowLegendaryButtonHint` callbacks, the static `StatsMenu.SkillStatsA` and
`PerkNamesA`, `UpdatePerkText`, the `MoveCamera` / `SetFadedIn` calls, the
level up popup's `addHealth` / `addMagicka` / `addStamina`); CommonLibSSE's
`StatsMenu` and `BGSSkillPerkTreeNode` (the tree nodes' grid cells and
offsets); the star models in `Skyrim - Meshes1.bsa`
(`Interface/INTStatMenuStars.nif`, `INTPerkSkydome.nif`); the game settings
`sLegendaryResetConfirm`, `sLegendaryResetSecondConfirm`,
`sRequirementsText`; `Interface/Controls/PC/controlmap.txt` (its Stats
context binds only `Rotate`, to the right stick).

## Settled (by the movies)

- The movie only draws; the game keeps the selection and input. The ring is
  18 skills, 20 degrees each (`SKILL_ANGLE`); `SetDirection(angle)` turns it
  and the skill at `angle / 20` is the centre one.
- The ring's order is the movie's constants: Alteration, Conjuration,
  Destruction, Illusion (`MYSTICISM`), Restoration, Enchanting; Light Armor,
  Pickpocket, Lockpicking, Sneak, Alchemy, Speech; One-handed, Two-handed,
  Archery, Block, Smithing, Heavy Armor.
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
- The level up popup is three buttons with the attribute names baked in;
  its calls are `addHealth`, `addMagicka` and `addStamina`, each followed by
  `PlaySound("UIMenuOK")`. Accept reaches the focused button twice: CLIK's
  `Button.handleInput` presses it on Enter, and so does the player's own
  press of the focused clip (Ruffle patch 8).

## Unconfirmed: choices made

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
  `YNAM` + `VNAM`) fitted flat to the stage between 8% and 92% across and
  27% and 66% down (the top below level up mode's 175 pixels), then labels
  whose text would overlap pushed apart the shorter way, their widths
  estimated at 10 pixels a character at full scale (measured on the movie's
  font) for the longest label a node can have. The game
  projects its 3D constellations, so the labels' places and the lines
  between them differ; there are no stars drawn at the perks or lines
  between them (`INTPerkStars01.nif`, `INTPerkLine01.nif` have their own
  animations that aren't played). Arrows pick the nearest node in that
  direction within a slightly wider than quarter turn.
- **The sky**: the ring shows `INTStatMenuStars.nif` seen from its
  `CameraPosition` node (tilted up 30 degrees), the whole model turned with
  the ring (the game turns its `ConstellationGroup` node, so the starfields
  turn here too); a skill's perks show `INTPerkSkydome.nif` from the origin,
  not moving (its `CameraIntro` / `CameraOutro` / `Idle` sequences and the
  camera path `MoveCamera` asks for aren't played). The three constellation
  planes whose textures aren't in the installed archives (`INTmageConst`,
  `INTtheifConst`, `INTwarConst`: `const01:0`, `const05:0`, `const09:0`) are
  left out; the others' art is shared between the constellations, so the
  ring does not show a constellation per skill.
- **Sounds**: `UISkillsForwardSD` / `UISkillsBackwardSD` turning the ring,
  `UISkillsTransitionUp` opening a tree, `UISkillsFocusSD` between perks,
  `UISkillsPerkSelectASD` taking one, `UIMenuCancelSD` going back: chosen
  from their names alone.
