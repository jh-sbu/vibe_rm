//! The music queue. The location's music type is the implicit entry; music
//! types pushed on top of it (`MusicType.Add`, and the engine's discovery and
//! cleared stingers) play when their priority beats it. Choices:
//! `known_ambiguities/remaining-natives.md`. Saved: `MUSQ` (script entries).

use esp::FormId;

use crate::engine::Engine;

/// MUSC `FNAM` flags.
pub const PLAYS_ONE_SELECTION: u32 = 0x01;
pub const ABRUPT_TRANSITION: u32 = 0x02;
pub const CYCLE_TRACKS: u32 = 4;
pub const MAINTAIN_ORDER: u32 = 8;
pub const DUCKS_CURRENT: u32 = 0x20;

pub struct PendingDuration {
    pub path: String,
    pub loop_region: Option<crate::audio::LoopRegion>,
}
#[derive(Default)]
pub struct Playback {
    pub voices: Vec<crate::audio::VoiceId>,
    pub pending_duration: Option<PendingDuration>,
    pub layers: Vec<Layer>,
    pub until: Option<f64>,
    pub started: f64,
    pub fade_out: f32,
    pub fading: bool,
    pub cues: Vec<f32>,
    pub outro: Option<String>,
}
pub struct Layer {
    pub candidates: Vec<FormId>,
    pub playing: Box<Playback>,
}
impl Playback {
    pub fn active(&self, now: f64, playing: &impl Fn(crate::audio::VoiceId) -> bool) -> bool {
        if let Some(end) = self.until {
            return now < end;
        }
        self.pending_duration.is_some()
            || self.voices.iter().any(|&v| playing(v))
            || self.layers.iter().any(|l| l.playing.active(now, playing))
    }
    pub fn voice_ids(&self, ids: &mut Vec<crate::audio::VoiceId>) {
        ids.extend(&self.voices);
        for l in &self.layers {
            l.playing.voice_ids(ids);
        }
    }
    pub fn next_cue(&self, position: f32) -> Option<f32> {
        self.cues
            .iter()
            .copied()
            .filter(|c| *c > position + 0.001)
            .min_by(f32::total_cmp)
    }
}
pub struct Background {
    pub music: FormId,
    pub from_queue: bool,
    pub playback: Playback,
}

pub fn duck_gain(db: f32) -> f32 {
    10f32.powf(-db.clamp(0.0, 100.0) / 20.0)
}

/// Per-type cycle state, independent of playback and the audio device.
#[derive(Default)]
pub struct TrackOrder(std::collections::HashMap<FormId, std::collections::VecDeque<FormId>>);
impl TrackOrder {
    pub fn pick(
        &mut self,
        music: FormId,
        candidates: &[FormId],
        flags: u32,
        mut random: u64,
    ) -> Option<FormId> {
        if candidates.is_empty() {
            return None;
        }
        if flags & CYCLE_TRACKS == 0 {
            return Some(candidates[random as usize % candidates.len()]);
        }
        let cycle = self.0.entry(music).or_default();
        cycle.retain(|id| candidates.contains(id));
        if cycle.is_empty() {
            let mut tracks = candidates.to_vec();
            if flags & MAINTAIN_ORDER == 0 {
                for i in (1..tracks.len()).rev() {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    tracks.swap(i, (random % (i + 1) as u64) as usize);
                }
            }
            cycle.extend(tracks);
        }
        cycle.pop_front()
    }
}

/// A priority for a music type with no `PNAM`: the lowest.
const LOWEST_PRIORITY: u16 = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub music: FormId,
    /// Added by `MusicType.Add` (kept in saves), not by the engine.
    pub script: bool,
    /// A track of it has started.
    pub started: bool,
}

#[derive(Default)]
pub struct Queue {
    /// In the order added.
    pub(crate) entries: Vec<Entry>,
}

impl Queue {
    /// Queue `music`; one already queued stays where it is.
    pub fn add(&mut self, music: FormId, script: bool) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.music == music) {
            e.script |= script;
            return false;
        }
        self.entries.push(Entry {
            music,
            script,
            started: false,
        });
        true
    }

    pub fn remove(&mut self, music: FormId) -> bool {
        let n = self.entries.len();
        self.entries.retain(|e| e.music != music);
        self.entries.len() != n
    }

    pub fn remove_engine(&mut self, music: FormId) {
        self.entries.retain(|e| e.music != music || e.script);
    }

    pub fn contains(&self, music: FormId) -> bool {
        self.entries.iter().any(|e| e.music == music)
    }

    /// The music type to play: the queued entry with the highest priority (the
    /// lowest number; the first added among equals), over the location's
    /// unless that one has a strictly higher priority. `.1` is whether it is a
    /// queued entry.
    pub fn choose(
        &self,
        location: Option<FormId>,
        priority: impl Fn(FormId) -> u16,
    ) -> Option<(FormId, bool)> {
        let queued = self
            .entries
            .iter()
            .map(|e| e.music)
            .min_by_key(|&m| priority(m)); // min_by_key keeps the first of equals
        match (queued, location) {
            (Some(q), Some(l)) if priority(l) < priority(q) => Some((l, false)),
            (Some(q), _) => Some((q, true)),
            (None, l) => l.map(|l| (l, false)),
        }
    }

    /// The entries kept in a save, in order.
    pub fn script_entries(&self) -> Vec<FormId> {
        self.entries
            .iter()
            .filter(|e| e.script)
            .map(|e| e.music)
            .collect()
    }

    /// Replace the queue with the saved script entries.
    pub fn restore(&mut self, music: Vec<FormId>) {
        self.entries.clear();
        for m in music {
            self.add(m, true);
        }
    }
}

impl Engine {
    /// A music type's `PNAM` priority (1 highest, 100 lowest).
    pub(crate) fn music_priority(&self, music: FormId) -> u16 {
        self.lo
            .get(music)
            .and_then(|r| {
                r.get(b"PNAM")
                    .filter(|d| d.len() >= 2)
                    .map(|d| d[0..2].try_into().unwrap())
            })
            .map_or(LOWEST_PRIORITY, u16::from_le_bytes)
    }

    /// A music type's `FNAM` flags.
    pub(crate) fn music_flags(&self, music: FormId) -> u32 {
        self.lo
            .get(music)
            .and_then(|r| {
                r.get(b"FNAM")
                    .filter(|d| d.len() >= 4)
                    .map(|d| d[0..4].try_into().unwrap())
            })
            .map_or(0, u32::from_le_bytes)
    }

    fn music_duck_gain(&self, music: FormId) -> f32 {
        if self.music_flags(music) & DUCKS_CURRENT == 0 {
            return 1.0;
        }
        self.lo
            .get(music)
            .and_then(|r| {
                r.get(b"PNAM")
                    .filter(|d| d.len() >= 4)
                    .map(|d| u16::from_le_bytes(d[2..4].try_into().unwrap()) as f32 / 100.0)
            })
            .map_or(1.0, duck_gain)
    }
    pub(crate) fn apply_music_ducking(&mut self, seconds: f32) {
        let mut gain = self
            .music
            .music_type
            .map_or(1.0, |m| self.music_duck_gain(m));
        let base = self.audio_defaults.get(b"MDSC").map_or(1.0, |c| {
            crate::world::sound::category_static_volume(&self.lo, c)
        });
        let mut changes = Vec::new();
        for b in self.music.background.iter().rev() {
            let mut voices = Vec::new();
            b.playback.voice_ids(&mut voices);
            changes.extend(voices.into_iter().map(|v| (v, base * gain)));
            gain *= self.music_duck_gain(b.music);
        }
        if let Some(a) = self.audio.as_mut() {
            for (voice, volume) in changes {
                a.set_volume_with_fade(voice, volume, seconds);
            }
        }
    }

    /// Papyrus `MusicType.Add`.
    pub(crate) fn add_music(&mut self, music: FormId) {
        if self.lo.get(music).is_some_and(|r| r.tag().0 == *b"MUSC") {
            self.music.queue.add(music, true);
        }
    }

    /// Papyrus `MusicType.Remove`.
    pub(crate) fn remove_music(&mut self, music: FormId) {
        self.music.queue.remove(music);
    }

    /// Queue a music type the engine plays once (discovery, cleared).
    pub(crate) fn play_music_once(&mut self, music: FormId) {
        self.music.queue.add(music, false);
    }

    /// Restore the script entries a save kept, in place of this session's queue.
    pub(crate) fn restore_music_queue(&mut self, music: Vec<FormId>) {
        let mut voices = Vec::new();
        self.music.playback.voice_ids(&mut voices);
        for b in &self.music.background {
            b.playback.voice_ids(&mut voices);
        }
        if let Some(a) = &self.audio {
            for voice in voices {
                a.stop_with_fade(voice, 0.0);
            }
        }
        self.music = Default::default();
        self.music.queue.restore(music);
        self.audio_director.inputs = Default::default();
        self.audio_director.regions = Default::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: FormId = FormId(0x800);
    const B: FormId = FormId(0x801);
    const LOC: FormId = FormId(0x802);

    fn prio(m: FormId) -> u16 {
        match m {
            A => 40,
            B => 40,
            LOC => 60,
            _ => 100,
        }
    }

    #[test]
    fn script_add_promotes_an_engine_entry_without_moving_it() {
        let mut q = Queue::default();
        q.add(A, false);
        q.add(B, true);
        assert!(!q.add(A, true));
        assert_eq!(q.script_entries(), [A, B]);
        q.remove_engine(A);
        assert!(q.contains(A));
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn ducking_resumes_the_underlying_selection_and_restore_restarts_policy() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-music-stack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = [
            Node::top(
                b"MUST",
                vec![
                    RecordBuilder::new(b"MUST", FormId(0x810))
                        .u32(b"CNAM", crate::world::sound::MUST_SILENT)
                        .field(b"FLTV", &30f32.to_le_bytes()),
                ],
            ),
            Node::top(
                b"MUSC",
                vec![
                    RecordBuilder::new(b"MUSC", A)
                        .field(b"PNAM", &[60, 0, 0, 0])
                        .u32(b"TNAM", 0x810),
                    RecordBuilder::new(b"MUSC", B)
                        .field(b"PNAM", &[10, 0, 0x58, 2])
                        .u32(b"FNAM", DUCKS_CURRENT)
                        .u32(b"TNAM", 0x810),
                ],
            ),
        ];
        std::fs::write(
            dir.join("Music.esp"),
            write_plugin(0, &[], "test", "stack", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Music.esp");
        e.add_music(A);
        e.update_music();
        assert_eq!(e.music.playback.until, Some(30.0));
        e.add_music(B);
        e.update_music();
        assert_eq!(e.music.music_type, Some(B));
        assert_eq!(e.music.background[0].music, A);
        e.scripts.wall_time = 5.0;
        e.remove_music(B);
        e.update_music();
        assert_eq!(e.music.music_type, Some(A));
        assert!(e.music.background.is_empty());
        assert_eq!(e.music.playback.until, Some(30.0));
        e.restore_music_queue(vec![A]);
        assert!(e.music.music_type.is_none());
        assert!(e.music.playback.until.is_none());
        e.update_music();
        assert_eq!(e.music.playback.until, Some(35.0));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn no_music_type_plays_without_a_game() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-music-no-game-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = [
            Node::top(
                b"MUST",
                vec![
                    RecordBuilder::new(b"MUST", FormId(0x810))
                        .u32(b"CNAM", crate::world::sound::MUST_SILENT)
                        .field(b"FLTV", &30f32.to_le_bytes()),
                ],
            ),
            Node::top(
                b"MUSC",
                vec![
                    RecordBuilder::new(b"MUSC", A)
                        .field(b"PNAM", &[60, 0, 0, 0])
                        .u32(b"TNAM", 0x810),
                ],
            ),
        ];
        std::fs::write(
            dir.join("Music.esp"),
            write_plugin(0, &[], "test", "no game", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Music.esp");
        e.enter_main_menu();
        e.add_music(A);
        e.update_music();
        assert!(e.music.music_type.is_none());
        assert!(e.music.playback.until.is_none());
        e.no_game = false;
        e.update_music();
        assert_eq!(e.music.music_type, Some(A));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn silent_playback_uses_its_duration_and_cues_choose_the_next_boundary() {
        let p = Playback {
            until: Some(15.0),
            cues: vec![3.0, 6.0, 12.0],
            ..Default::default()
        };
        assert!(p.active(14.9, &|_| false));
        assert!(!p.active(15.0, &|_| false));
        assert_eq!(p.next_cue(3.0), Some(6.0));
        assert_eq!(p.next_cue(13.0), None);
        let palette = Playback {
            until: Some(10.0),
            layers: vec![Layer {
                candidates: vec![],
                playing: Box::new(Playback {
                    until: Some(20.0),
                    ..Default::default()
                }),
            }],
            ..Default::default()
        };
        assert!(
            !palette.active(10.0, &|_| false),
            "a palette deadline bounds even a longer layer"
        );
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn palette_layers_and_silent_tracks_advance_without_a_device() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-music-palette-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ids: Vec<u8> = [0x800u32, 0, 0x800]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        std::fs::create_dir_all(dir.join("sound/fx")).unwrap();
        let mut wav = b"RIFF".to_vec();
        wav.extend(16036u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(8000u32.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(16000u32.to_le_bytes());
        wav.extend([0; 16000]);
        std::fs::write(dir.join("sound/fx/test.wav"), wav).unwrap();
        let mut loops = 0.25f32.to_le_bytes().to_vec();
        loops.extend(0.75f32.to_le_bytes());
        loops.extend(2u32.to_le_bytes());
        let records = [Node::top(
            b"MUST",
            vec![
                RecordBuilder::new(b"MUST", FormId(0x800))
                    .u32(b"CNAM", crate::world::sound::MUST_SILENT)
                    .field(b"FLTV", &2f32.to_le_bytes()),
                RecordBuilder::new(b"MUST", FormId(0x801))
                    .u32(b"CNAM", crate::world::sound::MUST_PALETTE)
                    .field(b"FLTV", &10f32.to_le_bytes())
                    .field(b"DNAM", &1f32.to_le_bytes())
                    .field(b"SNAM", &ids),
                RecordBuilder::new(b"MUST", FormId(0x802))
                    .u32(b"CNAM", crate::world::sound::MUST_SINGLE)
                    .zstring(b"ANAM", "fx/test.wav")
                    .field(b"LNAM", &loops),
            ],
        )];
        std::fs::write(
            dir.join("Music.esp"),
            write_plugin(0, &[], "test", "palette", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Music.esp");
        let mut run = e.start_music_track(FormId(0x801), 0).unwrap();
        assert_eq!(run.layers.len(), 2);
        assert_eq!(run.until, Some(10.0));
        e.scripts.wall_time = 3.0;
        e.tick_music_track(&mut run, 0);
        assert_eq!(run.layers[0].playing.until, Some(5.0));
        e.scripts.wall_time = 9.0;
        e.tick_music_track(&mut run, 0);
        assert!(run.fading);
        let mut single = e.start_music_track(FormId(0x802), 0).unwrap();
        assert!(single.pending_duration.is_some());
        assert!(single.active(9.0, &|_| false));
        let timeout = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while single.pending_duration.is_some() {
            assert!(
                std::time::Instant::now() < timeout,
                "duration worker did not finish"
            );
            e.tick_music_track(&mut single, 0);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(single.until, Some(11.0));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn track_cycles_keep_order_or_shuffle_once_without_repeating() {
        let mut o = TrackOrder::default();
        for want in [A, B, LOC, A] {
            assert_eq!(
                o.pick(A, &[A, B, LOC], CYCLE_TRACKS | MAINTAIN_ORDER, 7),
                Some(want)
            );
        }
        let mut o = TrackOrder::default();
        let mut got: Vec<_> = (0..3)
            .map(|_| o.pick(A, &[A, B, LOC], CYCLE_TRACKS, 19).unwrap())
            .collect();
        got.sort_by_key(|id| id.0);
        assert_eq!(got, [A, B, LOC]);
        assert_eq!(o.pick(A, &[B], CYCLE_TRACKS, 1), Some(B));
    }
    #[test]
    fn ducking_is_db_attenuation() {
        assert_eq!(duck_gain(0.0), 1.0);
        assert!((duck_gain(6.0) - 0.501187).abs() < 1e-5);
        assert!((duck_gain(100.0) - 0.00001).abs() < 1e-8);
    }
    #[test]
    fn empty_queue_plays_the_locations() {
        let q = Queue::default();
        assert_eq!(q.choose(Some(LOC), prio), Some((LOC, false)));
        assert_eq!(q.choose(None, prio), None);
    }

    #[test]
    fn add_does_not_duplicate_or_move() {
        let mut q = Queue::default();
        assert!(q.add(A, true));
        assert!(q.add(B, true));
        assert!(!q.add(A, true));
        assert_eq!(q.script_entries(), [A, B]);
        assert!(q.remove(A));
        assert!(!q.remove(A));
        assert_eq!(q.script_entries(), [B]);
    }

    #[test]
    fn priority_beats_the_location_and_ties_go_to_the_first_added() {
        let mut q = Queue::default();
        q.add(B, true);
        q.add(A, true);
        assert_eq!(q.choose(Some(LOC), prio), Some((B, true)));
        q.remove(B);
        assert_eq!(q.choose(Some(LOC), prio), Some((A, true)));
        // A queued entry beats a location of equal priority, not a better one.
        let better = |m: FormId| if m == LOC { 10 } else { prio(m) };
        assert_eq!(q.choose(Some(LOC), better), Some((LOC, false)));
        let equal = |_: FormId| 50;
        assert_eq!(q.choose(Some(LOC), equal), Some((A, true)));
    }

    #[test]
    fn only_script_entries_are_saved_and_restored() {
        let mut q = Queue::default();
        q.add(A, false);
        q.add(B, true);
        assert_eq!(q.script_entries(), [B]);
        q.restore(vec![B, A]);
        assert_eq!(q.script_entries(), [B, A]);
        assert!(q.entries.iter().all(|e| e.script && !e.started));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn music_type_natives_queue_only_music_records() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        use papyrus::Value;
        let dir = std::env::temp_dir().join(format!("vrm-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = vec![
            Node::top(
                b"MUSC",
                vec![
                    RecordBuilder::new(b"MUSC", FormId(0x800))
                        .field(b"PNAM", &[10, 0, 0, 0])
                        .u32(b"FNAM", PLAYS_ONE_SELECTION),
                    RecordBuilder::new(b"MUSC", FormId(0x801)),
                ],
            ),
            Node::top(b"SNDR", vec![RecordBuilder::new(b"SNDR", FormId(0x802))]),
        ];
        std::fs::write(
            dir.join("Music.esp"),
            write_plugin(0, &[], "test", "synthetic music", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Music.esp");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(e.music_priority(FormId(0x800)), 10);
        assert_eq!(e.music_priority(FormId(0x801)), 100);
        assert_eq!(e.music_flags(FormId(0x800)), PLAYS_ONE_SELECTION);
        for (id, func) in [(0x800, "add"), (0x802, "add"), (0x999, "add")] {
            let this = e.object_value(FormId(id));
            crate::script::natives::call(&mut e, "musictype", func, Some(&this), &[]);
        }
        assert_eq!(e.music.queue.script_entries(), [FormId(0x800)]);
        let this = e.object_value(FormId(0x800));
        crate::script::natives::call(&mut e, "musictype", "remove", Some(&this), &[Value::None]);
        assert!(e.music.queue.entries.is_empty());
    }
}
