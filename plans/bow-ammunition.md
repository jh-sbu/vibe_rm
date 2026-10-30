# Plan: equipped ammunition and quivers

**Status: drafted** (2026-10-30). ROADMAP Next item 5, "inventory ... ammo /
quivers (the weakest arrows equipped with a bow, UESP)". Each numbered step
is one commit; run `cargo fmt` before each.

When the player equips a bow, the weakest arrows they carry equip
themselves. Arrows stay equipped when the bow goes away, and the quiver
shows on the back. Here the player's shot takes an equipped arrow if there
is one, else the strongest carried, and no quiver is drawn.

## The evidence

- UESP [Skyrim:Archery](https://en.uesp.net/w/index.php?oldid=3575828)
  (revision 3575828), Notes:
  - "When you equip a bow, the weakest arrows in your inventory will
    automatically equip themselves by default", and they can be replaced by
    stronger ones from the inventory or favorites;
  - crossbows do the same with bolts (Dawnguard);
  - "Arrows will remain equipped (and appear on the back of your character
    accordingly) even if you have unequipped the bow."
- `sNoArrows` ("No arrows equipped.") exists among the game settings, and
  the player's shot already shows it when there are no arrows.
- Arrow models hold both looks: `Weapons\Iron\IronArrow.nif` is a
  `BSFadeNode` "Arrow" with `Prn` = `QUIVER`, an `ArrowQuiver` node (the
  quiver and five arrows) and a separate `Arrow:0` shape (one arrow). The
  character skeleton has a `QUIVER` node beside `WeaponBow`.
- `known_ambiguities/archery-ammunition.md` records the current selection
  policy and what is still open (an already-equipped stack, incompatible
  ammunition, an exhausted stack).

## What exists (2026-10-30)

- `Engine::equip_in` (`src/items.rs`): ammunition is worn in place of other
  ammunition. Bows are held in both hands. Nothing equips ammunition when a
  bow is equipped.
- `arrows_in` (`src/ai/archery.rs`) prefers equipped, carried, compatible
  arrows (no bolts), else the strongest carried, else iron for NPCs.
  `player_loose` uses it and shows `sNoArrows` only when no arrow is
  carried at all.
- `dress` (`src/world/actor.rs`) hangs the held weapons' models from their
  `Prn` bone. Worn ammunition isn't drawn.
- Crossbows: only `ANIM_BOW` (7) is a bow to the archery code. Crossbow
  firing isn't implemented (`archery-ammunition.md`).

## Design

- **Weakest** is the lowest `AMMO` `DATA` damage among carried ammunition
  compatible with the weapon (arrows for bows, bolts for crossbows: the
  non-bolt flag). A tie is broken by the order the inventory lists them
  (step 0 checks this against SE).
- **When.** Equipping a bow or crossbow (inventory, favorites, hotkeys,
  `EquipItem`) for the player equips the weakest compatible ammunition, but
  only when no compatible ammunition is equipped. Ammunition the player
  chose stays. Taking the bow off leaves the ammunition equipped.
- **Shots.** The player's shot uses equipped compatible ammunition only.
  With none equipped, `sNoArrows` shows and nothing flies, as the setting's
  wording suggests. Step 0 decides what happens when the equipped stack runs
  out.
- **NPCs** keep the current rule (equipped, else the strongest, else iron).
  The UESP note is about the player. Record that in
  `known_ambiguities/archery-ammunition.md`.
- **The quiver** is drawn for any actor with ammunition worn: the model's
  `ArrowQuiver` subtree hung from its `Prn` bone, without the loose
  `Arrow:0` shape.

## Steps

0. **SE observation, automatic.** `vrm-tool test-mod ammo`: a quest script
   with the player's inventory set by `AddItem` / `RemoveItem`. It reads
   `IsEquipped` for each ammunition after each step:
   - equip a bow with iron, steel and elven arrows carried;
   - equip the bow with steel arrows already equipped;
   - unequip the bow;
   - remove all the equipped arrows, then read again: does another stack
     equip itself, and which?
   - equip a bow while carrying only bolts;
   - two arrow kinds with the same damage (copies of a vanilla arrow made
     in the plugin, one listed first);
   - equip a crossbow (Dawnguard).

   Readings go into `Result*` variables and a save, read with
   `vrm-tool test-results`, about 20 lines per page (the test-mod
   conventions). Whether `EquipItem` from a script behaves as the inventory
   does is part of the question, so one manual step is listed for the user
   too: equip the bow from the inventory and look at which arrows are
   marked. Commit the plugin builder. The user runs it.
1. **Weakest compatible ammunition.** Move the compatibility check
   (`bow_ammo_damage`) into a shared `ammo_for(lo, weapon, ammo) ->
   Option<f32>` that knows bows and crossbows. Add
   `Engine::weakest_ammo(actor, weapon)`. Unit tests on synthetic records:
   weakest picked, bolts excluded for bows, arrows excluded for crossbows,
   the tie rule.
2. **Auto-equip with the bow.** In `equip_in`, after a bow or crossbow is
   held by the player and no compatible ammunition is worn, wear the
   weakest. Tests through `use_listed_item` and `EquipItem`: auto-equip,
   an existing choice kept, the bow taken off and the arrows still worn.
3. **Shots from equipped ammunition only.** `player_loose` reads the
   equipped stack. With none, `sNoArrows`. When the stack runs out, apply
   step 0's reading (or, before it comes in, take the arrows off and show
   `sNoArrows` on the next shot, recorded as a choice). NPCs' `arrows_of`
   is unchanged. Update `archery_ammo_tests.rs`.
4. **The quiver.** `dress` adds worn ammunition's model, limited to its
   `ArrowQuiver` node, hung from the `Prn` bone. That needs a way to keep a
   named subtree of a model (check what `crate::world` model loading
   offers; add a node filter if nothing does). Offscreen capture in third
   person (`--console "@5 pthird"`) of the player with arrows worn, with and
   without a bow. Bolts' models (`Dawnguard`) get the same check for their
   node names before relying on `ArrowQuiver`.
5. **Docs.** Rewrite `known_ambiguities/archery-ammunition.md` with the
   observed rules and the choices left. Move the ROADMAP clause to Done.

## Verification

- `cargo test` for steps 1–3.
- The offscreen capture for step 4.
- Inventory menu: the worn arrows show the equipped icon after equipping a
  bow (offscreen with `--console` opening the inventory, as earlier item
  menu captures did).

## Open questions

- What the game does when the equipped stack is used up (step 0).
- Whether NPCs follow the weakest rule. They aren't observed, and their
  combat chooses ammunition differently.
- Crossbow firing and reloading stay out of this plan.
