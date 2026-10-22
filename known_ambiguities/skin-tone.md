# NPC skin tone

Implemented in `src/world/actor.rs` (`NpcTraits::skin_tone`) and
`src/render/shaders/object.wgsl` (`fs_main`). An NPC's skin tone colours its
skin-tint shapes (`BSLightingShaderProperty` shader type 5: the body, hands
and feet), in place of the colour their model carries.

## Public evidence and assets

- [UESP: NPC_ format](https://en.uesp.net/w/index.php?oldid=3650076) lists
  `QNAM` "Skin Tone", three floats (R, G, B over 255).
- `refs/CommonLibSSE-NG`: `TESNPC::bodyTintColor // 246 - QNAM` and
  `NiAVObject::UpdateBodyTint(const NiColor&)` name it as the body's tint.
- Assets (read here): Suvaris Atheron's (`00014122`) `QNAM` is
  (85, 118, 136) / 255, and her FaceGen tint mask
  (`facetint/skyrim.esm/00014122.dds`) is (82, 117, 140) wherever no face
  layer is painted. `femalebody_1.dds` and `femalehead.dds` average about
  the same brightness (73, 60, 52 and 75, 63, 58).

## Choices

- **Blend.** The tone is applied as the head's tint mask is: an overlay,
  `albedo * tone * 2` clamped. Since the mask's skin is the tone, this makes
  the body meet the head's colour. No public source states the game's
  formula for either; a plain multiply left the body about half as bright as
  the neck above it.
- **All-zero tones.** 1134 of Skyrim.esm's 5118 NPCs have `QNAM` (0, 0, 0):
  NPCs with templates (whose traits template gives the tone) and 17 Dremora.
  Zero is taken as no tone, keeping the model's own tint, rather than a
  black body. Whether the game draws those Dremora bodies black, or their
  skins use another shader, is unchecked.
- Hair tint (shader type 6, `HCLF`) still comes from the model.
