# Fast travel

Research uses public documentation and synthetic plugin records only. No engine
binary data is examined and no game assets are committed.

## Sources

- Local UESP [Transport, revision 3560422](https://en.uesp.net/w/index.php?oldid=3560422),
  Fast Travel: discovered exterior destinations, distance-dependent elapsed game
  time, hostile enemies, overencumbrance and airborne restrictions. It also lists
  restricted exterior areas, Survival Mode and dragon-riding exceptions.
- Public CK documentation mirrored by the Papyrus Index:
  [Game.EnableFastTravel](https://papyrus.bellcube.dev/skyrimse/script/game/function/enablefasttravel/)
  and [Game.IsFastTravelEnabled](https://papyrus.bellcube.dev/skyrimse/script/game/function/isfasttravelenabled/).
  The enable argument defaults to true. The Enable page says player MoveTo and
  SetPosition reenable travel. The CK site itself returned HTTP 403.

## Script control

The global gate starts enabled and is distinct from temporary eligibility checks.
Only script MoveTo / SetPosition of the player reset it; moving another reference
or changing the player's angle does not. Exact timing relative to deferred
player movement is unverified: this runtime reenables on accepting the native.

## Marker permissions and native saves

[ObjectReference.EnableFastTravel](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/enablefasttravel/)
documents explicit disabling even for an otherwise accessible map marker, with
an undiscovered-location error. This runtime keeps denial separately from actual
discovery: it changes the icon/permission without erasing the discovery statistic
or announcing rediscovery. AddToMap(true) and EnableFastTravel(true) clear denial.
Exact icon/discovery interactions in SE remain unverified.

Optional native `MAPS` saves the script gate, shown/discovered markers, explicit
travel grants and denials. Loading replaces these sets. Legacy saves clear them
and enable the global gate; plugin flags still apply. This is our continuity
format, not the original `.ess` representation.

## Refusals

The worldspace DATA bit 1 forbids departure, as documented by local UESP
[WRLD revision 3192583](https://en.uesp.net/w/index.php?oldid=3192583).
The runtime checks the script gate, life, exterior/world flag, carry weight,
airborne state, pursuing guards, detrimental health effects, combat/search and
nearby hostile actors. Nearby uses the existing combat engagement radius without
requiring line of sight; the exact Skyrim refusal radius and priority are not
published. Damage checks share waiting's detrimental-health-effect predicate,
not every possible damage source. GMST lookups use English fallbacks when absent;
exact message-key coverage and priority remain unverified.

Mounted exceptions, dragon riding and Survival Mode are not implemented because
the runtime lacks those systems. Script gate state does not claim to report all
transient player eligibility. Invalid/unknown location never permits departure.

## Arrival references

The CK [Game.FastTravel documentation mirror](https://papyrus.bellcube.dev/skyrimse/script/game/function/fasttravel/)
says linked fast-travel points on map markers are honored. The map destination
resolver uses the unkeyed XLKR link (keyworded links do not redirect arrival),
otherwise the marker itself, with its current position and heading. Exactly
which links Skyrim prioritizes if multiple unkeyed links exist is unverified;
the existing first-matching-link reader is used. Broken, disabled, deleted,
nonfinite or unplaced targets fail before any cells unload. No link chains are
followed. Interior linked destinations are accepted; native edge cases and
ragdoll behavior remain unverified.

## Relocation

Accepted map travel uses the existing exterior/interior loaders, including feet
clearance, ground correction, heading, weather setup, streaming and party hooks.
All known refusals and destination validation happen before unloading. The map
closes only on successful arrival. A loading-screen presentation, autosave timing
and the distinct scripted Game.FastTravel native remain separate work.
