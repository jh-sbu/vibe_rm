# Carriage attachment and tether choices

Research: `research/carriages.md`. The scripts establish independent passenger
attachments, animation idles, and horse tethers. Their attachment frames and
native-created hitch constraint are not described publicly.

Choices: keep a passenger's relative position and rotation at SetVehicle time;
apply the cart's simulated root transform on subsequent updates. Preserve
script-selected idles and packages. A player passenger receives the dynamic
RidingInCarriage keyword (000DD631), and movement input does not walk them out
of the cart. SetVehicle(None) releases the attachment and collider.

For dynamic carts, tether a kinematic anchor following the horse to the cart's
front body with a joint that holds the hitch only along the ground (world X
and Y), leaving its height and every rotation to the cart's wheels. A
spherical joint, which also held the height, flipped MQ101's carts where the
road steepens: `PrisonerCarriage01.nif`'s FrontBar hinge only steers (a
vertical axis, ±0.698 rad), so a hitch held at the horse's height levers the
cart round its front axle by about atan(drop / 180) (2026-11-08). The joint holds the cart's root
(heaviest) body at a cart-local hitch (0,180,53), based on the FrontBar's
position and front geometry, preserving its initial relation to the horse.
Picking the front-most body origin took the left front wheel (0.24 units
ahead of the bar), which the horse dragged by its axle; hitching the 100 kg
bar pulled 12000 kg through its hinge and threw the cart about (2026-11-08). The native joint's exact pivot,
limits and compliance remain unmeasured. Existing NIF wheel/steering hinges
continue to run; static carts remain static. Rebuild joints after cell loads;
save the relationship and passenger offsets rather than physics handles.
Unloaded carts preserve their initial relative angle and hitch position while
following the horse; their passengers follow their saved local frames. This
logical away movement does not simulate the cart's individual wheel bodies.

Queued SE comparison: move and turn the horse with one and several passengers;
measure the hitch location, cart yaw, passenger offset, SetVehicle(None), keyword
and collider timing, and unload/save/reload through a turn.
