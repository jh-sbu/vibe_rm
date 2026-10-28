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

