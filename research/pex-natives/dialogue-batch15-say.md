# ObjectReference.Say and Topic.Add

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `Say(akTopicToSay, akActorToSpeakAs = None, abSpeakInPlayersHead = false)`:
  the reference says the topic. `akActorToSpeakAs` is the actor used for
  conditions and voice selection, ignored when called on an actor; with None
  no actor is used for conditions. `abSpeakInPlayersHead`: the voice comes from
  the player's head. Notes: important statements in a Say topic's INFO
  fragments are often not processed; object references need a `.seq` file for
  Say topics of start-game-enabled quests.
- `Topic.Add()`: "Adds this topic to the player's list of known topics."

## Game-asset observations

- `vrm-tool pex-calls ... add` matched against the scripts' variable types:
  the only vanilla `Topic.Add` is `TIF__000B396B` adding `MQ204DragonTopic`.
  The other `Add` calls are `MusicType`, `FormList` and leveled lists.

## Choices

- Say picks the topic's first eligible INFO (as the package Say procedure
  does), runs its begin fragment, says each response in turn (voiced by the
  actor's voice type, placed at the reference, or unplaced in the player's
  head), then runs its end fragment. A new Say on the same reference stops
  the old line. Conditions run on the actor, else the reference itself; the
  listener is the player. Subtitles show as for scene lines; `IsTalking` is
  true meanwhile. The `.seq` restriction isn't modelled.
- `Topic.Add` does nothing: Skyrim's dialogue menu lists topics by their
  conditions, with no known-topics list (Morrowind's `AddTopic` had one). No
  source says what Skyrim keeps it for.
