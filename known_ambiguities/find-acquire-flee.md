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
