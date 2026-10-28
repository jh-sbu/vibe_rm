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
