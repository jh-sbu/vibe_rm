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
