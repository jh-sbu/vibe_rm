# Further PEX natives

These implementations use public community documentation and existing runtime
systems. They have not been compared experimentally with Skyrim SE. References
and source-versus-inference distinctions are in refs/pex-natives/batch*.md.

## Batch 1

Alias game timers inherit our Form registration policy: one timer per object and
event, replacing previous registrations. Exact multiple-script registration
interaction and zero/negative/NaN intervals remain unverified. Ingredient
hostility considers all its effects; whether only learned effects matter is not
specified by the terse public native description. Missing string settings return
an empty string; original diagnostic text is unverified.

## Batch 2

Cell attachment uses membership in the runtime loaded-cell map. Precise Skyrim
attachment/detachment phase timing is unverified. IsStopped uses !running because
our quest start/stop transitions are synchronous; Skyrim's intermediate enabled
and stopping states are not represented. GetKey follows the existing shared
load-door lock policy; unusual pairs with two different keys are unverified.
GetVoiceType inherits the existing actor template precedence.

## Batch 3

HasCommonParent compares strict ancestors on both sides. Whether Skyrim considers
one of the two input locations itself a shared parent is unspecified. HasRefType
includes descendant locations, following our existing location reference index;
exact descendant and disabled-reference inclusion remains unverified. Reservations
use running quest reference aliases with Reserves Reference, not furniture use.
Malformed parent cycles terminate at the first repeated location.

## Batch 4

GetAllItemsCount sums copies rather than distinct base forms. The public wording
is not precise about that distinction, quest items, unplayable items or gold.
We count all positive inventory quantities; IsContainerEmpty uses the same rule.
WornHasKeyword includes every equipped item in the runtime equipment list; exact
hand-held weapon and hidden-skin inclusion in Skyrim remains unverified.

## Batch 5

GetCurrentPackage returns None for a loaded actor without a selected package;
unloaded actors report the schedule selection. Skyrim's timing and interrupt
package reporting are unverified. GetDialogueTarget uses the scene's explicit
headtracking target while speaking; headtracking and dialogue listeners need not
coincide in all scenes. Implicit scene listeners, idle pauses between lines and
non-scene ambient dialogue are unresolved and return None. Encumbrance uses our
existing NPC capacity calculation, whose precise Skyrim enforcement is unverified.
