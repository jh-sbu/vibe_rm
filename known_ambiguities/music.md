# Music policy choices

Sources: [MUSC revision 2384538](https://en.uesp.net/w/index.php?oldid=2384538),
[MUST revision 3089093](https://en.uesp.net/w/index.php?oldid=3089093),
DOBJ/game data and Skyrim_Default.ini (research/audio/defaults.md).

## Transitions (4a)

MUSC provides WNAM and abrupt-transition flags but does not say whether the
outgoing or incoming type owns the duration. We choose incoming WNAM; abrupt
cuts bypass the ramp. Selection policy runs without an audio device. Fades
restart on load, as playback cursors were never saved.

## Ducking (4b)

Flag 0x20 leaves underlying tracks advancing and stacks attenuation from each
upper type (PNAM dB/100). INI times are 6 seconds down and 8 up. Background
tracks that naturally end are not restarted until selected again. Playback
and fade/duck envelopes restart on load; only the existing MUSQ is saved.

## Ordering and queue (4c)

Cycle Tracks exhausts a type before repeating; Maintain Track Order keeps TNAM
order, otherwise Fisher–Yates shuffles once per cycle. Conditions are rechecked
for each selection. Between tracks we retain a provisional 20–60 second gap;
no inspected source specifies this interval. The gap starts when a track ends.

Tie-breaking at equal priority (we take the first queued, and a queued type
over the location's), whether `Add` of an already queued type moves it (we keep
its place), and whether script-added entries survive a save (we save them as
`MUSQ`) are unverified. An abrupt versus graceful change is our reading of the MUSC flags. **test-mod** wanted
for the last two: `Add` a type, save, load and see whether it still plays; and
`Add` two types of equal priority and see which plays.


## Track interpretation (4d)

The asset survey found 283 singles, 21 palettes and 5 silent tracks, including
combat cue points/outro BNAM paths and a finite Sovngarde loop. We implement
all these features. LNAM count means additional repeats of the begin/end
segment, then the tail. Palette zero IDs split simultaneous layers; within
each layer one condition-eligible entry is chosen at random, then another
when it ends, until FLTV expires. Duplicate entries remain weights (many are
silent spacers). DNAM fades the palette before its deadline. These semantics
fit the records but need listening confirmation. A graceful type change may
cut at the next cue; its BNAM outro plays once over the transition. Cue and
outro timing beyond this is not publicly specified.

## Default music triggers (4e)

DFMS fills an absent location type. BTMS is active while any living loaded
actor's combat target is the player; this is a provisional interpretation of
fCombatMusicPlayerTargetedThreatRatio=1, not an implementation of its unknown
threat formula. Death pushes DTMS once per transition to dead. LUMS plays
when a pending level is taken. SCMS plays once when a journal-visible quest
completes, via CompleteQuest or a completing log entry (both converge in the
existing completion API); hidden type-None quests stay silent. The asset
survey shows both paths in vanilla use, but cannot establish the exact music
trigger. DCMS now comes from DOBJ. Identical queued stingers coalesce as
existing queue policy; listening could refine all of these trigger choices.
