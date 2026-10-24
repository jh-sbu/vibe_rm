# Batch 1: alias game timers, string settings and ingredient hostility

Public community documentation consulted 2026-10-24 (Papyrus Index, with
Creation Kit wiki attribution):
- https://papyrus.bellcube.dev/skyrimse/script/alias/function/registerforsingleupdategametime/
- https://papyrus.bellcube.dev/skyrimse/script/alias/function/registerforupdategametime/
- https://papyrus.bellcube.dev/skyrimse/script/alias/function/unregisterforupdategametime/
- https://papyrus.bellcube.dev/skyrimse/script/game/function/getgamesettingstring/
- https://papyrus.bellcube.dev/skyrimse/script/ingredient/function/ishostile/

Timers deliver OnUpdateGameTime after game hours; repeating registration repeats,
and unregistration cancels that event. These now share the Form timer path.
String settings use the existing localized GMST reader. Ingredient hostility uses
the existing magic-item effect flags, including effects not learned by the player.
That last choice follows the existing potion/spell classification and is an
implementation inference rather than a Skyrim SE experiment.

Local UESP index searched first (Papyrus); its en.uesp.net dump does not contain
the separate CK wiki. Installed PEX native names were inspected read-only. No
assets were copied; no engine binaries were inspected. Tests are synthetic.
