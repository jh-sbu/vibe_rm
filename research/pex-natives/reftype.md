# ObjectReference.HasRefType

Public reference consulted 2026-10-24: [Papyrus Index documentation](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/hasreftype/),
which attributes the function documentation to the community Creation Kit wiki.

ObjectReference.HasRefType reads the existing union of XLRT and location-indexed reference types; None is false.

Local UESP format references (consulted before web research):
- [NPC revision 3650076](https://en.uesp.net/w/index.php?oldid=3650076): ACBS unique bit, Use Stats, CNAM.
- [CELL revision 3570761](https://en.uesp.net/w/index.php?oldid=3570761): DATA interior bit and XLCN.
- [LCTN revision 3507725](https://en.uesp.net/w/index.php?oldid=3507725): ACSR/LCSR and PNAM.

Implementation choices are distinguished from source claims in
known_ambiguities/query-natives.md. Regression tests use generated records only.
Installed PEX declarations were inspected read-only; no game assets copied and
no engine binaries examined. The public references are summarized here, not reproduced.
