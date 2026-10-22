# NPC body weight

Implemented in `src/world/loader.rs` (a `_1.nif` loaded with its `_0.nif`),
`render::model::morph_thin` and the skinned vertex shaders (`object.wgsl`,
`shadow.wgsl`). An actor's skinned models are drawn between their thin (`_0`)
and heavy (`_1`) shapes by its weight: `_0 + (_1 - _0) × NAM7 / 100`.

## Public evidence and assets

- [UESP: NPC_ format](https://en.uesp.net/w/index.php?oldid=3650076) lists
  `NAM7`, the NPC's weight (0..100). ARMA model paths name the `_1` model
  only (Skyrim.esm has 1581 `_1.nif` paths and 60 `_0.nif` ones).
- Assets (read with `vrm-tool neck-seam`): each FaceGen head's open neck
  edge meets the body's skin, blended by vertex index at the NPC's own
  `NAM7`, to 0.0001 units, and nowhere else: Suvaris Atheron (80, base body
  and `farmclothes02/f/dress_1`), Temba Wide-Arm (100), Avrusa Sarethi (0),
  Hod (100), Belethor (35), Faendal (40). The `_1` model alone left gaps of
  up to 1.1 units (male `_0` / `_1` bodies differ about twice as much as
  female ones).

## Choices

- **Pairing.** A `_1` shape blends with the `_0` shape of the same name and
  vertex count (in order, for repeated names); a shape without one is drawn
  as it is. All 45 `_1` models loaded around Windhelm paired every shape.
- **What blends.** Positions and normals; tangents and bitangents are the
  heavy model's.
- **Without `NAM7`** the weight is 50, as the behaviour graph's `weapAdj`
  was given before. Every Skyrim.esm NPC has one; creatures' models have no
  `_0` beside them.
