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
