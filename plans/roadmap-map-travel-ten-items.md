# Ten map and travel items

Use only public references and permitted assets, never executable data. Tests
use generated records. Each numbered implementation is committed separately.

1. Game.EnableFastTravel / IsFastTravelEnabled and scripted movement reset.
2. Durable global and per-marker travel permissions, including explicit denial.
3. Eligibility checks and localized refusal reasons.
4. Linked arrival reference resolution and destination validation.
5. Relocation through the existing exterior/interior loaders.
6. Distance-dependent game time and subsystem catch-up.
7. Marker selection, confirmation and revalidation from the world map.
8. Following teammates across travel and load doors.
9. Child-worldspace markers on the parent's map.
10. Active quest targets on the world map.

Progress and validation are recorded below as each item completes.

## 1 completed

Implemented global script control, default-true enable and player MoveTo /
SetPosition reset. The production native-dispatch regression passed with host
Vulkan. The sandbox adapter cannot create the existing terrain pipeline.
