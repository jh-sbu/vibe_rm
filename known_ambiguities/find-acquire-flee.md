# Find / Acquire / Flee choices

Research: `research/find-acquire-flee.md`, vanilla bound trees and MQ101
records/PEX. Exact search mechanics are not documented by those sources.

Find searches the shared package location resolver, and holder inventories
inside that area for item criteria. The Ralof corpse-key trace requires the
latter. Own inventory and known spells are included only when requested.
Faction criteria mean nonnegative faction membership; FLST criteria expand
recursively and include script additions, with cycle protection. Results are
nearest first (own inventory first), or shuffled once per successful Find.
Lists have at most 4096 entries. Empty searches stay active and retry every
three real seconds beside Wander; repeat restarts Find and replaces its list.
These ordering, retry and bounds choices are implementation policies, not
observations of SE. Saved lists retain their ordering; references are resolved
again before use, so stale results cannot resurrect deleted items.

An SE comparison probe should place matching loose items and stocked containers
at different distances, with and without own-inventory/random flags, then log
which item the actor chooses and how an empty search resumes when stocked.
Also compare nested/faction FLST criteria and save/reload between Find/Acquire.

Acquire counts matching own items toward its quota without transferring them.
It walks within 96 units of each eligible result, preserves copy/charge/ownership
state and sends inventory/OnContainerChanged events. Loose reference stacks are
taken whole, as player pickup does; container/corpse transfers stop at the quota.
Exhausted lists search again every three seconds while retaining the quota; own
items already counted are excluded from retries. These approach, stack and
retry rules are implementation policies, not SE observations. Owned items and
living holders are never eligible, whatever AllowStealing/AllowPickpocketing
say: NPC theft is blocked (`plans/ai-depth.md`, "Left out: blocked"), and
Killing remains unimplemented (no enabled users). Probe corpse/container
acquisition and save/reload between Find and Acquire.

Flee reuses combat’s open-ground navmesh sampling and running gait. It finishes
at the destination’s goal radius or when all same-space named threats are at
least DistanceToFlee away. Unnamed targets fall back to current attackers. Dead
threats are discarded only with ForgetDeadThreats. Distance zero uses the
vanilla fFleeDistanceInterior 3000 / Exterior 5000 settings. Quiet suppresses
FLEE; otherwise the existing bark channel is attempted every four real seconds.
CountsAsIsFleeing is honored when supplied; older Flee trees default true.
Avoid-node radii are used as destination tolerance; the undocumented cover/door
selection remains blocked. Away movement uses the existing walker speed until
step 12 routing. Probe timing, completion distances, quiet and IsFleeing flags.
