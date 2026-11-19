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
package, and sheathes an otherwise unwanted draw after 1.5 s drawn. It
was 0.6 s, which rested on a misread: autotest21's AlertFlag loop (an unalerted NPC told to
DrawWeapon, "sheathed" 0.52 to 0.62 s later) stopped at its 0.5 s floor
because `IsWeaponDrawn` hadn't yet read true. autotest27 (SheatheFree,
`research/autotest27.md`) repeated it with a free NPC and never saw the
weapon drawn within the loop. The only timing known is autotest19's: drawn
at 1 s, sheathed by 3 s. 1.5 s, between the two, is a choice until a
reading times it (`plans/ROADMAP.md`, autotest27's follow-ups). Whether a repeated DrawWeapon restarts the delay
wasn't measured; chosen here: the delay counts from the last
accepted/requested draw and resets while readiness is wanted. Swim
state prevents automatic draws.

