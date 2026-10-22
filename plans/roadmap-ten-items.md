# Ten roadmap implementations

Each numbered item is planned before implementation, delegated to a Luna 6 High
subagent, reviewed and validated by the parent, and committed independently.
Only public sources (including local references) and game assets may be used;
engine binary inspection and leaked material are forbidden. No game assets may
be committed. Unresolved behavior belongs in `known_ambiguities/`.

## 1. Actor.GetKiller

Roadmap: Game logic / death conditions. Expose the existing durable death record's
killer through the Actor Papyrus native. Return an Actor reference or None when
no killer is recorded; preserve existing death, resurrection and save behavior.
Research the public native contract using local UESP first and public CK or
community source as needed. Test the actual native dispatcher for known and
unknown killers, living actors, resurrection and restoration where practical.
Record unsupported death-cause/limb queries and any attribution ambiguity.

Outcome: implemented and reviewed. Parent validation: all five death-condition
tests and the existing DETH roundtrip/old-save regression passed using system
Vulkan. Formatting and diff checks passed. Follower/summon attribution remains
unverified and is recorded in `known_ambiguities/death.md`.

## 2. Created-reference starting transforms

Roadmap: Game logic / positional conditions. Retain an immutable starting
position and rotation when a reference is created in the world, and use these
for `GetStartingPos` / `GetStartingAngle` after moves or physics updates.
Keep placed-reference queries on their plugin DATA. References created inside
inventories have no world starting transform and remain unsupported. Extend
native saves with an optional section without changing the existing WRLD layout;
old saves cannot recover a lost origin and must not invent one. Test creation,
movement, per-reference isolation, angle units, save roundtrip, old saves and
malformed payload rejection. Research public semantics first; explicitly record
unverified Skyrim SE created-reference semantics rather than claiming parity.

Outcome: implemented and reviewed. Parent validation passed all nine geometry
tests, both ORIG codec tests, and capture/application into another Engine with
system Vulkan. World creation paths retain distinct immutable origins; legacy
saves keep origins unknown. Source and save-format limits are documented.

## Selection

Items 1–8 are bounded roadmap subitems. Items 9–10 address
save-continuity gaps found while reading the roadmap's remaining magic/save work:
their inventory and effects can survive a native save while the restriction's
runtime memory currently does not. Each receives a concrete plan below before
delegation; candidates blocked by evidence will be documented and replaced.

## 3. Quest-alias keys

Roadmap: Game logic / locks' alias-given keys. Research alias CNTO inventory
lifecycle using the local QUST reference (revision 3507775) and public CK
documentation. Parse/remap reference-alias inventory entries and make authored
keys available through normal inventory and lock access. Apply grants only after
a successful fill; support forced refills/clears, quest stop/restart and native
save restoration without replaying grants. Public sources conflict on grant
lifetime: UESP says duration-of-quest, while the public CK page mirror says
permanent. Follow the more specific CK alias-tab description and document the
uncertainty. Support direct inventory forms only; reject counts above the signed
runtime limit and cap additions to prevent overflow. Test malformed/location-
alias entries, startup failure, independent aliases, key use, native clear/refill
and save continuity. Document unsupported list forms and same-reference
ForceRefTo policy.

Research refinement before implementation: the public CK documentation mirror at
<https://tesck.ru/index.php/Вкладка_Quest_Alias> preserves the English Alias Data
description: inventory is granted on fill and remains after the reference leaves
the alias. This conflicts with UESP's duration wording. The CK original returned
403. Implement permanent grants, record this source conflict, and avoid inventing
temporary inventory/provenance removal. Actual fill transitions grant; loading
already filled aliases must not replay grants. Same-reference ForceRefTo and
refill repetition need an explicit, tested policy if evidence does not settle it.

Outcome: implemented and reviewed; all seven lock tests passed with system
Vulkan, including the new alias-inventory native/lock/save regression.
Direct valid items are granted
through normal AddItem handling after successful fills; list forms remain outside
scope. Same-reference ForceRefTo is idempotent, while clear/refill and quest
restart are new fills. Synthetic regression coverage exercises failed startup,
key access, save restoration, repeated grants and signed-count saturation.

## 4. Package placement headings

Roadmap: AI depth / headings at placement. Use the already recorded autotest6/7
observations: Sandbox about a reference adopts that reference's heading; Travel
turns toward its marker's heading after loading. Resolve the current package's
reference, linked-reference and quest-alias target consistently with location
resolution. Apply heading only for scheduled placement/arrival of living actors;
preserve explicit script moves, dead actors, newly created actor placement and
unrelated package types. Keep ordinary movement turning behavior intact. Test
target resolution, moved markers, angle convention and lifecycle exclusions
through the production helper/placement path. Document the distinction between
observed cases and extrapolation to alias/linked targets or waiting jumps.

Outcome: implemented and reviewed. Parent validation passed all six heading
regressions (including delayed initial evaluation) and all five schedule tests
with system Vulkan. Formatting and diff checks passed. Sandbox headings are applied
to scheduled spawn transforms; Travel heading turns are queued only for actors
that crossed the observed stop-distance threshold and are applied gradually
while idle. Runtime updates clear stale turns on combat, movement, package or
facing changes. The threshold and additional linked/alias target forms are
inferences recorded in `known_ambiguities/unloaded-placement.md`.

## 5. Sleep screen fades

Roadmap: Game logic / sleeping fading the screen out and in. Add a sleep-owned
fade layer: fade the world to black before advancing sleep hours, hold it while
sleeping, and fade back after completion or interruption. Ordinary waiting and
canceling the hours chooser do not fade. Ensure sleep's fade does not replace a
script's Game.FadeOutGame fade and cannot leave the screen black after an external
menu close or save load. Use the existing image-space/post-processing path and
menu-time updates. Test transition boundaries, hour/event timing, cancellation,
interruption, waiting exclusion and independent script fades. Record chosen
durations and original-game timing uncertainties as policies.

Outcome: implemented and reviewed; all five sleep/fade regressions passed with
system Vulkan, including actual Engine update and native-load paths. Formatting
and diff checks passed. A sleep-owned black layer
fades over 0.5 seconds before the first hour, holds while sleep advances, then
fades back over 0.5 seconds. Menu time advances only this layer, leaving script
image-space clocks paused. External menu close and native load clear transient
wait/fade state; original-game timing and external-close event behavior are
recorded in `known_ambiguities/sleep-wait.md`.

## 6. Detection stealth-point persistence

Roadmap: Saves / detection. Persist the player and actors' existing stealth-point
reservoirs and regeneration delays in an optional native section. Restore by
replacement, including clearing old in-memory entries for legacy saves; invalidate
derived detection caches and schedule a fresh observation. Keep the existing
drain/refill algorithm. Validate finite 0..100 points and finite nonnegative
delays before apply; deterministic encoding, roundtrip, malformed/duplicate
entries and legacy replacement need tests. Verify continuation of a partially
drained reservoir and delayed regeneration through the production capture/load
path. Search/combat execution and AI progression remain separate roadmap work;
describe this bounded subset explicitly. This is native continuity, not an
assertion about Skyrim's original save payload.

Outcome: implemented and reviewed; the codec regression and production
capture/read/apply continuation test passed with system Vulkan. Formatting and
diff checks passed. Optional `STPT` replaces
player/actor reservoirs and regeneration delays; loading clears stale detection
observations and schedules a fresh update. Codec limits, legacy absence and the
bounded search/combat/AI exclusion are documented in `known_ambiguities/`.

## 7. Message-box waits across native saves

Roadmap: Saves / threads waiting on message boxes. Extend VM snapshot support to
retain a signal wait when its host-side message box is also saved. Preserve the
box queue, displayed title/text, filtered button indices, next key and the
thread's return destination. Optional sections must preserve existing VM/VMAR
layouts and legacy timed-wait fallback. Retain only waits with supported saved
host state; unimplemented signal sources keep existing fallback. Reuse one
serialized thread ordering/filter for wait metadata. Restore queues without
replaying Show or reevaluating button conditions, and replace stale live boxes.
Test an actual suspended script answered after roundtrip, multiple boxes and
keys, saved filtered indices, no premature resume, legacy behavior and malformed
metadata rejection. Handle or explicitly exclude native action boxes coherently.

Outcome: implemented and reviewed. Parent validation passed the actual two-script
Message.Show save/load/answer regression, all five VM codec tests, and all four
Papyrus crate tests. The integration fixture required its synthetic Message native
declaration; that was corrected before validation passed. Optional `MBXW` stores
message-box presentation/actions and links supported signal keys to the exact
filtered `VM` thread order; `VM` / `VMAR` layouts remain unchanged. Generated
`Message.Show` roundtrip coverage, legacy timed fallback, malformed-link checks,
and key bounds are implemented. Animation-event waits remain item 8.

## 8. Animation-event waits across native saves

Roadmap: Saves / threads waiting on animation events. Build on item 7's VM wait
metadata while preserving a semantic (sender reference, case-insensitive event)
identity for `PlayAnimationAndWait`. Track these identities when the native waits;
save only live waits and remap to the current runtime's wait keys on restoration,
rather than depending on DefaultHasher stability across binaries. Preserve
remaining timeout and return destination. Test event matching after VM/native
roundtrip, wrong sender/event exclusion, case-insensitivity, timeout, no replay
of PlayAnimation, stale-state replacement, old saves and malformed metadata.
Object/actor graph execution state remains separate roadmap work: a resumed
wait can hear a subsequent matching event, but this item must not claim that
loading resumes its animation pose or graph timeline.

Outcome: implemented and reviewed. Parent validation passed the generated-script
integration test with system Vulkan and all seven VM codec tests, including
repeated save/load retention and legacy fallback. Optional `ANMW` links semantic sender/event
identities to the filtered saved-thread order and recomputes the signal key on
load. Synthetic PEX and graph integration coverage verifies multiple waiters,
wrong sender/event filtering, case-insensitive delivery, timeout fallback and
repeated save/load retention. Legacy files without `ANMW` retain the timed
fallback; malformed metadata is rejected. Behavior graph pose, timeline and
events raised before load remain outside this item.

## 9. Greater-power cooldown persistence

Save-continuity blocker found during roadmap Game logic / magic and Saves review:
`Casting.powers_used` is not saved, allowing reload to reset daily-use limits.
Persist the existing per-power last-use game-day values in an optional native
section, restore by replacement, keep the existing one-game-day rule and lesser
power behavior. Test actual power use, save/load refusal before cooldown expiry,
availability afterward, independent powers, old-save clearing and malformed
values/duplicates. Establish the public rule through local UESP first, and
document original-save-format uncertainty separately from native policy.

Outcome: implemented and reviewed. Parent validation passed both the codec and
actual-use save/load/expiry regressions with system Vulkan. Formatting and diff
checks passed. Optional `PWCD` stores sorted Greater Power
form ids and last-use game days; load replaces the map and legacy saves clear
it. The production regression exercises actual `player_use_power` calls before
and after load, the 24-game-hour expiry boundary, independent Greater Powers,
the unrestricted Lesser Power path, and legacy replacement. Malformed,
duplicate, truncated, trailing and unsorted entries are rejected. UESP revision
3595900 documents once per game day; retaining the engine's existing rolling
24-hour rule is a native policy, and original `.ess` cooldown persistence
remains unverified.

## 10. Spent-soul persistence

Save-continuity blocker found during roadmap magic / soul trap and Saves review:
`Magic.souls_taken` is not saved while filled gems and dead bodies are. Persist
the existing spent-body set in an optional native section and restore it by
replacement. Keep failed captures retryable and reject another successful
capture from the same body after load. Test successful capture across production
save/load, failed capture followed by success, independent victims, old-save
replacement, deterministic encoding and malformed entries. Do not change soul
gem inventory/drop policies. Cite public Soul Trap/Soul Gem documentation and
record resurrection/reset and original-save payload behavior as unresolved.
