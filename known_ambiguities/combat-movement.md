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
