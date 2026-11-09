# Carriage attachment and tether choices

Research: `research/carriages.md`. The scripts establish independent passenger
attachments, animation idles, and horse tethers. Their attachment frames and
native-created hitch constraint are not described publicly.

Choices: keep a passenger's relative position and rotation at SetVehicle time;
apply the cart's simulated root transform on subsequent updates. Preserve
script-selected idles and packages. A player passenger receives the dynamic
RidingInCarriage keyword (000DD631), and movement input does not walk them out
of the cart. SetVehicle(None) releases the attachment and collider.

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
