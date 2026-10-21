# Clip enforced-duration inputs

The runtime retains `hkbClipGenerator.enforcedDuration` and resolves its
graph-variable or read-only character-property binding alongside the existing
clip speed inputs. Variable bindings take precedence over duplicate properties;
property names resolve case-insensitively. The member is a real scalar and its
binding stays outside the animation-variable output table.

## Public evidence and provenance

The community-authored [Skyrim Behavior Editor clip class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbclipgenerator.cpp)
reads and writes `enforcedDuration` as a real. Its [clip UI](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/ui/hkxclassesui/behaviorui/generators/clipgeneratorui.cpp)
offers variable and character-property bindings for the member. A public
community [CommonLibSSE-NG layout](https://github.com/CharmedBaryon/CommonLibSSE-NG/blob/master/include/RE/H/hkbClipGenerator.h)
places it at clip offset `0x68`. The community-authored [Souls Modding DS3
tutorial](https://www.soulsmodding.com/doku.php?id=tutorial:how-to-create-a-new-spell-in-ds3)
says `playbackSpeed` is a percentage multiplier and `enforcedDuration` speeds
the animation up to complete in the specified number of seconds. This tutorial
describes Dark Souls III, not Skyrim, so it supports the general field role but
does not establish Skyrim's exact interaction, crop behavior or update timing.
The local UESP index had no `enforcedDuration` result.

## Supported policy and unresolved questions

For a positive finite duration, this runtime sets the speed magnitude to
`activated cropped length / enforcedDuration`. This treats the supplied value
as the target time to traverse the full playable cropped length and overrides
the authored or bound `playbackSpeed` magnitude; the sign of playback speed
continues to select the direction fixed at activation. Thus a nonzero
`startTime` shortens the remaining wall-clock playback time without changing
that start position. A zero or negative finite duration disables the override.
An invalid bound duration falls back to the authored duration when finite; an
invalid authored duration disables the override. A positive duration cannot
produce a rate when the activated length is zero or unusable, so playback
retains the existing speed path in that case.

The effective rate is sampled at the runtime's existing speed read points:
each clip advance, synchronized-cycle calculation and the supported
single-play-source calculation used by transition `endMode`. This lets these
paths agree on rate while allowing bound values to affect an active clip. The
original engine's sampling time is unverified. User-controlled clips retain
their fraction behavior, and ping-pong clips retain the existing speed path;
`enforcedDuration` does not alter either mode. Crop inputs remain activation
fixed, and the transition end-mode calculation uses the same rate for remaining
time. Cycle calculations accept any finite positive effective rate, including
rates below the old `1e-3` cutoff, so long enforced durations retain their
requested timing.

The rate formula, whether authored speed magnitude should compose with the
target duration, interaction with non-default start times and crops, handling
of invalid values, and update sampling have not been verified in Skyrim SE.
The rate override for positive duration is an implementation policy informed
by another game's community tutorial, not a parity claim. Synthetic tests
cover completion time, cropped reverse playback, unchanged start time,
zero/invalid fallback, binding precedence, synchronized timing, transition
end-mode integration, and preserved user-controlled/ping-pong behavior. No
game assets, SDK source, engine executable/DLL data, disassembly or other engine
binary data were examined or added.
