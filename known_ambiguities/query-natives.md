# Record and location query natives

References: research/pex-natives/. These choices have not been verified in Skyrim SE.

## unique

IsUnique reads the queried base directly, consistent with the public flag description. Whether unusual templated NPCs inherit uniqueness is unverified.

## class

GetClass follows Use Stats templates per the public NPC format. A leveled template is resolved with the existing base-query seed of zero; the original return for an unresolved leveled template is unverified.

## editor

GetEditorLocation delegates to the existing location index, including its created-reference and player policies. Their precise Skyrim behavior remains unverified.

## current

GetCurrentLocation uses runtime cell, actor schedule, explicit moved or created place, then editor lookup. Public documentation mentions navmesh-dependent None results for airborne dragons; that state is not yet tracked. Exact inventory-held reference location behavior is unverified.

## alive

Location population queries include descendant locations, as the existing condition helper does; public documentation does not specify recursion clearly. We do not reproduce the reported plugin/master double-counting bug. Counts follow enabled editor references rather than actors wandering in; no Skyrim runtime comparison was performed.

## loaded

IsLoaded treats a loaded child location as loading its parents, matching our existing condition implementation. The public native description is terse and does not resolve parent inclusion or attachment-transition timing.
