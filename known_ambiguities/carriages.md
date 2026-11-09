# Carriage attachment and tether choices

Research: `research/carriages.md`. The scripts establish independent passenger
attachments, animation idles, and horse tethers. Their attachment frames and
native-created hitch constraint are not described publicly.

Choices: on a cart (a vehicle that isn't an actor), a passenger's root goes
to the cart's origin, facing its way, and the seat is the idle's: the cart
idles' clips (`cartprisonerasway.hkx` to `cartprisonerdsway.hkx`,
`cartdriveridlesway.hkx`) hold the body 63 to 200 units back, about 40 to
either side and 139 up from the root, as benches in the bed and the
driver's seat. MQ101 calls `SetVehicle` for its other cart's prisoners
while they stand beside the player's cart, then plays their seat idles, so
SE can't seat them where they stood (2026-11-08). On an actor (a horse, a
dragon), the relative position and rotation at SetVehicle time are kept.
The vehicle's transform is applied on subsequent updates. Preserve
script-selected idles and packages. A player passenger receives the dynamic
RidingInCarriage keyword (000DD631), and movement input does not walk them out
of the cart. SetVehicle(None) releases the attachment and collider.

The riding player's view follows the seated body, since their root is at the
cart's origin: the eye is the posed third-person skeleton's `NPC Head [Head]`
(its bind height, 120.3, is the standing eye's 120), and the seat's heading,
from which `Game.SetSittingRotation` turns the first-person view, is the
cart's plus the idle's turn of `NPC COM [COM ]` from its bind pose. The body
itself keeps the cart's heading. In the opening the player's idle holds the
body 15 left of the cart's centre line facing about 95 degrees right, across
the bed, and MQ101 stage 15 calls `SetSittingRotation(-55)`. Measured from
the cart, -55 put the view against the left side wall beside the player.
Measured from the seated body, it looks forward across the bed at the
other prisoners (2026-11-09). The player's body rides
as other passengers do, tilting with the cart: an upright body at the cart's
origin dropped the seated head back and down past the next seat on MQ101's
slopes. The view tilts with the seat too (its pitch follows the floor's
slope along the view's heading), so it stays level with the cart. The
seats are the clips' own. MQ101 seats the player in C (`IdleCartPrisonerCIdle`), Ralof in D,
Lokir in B and Ulfric in A. The player's head rides at about (-16, -110, 185)
in the cart's frame and Ralof's at (21, -69, 182): across the bed and about
40 units forward of it, as the NPC clip for seat C (`CSway`, a soldier in
the other cart) also has it. The first-person `cartprisonercidle.hkx` puts
`Camera1st [Cam1]` at about (-38, -107, 187), level with the head bone
(2026-11-09). Unverified in SE: whether SE uses the torso's facing, the
head's (about 84 degrees here), or a seat heading of its own; whether its
camera is the first-person rig's `Camera1st` (about 23 units behind the head
here) rather than the head; whether its view tilts with the cart; and
whether the view sways with the idle as it does here.

Loaded carts, while hitched, are keyframed rather than simulated (decided
with the user, 2026-11-08). Each frame the hitch point, a cart-local
(0,180,53) based on the FrontBar's position and front geometry, goes where
the horse holds it (its relation to the horse kept from when the tether was
made); the rear axle trails it at its distance, as a trailer's does; and the
cart sits on the ground under its wheels: rays down to the world's fixed
collision at each wheel's underside, the cart's height, pitch and roll
fitted to them (pitch and roll eased in, a fifth of the way each frame, and
held within a slope of 0.8). Wheels are the cart's bodies reaching lowest;
they spin with the distance gone. The NIF's hinges have no say meanwhile,
and the front wheels don't steer. Untethered, the bodies go back to the
simulation.

Why: SE's carts roll down MQ101's mountain road upright and steady (the
user's observation). Simulated in Rapier, with the horse pulling the cart
along the ground, they lagged, tipped and tumbled: `PrisonerCarriage01.nif`
joins a 12000 kg body to 150 kg wheels and a 100 kg steering bar, ratios
Rapier's joints don't hold, and a hitch that also held the height levered
the cart round its front axle where the road steepens. Hitched by the
front-most body origin, the horse dragged the cart by its left front wheel.
What SE does (a Havok constraint between horse and cart, its pivot, limits
and compliance) remains unmeasured, and keyframed carts don't react to
being hit while hitched. Static carts remain static. Rebuild hitches after
cell loads; save the relationship and passenger offsets rather than physics
handles.
Unloaded carts preserve their initial relative angle and hitch position while
following the horse; their passengers follow their saved local frames. This
logical away movement does not simulate the cart's individual wheel bodies.

Queued SE comparison: move and turn the horse with one and several passengers;
measure the hitch location, cart yaw, passenger offset, SetVehicle(None), keyword
and collider timing, and unload/save/reload through a turn.
