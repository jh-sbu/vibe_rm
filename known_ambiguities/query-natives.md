# Record and location query natives

References: research/pex-natives/. These choices have not been verified in Skyrim SE.

## class

GetClass follows Use Stats templates per the public NPC format. A leveled template is resolved with the existing base-query seed of zero. Skyrim SE (autotest21, `research/autotest21.md`, QueryNatives and ActorNatives): `ActorBase.GetClass` on a base whose stats template is `LCharBanditMelee1H` gave `EncClassDremoraMelee` (0x17008), neither the base's own `Citizen` nor a bandit class (the placed actor's leveled base had `EncClassBanditMelee`); `GetRace` on a base whose traits template is a leveled list gave `FoxRace` (0x109C7C), though its own `RNAM` is Nord and the placed actor was Nord. Nothing in the records links either answer to the base, so they can't be reproduced from the data; ours keeps the seeded pick, a class or race the list can give (compared 2026-11-07). Uncertain: where the game's answers come from (perhaps another actor's resolved base).

## editor

GetEditorLocation delegates to the existing location index. Skyrim SE (autotest21; autotest23, `research/autotest23.md`, CellLocation and Outdoors): the player's is None, in a cell with a location and outdoors alike (ours since 2026-11-10). A created reference's is the location where it was made: our test cell's Whiterun Hold indoors; outdoors near Riverwood `BleakwindBasinLocation`, while its current location (and the player's) read `RiverwoodLocation`. Ours takes a created reference's from its current location when made, so outdoors it may differ; how the game picks the two outdoors (the exterior cell's `XLCN` against a wider location) is open. A plugin marker and a plugin actor in our cell read the cell's location, and far map markers their cities', as ours.

## current

GetCurrentLocation uses runtime cell, actor schedule, explicit moved or created place, then editor lookup. Public documentation mentions navmesh-dependent None results for airborne dragons; that state is not yet tracked. A created item taken into the player's inventory by `AddItem(ref)` kept both the current and editor locations it had on the floor (autotest23); ours agrees, as taking a reference only disables it and leaves its place. `Location.IsLoaded` read True for the player's location and its hold, False for other cities and dungeons (autotest23).

## alive

Checked in Skyrim SE (autotest21, 2026-11-01): `GetRefTypeAliveCount` and `Location.HasRefType` count the location's own references only. Whiterun Hold read Boss alive 0 and HasRefType False, while Bleak Falls Barrow inside it read 1 and True. The conditions (`GetRefTypeAliveCount`/`DeadCount`, the location-alias forms, `LocationHasRefType`) follow the natives. That part is inferred (assumed to be the same query), not observed. Location alias fills and `IsCleared` still look inside child locations; that is unverified. We do not reproduce the reported plugin/master double-counting bug. Counts follow enabled editor references rather than actors wandering in; no Skyrim runtime comparison was performed.

## loaded

IsLoaded treats a loaded child location as loading its parents, matching our existing condition implementation. The public native description is terse and does not resolve parent inclusion or attachment-transition timing.
