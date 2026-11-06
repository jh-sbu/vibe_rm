# NPC horse riding

Research: `research/npc-mounts.md`, vanilla XHOR, package inputs and behavior
graphs. Public documentation and assets establish riding and its associations,
but do not settle these details. Choices made to execute riding packages:

- Reserve one living, enabled horse in the same space, in stable rider order.
  Prefer XHOR, then a linked horse, then nearest horse owned by the NPC base.
  Do not borrow arbitrary unowned horses. Approach within 128 units, play
  HorseEnter, and enter the riding idle after 2.5 seconds. Exit takes 2.5 seconds
  and places the actor 100 units to the side, floor checked. IsOnMount and
  IsRidingMount include entering and exiting, but exclude approaching.
- The rider keeps its package/cursor; movement uses the horse's movement type
  and collider, and rendering follows SaddleBone. Combat/package changes or
  death release the relationship. Dismount inhibits remounting that package.
- Away transitions are immediate. The rider advances at the horse's walk/run
  speed according to package gait; the horse follows its position and does not
  execute independent travel. This speed is a choice: autotest16 measured
  walkers only. Existing explicit-jump timing remains unchanged.
- MOUN saves association, phase, elapsed transition time and movement speeds;
  bodies and attachments are derived after loading. Old saves have no mounts.

Queued SE comparison: a rider with XHOR and another owned horse, two riders
competing for one horse, query timing during mount/exit, Dismount return value,
combat and load-door interruptions, and away travel with walking/running gaits.
