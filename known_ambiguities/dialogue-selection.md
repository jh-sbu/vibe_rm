# Dialogue selection and pooled rumors

Implemented in `src/dialogue/mod.rs`; both dialogue UIs consume the same
selected `DialogueOption`. Research and asset inspection: 2026-10-14.
No game executable or leaked source was inspected.

## Sources and what they establish

- [Beyond Skyrim: Dialogue Systems for Writers, Rumors](https://wiki.beyondskyrim.org/wiki/Arcane_University:Dialogue_Systems_for_Writers#Rumors)
  describes rumor topics from different quests sharing one menu option,
  with responses selected randomly. It identifies the default prompt as
  `sTopicSubtypeTextPlayerDialogueRumors`. Its Hulda example reports the
  option changing to "Give me the latest gossip." when the Companions
  response is selected next. This supports selection before display and
  grouping by subtype even when prompts differ; it does not establish a
  general rule for merging identical Custom-topic text.
- [TES Alliance: My First Quest, Topic Info flags](https://tesalliance.org/forums/index.php?/topic/7243-modding-workshop-my-first-quest/)
  documents Random groups: collect qualifying responses through a Random
  End or a nonrandom response, include that terminating response, then
  select randomly. Say Once and Hours Until Reset restrict eligibility.
  This is a general authoring description, not a measurement of SSE's
  cross-quest rumor probabilities.
- [Bethesda Tutorial Advanced Dialogue](https://ck.uesp.net/wiki/Bethesda_Tutorial_Advanced_Dialogue)
  was inaccessible during research. The accessible
  [Russian translation](https://tesck.ru/index.php/Учебник_Bethesda_Продвинутые_диалоги)
  describes special-topic response lists from running quests ordered by
  descending quest priority, then condition checks, random groups and
  reset timers. Applying this ordering to RUMO is an inference; the
  example in the tutorial is Hello dialogue.
- [xEdit's TES5 record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
  define DIAL's topic priority, category/subtype and subtype name, and
  INFO's flags and reset interval. In particular Random is `0x2`, Random
  End is `0x20`, and Invisible Continue is `0x40`. These definitions
  establish field interpretation, not runtime arbitration.

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
encodes approximately one game hour until reset (`ENAM = 02 00 AA 0A`).
The live remote console confirmed the three owning quests were running
while the player was in the Sleeping Giant Inn. That confirms their
quest gates, not SSE's response-selection algorithm.

Hulda's `WhiterunInnkeeperRumorsBranchTopic` (`0002BDDE`) is also RUMO;
its INFO `0002BDE0` has RNAM "Give me the latest gossip.", matching the
guide's concrete example. `MS13RumorsTopic` (`000153B6`) is RUMO with
different FULL text, "Have you heard any rumors lately?". Matching
strings alone therefore cannot describe the documented pooling rule.
The fallback GMST exists as `00055958`.

Reproduce inspection without the executable (DATA/ENAM remain raw;
`LSTR=1` decodes FULL/RNAM):

```sh
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" DBRumorsTopic
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 000153BA
LSTR=1 target/debug/vrm-tool esp-dump "$SKYRIM_DATA" 0002BDE0
target/debug/vrm-tool topic-lines "$SKYRIM_DATA" RUMO
```

## Current implementation choices

- Top-level conversations collect all RUMO topics whose owning quest and
  INFO conditions pass, including topics without a top-level branch or
  without FULL text. Ordinary branch entries skip RUMO. There is one
  rumor entry if a response qualifies, none otherwise.
- The response pool is ordered by descending quest priority, then
  ascending topic FormID for ties, retaining each topic's INFO order.
  The first qualifying random group is sampled uniformly per INFO
  using the engine RNG. Lower-priority quests can join an unterminated
  random group. These are provisional choices, not proven SSE internals.
- The chosen topic and INFO are retained with the menu entry. Clicks
  play that INFO without another random draw, preserving its prompt,
  voice path, quest context, linked choices and begin/end fragments.
  The selected INFO's heard state determines white/grey text.
- Prompt precedence is INFO RNAM, then DIAL FULL, then the localized
  rumor GMST for RUMO without text (English fallback if absent).
- Rumors have one logical UI identity even when their selected topic
  changes. Topic priority still controls menu order. Ordinary Custom
  topics retain distinct identities even with identical prompts.
- Explicit linked-choice lists retain their authored entries; pooling
  happens when returning to the top-level list.
- The shared single-topic selector now also honors Random End and
  includes a terminating nonrandom response in its random group. The
  previous `0x20` Invisible Continue constant was corrected to `0x40`;
  automatic Invisible Continue playback is still unimplemented.

## Remaining open questions and limitations

- Does SSE sample uniformly per eligible INFO, per topic, or with
  another weighting? Do quest priorities delimit rumor groups or only
  order them? Does Random End terminate across quest/topic boundaries
  exactly as in a single-topic list?
- How are equal-priority quests/topics ordered: FormID, plugin order,
  quest start order, or another rule? Our FormID fallback is unverified.
- Does RUMO require a branch, and do branch flags affect eligibility?
  What happens to explicit linked lists containing multiple RUMO topics?
- Which priority positions the single rumor option when contributing
  topics have different PNAM values? Currently the selected topic's.
- Exactly when is a new rumor chosen (menu rebuild, end of response,
  click)? What if conditions change after display? Currently selection
  occurs at rebuild and the displayed response remains committed through
  the click. New/grey behavior for a mixed heard/unheard pool is also
  unverified.
- Say Once and Hours Until Reset are documented eligibility rules but
  are not yet enforced by this engine's dialogue selector. This is an
  existing implementation gap, not evidence that SSE ignores them.
  DIAL's Do All Before Repeating is also not implemented. Their precise
  scope (speaker/player/global), interactions and reset behavior need
  controlled tests, including leaving/reopening dialogue.

See `plans/autotest-survey.md` for a manual test design. Asset metadata
cannot prove probability distributions or resolve equal-priority ties.

## Engine regression verification (not an SSE experiment)

On 2026-10-14, all 45 engine unit tests passed, including five dialogue
regressions covering cross-quest response groups, Random End/nonrandom
boundaries, prompt selection, stable rumor identity and distinct Custom
topics. The release offscreen check against the installed assets used:

```sh
RUST_LOG=vibe_rm=debug VRM_SEED=1 target/release/vibe_rm \
  --cell RiverwoodSleepingGiantInn --talk 00013486 --wait 600 \
  --choose 3 --choose 3 --screenshot /tmp/rumor-pooling-release.png
```

Each of four logged menu builds contained exactly one rumor entry. The
first click played the retained Riverwood INFO `000453ED`; the second
played the retained Dark Brotherhood INFO `000153B8`. The next entry
selected the College INFO `000153BA`. Voice paths retained the original
topic/quest, and the rendered SWF menu showed one rumor option. This
checks our implementation; it does not settle the SSE questions above.
