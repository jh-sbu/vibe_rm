# Batch 8: movement, arrest and loaded 3D

Public Papyrus Index / community CK wiki pages, consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/isrunning/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/issprinting/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/isalarmed/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/isarrestingtarget/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/is3dloaded/

Running uses actual movement and running gait, excluding the player's sprint.
NPCs share the existing movement-type gait resolver; they do not sprint in this
runtime. Alarm uses the existing guard pursuit state. Arresting requires a guard,
an alarm and that guard's current arrest conversation (a narrow interpretation).
Loaded 3D checks actual render instances, paired actor render/runtime entries,
or the player's instantiated body. Occlusion and hidden render flags do not
remove the 3D. Cell membership alone is insufficient; distant LOD is excluded.

Local UESP searched first: Crime revision 3639929,
https://en.uesp.net/w/index.php?oldid=3639929 and footstep sets revision 3507711,
https://en.uesp.net/w/index.php?oldid=3507711 . These provide context, not precise
native transition contracts. Policies and unresolved timing are recorded in
known_ambiguities/remaining-natives.md. No game assets copied or engine binaries
inspected. Tests use synthetic records and synthetic render entries only.
