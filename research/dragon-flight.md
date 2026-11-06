# Dragon flight and Shout procedures

2026-11-06. Examined the installed five vanilla masters, Actor.pex, dragon NIF
and behavior project externally, using pack-tree, pack-census, esp-dump,
pex-natives, pex-dump and hkb-events. No assets are included in the repository.

- DragonRace 00012E82 links FLMV to Dragon_Flying_MT 0001BCFA. Its SPED is
  40 bytes, with walk/cruise speed 2000, run speed 7400 and two turn rates
  about 1.571 radians/s. The generic MOVT parser previously required 44 bytes;
  the omitted moving turn rate now defaults to the run turn rate.
- DragonProject.hkx names DragonBehavior.hkx. TakeOff enters ST_TakeOff;
  FlyStartCruise selects ST_Flight; HoverStart/FlyStartHover select hovering;
  Glide changes flapping to gliding. FlyStopDefault, FlyStopInjured and the
  FlyStopPerch* events enter the corresponding landing paths. LandEnd and
  PerchLandEnd leave landing. These are graph observations, not an SE run.
- Orbit arguments are Location, InnerRadius, OuterRadius, Height. Hover takes
  Location, Height. DefaultDragonMasterPackageTemplate selects a linked perch,
  linked Patrol, or Orbit. ShoutFromOrbit runs Shout and Orbit simultaneously;
  ShoutFromHover sequences Hover then Shout, demonstrating a finite hover
  arrival. DragonPerchAndShout sequences Sit then repeating Shout.
- Shout arguments are travel location, SHOU selector, target, HoldWhenBlocked,
  optionally Just Headtrack. SHOU words identify backing spells and recovery.
  The prior code parsed shouts and possession but had no package or combat
  Shout executor. Both are now connected to the backing spell pipeline.
- Alduin's three FlightGrab templates sequence a finite Patrol before and after
  FlightGrab. The second FlightGrab argument is true in the explicitly named
  NoCollisionCheck template and in MQ304. MQ304 also checks a package actor
  variable before grabbing. Its package fragment PF_MQ304AlduinFlightGrabLostS_
  000E9977 sets quest stage 80; this remains a normal package-fragment action.
- `to_Flight_Kill_Grab_Enter` selects the approach clip;
  `to_Flight_Kill_Grab_Action` selects Special_TalonGrabExitVert.HKX. That clip
  raises FlightActionGrab at 0 with payload `NPC RLegFoot`, and
  FlightActionRelease at 1.067 seconds with the same bone payload. Payload
  events remain internal, consistent with autotest19; the new body handler
  consumes the release without forwarding it to Papyrus.

[UESP CSTY, revision 3089071](https://en.uesp.net/w/index.php?oldid=3089071)
documents the eight CSFL flight-style fields, including hover, ground and perch
chances and time multipliers. It does not prescribe a selection algorithm.
[UESP Dragon, revision 3590310](https://en.uesp.net/w/index.php?oldid=3590310)
and [Dragonrend, revision 3415114](https://en.uesp.net/w/index.php?oldid=3415114)
describe flight/landing, including the need for a valid landing place.

Local UESP searches for native flight documentation found only function indices.
The CK native pages returned HTTP 403. Vanilla Actor.pex confirms SetAllowFlying,
SetAllowFlyingEx, SetForcedLandingMarker, GetForcedLandingMarker, IsFlying and
GetFlyingState signatures. Public Papyrus Index displays of vanilla native
comments describe state values 0 ground, 1 takeoff, 2 cruise, 3 hover, 4 landing,
and a forced marker as the location to use when landing, rather than a landing
request on its own. External reference links and bounded notes are retained in
ignored refs/ai-flight-sources.md. No public source implementation was used.

Validation includes the real vanilla graph taking the controller's flight/grab
state events, and synthetic loaded/away procedure, attachment and save tests.
