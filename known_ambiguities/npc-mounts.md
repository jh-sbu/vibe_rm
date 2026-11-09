# NPC horse riding

Research: `research/npc-mounts.md`, vanilla XHOR, package inputs and behavior
graphs. Public documentation and assets establish riding and its associations,
but do not settle these details. Choices made to execute riding packages:

- Reserve one living, enabled horse in the same space, in stable rider order.
  Prefer XHOR, then a linked horse, then nearest horse owned by the NPC base.
  Only the NPC's own ownership counts: faction-owned horses are not used, and
  arbitrary unowned horses are not borrowed. Approach within 128 units, play
  HorseEnter, and enter the riding idle after 2.5 seconds. Exit takes 2.5 seconds
  and places the actor 100 units to the side, floor checked. IsOnMount and
  IsRidingMount include entering and exiting, but exclude approaching.
- A package of a single procedure that rides (`MQ101TulliusHoldPosition`, a
  Travel to where stage 10 put him) still asks for the horse once it is done.
  A change to another package that rides keeps the ride, mounted or getting
  on, rather than releasing it. In SE, Tullius is on his horse when the
  opening fades in, and doesn't dismount between packages until Helgen
  (observed by the user, 2026-11-08); the data's only riding packages for
  him are that hold and his patrols.
- Spawned with its cell (`fresh`, as actors who start out in furniture), an
  actor whose package rides starts out on its horse at once (`HorseEnterInstant`),
  where it was placed. Tullius is on his horse when the opening fades in and
  leads the carts from the start; mounting on foot let the cart horses
  overtake him.
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
