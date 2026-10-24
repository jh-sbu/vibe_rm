# Batch 6: reference spatial queries

Public Papyrus Index / community CK wiki documentation consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getworldspace/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getscale/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getlength/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/ (GetWidth/GetHeight).

Worldspace follows current placement, including explicit moves, loaded actors,
created references and the player; interiors return None. Scale uses loaded
actor scale or placed XSCL multiplied by the same NPC and race height factors
used when spawning actors. The public scale note mentions actor multipliers.
Dimensions use X/Y/Z bounds for width/length/height, multiplied by scale.

Local UESP consulted first: OBND revision 2309011,
https://en.uesp.net/w/index.php?oldid=2309011 documents six signed 16-bit
coordinates; NPC revision 3650076 and RACE revision 3650080 describe heights:
https://en.uesp.net/w/index.php?oldid=3650076 and
https://en.uesp.net/w/index.php?oldid=3650080 . Choosing plugin bounds rather
than animated 3D bounds is an implementation policy, not verified original behavior.
No assets copied or engine binaries inspected; tests use synthetic records only.
