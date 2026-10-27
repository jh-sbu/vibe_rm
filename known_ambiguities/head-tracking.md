# Head tracking targets

The 2026-10-27 `vrm-tool animation-survey <Data>` survey found BSLookAtModifier
in 35 of 49 project files (three character projects and 32 creature projects). No
hkbLookAtModifier objects were found. Most creatures bind lookAtTarget to
bHeadTrackingOn, humanoids/chicken/hare and several others to bHeadTracking;
dragons also use bLookAtTarget. Several enable gates use IsNPC, and two use
bCanHeadTrack (default 1). The implementation derives target-input names from
the project's bindings, sets IsNPC for every NPC, and preserves graph-authored
modifier enable gates and per-bone movement gates. It does not force every
modifier on. No extra hkbLookAtModifier parser is needed for these assets.

Chosen target rules: a nearby player, a combat target in preference to the
player, and a scene/conversation partner in preference to either. The existing
player proximity distance is retained. Creature targets now follow the same
rules as humanoids. NPC head positions use 90% of scaled skeleton height;
player targets use the eye. Species-specific attention, LOS rules and head
points are not established by the graph bindings and remain unverified.
The authored modifiers retain their own angular limits and easing.

## Passing NPC hellos and scene listeners

Step 7a's public Scenes documentation establishes passing AHEL events, not
standalone HELO/IDLE reply chains. Running packages must allow Random
Conversations (PKDT interrupt flags +8, bit 1). Moving/stationary contacts now
send AHEL once on entering greeting range; normal Story Manager quests/scenes
decide whether anything follows. We use fAIMinGreetingDistance (150 fallback)
and a 32-unit exit band as a provisional proximity rule: the public text says
"close enough" without specifying the exact AHEL threshold. Candidate order is
FormID for determinism. No timer/chance algorithm is invented for ADIA here.

Scene INFO run-on-target conditions and LNAM listener idles use the action's
headtracking recipient where authored; actions without one retain the prior
player context. Headtracking and dialogue-recipient identity may differ in
some scenes; that link is an inference and needs a targeted SE observation.
The larger ADIA approach/interrupt/phase-zero controller remains ROADMAP work.
