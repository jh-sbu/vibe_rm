# Observation, idle markers and group packages

Read vanilla master PACK trees and Actor.pex with vrm-tool at their external
paths, 2026-11-06. No game assets are copied into the repository.

UseIdleMarker 000283F0 has one procedure with one Marker reference. It has no
wander or timeout input. SandboxAndKeepEyeOn 0006C872 runs Sandbox alongside
KeepAnEyeOn(TargetToObserve, ObservationArea, EndPursuitArea).

GroupMovement 0006DE44 is simultaneous Guard (conditional, remains blocked)
and a Stacked body tree. Actor1 patrols or alternates Destination1/2. Authored
GetDead/GetIsReference conditions promote the next living actor when needed.
Other actors Follow the first living leader with the template's radii. The
tree rather than a replacement leader algorithm decides those roles.
SingleFileFollow 000C5811 explicitly makes Follower1 follow Leader,
Follower2 follow Follower1 (fallback Leader), and Follower3 follow Follower2
(fallback Follower1/Leader). If Leader dies it Travels to its destination.
FollowTo 00025E6D is FollowTo(target, destination, min/max, accompany) then
Travel(destination). FollowPlayer 000750BE and FollowerPackageTemplate 000D530D
branch on actor value WaitingForPlayer (95); their Follow leaf passes MinRadius
twice. We retain that authored choice rather than substitute the declared Max.

TravelCrouchWhenNearTemplate 00054868 is two Travel leaves: input 0 is
"Standing: Place to Travel", input 3 "Crouching: Place to Travel". There is no
separate proximity condition or numeric sneak radius in its tree. The two
locations supply the approach boundary and the crouched final leg.

Actor.pex declares vanilla natives SetLookAt(ObjectReference akTarget,
Bool abPathingLookAt) and ClearLookAt(). The package/PEX assets do not reveal
the exact controller priority, pursuit or pathing-look-at behavior.
