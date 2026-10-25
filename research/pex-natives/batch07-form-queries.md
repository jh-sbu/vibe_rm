# Batch 7: form queries

Public Papyrus Index / community CK wiki pages, consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/form/function/getgoldvalue/
- https://papyrus.bellcube.dev/skyrimse/script/actorbase/function/getdeadcount/
- https://papyrus.bellcube.dev/skyrimse/script/topicinfo/function/getowningquest/

Gold value reads base item data (no enchantment/barter adjustment), returning -1
for forms without a value. Dead counts enumerate distinct placed/created actors
using the queried base, including initially dead references; this is a
current-count interpretation.
TopicInfo ownership follows the INFO group's DIAL then its QNAM quest.

Local UESP consulted first: DIAL revision 2640560,
https://en.uesp.net/w/index.php?oldid=2640560 and INFO revision 2346674,
https://en.uesp.net/w/index.php?oldid=2346674 . Item value layouts use the
existing inventory parser, with ARMO revision 2308925 already consulted.

No engine binary was read. No game assets copied; tests are synthetic only.
Uncertainties are in known_ambiguities/remaining-natives.md.
