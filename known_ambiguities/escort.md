# Escort packages

Implemented in `src/ai/mod.rs` (`Behaviour::Escort`). Based on the Creation Kit's
Escort procedure template inputs as the data lists them and how the base game's six
Escort packages use them.

## What is known

- The `Escort` template's inputs (`vrm-tool esp-dump <data> Escort`): Destination,
  Target to Escort, Number of Followers (1), Distance to Wait for Follower(s) (512),
  Follower Min / Max Distance (120 / 256), Ride Horse?, PreferPreferredPath?,
  Run If Behind Distance (500), and Initial Location to find / Follower(s) /
  Follower(s) ObjectList.
- Users (`SHOW=Escort vrm-tool pack-templates <data>`): Illia in Darklight Tower
  (four stages leading the player), Telrav, Nirya (MQ205).

## Open questions

- "Run If Behind Distance": read here as the escort running when the escorted actor
  is that far *ahead* of it (nearer the destination), to catch up. It could instead
  apply to the followers (escorted NPCs) running to keep up with the escort.
- Facing: the escort turns in place to face the one it waits for once they are
  more than ~35° off to one side. The real threshold and whether the game does this
  at all (rather than only head tracking) is unverified.

## Interpreted escort inputs (2026-11-06)

The complete census corrects the earlier six-package sample: six Escort leaf
occurrences belong to four template records: Escort 00023B73,
EscortPlayerWhenNear 00069665, WEEscortPrisoner 0009D5A2 and PatrolAndHerd
000964CF. The latter passes its Find ObjectList as the follower selector and
PathStart as the destination. Its Travel leaf also consumes that ObjectList.
These are vanilla game-asset observations, obtained with pack-tree and
SHOW=procedure:Escort pack-census. The shipped Escort has nine arguments;
its currently shipped input labels do not include the older note's Initial
Location / additional Follower(s) fields. Those optional named fields are
bound by name if supplied by a template, with synthetic coverage, rather than
inventing additional native argument positions for vanilla leaves.

Escort resolves direct targets, faction/type selectors and Find ObjectLists,
deduplicates living enabled actors and takes up to NumberOfFollowers in list
order. The furthest lagging follower controls the pace, so waiting for the
first one does not leave the rest behind. A follower in another space holds
the escort. InitialLocation, when present, is reached before leading begins;
that arrival is durable cursor progress and cannot complete the Escort leaf.
The herd follows PathStart's existing linked patrol route while its parallel
Wait controls the escort interval. These list/route readings follow the
names and tree structure; compare their details in SE.

FollowerMin/Max define a waiting band: after exceeding DistanceToWait the
escort closes to Max, faces the followers and waits until they reach Min.
This hysteresis belongs to the cursor and survives unload/save. RunIfBehind
continues to mean the escort catching up with an escorted actor ahead.
The exact SE waiting-band geometry remains unverified; this is the chosen
functional reading of all three distance inputs. Away escorts use the same
selection/waiting rules and existing away walking speed.

## Preferred paths

Loaded A* multiplies entry costs into unpreferred triangles by four when the
current Travel, Patrol or Escort leaf requests preference. Force-greet approach
Travel leaves use the same flag. The factor is a policy: xEdit's public NAVM
layout and the game's triangle flags identify preferred triangles but supply
no multiplier. Sandbox/Wander PreferredOnly restricts random destinations and
route transitions to flagged triangles, allowing the actor's initial triangle
as an entry to the preferred corridor. Default routes retain their old costs.

Probe a two-follower escort with different lag distances and a follower across
a load door; compare waiting-band motion and initial-location save/reload.
Also compare Hadvar's MQ102 near/far branch switch, a herder's linked route,
and a traveller/sandboxer with short unpreferred and longer preferred routes.
