# Lip/face research gate (2026-10-27)

Step 9a stops here. No runtime LIP decoder, head morph renderer or speaking
phoneme driver is enabled. Open questions belong in this note and ROADMAP,
not known_ambiguities: no guessed lip interpretation was chosen.

## Permitted evidence

Only game assets were inspected, read through VFS/BSA into memory. No game
assets were written into the repository (including refs), installed, or run.
The local UESP index was checked first: title searches for LIP, TRI, lip sync
and morph under Skyrim Mod returned no format documentation. No third-party
source code was used for this spike.

Reproduction:

- `vrm-tool lip-dump <Data> [voice path.fuz]`: omitted path lists candidates;
  a path prints the bounded FUZE envelope and raw LIP header interpretations.
  The latter explicitly does not assign fields or infer phoneme tracks.
- `vrm-tool tri-dump <Data> [path.tri]`: bounded diagnostics and, for the
  supported layout, morph names and peak scaled deltas.
- `vrm-tool tri-dump <Data> --survey`: checks every TRI's supported layout to
  exact EOF, counting morph names and reporting unsupported variants.

## FUZE and LIP

FUZE version 1 has a 12-byte envelope: magic, little-endian version, lip byte
count, then the lip payload followed by encoded audio. Its ranges can be
validated; zero lip size means no payload. Sampled payloads:

| Voice / INFO path suffix | LIP bytes | First three u32 interpretations |
| --- | ---: | --- |
| crdragonvoice/dlc1vqdrag_dlc1vqdragonint_0001156b_1 | 3777 | 1, 10172, 9 |
| malenord/bardscolle_bardscollegepoe_000e774d_1 | 5487 | 1, 19960, 13 |
| femalenord/bqshared_bqsharedsharedinf_000cba95_1 | 7343 | 1, 20092, 13 |

Dragon sample comes from dawnguard.esm, human samples from skyrim.esm in
Skyrim - Voices_en0.bsa. Following bytes do not establish a simple aligned
float-key layout. The second word could describe unpacked data, but neither
compression nor key layout nor track-to-phoneme mapping has been established.
Treating arbitrary words as float tracks would be a guess. Further asset
research is needed before implementing 9b/9c; audio without LIP stays without
lip motion.

## TRI observations

Meshes0 contains 569 TRI assets. The diagnostic reader validates 560 to exact
EOF; nine have nonzero header fields that it deliberately rejects (including
childhead and some Khajiit/Argonian parts). The supported FRTRI003 layout has
a 64-byte header; vertex/triangle/quad/UV counts at 8/12/16/28, UV topology
flag at 32, morph count at 36. It contains xyz float positions, indexed faces,
UV floats and optional UV topology, then length-prefixed names (usually NUL
terminated), float scale and signed-int16 xyz deltas per vertex. Unsupported
header fields are not given guessed meanings. Diagnostic peak deltas use
scale times the signed integer components.

Malehead has 898 vertices, 1694 triangles and 44 morphs. Repeated names across
parts include Aah, BigAah, BMP, ChJSh, DST, Eee, Eh, FV, I, K, N, Oh, OohQ, R,
Th, W, brow/blink/squint targets, moods and dialogue expressions. There are
spelling/case variations in the assets. This establishes named targets but
not correspondence to LIP tracks, nor correspondence between each TRI's
vertices and a generated FaceGen NIF. iLipMorphCount=31 does not settle either.
Those mappings and the nine TRI variants remain open. GPU morph support
(9b) and animation driving (9c) remain blocked by this gate.
