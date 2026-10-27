# Combat movement

UESP's [CSTY revision 3089071](https://en.uesp.net/w/index.php?oldid=3089071)
names the CSCR circle, fallback, flank distance and stalk time multipliers,
CSLR strafe multiplier, and DATA dueling/flanking flags. It does not give
an algorithm. Vanilla autotest19 observed walking strafes at 74–200 units,
17/40 dueling samples and 9/40 flanking samples, with some backing off.

Chosen policy: between attacks, begin a 0.7–1.7 s orbit with probability
`circle * 0.6 * dt`; flanking uses `circle * 0.25 * dt`, a side of the target
and `1 + stalk_time * 3` seconds, with radius `reach * (0.75 + flank_distance
* 0.25)`. Close fighters may instead back off according to fallback. Archers
use CSLR for lateral movement between shots. Motion is at walking speed,
faces the target, and obeys navigation heights, collisions and keep_apart.
These rules approximate the observed magnitudes; pacing, flank units and
interaction with attack selection remain unverified. No new autotest is
built for this choice in this implementation.

NPC charging sprints use the graph's authored `iState_*Sprinting` movement
type and actions. Start beyond 1000 units and continue to the target while
stamina and action conditions permit. Fleeing actors always run (autotest19).
Charging has no successful SE observation yet; autotest20 is deferred by the
user. Sprint attacks are eligible only during a charge, with ordinary attacks
as fallback when the race has no sprint attack.

Asset survey: `hkb-vars meshes/actors/character sprint` exposes
`iState_NPCSprinting=1`, but no `AIControlledNPCSprinting` state. Both MOVT
records have forward speeds 500; AIControlled's turn rates are four times
NPCSprinting's. We choose the graph-named NPCSprinting record, rather than
substituting the faster-turning AI record without evidence for when it applies.
The original distinction remains uncertain. Stamina uses the existing player
GMST formula, including equipped armor weight, for NPCs too.
