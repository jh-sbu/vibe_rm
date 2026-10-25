# Activate packages

Implemented in `src/ai/mod.rs` (`Behaviour::Activate`, `activation_done`) and
`src/ai/package.rs`. No public source for the `Activate` /
`ActivateSpecialFurniture` templates was reachable; read from the data
(`vrm-tool pack-tree <data> <package>`).

## What is known

- Inputs: `Target` (a `SingleRef`) and `Number to Activate` (an `Int`). The ten
  sawmill workers (Hod, Hern, Hert, Bolund, Gilfre, Aeri, Thonnir, Lodvar, Temba,
  Grosta) use them on their mill's lever (a furniture with a push marker) and log
  pile (`ResourceObject...`, furniture with `ResourceObjectSawMillScript`); one DLC2
  test package names a dragon.
- The packages have no location input, so a package's place is its target's.

## Choices made without a source

- The actor walks to the target and, for furniture, uses it as a Sit package
  uses its chair (any marker kind); anything else is activated where it stands.
  It starts beside the target on loading, not in it.
- Each use counts as one activation; after "Number to Activate" the package is
  done and passed over while it still applies (as a finished UseWeapon package).
- A use of furniture whose idle loops lasts 3 to 6 seconds; a one-shot idle lasts
  its clip.
