# The item menus' and crafting's 3D item

Implemented in `src/item_view.rs` (the model, its place, turn and zoom) and
the item and crafting menus (`src/swf_ui/item_menu.rs`,
`src/swf_ui/crafting_menu.rs`). Plan and sources: `plans/item-3d.md`. The
magic menu's effects (`src/swf_ui/magic_menu.rs`): `plans/magic-3d.md`, and
"The magic menu" below.

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

## The magic menu

Settled by the sources (`plans/magic-3d.md`, "What the sources say"): the
movie asks with `UpdateItem3D`, as the item menus do; spells, magic effects
and shouts each have a menu display object (`MDOB`, a `STAT`); the 37
vanilla models are effects moved only by standalone controllers (none has a
controller manager or sequences, though eleven carry the hand art's
`mIdle`... text keys and ten name a behaviour graph).

Choices:

- **Which object.** A spell's or shout's own. An active-effects row (ours
  groups the effects of one item) shows its source spell's, else the first
  of its shown effects' that has one: potions and enchantments have none of
  their own, and every vanilla potion effect names the same `STAT`
  (`0435A5`). A row with none, or whose object is an editor marker
  (`marker_map.nif`: seven script effects), shows nothing.
- **What plays.** The model's standalone controllers as authored, each over
  its own span and looping or holding by its flags, with its particles; not
  its behaviour graph, which has no sequences to play. Where the text keys
  mark an `mIdle`, the controllers' spans don't follow it (0–10.8 s and
  0–3.33 s in models whose `mIdle` is 0–6.67 s; Detect Life and Paralyze
  have one at 10.03–19.67 s, their `mCharge` to `mCastCon`), so nothing
  selects a phase. A row starts its effect afresh; the same row again keeps
  it playing. It plays on the menu's real time, while the world's clock
  stands still.
- **Its size.** The marker's zoom alone, about the model's origin, as if
  every model had a radius of 15 (`MAGIC_RADIUS`), against the items'
  bounding sphere brought to 0.6 of the rect's width. Evidence: the zooms
  range 0.08 to 1.36 and the models' radii 20 to 377, but radius times zoom
  only 23 to 81, the larger art zoomed down in step; under the items' rule
  the shouts (zoom 0.08) drew a few pixels across. Their bounding centres
  sit off the visible effect (the shouts' at (0, -33, -52), wide glow
  cards), so the origin is used. The 15 was picked from captures.
- **Where.** The card's column, from the top down to 0.55 of the stage, read
  once as the menu opens: where the shared `ItemMenu` code would put its
  `MouseRotationRect` (`PositionElements`), which `magicmenu.swf` doesn't
  have. Later the card holder's width follows the card (an active effect's
  is wider).
- **No turning or zooming.** Without a `MouseRotationRect` the movie never
  calls `StartMouseRotation` or `ZoomItemModel`; the Item Zoom key isn't
  taken in the magic menu either. Whether SE zooms a spell with that key is
  unknown.
- **No light** from `AttachLight` nodes, as items light nothing.

Screenshots or a short video of SE's magic menu would settle the size and
place, the phase shown, what an active effect and a potion show, whether an
effect restarts when the highlight returns, and whether Item Zoom works
there.

## Unresolved

Screenshots of Skyrim SE's inventory would settle the item's size and place,
the field of view, the light, scabbards, the zoomed size and timing, the
turning speed and whether vertical motion tilts the item, and what alchemy
shows with ingredients chosen.
