# Math.radianstodegrees

Public source consulted 2026-10-24: [Creation Kit Math reference, revision 23257](https://ck.uesp.net/w/index.php?title=Math_Script&oldid=23257).
Documents radian to degree conversion. Implemented using f32 operations with explicit
angle conversion at the API boundary. Synthetic regression cases include negative,
zero and positive values. Exceptional input uncertainty: known_ambiguities/math-natives.md.
Installed Math.pex native declarations were inspected read-only; no assets copied
and no engine binary examined.
