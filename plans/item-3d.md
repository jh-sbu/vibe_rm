# Plan: the item's 3D model in the item menus and crafting

**Status: in progress** (drafted 2026-11-11). ROADMAP item 6 (UI), "The
inventory's rest": the item's model beside the card (`UpdateItem3D`, turning
and zooming it), including crafting previews (`plans/crafting.md` left
`ShowItem3D` out for this).

## What the sources say

- **The movies** (`inventorymenu.swf`, `containermenu.swf`,
  `bartermenu.swf`, `giftmenu.swf`, `magicmenu.swf` share the `ItemMenu`
  class; `craftingmenu.swf` has its own; read with `swfspike dis` and
  `fold.py`). The item menus call `UpdateItem3D(true)` when a row is
  highlighted or the list shows, and `UpdateItem3D(false)` when the list
  hides. The crafting menu calls `ShowItem3D(bool)` after each selection
  (`SetSelectedItem` first), with false when nothing is highlighted. Both
  have a `MouseRotationRect`: the item card's column (`ItemCard_mc._parent`
  or `ItemInfo._parent`, its `_x` and `_width`), locked to the top, 0.55 of
  the visible stage high. A press in it calls `StartMouseRotation`, the
  release `StopMouseRotation` (a press shorter than 150 ms is a click on the
  list instead). The wheel over it calls `ZoomItemModel(delta)` in the item
  menus. The game toggles the menu's fade with the `ToggleMenuFade`
  callback (the lists stop taking input until `SetFadedIn`).
- **`controlmap.txt`**, Item Menus context: `Item Zoom` is C (`0x2e`) or the
  right stick's press; `Rotate` is the right stick.
- **CommonLibSSE's `Inventory3DManager`** (MIT): the shown models
  (`loadedModels`), an item position and scale, a `zoomProgress` that is
  "1 if zoomed in, 0 if not, in-between during transition", and
  `UpdateItem3D` / `UpdateMagic3D` / `Clear3D` / `Render`. No values.
- **The meshes**: 2058 vanilla models carry a `BSInvMarker` extra data named
  `INV`. niftools' nif.xml documents it as "Orientation marker for Skyrim's
  inventory view": three rotations as `ushort`s in thousandths of a radian
  and a zoom factor, with default values when it's absent. The values
  confirm the layout (14-byte blocks; 1570, 3141, 4712 and 6283 are the
  quarter turns) and suggest the rest: armor ground models (which lie on
  their backs) turn 1570 about X, standing them up facing -Y, the way a
  camera at yaw 0 looks; daggers zoom 0.75 and battleaxes 1.25, which only
  makes sense if every model is first brought to one size.
- **Records**: `WEAP`, `MISC`, `ALCH`, `INGR`, `AMMO`, `KEYM`, `SLGM`,
  `SCRL`, `LIGH` show `MODL`; `ARMO` its ground models, `MOD2` (male) and
  `MOD4` (female); `BOOK` its `INAM` inventory art (`STAT`, UESP "Mod File
  Format/BOOK"), else `MODL`.

The game's INIs in the install (`Skyrim_Default.ini`, `SkyrimPrefs.ini`)
set nothing about the inventory's 3D item.

## Steps

Each numbered step is one commit, in this order.

1. **Read `BSInvMarker`** in the `nif` crate (`ExtraData::InvMarker`,
   `Nif::inv_marker`), with a test.
2. **Draw a scene over the frame** (`Renderer::render_over`): the target
   loaded, not cleared; its own depth; no sky, shadows, precipitation map or
   post pass; the world's per-frame caches (shadow cascades, precipitation
   map) left alone.
3. **The item's view** (`src/item_view.rs`): which model a form shows (the
   list above, the player's sex for armor, `#rigid` so skinned bows show at
   rest), and its placement: the bounding sphere's centre at the middle of
   the movie's `MouseRotationRect`, its radius scaled to a share of the
   rect's width times the marker's zoom, turned by the marker's rotations,
   lit by a light of its own. A camera of its own at yaw 0. Tests for the
   model choice and the transform.
4. **The item menus show it**: `UpdateItem3D(true/false)` shows the
   highlighted row's item or hides it; closing clears it; the frame draws
   it over the world (windowed and screenshot runs). `ui_swf::Menu` reads a
   clip's rect in viewport pixels.
5. **Turning it**: between `StartMouseRotation` and `StopMouseRotation` the
   cursor's horizontal motion turns the item about the vertical axis.
6. **Zooming**: `ZoomItemModel(delta)` and the Item Zoom key zoom in
   (positive) or out (negative), calling `ToggleMenuFade` when the state
   changes; the item moves to the middle of the frame and grows over a
   short transition.
7. **Crafting previews**: `ShowItem3D(bool)` shows what the card shows:
   the recipe's output (forge), the item improved (grindstone, workbench),
   the item chosen or the highlighted row's item (enchanter; nothing for
   an enchantment), the highlighted ingredient (alchemy). Turning works as
   in step 5.
8. **Docs**: `known_ambiguities/item-3d.md` for the choices; ROADMAP and
   `plans/crafting.md` updated; this plan's status.

## Left out

- The magic menu's models (`UpdateMagic3D`; spells, effects and shouts have
  a `MDOB` menu display object). The same view would show them; not asked
  for here, left in ROADMAP.
- The gamepad's `Rotate` (right stick) and Item Zoom press: no gamepad yet.
- Effects on the item: enchantment shaders, particles (a torch's flame),
  lights. Shown as the bare model.
- Inspecting an item for a hidden object (the Golden Claw's carving, the
  Dwemer puzzle cubes): nothing in vanilla asks this view what's under the
  cursor; quests use their own scripts. Not checked further.

## Open questions (for an SE comparison)

Not settled by any source; screenshots of SE's inventory would settle them:

- The item's size and place in the rect, the camera's field of view.
- The rotations' order (the marker gives X, Y and Z angles, not an order).
- The light on the item.
- Whether a sword shows its scabbard.
- The zoom's size, place and transition time; the mouse's turning speed.
- What alchemy shows while ingredients are chosen (the potion has no model
  of its own until made).
