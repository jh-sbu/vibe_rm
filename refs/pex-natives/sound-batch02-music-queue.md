# MusicType.Add / MusicType.Remove

Public references consulted 2026-10-24: Papyrus Index pages for MusicType.Add /
Remove (community Creation Kit wiki text); `refs/CommonLibSSE-NG` `BSMusicManager`
(a queue of music types with a current one); local UESP
[MUSC revision 2384538](https://en.uesp.net/w/index.php?oldid=2384538): `PNAM`
priority (1 highest, 100 lowest) and ducking, `FNAM` flags (Plays One
Selection 0x01, Abrupt Transition 0x02, Cycle Tracks 0x04, Ducks 0x20).
Summarized, not copied.

Implementation (inference where marked):
- `src/music.rs`: `Queue` of `(music, script, started)` in order added. The
  location's music is the implicit entry at its own `PNAM` priority.
- Chosen: lowest priority number wins; ties go to the first queued; a queued
  entry beats a location of equal priority (inference), never a better one.
- `Add` of a queued type is a no-op (does not move it). Only MUSC records queue.
- A queued type with Plays One Selection leaves the queue when its track ends.
- Abrupt Transition: a change involving the queue waits for the playing track
  to end unless the incoming type is abrupt. Location-only changes cut off, as
  before. Fades and ducking are not done.
- Engine stingers (`play_music_once`: discovery, cleared) are non-script queue
  entries, so they are not saved. Script entries are saved as `MUSQ`; legacy
  saves load an empty queue.

Tests: generated MUSC records and queue logic only.
