# Dialogue selection and pooled rumors

Implemented in `src/dialogue/mod.rs`; both dialogue UIs consume the same
selected `DialogueOption`. Research and asset inspection: 2026-10-14.
No game executable or leaked source was inspected.

## Sources and what they establish

- [Beyond Skyrim: Dialogue Systems for Writers, Rumors](https://wiki.beyondskyrim.org/wiki/Arcane_University:Dialogue_Systems_for_Writers#Rumors)
  says the Rumor subtype lets "topics from different quests appear under
  the same dialogue option as if their infos were pooled under one
  topic", clicked repeatedly for lines "in a random order". When a rumor
  topic "has no topic text (and they typically don't)" the option reads
  `sTopicSubtypeTextPlayerDialogueRumors`. Of Hulda it says the option
  reads "Heard any rumors lately?" "if any of the other rumors were
  randomly selected to be said next", but "Give me the latest gossip" "if
  the companions rumor is next", which it calls buggy and an error for
  writers to avoid. This supports selection before display and pooling
  per INFO by subtype. It does not establish a general rule for merging
  identical Custom-topic text.
- [TES Alliance: My First Quest, Topic Info flags](https://tesalliance.org/forums/index.php?/topic/7243-modding-workshop-my-first-quest/)
  documents Random groups: qualifying Random INFOs go on a stack "until an
  Info that is not marked Random is found, or an info marked Random End is
  found. The final Info is then added to the stack, and one of the Infos
  is selected randomly." Say Once ("only be said once by any given
  Actor") and Hours Until Reset restrict eligibility. This is a general
  authoring description, not a measurement of SSE's cross-quest rumor
  probabilities.
- [Bethesda Tutorial Advanced Dialogue](https://ck.uesp.net/wiki/Bethesda_Tutorial_Advanced_Dialogue)
  is behind a Cloudflare check, but the
  [archived English page](https://web.archive.org/web/2024/https://ck.uesp.net/wiki/Bethesda_Tutorial_Advanced_Dialogue)
  is readable (as is the
  [Russian translation](https://tesck.ru/index.php/Учебник_Bethesda_Продвинутые_диалоги)).
  "All game's dialogue for a given type of topic lives in one giant pool":
  every info of every topic of that type in running quests, stacked "in
  the order of quest priority", then the first info whose conditions pass
  fires. Random End keeps "the info selection system from dropping below
  this particular set", and Combat topics work "the same way". Its example
  is Hello dialogue; applying it to RUMO is an inference, but it is the
  same kind of pooling by type. Its example also says "he will deliver the
  first three lines randomly, and when they've been exhausted, he will
  only say the last line [the Random End one]", which taken alone could
  mean the terminating line is not in the draw; see Geldis Sadri below.
- [xEdit's TES5 record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
  define DIAL's topic priority, category/subtype and subtype name, DLBR's
  flags and INFO's flags and reset interval. In particular Random is `0x2`,
  Random End is `0x20`, and Invisible Continue is `0x40`; reset hours are
  ENAM's second u16 divided by 2730.625. These definitions establish field
  interpretation, not runtime arbitration.
- UESP describes in-game behaviour for the asset cases below:
  [Geldis Sadri](https://en.uesp.net/wiki/Skyrim:Geldis_Sadri),
  [Rorik](https://en.uesp.net/wiki/Skyrim:Rorik) and
  [Scouts-Many-Marshes](https://en.uesp.net/wiki/Skyrim:Scouts-Many-Marshes).

## Evidence in the installed assets

All three records in `Skyrim.esm` have the exact FULL text "Heard any
rumors lately?", PNAM 50, DATA `00 00 02 00` (Player/Rumors; Do All Before
Repeating false), and SNAM `RUMO`:

| DIAL | FormID | Owning quest | Quest priority |
| --- | --- | --- | --- |
| DBRumorsTopic | 000153B3 | DarkBrotherhood | 69 |
| MGRumorsTopic | 000153B4 | DialogueWinterholdCollege | 30 |
| DelphineRumorsBranchRevisedTopic | 000453E6 | DialogueRiverwood_Revised | 30 |

The Orgnar-specific Dark Brotherhood response `000153B8`, College
response `000153BA`, and local responses `000CA6DA`, `000CA6DB`,
`000453ED` have Random set and Random End clear. The College INFO also
encodes one game hour until reset (`ENAM = 02 00 AA 0A`). The live remote
console confirmed the three owning quests were running while the player
was in the Sleeping Giant Inn. That confirms their quest gates, not
SSE's response-selection algorithm.

These Riverwood topics are atypical in having FULL text: of the 23 RUMO
topics in the vanilla masters, 17 have none. The others:

- `MS13RumorsTopic` (`000153B6`, also offered by Orgnar) has FULL "Have
  you heard any rumors lately?". Matching strings alone therefore cannot
  describe the pooling rule, and Orgnar's pool mixes two FULL texts.
- Hulda's `WhiterunInnkeeperRumorsBranchTopic` (`0002BDDE`) has FULL
  "WhiterunInnkeeperRumorsBranchTopic", a placeholder copy of its editor
  ID. Its one INFO `0002BDE0` has RNAM "Give me the latest gossip.",
  matching the guide's example, and resets after 24 hours (`ENAM = 02 00
  FF FF`). Her other rumors (`T03TreeRumorsTopic`,
  `DA08BalgruufRumorsTopic`) have no FULL.
- `DialogueWindhelmScoutsManyMarshesTopic2Topic` (`000C825A`) is RUMO in a
  top-level branch with FULL "How long have you lived in Windhelm?", and
  UESP lists that text as one of his player options. So FULL is shown for
  a RUMO topic at least when it is the only one the speaker has.

Branches: every vanilla RUMO topic but one starts a top-level branch
(DLBR DNAM `0x1`). The exception, `RorikPatrolCommentTopic` (`0009B00A`),
is in `RorikPatrolCommentBranch` (`0009B041`, DNAM 0), has no FULL, and its
INFOs need only GetIsID Rorik; its quest `DialogueRorikstead` starts
enabled. UESP lists no rumor option for Rorik and says the lines were
meant as patrol comments that never play.

Random End: Geldis Sadri's `DLC2DRRRumorsBranchTopic` (`040271DF`,
Dragonborn.esm) chains ten Random INFOs, then `040271E4` "Haven't heard
anything lately, but check back with me... you never know." (Random |
Random End, conditioned only on the speaker), then `0403A29B` (Thirsk Mead
Hall, Random). UESP records the bug, fixed by USSEP, that he says the
"Haven't heard anything" line "when he has plenty more rumors". That is
what happens if the terminating INFO is part of the draw, as TES Alliance
says. UESP also says the Thirsk rumor never plays and puts that down to
its conditions; under the stack rules the Random End line before it would
block it as well.

The fallback GMST exists as `00055958`.

Reproduce inspection without the executable (DATA/ENAM remain raw;
`LSTR=1` decodes FULL/RNAM):

```sh
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" DBRumorsTopic
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 000153BA
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 0002BDE0
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 0009B041
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 040271E4
target/debug/vrm-tool topic-lines "$SKYRIM_DATA" RUMO
```

## Current implementation choices

- Top-level conversations collect the RUMO topics that start a top-level
  branch and whose owning quest and INFO conditions pass, including
  topics without FULL text. (Rorik is the evidence for requiring the
  branch.) Ordinary branch entries skip RUMO. There is one rumor entry if
  a response qualifies, none otherwise.
- The response pool is ordered by descending quest priority, then
  ascending topic FormID for ties, retaining each topic's INFO order.
  The first qualifying random group, including the INFO that ends it, is
  sampled uniformly per INFO using the engine RNG; a Random End or
  nonrandom INFO ends the group even across quests. Lower-priority quests
  can join an unterminated random group. This follows the two tutorials'
  one stack and the guide's "pooled under one topic"; it is not measured.
- The chosen topic and INFO are retained with the menu entry. Clicks
  play that INFO without another random draw, preserving its prompt,
  voice path, quest context, linked choices and begin/end fragments.
  The selected INFO's heard state determines white/grey text.
- Prompt precedence is INFO RNAM, then DIAL FULL, then the localized
  rumor GMST for RUMO without text (English fallback if absent). This is
  unsettled when FULL texts are pooled; see below.
- The pooled rumor entry has one logical UI identity even when its
  selected topic changes. Topic priority still controls menu order.
  Ordinary Custom topics, and RUMO topics listed among an INFO's linked
  choices, keep distinct identities even with identical prompts.
- Explicit linked-choice lists retain their authored entries; pooling
  happens when returning to the top-level list.
- The shared single-topic selector honors Random End and includes the
  INFO that ends a random group in the draw. It serves every caller, so
  this also applies to greetings, blocking-branch greetings, barks, AI
  greetings and scene lines. The previous `0x20` Invisible Continue
  constant was corrected to `0x40`; automatic Invisible Continue playback
  is still unimplemented.

## Remaining open questions and limitations

- Weighting: the sources point to a per-INFO pool, but does SSE sample
  uniformly per eligible INFO, per topic, or with another weighting?
  Both tutorials describe one stack across quests that priority only
  orders, with Random End stopping it there; nothing tested measures
  this for RUMO.
- How are equal-priority quests/topics ordered: FormID, plugin order,
  quest start order, or another rule? Our FormID fallback is unverified.
- Branches: Rorik suggests a non-top-level branch keeps a RUMO topic out
  of the menu, but his quest might fail some other gate. Do the blocking
  or exclusive branch flags matter? What happens to explicit linked lists
  containing several RUMO topics?
- Prompt: when a pool mixes topics with and without FULL (Orgnar: "Heard
  any rumors lately?" and "Have you heard any rumors lately?"), does the
  entry show the drawn topic's FULL, the GMST, or one fixed text? Hulda's
  placeholder FULL would read as "WhiterunInnkeeperRumorsBranchTopic" if
  her INFO had no RNAM; that no player reports it is consistent with
  RNAM first but says nothing about FULL against the GMST.
- Which priority positions the single rumor option when contributing
  topics have different PNAM values? Currently the selected topic's.
- Exactly when is a new rumor chosen (menu rebuild, end of response,
  click)? What if conditions change after display? Currently selection
  occurs at rebuild and the displayed response remains committed through
  the click. New/grey behavior for a mixed heard/unheard pool is also
  unverified.
- Say Once and Hours Until Reset are documented eligibility rules but
  are not yet enforced by this engine's dialogue selector. This is an
  existing implementation gap, not evidence that SSE ignores them, and
  vanilla rumors depend on them (Hulda's 24 hours, the College's one).
  DIAL's Do All Before Repeating is also not implemented. Their precise
  scope (speaker/player/global), interactions and reset behavior need
  controlled tests, including leaving/reopening dialogue.

See `plans/autotest-survey.md` for a manual test design. Asset metadata
cannot prove probability distributions or resolve equal-priority ties.

## Engine regression verification (not an SSE experiment)

On 2026-10-14, all engine unit tests passed, including dialogue
regressions covering cross-quest response groups, Random End/nonrandom
boundaries, prompt selection, stable rumor identity and distinct Custom
and linked RUMO topics. The release offscreen check against the
installed assets used:

```sh
RUST_LOG=vibe_rm=debug VRM_SEED=1 target/release/vibe_rm \
  --cell RiverwoodSleepingGiantInn --talk 00013486 --wait 600 \
  --choose 3 --choose 3 --screenshot /tmp/rumor-pooling-release.png
```

Each of four logged menu builds contained exactly one rumor entry. The
first click played the retained Riverwood INFO `000453ED`; the second
played the retained Dark Brotherhood INFO `000153B8`. The next entry
selected the College INFO `000153BA`. Voice paths retained the original
topic/quest, and the rendered SWF menu showed one rumor option. After the
top-level-branch requirement, `--cell RoriksteadRoriksManor --talk
0001A6A6` lists Rorik's three topics and no rumor entry; before it, the
list also had "Heard any rumors lately?" with his patrol comment
`0009B034`. This checks our implementation; it does not settle the SSE
questions above.
