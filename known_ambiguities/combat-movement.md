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

NPCs don't sprint to charge. In Skyrim SE (`test-mod autotest27`,
`research/autotest27.md`) two level 15 fighters with swords and 50 stamina,
fighting each other, ran (`IsRunning`) or walked to close 1500 and 2500
units after one was moved away, never sprinting. Fleeing actors run
(autotest19). Ours runs to the target in combat. Its NPC sprint attacks
were made only at the end of a charge, so ours makes none now; whether SE's
NPCs make them otherwise is unread. autotest20's fighter never
engaged its held target, so it gave no reading. Unconfirmed: other races
and actors (creatures, a fighter with more stamina) were not read, and none
sprint here.

Asset survey (kept for the player's sprint and any NPC sprint later found):
`hkb-vars meshes/actors/character sprint` exposes `iState_NPCSprinting=1`,
but no `AIControlledNPCSprinting` state. Both MOVT records have forward
speeds 500; AIControlled's turn rates are four times NPCSprinting's.

## Scripted draws and idle sheathing (autotest19)

All four test NPCs drew on DrawWeapon, including unalerted commoners and guards.
Unalerted NPCs out of combat sheathed again between the 1 s and 3 s readings;
SetAlert(true) drew by itself. The native remains unconditional. The humanoid
AI now draws while explicitly alerted, in combat, or running a Weapon Drawn
package, and sheathes an otherwise unwanted draw 2.5 s after it. Skyrim SE
(autotest28, SheatheDelay, `research/autotest28.md`): an unalerted, free
NPC told to `DrawWeapon` read drawn 0.59 to 0.72 s later and sheathed 2.46
to 2.61 s after the call, and a second `DrawWeapon` 0.55 s after it read
drawn didn't put the sheathe off; ours doesn't either (the clock restarts
only on a draw from sheathed, and while readiness is wanted). Earlier
readings were artefacts: autotest21's "sheathed 0.52 to 0.62 s later"
stopped at its loop's 0.5 s floor before `IsWeaponDrawn` read true, and
autotest27's loop never saw it drawn. Ours reads drawn as the draw starts,
not 0.6 s in: differs, open (`plans/ROADMAP.md`). Swim
state prevents automatic draws.

