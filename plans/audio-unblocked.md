# Audio: the unblocked items

**Status: in progress** (2026-10-27). Covers what can be built today from ROADMAP
Next item 7 (Audio), as sorted on 2026-10-27. It also covers three audio gaps
the research turned up that ROADMAP doesn't list: the default music types
(combat, death, level-up), acoustic spaces' ambient loops, and regions'
ambient sounds (`RDSA`). The items this plan leaves out, and why:

- **The Audio menu's sliders, the UI.** They live on the journal's System page
  (ROADMAP item 6). Step 8 builds everything below them: stored settings,
  applying them and a console command. The page then only needs to call it.
- **Pausing in the loading menu** (`PDLC`). The engine has no loading menu
  yet. Step 2 wires the category so that menu only has to report itself.
- **Kill-move sounds** (`TSSC`, "Time Sensitive Sound Category" ->
  `AudioCategoryNPCKillMove`). These wait on killmoves (ROADMAP item 5).

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries. Choices made without a source go in `known_ambiguities/`;
open questions go here or in ROADMAP. `cargo fmt` before each commit. Each
sub-step is one commit.

## What the research found (2026-10-27)

Sources: the local UESP dump, xEdit's record definitions
(`TES5Edit/dev-4.1.6/Core/wbDefinitionsTES5.pas`, read from GitHub, not
vendored), the game's data (`vrm-tool esp-list` / `esp-dump`) and the game's
own default INI (`Skyrim_Default.ini`, a game file). Write these up as
`research/audio/defaults.md` in step 1. This section is the summary.

### Default objects name the behaviour categories

Skyrim.esm's `DOBJ` (xEdit's names, UESP has no page on them) binds the
categories whose editor ids describe a behaviour:

| DOBJ | xEdit name | Skyrim.esm target |
| --- | --- | --- |
| `DDSC` | Dialogue Voice Category | `AudioCategoryVOC` 876BB |
| `NDSC` | Non-Dialogue Voice Category | `AudioCategoryNonVOC` 876BC |
| `SFDC` | SFX To Fade In Dialogue Category | `AudioCategorySFXFadeDuringDialogue` E9E |
| `PIMC` | Pause During Menu Category (Immediate) | `AudioCategoryPausedDuringMenu` DDDC5 |
| `PDMC` | Pause During Menu Category (Fade) | `AudioCategoryPausedDuringMenuFade` 9F254 |
| `PDLC` | Pause During Loading Menu Category | `AudioCategoryPausedDuringMenuLoad` 10AA60 |
| `MDSC` | Music - Sound Category | `AudioCategoryMUS` 71E64 |
| `SMSC` | Stats Mute Category | `AudioCategoryAMBr` 71E63 |
| `SSSC` | Music - Stats | MUSC `_NONE` 1BA72 |
| `TSSC` | Time Sensitive Sound Category | `AudioCategoryNPCKillMove` EA51F |
| `DOP2` | Dialogue Output Model (2D)* | SOPM `SOMDialogue3DDefault` B5184 |
| `DOP3` | Dialogue Output Model (3D)* | SOPM `SOMDialogue2D` B5183 |
| `POPM` | Player's Output Model (1st Person) | `SOMMono01400Player1st` B4058 |
| `P3OM` | Player's Output Model (3rd Person) | `SOMMono01400Player3rd` 428B6 |
| `IOPM` | Interface Output Model | `SOMUIDefault` B75FB |
| `RVBT` | Reverb Type | REVB `DefaultReverb` C5B6E |
| `UWLS` | Underwater Loop Sound | SNDR `AMBUnderwater2DLPM` 44ED8 |
| `BTMS` / `DTMS` / `SCMS` / `LUMS` / `DCMS` / `DFMS` | Music - Battle / Death / Success / Level Up / Dungeon Cleared / Default | `MUSCombat`, `MUSSpecialDeath`, `MUSSpecialSuccess` (both), `MUSDungeonCleared`, `MUSExploreTestb` |

\* xEdit's 2D/3D labels are the reverse of the targets' editor ids. Go by
the data: `DOP2` is the 3D model and `DOP3` the 2D one. Record this
in the research note.

The category tree (`PNAM`; flags `FNAM`: 1 Mute When Submerged, 2 Should
Appear on Menu) shows how the behaviour categories sit above the
ordinary ones:

```
_AudioCategoryMaster
├── PausedDuringMenuLoad
│   └── SFX [menu]
│       ├── SFXFadeDuringDialogue
│       │   ├── FST [menu], FSTnpc, NPCKillMove, AMB [submerged]
│       │   └── PausedDuringMenuFade
│       │       └── MuteSubmerged [submerged], MAG, AMBr [submerged]
│       └── UI
├── PausedDuringMenu
│   └── VOCGeneral [menu]
│       └── VOC, NonVOC
└── MUS [menu]
```

So each behaviour can be applied as a state on one category. The mixer
already passes category state down the chain (`CategoryState`). For
example, UI sounds sit outside the dialogue fade, and footsteps fade in
dialogue and still play in menus. Voices pause at once in a menu; magic
and region ambience fade out instead.

### Timings in the default INI

`Skyrim_Default.ini` `[Audio]`: `fMusicDuckingSeconds=6.0`,
`fMusicUnDuckingSeconds=8.0`, `fMenuModeFadeOutTime=3.0`,
`fMenuModeFadeInTime=1.0`. `SkyrimPrefs.ini` `[AudioMenu]` stores the sliders
as `uIDn` (a category form id in decimal) and `fValn` pairs, plus
`fAudioMasterVolume`. The four ids are SFX, FST, VOCGeneral and MUS, at their
`UNAM` defaults. The engine reads no INI today. Read these values into
named constants that cite the file, and keep the INI out of the repo.

### Records

- **MUSC** (UESP oldid=2384538): `FNAM` flags (1 plays one selection,
  2 abrupt, 4 cycle tracks, 8 maintain track order, 0x20 ducks current track),
  `PNAM` priority + ducking dB x100, `WNAM` fade duration seconds. For example
  `MUSCombat`: flags 0x24, ducking 100 dB, fade 9 s. The engine uses only
  flags 1 and 2 and the priority.
- **MUST** (oldid=3089093): `FLTV` duration (silent and palette), `DNAM`
  fade-out (palette), `FNAM` cue points, `LNAM` loop begin / end / count,
  palette `SNAM` lists with zero form ids separating layers. The engine
  plays `ANAM` only.
- **ASPC** (oldid=3089064; xEdit): `OBND`, `SNAM` ambient sound, `RDAT` "Use
  Sound from Region (Interiors Only)", `BNAM` reverb. 52 records; cells name
  one in `XCAS`.
- **REVB** (oldid=3089102; xEdit "Reverb Parameters"): decay ms, HF reference Hz,
  int8 room / room HF / reflections / reverb amp, decay HF ratio /100,
  scaled reflect delay, reverb delay ms, diffusion %, density %. The
  fields are I3DL2's (and XAudio2's `XAUDIO2FX_REVERB_I3DL2_PARAMETERS`).
  The int8 levels read as dB: `DefaultReverb` has room -100 (off), and
  `Underwater` has room -13, room HF -22, reflections -54, reverb +17,
  decay 2044 ms. 11 records.
- **SOPM** (oldid=3089107; xEdit): `NAM1` reverb send %, `MNAM` HRTF or
  defined speaker output, `ONAM` three rows (mono, stereo left, stereo right)
  of eight speaker levels (L R C LFE RL RR BL BR).
- **SNDR** `BNAM` byte 2: priority (unused).
- **REGN** `RDSA` (UESP REGN): sound, flags (pleasant / cloudy / rainy /
  snowy), chance, under a type-7 `RDAT`.
- **GMST**: `iLipMorphCount = 31`, `fCombatMusicPlayerTargetedThreatRatio = 1`.

### What the engine has (2026-10-27)

`src/audio/mod.rs` is a cpal mixer. Its voices carry categories, a rate
and a five-point `Falloff`, panned by the listener's right vector, with no
interpolation, ramps, buses or voice limit. `src/world/sound.rs` handles
descriptors, categories (`category_volume`) and output models;
`src/music.rs` / `Engine::update_music` the music queue; `src/script_sounds.rs`
the Papyrus sound natives. No headless engine has an audio device
(`Engine::audio` is `None`), so logic that has to be testable can't
live behind it.

### Evidence limits

Nothing here is visible to Papyrus, so no autotest can measure it. The
evidence is the data, the INI and listening in Skyrim SE.
Where a choice would benefit from listening, it is marked **listen**. The user can
compare by ear if they want; it is never a gate.

## Design rule: decide outside the device

Add `src/audio/director.rs`: a pure, device-free state machine that the
engine updates every frame with what it knows: menu mode, the dialogue
state, the listener's submersion, the acoustic space, the music state, the
settings and dt. It returns targets: per-category gain / pause targets with
ramp times, the reverb to use, ambient loops to run and music commands. The
engine applies the targets to `Audio` when there is one. Engine tests read the
director with no device. Mixer tests feed synthetic clips straight to
`Mixer::mix` (as the current tests do).

## Order

0 is the foundation. 1 to 3 build on it and on each other. Music (4),
acoustic spaces (5), speaker output (6), priorities (7) and settings (8) are
independent after 1. Lip sync (9) is last and largely separate.

## 0. Mixer foundation

- **0a Ramps.** Each voice gets a smoothed gain (current -> target over a
  given time, linear in dB or amplitude: pick one and document it). Each
  category state gets a ramped `volume` and a `paused` that takes effect once
  the ramp reaches zero. `Audio::set_category` takes a ramp time. Volume and
  stop changes ramp over a few ms to avoid clicks (`stop_with_fade(id, secs)`).
  Tests: gain at given frames across a ramp; a fade-then-pause holds position.
- **0b Interpolation.** Linear interpolation between frames in `mix`
  (`known_ambiguities/sound-variation.md` notes rates off 1 are rough without
  it). Test: half-rate playback of the ramp clip gives midpoints.
- **0c Voice metadata.** `Pending::Play` / `Voice` carry a `VoiceParams`
  struct in place of the growing argument list: volume, rate, looping,
  position, falloff, categories, priority, reverb send, output mode. Callers
  (`Engine::play_sound`, dialogue, scenes, barks, music, weather, references'
  loops) build it through `SoundDesc` helpers, which removes the
  `too_many_arguments` allows.
- **0d Buses.** The mixer mixes into a dry bus and one reverb send bus
  (silent until step 5), then sums into the device's channels (stereo
  only until step 6). Keep the callback allocation-free: scratch buffers sized
  at stream creation.

## 1. Default objects and the behaviour categories

- **1a `vrm-tool audio-defaults <data>`**: decode `DOBJ` and print the audio
  entries above with editor ids (the one-off survey's Python, made a tool).
  Add `sndr-stats`' priority column. Write `research/audio/defaults.md` with the
  findings above, including the xEdit 2D/3D label mismatch.
- **1b `world::sound::AudioDefaults`**: parsed once from the load order's
  winning `DOBJ` (Survival's `.esl` overrides `DOBJ` with a copy; take the
  winning record): category ids, output models, reverb, underwater loop,
  music types. Tests on a `RecordBuilder` `DOBJ`.
- **1c Use them.** Dialogue lines go in `DDSC`'s category, so the voice
  category is no longer guessed. Non-dialogue voices (barks, combat
  grunts?) use `NDSC` only if a path plays them without a descriptor; check
  barks. Dialogue fades by `DOP2` (3D) / `DOP3` (2D) instead of the named
  `SOMDialogue3DDefault`. Player-made sounds without their own model
  use `POPM` / `P3OM` by camera mode. Menu sounds without a model use
  `IOPM`. Music uses `MDSC`. Update `known_ambiguities/audio-categories.md`
  and `sound-output-models.md`: the categories and dialogue models are now
  sourced from the data (keep the open parts: the multiplication, and
  dialogue's `DOP2` vs `DOP3` choice by conversation mode).

## 2. Menus, dialogue and the stats menu

- **2a Menu pausing.** While `Engine::in_menu_mode()`, `PIMC` pauses at once
  and `PDMC` fades out over `fMenuModeFadeOutTime` (3 s) then pauses. On
  leaving, both resume, `PDMC` fading in over `fMenuModeFadeInTime` (1 s).
  `PDLC` gets a `loading` input that stays false until a loading menu exists.
  This answers `known_ambiguities/messages.md`'s "whether world sounds already
  playing pause with the game": record the DOBJ source there. Open choice
  (**listen**): whether the fade-in also applies to the immediate category, and
  whether the console counts (it is menu mode here).
- **2b Dialogue fade.** While the player is in a conversation (the dialogue
  menu), `SFDC` ramps down. The level and times aren't public. Choose
  them (e.g. -6 dB over the menu-mode fade times) and record them in
  `known_ambiguities/audio-categories.md` (**listen**). Scenes the player
  isn't in don't count.
- **2c Stats menu.** While the skills menu (`StatsMenu`) is open, `SMSC`
  (ambient region sounds) is muted and the music switches to `SSSC` (`_NONE`:
  silence) through the music queue. Whether this cuts the track or fades it
  goes with step 4's fade rules.
- Tests: director tests of each state's targets and ramp times; one engine
  test that opening a pausing menu sets the director's menu input.

## 3. Under water

- The listener is submerged when the camera is below `water_level` at its
  position (`src/water.rs`). Use the same surface the swim code uses, and
  hysteresis only if it flickers.
- While submerged: categories flagged Mute When Submerged ramp to silence
  (`AudioCategoryMuteSubmerged`, `AMB`, `AMBr`); `UWLS` loops 2D; once step 5
  exists, the `Underwater` REVB replaces the acoustic space's reverb.
  Nothing links that REVB: no ASPC or DOBJ names it, so using it is a
  choice (record in `known_ambiguities/water.md`). Open choice: ramp time
  (none in the data; pick a short one, **listen**).
- Tests: director targets on entering and leaving; the flag read from
  `FNAM` bit 0.

## 4. Music

Each sub-step changes `Engine::update_music` / `src/music.rs`. Move the
selection logic into the director where it needs testing without a device.

- **4a Fades.** A change of music type fades the current track out over the
  incoming type's `WNAM` instead of stopping it. Abrupt types (flag 2) cut, as
  now. Open: whose fade time (outgoing vs incoming type's); take the
  incoming, record it.
- **4b Ducking.** A type with "Ducks Current Track" (0x20) pushed over a
  playing one lowers that one by its `PNAM` ducking dB over
  `fMusicDuckingSeconds`. The ducked type keeps playing, and comes back over
  `fMusicUnDuckingSeconds` when the ducking type leaves. This needs two music
  voices at once: `MusicState` becomes a small stack of playing tracks.
- **4c Track order.** Cycle Tracks (4) plays the type's tracks in turn rather
  than at random. Maintain Track Order (8) keeps `TNAM` order (else shuffle
  once per cycle). The 20-60 s silence between tracks (`next_at`) is unsourced:
  record it in a new `known_ambiguities/music.md`, together with the queue
  choices moved from `remaining-natives.md`.
- **4d Tracks.** `LNAM` loops (begin, end, count), silent tracks lasting
  `FLTV`, palettes' layers (each layer between zero ids plays together; one
  track picked per layer?) and `DNAM` fade-out, and cue points as the
  places a graceful change may cut (in place of waiting for the track's end).
  Survey the vanilla use of each first (`vrm-tool music-survey`), and implement
  only what vanilla data uses. Layer and cue-point semantics are open
  (**listen**).
- **4e Default music types.** `DFMS` when the location has none. `BTMS` while
  the player is in combat: the trigger is open; the GMST
  `fCombatMusicPlayerTargetedThreatRatio` suggests "enemies targeting the
  player", so start with any hostile actor in combat whose target is the
  player, record it. `DTMS` on the player's death. `LUMS` on a level-up (with
  `src/skills.rs`). `SCMS` on quest completion (a quest's "complete" via
  `CompleteQuest`, or its stage flag? Check which vanilla quests rely on
  it; open). `DCMS` is already pushed by clearing (see Done; switch it to the
  DOBJ entry). They are pushed onto the queue as the engine's own entries,
  as the discovery and cleared types are.
- Saves: the queue is saved already (`MUSQ`). Ducking and fades restart on
  load (a choice; record it).

## 5. Acoustic spaces, reverb and ambience

- **5a Which space.** Interiors: the cell's `XCAS`. Placed `ASPC` references
  (check how vanilla places them; `OBND`-bounded markers) override it inside
  their bounds: take the innermost / smallest containing the listener. Use
  the reference's rotation and scale (check the vanilla data for whether they
  rotate). Fallback: `RVBT` reverb, no ambient loop. Exteriors use placed spaces
  only. Survey first: how many cells have `XCAS`, how many ASPC references and
  where (`vrm-tool aspc-survey`).
- **5b Ambient loops.** The space's `SNAM` loops; in interiors, `RDAT`'s region
  sounds play as well (5d's rules). Crossfade on space changes (time open:
  choose ~1-2 s, record it).
- **5c Reverb.** A Jot-style feedback-delay network (or Freeverb-style
  comb/allpass) on the send bus, parameterised from REVB's I3DL2 fields: room and
  room-HF levels, reflections level and delay, reverb level and delay, decay
  time and HF ratio at the HF reference, diffusion and density. Each voice sends
  its output model's `NAM1` reverb send percent. 2D voices with no model send
  none (open). Crossfade between two reverb instances on a change.
  Record the mappings in `known_ambiguities/reverb.md`: int8 levels as dB, the
  "scaled reflect delay" (UESP's 0xF9 -> 300 ms), DefaultReverb's -100 dB as
  off. The algorithm can only approximate XAudio2's, so judge it by ear
  (**listen**). Keep it cheap: one network per active reverb, at most two
  during a crossfade.
- **5d Region sounds (`RDSA`).** The exterior regions of the player's cell
  pick from their sounds by weather class (pleasant / cloudy / rainy / snowy,
  from the current weather's classification, already read for
  `GetClassification`). The chance is open: per check interval, or a weight
  among looping beds? Survey which `RDSA` sounds are loops (most `AMBr` are
  `LP`) and which one-shots, and decide (record it). One-shots play at random
  positions around the listener? Check their output models: many are 2D.
- Saves: none needed (derived from position and weather).

## 6. Speaker output and panning

- **6a Device channels.** Mix to however many channels the device has (cpal
  config) with a standard downmix to stereo (C and surrounds at -3 dB, LFE
  dropped) when it has fewer than the eight `ONAM` outputs.
- **6b Defined speaker output** (`MNAM` 1): unpositioned sounds use the
  model's `ONAM` row levels (mono clips the mono row, stereo clips the left and
  right rows) instead of plain stereo. Levels are uint8: 0-100 as percent?
  Check the vanilla values' range and record it.
- **6c HRTF models** (`MNAM` 0, the positioned ones): replace the 60% linear pan
  with an equal-power pan and a mild rear attenuation / low-pass. A true HRTF is
  out of scope (record the choice). The "Stereo Rad" models' stereo radius
  exists only in their editor ids; no data field holds it, so it stays unused
  (record).

## 7. Priorities and voice limit

- Read `BNAM` priority into `SoundDesc` (and the `vrm-tool sndr-stats` column
  from 1a, to see the vanilla range).
- The mixer caps playing voices (the number is open: pick e.g. 64 and record it
  in `known_ambiguities/sound-variation.md` or a new `voices.md`). At the cap,
  a new voice takes the place of the playing voice with the lowest priority
  (open: which end of 0-255 wins; check vanilla: UI and dialogue should rank
  above ambient beds), ties broken by lowest current audibility (gain x falloff).
  Otherwise the new voice is dropped. Looping voices that lose their slot
  become virtual (keep position and time) and come back when a slot frees.
  Music and dialogue are exempt.
- Tests: mixer with a cap of 2 and three voices of given priorities and gains.

## 8. Audio settings (without the menu)

- `src/audio/settings.rs`: master volume plus the menu categories (those with
  flag 2), stored in the engine's own settings file in the user's config
  directory. Use `SkyrimPrefs.ini`'s `[AudioMenu]` key layout
  (`fAudioMasterVolume`, `uIDn` / `fValn`) so a player's existing values
  could be imported. Importing it is optional and off by default. A slider
  replaces its category's `UNAM` default in `category_volume`. The master
  replaces the mixer's made-up 0.8 headroom? Keep the headroom separate,
  record it.
- Console `audio [category value | master value]` to read and set them;
  `VRM_AUDIO_*` env overrides for offscreen runs. The System page (ROADMAP item 6)
  will call the same API.
- Tests: round-trip the file; a slider change reaching `category_volume`.

## 9. Lip sync (separate, last)

The largest item. It needs two formats the public sources don't document,
and work in the renderer:

- **9a Research spike** (game assets only, no engine binaries). The `.lip`
  chunk inside each `.fuz` (`FUZE`, version, lip size, then the lip data:
  header and keyed float tracks) and the head parts' `.tri` morphs (569 in
  Meshes0; the phoneme / expression morph names; `iLipMorphCount = 31`
  suggests 31 lip-driven targets, maybe 16 phonemes plus modifiers). Write
  `research/audio/lip-format.md` and a `vrm-tool lip-dump` / `tri-dump`. If
  the `.lip` tracks can't be decoded with confidence, stop and record
  the state in ROADMAP. Don't ship a guessed format. Voices without a `.lip`
  (loose `.wav` / `.xwm`) get no lip motion.
- **9b Morph targets on heads.** FaceGen heads (`facegeom/*.nif`) and their head
  parts' `.tri` files: load the matching morphs, and apply weighted morphs on
  the GPU (vertex offsets per target, weights per instance). This is the same
  machinery ROADMAP item 5 (the player's body: "the head's morphs (`.tri`)")
  and character creation need. Build it reusable, and mark that piece done
  there.
- **9c Driving it.** A speaking actor's lip tracks, sampled at the voice's
  playback position (`Audio` reports a voice's position), drive the phoneme
  weights. They blend with the speaker idles that already run. Mood and
  expression morphs stay out of scope.

## Roadmap and docs (with each step)

Move what's done into ROADMAP's Done with links here. ROADMAP's Audio item
then keeps what's blocked: the sliders' UI, loading-menu pausing, kill-move
sounds, and lip sync if 9a stalls. Answer open questions in the
`known_ambiguities/` files named above. Write the research notes under
`research/audio/`.

## Open questions (collected; decide in the step named)

- Dialogue fade level and times (2b). Fade-in of the immediate category (2a).
- Music: whose fade time (4a), palette layers and cue points (4d), the combat
  music trigger and success music trigger (4e).
- Acoustic spaces: rotation of placed spaces, crossfade time (5a / 5b); reverb
  field mappings and 2D sends (5c); `RDSA` chance semantics (5d).
- `ONAM` level scale (6b); voice cap and priority direction (7).
- `.lip` and `.tri` decoding (9a).

## Implementation log

- 0a: sample-clocked linear amplitude ramps; fade-then-pause; 5 ms volume/stop smoothing.
- 0b: linear frame interpolation, including the loop seam without a dropped frame.
- 0c: shared `VoiceParams` through pending decodes, descriptors and all playback callers.
- 0d: separate dry/send buses, fixed per-frame scratch (allocation-free for every callback size). Send remains silent until 5c.
- 1a: `audio-defaults` and priority in `sndr-stats`; public/data evidence in `research/audio/defaults.md`.
- 1b: cached `AudioDefaults`, with sparse override/null handling and synthetic DOBJ tests.
- 1c: DOBJ categories and dialogue/player/interface model fallbacks replace editor-ID guesses; INFO barks use DDSC.
- 2a: device-free director composes script/category policy; menu pause/fade and reserved loading input, with engine integration test.
- 2b: player dialogue fades SFDC by the documented provisional -6 dB / 3 s / 1 s choice; director test covers recovery.
- 2c: skills menu mutes SMSC and queues/forces SSSC until closing; music transition fade follows 4a.
- 3: camera submersion uses swimming water surfaces, SNCT bit-0 mutes, and UWLS loops with 150 ms transitions; pure tests cover enter/leave.
- 4a: incoming WNAM fades outgoing music; abrupt types cut; policy no longer requires a device.
- 4b: concurrent background music stack, PNAM ducking and INI 6/8 s envelopes; no saved playback cursors.
- 4c: per-type ordered/shuffled cycles and condition filtering; queue/gap choices consolidated in `known_ambiguities/music.md`.
- 4d: vanilla music survey; finite loops, timed silence, concurrent palette layers with duration/fade, cue transitions and combat outros.
- 4e: DFMS fallback; director-owned combat/death edges; level-up, visible quest completion and dungeon-cleared DOBJ stingers. Trigger choices researched/documented.
- 5a: acoustic survey and cached oriented marker selection, XCAS interiors/RVBT fallback; corrected placed bounds to XPRM from asset evidence.
- 5b: SNAM acoustic beds crossfade over 1.5 s; interior RDAT region reaches the director.
- 5c: original bounded eight-delay reverb network, I3DL2 field mapping, SOPM sends, underwater override and two-network crossfades; impulse/decay tests.
- 5d: weather/condition-filtered RDSA beds and independent one-shot checks, interior RDAT and exterior XCLR; chance/placement choices documented and scheduler tested.
- 6a: eight-speaker internal dry bus and device-channel mapping/downmix, including mono/quad/5.1/stereo.
- 6b: percentage ONAM mono/stereo rows, retained source channel metadata and per-speaker mixing tests.
- 6c: equal-power spatial pan and mild rear attenuation/filtering, with documented HRTF/radius limits.
- 7: BNAM priority, cap of 64 non-exempt audible voices, audibility tie-breaks and virtual loops; cap-2 mixer tests.
- 8: persistent AudioMenu settings, shared setter/console API and environment overrides; runtime sliders replace UNAM independently of headroom, with round-trip/volume tests.
