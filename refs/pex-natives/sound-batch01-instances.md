# Sound.Play / PlayAndWait / StopInstance / SetInstanceVolume

Public references consulted 2026-10-24: Papyrus Index pages for
[Sound.Play](https://papyrus.bellcube.dev/skyrimse/sound/function/play/),
`PlayAndWait`, `StopInstance` and `SetInstanceVolume` (community Creation Kit wiki
text); `refs/CommonLibSSE-NG` for the sound handle types. Summarized, not copied.

Source claims: `Play(akSource)` returns an integer instance id; `StopInstance`
and `SetInstanceVolume` take that id; `PlayAndWait` returns when the sound ends.

Implementation (inference where marked):
- `src/script_sounds.rs` keeps `id -> {voice, gain, looping, duration}`; ids
  start at 1 and are never reused. With no audio device an id is still handed
  out, mapped to no voice (scripts' bookkeeping keeps working).
- A descriptor without files, no descriptor, or a missing source reference
  returns 0.
- Looping descriptors loop until `StopInstance`.
- `SetInstanceVolume(id, f)` sets voice volume to the playing's gain x f.
- `PlayAndWait` waits on a per-instance key, signalled when the voice ends or
  is stopped; the timeout is the clip length (pitch-adjusted) plus half a
  second, or 60 s for loops and clips not yet decoded. With no voice it returns
  at once.
- Inference: SE does not persist sound handles, so script loops are not saved.

Tests use generated SNDR records only (no audio device).
