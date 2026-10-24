//! The music queue. The location's music type is the implicit entry; music
//! types pushed on top of it (`MusicType.Add`, and the engine's discovery and
//! cleared stingers) play when their priority beats it. Choices:
//! `known_ambiguities/remaining-natives.md`. Saved: `MUSQ` (script entries).

use esp::FormId;

use crate::engine::Engine;

/// MUSC `FNAM` flags.
pub const PLAYS_ONE_SELECTION: u32 = 0x01;
pub const ABRUPT_TRANSITION: u32 = 0x02;

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
        if self.entries.iter().any(|e| e.music == music) {
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
        self.music.queue.restore(music);
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
