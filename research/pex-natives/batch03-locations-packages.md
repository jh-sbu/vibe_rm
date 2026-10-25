# Batch 3: location hierarchy and package ownership

Public Papyrus Index / community CK wiki pages consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/location/function/ischild/
- https://papyrus.bellcube.dev/skyrimse/script/location/function/hascommonparent/
- https://papyrus.bellcube.dev/skyrimse/script/location/function/hasreftype/
- https://papyrus.bellcube.dev/skyrimse/script/package/function/gettemplate/
- https://papyrus.bellcube.dev/skyrimse/script/package/function/getowningquest/

IsChild tests whether the argument is a strict descendant of the receiver,
including grandchildren. HasCommonParent compares strict ancestor chains and
filters the shared ancestor by keyword; identical locations return false.
HasRefType excludes references reserved by running quest aliases.

Local UESP consulted first: LCTN PNAM and ACSR/LCSR, revision 3507725
(https://en.uesp.net/w/index.php?oldid=3507725), and PACK revision 2309013
(https://en.uesp.net/w/index.php?oldid=2309013): PKCU template at offset 4,
header QNAM owner versus later fragment QNAM. Package getters remap form IDs.
Cycle handling and descendant ref-type search are implementation policies;
see known_ambiguities/remaining-natives.md. No game assets copied and no engine
binaries inspected. Tests use synthetic records only.
