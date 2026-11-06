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
front body with a spherical joint. Pick the body nearest the hitch, using a
cart-local hitch (0,180,53), based on the FrontBar's position and front geometry;
preserve its initial relation to the horse. The native joint's exact pivot,
limits and compliance remain unmeasured. Existing NIF wheel/steering hinges
continue to run; static carts remain static. Rebuild joints after cell loads;
save the relationship and passenger offsets rather than physics handles.

Queued SE comparison: move and turn the horse with one and several passengers;
measure the hitch location, cart yaw, passenger offset, SetVehicle(None), keyword
and collider timing, and unload/save/reload through a turn.
