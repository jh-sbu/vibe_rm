# BSInterpValueModifier

Implemented in `crates/havok/src/behavior/runtime.rs` (parsed in
`crates/havok/src/behavior/mod.rs`).

## Public and asset evidence

- The vanilla humanoid project's `1hm_behavior.hkx` holds one, named
  `SpeedDamped_BSInterpValueModifier`, with `source` and `result` bound to
  `SpeedDamped`, `target` bound to `Speed`, and 0.1 at the member after them
  (`vrm-tool hkb-vars <Data> meshes/actors/character SpeedDamped`,
  `vrm-tool hkb-obj ... 1hm_behavior.hkx BSInterpValueModifier`). The member
  names `source`, `target`, `result`, `duration` and their order are taken
  from the community behaviour-editing class listings.
- `SpeedDamped` is the parameter of every melee attack's speed blend
  (`1HM_Forward_AttackRight_Blend` and the rest: standing, walking and
  running attack clips at 25, 82.45 and 288). Nothing else in the melee graph
  writes it, so without the modifier attacks on the move play standing.

## Choices not resolved by those sources

- **The step.** Each update `result` moves `dt / duration` of the way from
  `source` (read again every update) to `target`, reaching it once `dt`
  covers the duration. With `source` and `result` the same variable this
  chases `target`, settling to about 90% of it in a quarter of a second. Whether the game
  instead interpolates linearly from the value at activation over `duration`
  is unknown.
- **Where it starts.** `SpeedDamped` keeps its value between attacks, so an
  attack on the move starts from what the last one left (0 at first) rather
  than from the current `Speed`. Whether the game also sets `SpeedDamped`
  from code is unknown.
