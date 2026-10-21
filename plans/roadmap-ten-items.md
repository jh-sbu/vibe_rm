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
