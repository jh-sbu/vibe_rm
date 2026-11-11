# The item menus' and crafting's 3D item

Implemented in `src/item_view.rs` (the model, its place, turn and zoom) and
the item and crafting menus (`src/swf_ui/item_menu.rs`,
`src/swf_ui/crafting_menu.rs`). Plan and sources: `plans/item-3d.md`.

## Settled by the sources

- When the movies ask for the item, where they want it (`MouseRotationRect`:
  the card's column, the top 0.55 of the stage), what turns and zooms it
  (`StartMouseRotation` / `StopMouseRotation`, `ZoomItemModel`, the Item
  Menus context's `Item Zoom`, C), and that zooming fades the menu
  (`ToggleMenuFade`): the movies' ActionScript and `controlmap.txt`.
- That zoom is a state with a transition, not a level: CommonLibSSE's
  `Inventory3DManager::zoomProgress` ("1 if zoomed in, 0 if not,
  in-between during transition").
- The marker's layout (`BSInvMarker`: three `ushort` rotations in
  thousandths of a radian, a zoom): niftools' nif.xml, confirmed on the
  2058 vanilla meshes that have one.
- Which model a book shows: `BOOK`'s `INAM` inventory art (UESP, "Skyrim
  Mod:Mod File Format/BOOK").

## Choices

- **The marker's rotation order and sense.** Rx·Ry·Rz (the model turned
  about its own Z first), each counterclockwise by the right-hand rule, the
  camera looking along +Y with +Z up. Evidence from the meshes, checked in
  offscreen captures: armor ground models (lying on their backs) turn 1570
  about X and then face the camera; shields turn 4712 and show their face;
  the draugr sword's marker (1570 about X, 2356 about Z, zoom 1.74) lays it
  across the rect's diagonal in this order, where the other order stands it
  upright with 1.74 times the size and off the top of the screen; bows with
  785–959 about Z lie diagonally. Nothing public states the order.
- **One size for all.** Every model's bounding sphere is brought to 0.6 of
  the rect's width across, then multiplied by the marker's zoom (daggers
  0.75, battleaxes 1.25 only make sense against a common size). The share,
  the camera's 30° vertical field of view and its distance (300 units) are
  picked to look right in captures, not measured.
- **No marker** leaves the model as authored, at zoom 1 (nif.xml: "Skyrim
  will still show the nif in inventory, using the default values"). The iron
  sword's `LongSword.nif` has none and shows upright, point down.
- **The light**: one directional light from the camera's upper left (0.9)
  and an even ambient (0.45); no world lights, no fog, no image space or
  gamma (the item is drawn over the finished frame).
- **The whole model**: a sword shows with its scabbard, as its world model
  has it; skinned shapes (bows) are posed at rest. No enchantment shader,
  particles or lights.
- **Turning**: the cursor's horizontal motion while held turns the item
  about the vertical, a full turn per viewport width, rightward moving its
  front to the right. Vertical motion does nothing. A new item starts
  unturned.
- **Zooming**: in, the rect eases (smoothstep) to the middle of the frame
  (0.15 to 0.85 across, the full height) over 0.25 s, and out the same way.
  The wheel zooms in on a turn up and out on a turn down; the key toggles.
  Hiding the item (`UpdateItem3D(false)`) drops the zoom at once.
- **Before the movie's rect is read**, a 16:9 card column (0.62 to 0.94
  across, the top 0.55) stands in; the rect is read from the movie each
  time the item changes, through its parents' positions and scales.
- **Crafting** shows what the card shows: the forge's recipe output, the
  item improved, enchanting's chosen item once the card previews the
  enchantment (else the highlighted item, gem or item to disenchant;
  nothing for an enchantment), alchemy's highlighted ingredient (not the
  potion, which has no model before it is made).
- **The inventory fading out** (Esc, Tab to the tween menu) hides the item
  at once.

## Unresolved

Screenshots of Skyrim SE's inventory would settle the item's size and place,
the field of view, the light, scabbards, the zoomed size and timing, the
turning speed and whether vertical motion tilts the item, and what alchemy
shows with ingredients chosen.
