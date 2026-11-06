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
Permission filtering skips living pockets unless both stealing and pickpocketing
are allowed. Live attempts reuse the picker-specific player chance/perk formula.
The actual Ralof use is corpse loot, so this extension is a policy, not an
observed NPC pickpocket rule. Killing remains unimplemented (no enabled users).
Exhausted lists search again every three seconds while retaining the quota; own
items already counted are excluded from retries.

Autonomous theft preserves the stolen owner, uses actor-specific detection and
crime faction reporting rules, sends ADCR with the actual culprit, and reuses
theft reactions; reporting guards attack that culprit. It never adds to the
player’s bounty or statistics. Public Crime documentation does not describe
autonomous NPC arrest; this uses the existing actor combat/reaction machinery,
with no NPC arrest simulation invented. Probe owned-item acquisition in front
of owners/guards and live-pocket success/failure, including commanded followers.

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
