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
`sRequirementsText`.

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
- Perk labels are HTML names with the screen fractions they sit at; the movie
  scales them by their height on screen (`y * 165 + 10` percent) and skips
  those above 115 pixels. `UpdatePerkText(true)` fades them in a movie frame
  a call.
- `SetStatsMode(false, points)` is the movie's level up mode, where "Perks to
  increase: n" shows over the ring; `true` the plain stats mode.
- The level up popup is three buttons with the attribute names baked in;
  its calls are `addHealth`, `addMagicka` and `addStamina`.

## Unconfirmed: choices made

- **The level up popup** opens over the skills menu whenever a level up is
  waiting, again for each level up, and the menu then shows the level up
  mode (`SetStatsMode(false, ...)`) for the rest of its time open. When the
  game shows it (on opening the menu, or when the level is earned) is
  unconfirmed. One press can reach the focused button twice (the key and the
  press it makes), so a batch takes one level.
- **Keys**: A / D or the arrows turn the ring; E, Enter or Space opens the
  skill's perks (in a tree: picks a first node, then takes the perk);
  Esc, Tab or K go back from a tree, then close; R (the control map's Ready
  Weapon) makes the skill Legendary. The mouse: a click in the middle fifth
  opens the skill, left or right of it turns the ring, the wheel turns it.
  In a tree, a click picks the nearest perk within 6% of the screen's height
  and a second click on it takes it. No confirmation before a perk is taken
  (the previous egui menu had none either). None of these bindings is
  documented.
- **Legendary** asks `sLegendaryResetConfirm`, then
  `sLegendaryResetSecondConfirm` (Yes / No); the guide the rule comes from
  (`skills.md`) describes only one confirmation.
- **Perk requirements** on the card: "Requires" and what the perk's
  conditions ask that the player lacks, skills (`GetActorValue` /
  `GetBaseActorValue` of a skill) as "Alteration 30" and perks (`HasPerk`)
  by name, joined by commas. The card's format isn't documented.
- **Label colours**: the picked perk white, owned ones gold, available grey,
  locked dark grey; ranks as "name 2/5" once one is owned. Skill levels are
  always white (`#FFFFFF`; the colour string's source is unknown).
- **Where perks sit**: the tree's grid cells plus offsets (`XNAM` + `HNAM`,
  `YNAM` + `VNAM`) fitted flat to the middle of the screen. The game
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
  `UISkillsPerkSelectASD` taking one, `UIMenuCancelSD` going back,
  `UILevelUpSD` for a level up: chosen from their names alone.
