# Package dialogue

## Evidence consulted (2026-11-05)

External vanilla assets examined with `pack-tree`:

- Say (`0001CCB6`) is Travel(Location), then Say(Topic, Target,
  Finish Speaking, Look At Target, Force Subtitle).
- StartConversation (`0001CE0F`) is DialogueActivate(Target, Talk From,
  Look At Target). Its template has no topic input, contrary to the plan's
  initial description.
- SayToPlayer (`0009E0A1`) selects Say or its wait Sandbox/Travel branches.
  Its Say branch checks the player's trigger location, detection and
  IsPlayerMovingIntoNewSpace. The latter is still step 9's work.

The existing scene player provides voice paths, silent reading duration,
speaker/listener gestures and INFO fragment dispatch. The existing Story
Manager sends ADIA with the initiating actor and other actor; the authored
quest/scene chooses the ensuing NPC conversation. These are this project's
existing implementations, not new observations of SE. Procedure CK wiki pages
remain unavailable as the plan records.

## Implementation choices

- Say runs the selected INFO's responses in order. Its procedure completes
  after the final response; no eligible INFO or an empty INFO completes without
  speech, following scene dialogue's existing policy. It stays where the
  preceding Travel left it, rather than moving to its listener.
- Look At Target uses the existing scene head-tracking and standing facing
  path. Force Subtitle overrides general subtitle settings in both HUDs,
  retaining the existing nearby-speaker distance limit. Neither its native
  distance boundary nor effects on other subtitle policies have been measured.
- Finish Speaking lets the current response end after its owning package,
  branch or repeat epoch changes. It does not start remaining responses from
  the abandoned INFO. Without it, cancellation stops playback. Completion of
  an old response never finishes a new run. Normal completion runs the INFO's
  end fragment; immediate cancellation does not. Exact SE cancellation and
  multi-response fragment timing remain unmeasured.
- An unloaded speaker starts no response and consumes no queued responses.
  On returning, the current interrupted response replays from its beginning,
  without selecting another INFO or rerunning its begin fragment. This is a
  semantic continuation choice; partial audio seeks are not implemented.
- Optional PDLG saves the selected INFO/response, listener, flags and owning
  procedure token, including a finishing response whose old run was replaced.
  It restores that current response and suffix on attachment. Old saves have
  no pending package speech. This describes the engine's own save format,
  not Skyrim's ESS representation.
- DialogueActivate approaches Talk From before requesting a conversation.
  With the player, it opens the existing conversation path and selects a HELO
  greeting because the vanilla template supplies no topic. With an NPC, both
  actors must be loaded in the same space; it sends ADIA and lets authored
  quests/scenes choose the conversation. The procedure finishes after this
  request; it does not wait for an ensuing quest scene to finish. That request
  boundary and the player greeting fallback still need an SE comparison.

Synthetic runtime, codec and vanilla template fixtures check these rules.
An SE probe should compare cancellation with each Finish Speaking value,
multi-response INFO fragments, unload/reload, forced subtitles with general
subtitles disabled, and DialogueActivate's completion relative to its ADIA
scene. Queued in ROADMAP Test plugins.
