# Plan: the magic menu's 3D model

**Status: drafted** (2026-11-14). Not started. ROADMAP item 6 (UI), "The
inventory's rest": "the magic menu's models (`UpdateMagic3D`: spells',
effects' and shouts' `MDOB` menu display objects, in the item menus' 3D
view)". It follows `plans/item-3d.md`, which built that view and left this
out.

The magic menu shows the highlighted spell, shout or active effect as a
small animated effect beside its card, the way the item menus show an item.
Here the menu ignores the calls that ask for it.

## What the sources say

- **The movie** (`magicmenu.swf`, read with `swfspike dis` and `fold.py`):
  it shares the `ItemMenu` code with the item menus. It calls
  `UpdateItem3D(true)` in `onItemHighlightChange` and `onShowItemsList`,
  `UpdateItem3D(false)` in `onHideItemsList`, `StartMouseRotation` /
  `StopMouseRotation` around a press in `MouseRotationRect`, and
  `ZoomItemModel(delta)` for the wheel over it. It sizes
  `MouseRotationRect` as the item menus do: the card's column, locked to
  the top, 0.55 of the visible stage high. Our menu handles all of these as
  no-ops (`src/swf_ui/magic_menu.rs`, "none for spells").
- **CommonLibSSE-NG** (MIT): `Inventory3DManager::UpdateMagic3D(TESForm*,
  std::uint32_t)` sits beside `UpdateItem3D(InventoryEntryData*)`, so the
  magic path is given the form itself. `BGSMenuDisplayObject` (`MDOB`, one
  `TESBoundObject* menuDispObject`) is a component of `SpellItem`,
  `EffectSetting` (MGEF) and `TESShout`. The second argument's meaning is not
  given.
- **UESP**, "Skyrim Mod:Mod File Format/SPEL" (revision 2309037), "/MGEF"
  (revision 3507756), "/SHOU" (revision 3089104): `MDOB` is a form id, a
  "Menu display object" `STAT`. On SHOU it is "Model (STAT) shown in
  inventory".
- **The data** (Skyrim.esm, Update.esm and the three DLC masters, scanned
  2026-11-14): 1,023 of 1,353 SPEL, 1,347 of 1,571 MGEF and 114 of 117 SHOU
  have a nonzero `MDOB`. Every one resolves, to 40 distinct `STAT`s with 38
  distinct models. 1,444 uses point at `Magic\MAGINVGenericSpellArt.nif`.
  The SPEL without one are mostly abilities (192), plain spells the player
  never learns (112: perk and quest internals such as `PerkTrickShot`),
  diseases (16) and voice spells (7); one power and two lesser powers
  (`DLC1RevertForm`, `DLC2BlackBookNoFallingDamageSpell`, a test spell).
  Seven script MGEFs (`ScriptEffect`, the DLC2 standing-stone removals,
  `DLC1AurielsBowScriptEffect`...) point at the `MapMarker` STAT, whose
  model is the editor marker `marker_map.nif`. Scrolls' `MDOB`s include
  placeholders (`TestBook01HighPoly`, `CoastDriftWood01`), but scrolls are
  in the inventory, not the magic menu, and stay as `plans/item-3d.md` has
  them.
- **The models**: each is a small effect scene, not a mesh: `BSFadeNode`
  root, `BSInvMarker` (all 37 effect models have one, e.g. zoom 0.36 for the
  generic art, 0.08 with three rotations for `MAGINVShoutSelfWhite`),
  `NiBillboardNode`s, `BSEffectShaderProperty` materials with float and
  colour controllers, a `Rotator` node under a `NiTransformController`,
  `NiParticleSystem`s and `BSStripParticleSystem`s (up to six per model;
  four have none), and some an `AttachLight` node. Eleven carry the hand
  art's text keys (`mIntro`, `mIdle`, `mCharge`, `mReady`, `mCast`,
  `mIdleStaff`...; the ward and Unrelenting Force art their own set), three
  only `SpecialIdle`. None has a `NiControllerManager`, so the keys are not
  sequences. In `MAGINVSummon.nif` every controller's own timing runs 0 to
  6.667 s, looping (flags `0x48`): exactly `mIdle`'s range. The authored
  standalone controllers alone play the idle there.

## What exists to build on

- `src/item_view.rs`: the view, placement by bounding sphere and
  `BSInvMarker`, turning, zooming, the movie's rect, drawn by
  `Renderer::render_over`. It shows one static `GpuModel`.
- `spell_fx::EffectInstance` (`src/spell_fx/effect.rs`): an effect model's
  parts moved by their standalone controllers, its material and particle
  controllers, `step` simulating particles into `ParticleBatch`es and
  `draw` pushing its parts. `Engine::load_spell_model` loads one (the
  `#effect` variant) and configures it from the NIF.
- `render_over` draws a scene's particles as the world's frame does.

## Steps

Each numbered step is one commit, in this order. Run `cargo fmt` before
each.

1. **Read `MDOB`** on SPEL, MGEF and SHOU into the records ours keeps
   (`MagicItem`, `MagicEffect`, `Shout`: a `menu_object: FormId`, null when
   absent). Tests with `esp::writer` records.
2. **Which model a magic row shows** (`item_view::magic_model`): a
   `Row::Spell` its spell's `MDOB`, a `Row::Shout` its shout's, a
   `Row::Effects` its source item's if that is a SPEL with one, else the
   first of its shown effects' MGEF `MDOB`. The `STAT`'s `MODL`, or none:
   no `MDOB`, a null one, an unresolved form, a non-`STAT`, or an editor
   marker model (`marker_*.nif`, which the game doesn't draw). Tests for
   each case.
3. **The item view plays effect models.** A shown model is either the
   static item (as now) or an `EffectInstance` loaded as
   `load_spell_model` does. Its transform is the view's placement (bounding
   sphere of its root and parts, the marker's rotations and zoom, the
   player's turn), set each frame. `ItemView::update` advances it by the
   frame's real time (menus pause game time), steps its particles into the
   view's `Scene::particles` with the view camera's right and up, and draws
   its parts into the view's scene. Its controllers run as authored, so no
   phase is chosen: check with `vrm-tool nif-dump` that the other 13 keyed
   models' controller ranges are their `mIdle` (or `SpecialIdle`) range as
   in `MAGINVSummon.nif`, and if one isn't, clamp it to that range here.
   The item menus' static items are unchanged. Tests with a synthetic
   effect model: parts posed under the view's transform, particles
   collected, real time advancing it.
4. **The magic menu shows it.** `UpdateItem3D(true/false)` shows the
   highlighted row's model (step 2) or hides it; `StartMouseRotation`,
   `StopMouseRotation` and `ZoomItemModel` do what they do in the item
   menus, through the same code (`src/swf_ui/item_menu.rs`); closing the
   menu clears the view; the rect is read from the movie. A new row starts
   its effect afresh; the same row again keeps it running.
5. **Check and record.** Offscreen captures of the magic menu (the
   generic art, fireball, a shout, summon, an active effect) at several
   frames, to see the effects animate, sit in the rect and turn.
   `known_ambiguities/item-3d.md` gets a "The magic menu" section with the
   choices below; ROADMAP's entry and this plan's status are updated.

## Choices to record (step 5)

Nothing public settles these; each goes in `known_ambiguities/item-3d.md`
with this plan's evidence:

- An active-effects row (ours groups the effects of one item) shows the
  source spell's `MDOB`, else its first shown effect's. Potions and
  enchantments have no `MDOB` of their own.
- A row whose record has no `MDOB`, or one pointing at an editor marker,
  shows nothing.
- The models play their standalone controllers as authored: the idle
  (step 3's check).
- The size: the item view's one-size rule (bounding sphere to 0.6 of the
  rect, times the marker's zoom). Particles reach past the meshes' bounds,
  so the effect may look smaller or larger than SE's; adjust from captures
  only within that rule, and say so.
- `AttachLight` nodes give no light, as the item view lights nothing but the
  item.

## Left out

- The gamepad's Rotate and Item Zoom press (no gamepad yet), as for items.
- Lights from the effects (`AttachLight`, the spell's casting light).
- Particles that live in world space don't follow the model as the player
  turns it, as with standalone transforms elsewhere
  (`known_ambiguities/node-controllers.md`). Turning may leave them
  trailing.
- Scrolls' `MDOB` (placeholders in vanilla; the inventory shows their
  `MODL`).

## Open questions (for an SE comparison)

Screenshots or a short video of SE's magic menu would settle:

- What an active effect shows, and whether a potion's effects show a model.
- The effect's size and place in the rect, and whether it turns and zooms
  like an item.
- Whether the effects restart when the highlight returns to a row.
- What the powers and spells without an `MDOB` show (`DLC1RevertForm`).
