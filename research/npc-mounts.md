# NPC riding

2026-11-06. Examined installed vanilla Skyrim SE assets externally with
`esp-dump`, `pack-tree`, `pex-natives`, `pex-dump Actor`, and `hkb-events`.
No asset or script is included here.

ACHR XHOR associates riders 000198BA → 000A9DE7, 000198BC → 000527F1,
and 0002BFA2 → 000654E1. XHOR also associates DLC2 riders with dragons;
therefore XHOR alone does not identify a horse. ActorTypeHorse 00026110
identifies horses. Ownership supplies another association, and linked horses
are supported. The implementation chooses an explicit XHOR first, then a linked
horse, then the nearest available horse owned by the rider's NPC base.

Bound procedure argument positions (zero based): Travel 1, Patrol 4,
Follow 4 (slot 3 is accompany), Escort 6, Wander 2. GroupMovement and Acquire
reach these leaves through their trees.

The humanoid graph accepts HorseEnter, HorseEnterInstant, HorseExit and
HorseExitOut. HorseEnter/Exit play the paired mount/dismount clips;
HorseEnterInstant selects HorseRider_State. The horse skeleton has SaddleBone.
Actor.pex declares IsOnMount() → Bool and Dismount() → Bool as natives.

[UESP Horses, revision 3636263](https://en.uesp.net/w/index.php?oldid=3636263)
documents faster mounted travel but does not specify NPC selection, transition
query timing, or away travel speeds. Those choices are recorded separately.
