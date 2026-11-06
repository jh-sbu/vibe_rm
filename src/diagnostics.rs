//! Opt-in CPU wall timings. Nested stages are inclusive; no GPU synchronization
//! is introduced. Only work inside a redraw is collected (not startup loading).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn threshold() -> Option<Duration> {
    static CONFIG: OnceLock<Option<Duration>> = OnceLock::new();
    *CONFIG.get_or_init(|| {
        let value = std::env::var("VRM_FRAME_TIMINGS_MS").ok()?;
        let ms = value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0);
        match ms.and_then(|ms| Duration::try_from_secs_f64(ms / 1000.0).ok()) {
            Some(d) => Some(d),
            None => {
                log::warn!(
                    "VRM_FRAME_TIMINGS_MS must be a finite, nonnegative number; timings disabled"
                );
                None
            }
        }
    })
}

#[derive(Default)]
struct Sample {
    stages: BTreeMap<&'static str, Duration>,
    counts: BTreeMap<&'static str, usize>,
    cells: Vec<(i32, i32, Duration)>,
    schedule_phases: [Duration; SCHEDULE_PHASES.len()],
    schedule_top: Vec<ScheduleRecord>,
    schedule_batches: Vec<(usize, usize, usize)>,
}

impl Sample {
    fn add(&mut self, name: &'static str, elapsed: Duration) {
        *self.stages.entry(name).or_default() += elapsed;
    }

    fn schedule_entry(&mut self, entry: ScheduleRecord) {
        for (sum, time) in self.schedule_phases.iter_mut().zip(entry.phases) {
            *sum += time;
        }
        // Keep the slowest eight occurrences, including sub-millisecond entries:
        // many individually cheap entries can still make an expensive batch.
        let at = self
            .schedule_top
            .partition_point(|e| e.total >= entry.total);
        if at < 8 {
            self.schedule_top.insert(at, entry);
            self.schedule_top.truncate(8);
        }
    }

    fn report(&self, elapsed: Duration, limit: Duration) -> bool {
        elapsed >= limit || !self.cells.is_empty()
    }
}

#[derive(Default)]
struct State {
    sample: Option<Sample>,
    sequence: u64,
    last_end: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Owns one redraw, including early returns. Dropping it emits one compact report
/// for a slow frame or a frame that loaded cells. Logging is outside measured work.
pub struct Frame {
    start: Option<Instant>,
    gap: Duration,
}

impl Frame {
    pub fn begin() -> Self {
        let Some(_) = threshold() else {
            return Self {
                start: None,
                gap: Duration::ZERO,
            };
        };
        let now = Instant::now();
        let gap = STATE.with_borrow_mut(|state| {
            debug_assert!(state.sample.is_none());
            state.sequence += 1;
            state.sample = Some(Sample::default());
            state.last_end.map_or(Duration::ZERO, |end| now - end)
        });
        Self {
            start: Some(now),
            gap,
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let Some(start) = self.start else { return };
        let elapsed = start.elapsed();
        let (sequence, sample) =
            STATE.with_borrow_mut(|state| (state.sequence, state.sample.take().unwrap()));
        if sample.report(elapsed.saturating_add(self.gap), threshold().unwrap()) {
            use std::fmt::Write;
            let mut detail = String::new();
            for (name, time) in sample.stages {
                let _ = write!(detail, " {name}={:.3}ms", time.as_secs_f64() * 1000.0);
            }
            for (name, count) in sample.counts {
                let _ = write!(detail, " {name}={count}");
            }
            for (x, y, time) in sample.cells {
                let _ = write!(
                    detail,
                    " cell({x},{y})={:.3}ms",
                    time.as_secs_f64() * 1000.0
                );
            }
            for (name, time) in SCHEDULE_PHASES.iter().zip(sample.schedule_phases) {
                if !time.is_zero() {
                    let _ = write!(
                        detail,
                        " ai.schedule.entry.{name}={:.3}ms",
                        time.as_secs_f64() * 1000.0
                    );
                }
            }
            for (start, end, total) in sample.schedule_batches {
                let _ = write!(detail, " schedule_batch={start}..{end}/{total}");
            }
            log::info!(target: "vibe_rm::diagnostics",
                "frame={sequence} work={:.3}ms gap_before={:.3}ms{detail}",
                elapsed.as_secs_f64() * 1000.0, self.gap.as_secs_f64() * 1000.0);
            for entry in sample.schedule_top {
                detail.clear();
                for (name, time) in SCHEDULE_PHASES.iter().zip(entry.phases) {
                    if !time.is_zero() {
                        let _ = write!(detail, " {name}={:.3}ms", time.as_secs_f64() * 1000.0);
                    }
                }
                let base = entry
                    .base
                    .map_or_else(|| "none".to_owned(), |id| format!("{id:08X}"));
                let package = entry
                    .package
                    .map_or_else(|| "none".to_owned(), |id| format!("{id:08X}"));
                log::info!(target: "vibe_rm::diagnostics",
                    "schedule_entry redraw={sequence} index={} actor={:08X} base={base} package={package} total={:.3}ms{detail}",
                    entry.index, entry.actor, entry.total.as_secs_f64() * 1000.0);
            }
        }
        STATE.with_borrow_mut(|state| state.last_end = Some(Instant::now()));
    }
}

/// Cheap no-op when disabled or outside a redraw; all spans stay on the caller
/// thread, including waits for parallel asset preparation.
pub struct Span {
    start: Option<Instant>,
    name: &'static str,
    cell: Option<(i32, i32)>,
}

impl Span {
    pub fn new(name: &'static str) -> Self {
        let active = threshold().is_some() && STATE.with_borrow(|s| s.sample.is_some());
        Self {
            start: active.then(Instant::now),
            name,
            cell: None,
        }
    }

    pub fn cell(x: i32, y: i32) -> Self {
        Self {
            cell: Some((x, y)),
            ..Self::new("cell.total")
        }
    }

    /// Finish this phase and start the next, preserving a continuous timeline.
    pub fn next(&mut self, name: &'static str) {
        if let Some(start) = &mut self.start {
            let now = Instant::now();
            record(self.name, now - *start);
            *start = now;
        }
        self.name = name;
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let elapsed = start.elapsed();
            record(self.name, elapsed);
            if let Some((x, y)) = self.cell {
                STATE.with_borrow_mut(|s| {
                    if let Some(sample) = &mut s.sample {
                        sample.cells.push((x, y, elapsed));
                    }
                });
            }
        }
    }
}

fn record(name: &'static str, elapsed: Duration) {
    STATE.with_borrow_mut(|s| {
        if let Some(sample) = &mut s.sample {
            sample.add(name, elapsed);
        }
    });
}

pub fn count(name: &'static str, amount: usize) {
    if threshold().is_some() {
        STATE.with_borrow_mut(|s| {
            if let Some(sample) = &mut s.sample {
                *sample.counts.entry(name).or_default() += amount;
            }
        });
    }
}

// Fixed phase arrays avoid per-phase allocations and map lookups; only the
// bounded list of slow entries allocates storage per frame.
#[derive(Clone, Copy)]
pub enum SchedulePhase {
    Disabled,
    Body,
    Away,
    Reference,
    EditorPlace,
    BaseNpc,
    ScenePackages,
    AliasPackages,
    OwnPackages,
    Select,
    Place,
    Heading,
    Store,
}

const SCHEDULE_PHASES: [&str; 13] = [
    "disabled",
    "body",
    "away",
    "reference",
    "editor_place",
    "base_npc",
    "scene_packages",
    "alias_packages",
    "own_packages",
    "select",
    "place",
    "heading",
    "store",
];

struct ScheduleRecord {
    index: usize,
    actor: u32,
    base: Option<u32>,
    package: Option<u32>,
    total: Duration,
    phases: [Duration; SCHEDULE_PHASES.len()],
}

/// RAII keeps timings for entries that exit early (disabled, dead, missing, etc.).
/// IDs are recorded directly; no name/record lookup is performed for diagnostics.
pub struct ScheduleEntry {
    start: Option<Instant>,
    phase_start: Option<Instant>,
    phase: SchedulePhase,
    entry: ScheduleRecord,
}

impl ScheduleEntry {
    pub fn new(actor: u32, index: usize) -> Self {
        let active = threshold().is_some() && STATE.with_borrow(|s| s.sample.is_some());
        let start = active.then(Instant::now);
        Self {
            start,
            phase_start: start,
            phase: SchedulePhase::Disabled,
            entry: ScheduleRecord {
                index,
                actor,
                base: None,
                package: None,
                total: Duration::ZERO,
                phases: [Duration::ZERO; SCHEDULE_PHASES.len()],
            },
        }
    }

    pub fn base(&mut self, base: u32) {
        self.entry.base = Some(base);
    }
    pub fn package(&mut self, package: Option<u32>) {
        self.entry.package = package;
    }

    pub fn next(&mut self, phase: SchedulePhase) {
        if let Some(start) = &mut self.phase_start {
            let now = Instant::now();
            self.entry.phases[self.phase as usize] += now - *start;
            *start = now;
        }
        self.phase = phase;
    }
}

impl Drop for ScheduleEntry {
    fn drop(&mut self) {
        let Some(start) = self.start else { return };
        let now = Instant::now();
        self.entry.phases[self.phase as usize] += now - self.phase_start.unwrap();
        self.entry.total = now - start;
        STATE.with_borrow_mut(|s| {
            if let Some(sample) = &mut s.sample {
                sample.schedule_entry(ScheduleRecord { ..self.entry });
            }
        });
    }
}

pub fn schedule_batch(start: usize, end: usize, total: usize) {
    if threshold().is_some() {
        STATE.with_borrow_mut(|s| {
            if let Some(sample) = &mut s.sample {
                sample.schedule_batches.push((start, end, total));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_stages_accumulate_and_streaming_frames_survive_threshold() {
        let mut sample = Sample::default();
        sample.add("parse", Duration::from_millis(3));
        sample.add("parse", Duration::from_millis(7));
        assert_eq!(sample.stages["parse"], Duration::from_millis(10));
        let limit = Duration::from_millis(20);
        assert!(!sample.report(Duration::from_millis(10), limit));
        assert!(sample.report(limit, limit));
        sample.cells.push((1, -2, Duration::from_millis(4)));
        assert!(sample.report(Duration::from_millis(10), limit));
    }

    #[test]
    fn nested_phases_and_early_return_preserve_cell_timing_without_leaking() {
        STATE.with_borrow_mut(|s| s.sample = Some(Sample::default()));
        fn load() {
            let _cell = Span {
                start: Some(Instant::now()),
                name: "cell.total",
                cell: Some((4, -12)),
            };
            let mut phases = Span {
                start: Some(Instant::now()),
                name: "cell.references",
                cell: None,
            };
            phases.next("cell.objects");
            // Simulate an early exit from a loading phase.
        }
        load();
        let sample = STATE.with_borrow_mut(|s| s.sample.take().unwrap());
        assert_eq!(sample.cells.len(), 1);
        assert_eq!((sample.cells[0].0, sample.cells[0].1), (4, -12));
        assert_eq!(sample.cells[0].2, sample.stages["cell.total"]);
        assert!(
            sample.stages["cell.total"]
                >= sample.stages["cell.references"] + sample.stages["cell.objects"]
        );
        record("outside_frame", Duration::from_secs(1));
        STATE.with_borrow(|s| assert!(s.sample.is_none()));
    }

    #[test]
    fn schedule_top_is_bounded_but_totals_include_every_entry() {
        let mut sample = Sample::default();
        // Ascending entries exercise replacement, then a cheap entry is rejected.
        for index in 0..20 {
            let time = Duration::from_micros((index + 1) as u64);
            let mut phases = [Duration::ZERO; SCHEDULE_PHASES.len()];
            phases[SchedulePhase::Select as usize] = time;
            sample.schedule_entry(ScheduleRecord {
                index,
                actor: 0x100 + index as u32,
                base: Some(0x200),
                package: Some(0x300),
                total: time,
                phases,
            });
        }
        sample.schedule_entry(ScheduleRecord {
            index: 20,
            actor: 0x999,
            base: None,
            package: None,
            total: Duration::from_micros(1),
            phases: [Duration::ZERO; SCHEDULE_PHASES.len()],
        });
        assert_eq!(sample.schedule_top.len(), 8);
        assert_eq!(
            sample
                .schedule_top
                .iter()
                .map(|e| e.index)
                .collect::<Vec<_>>(),
            (12..20).rev().collect::<Vec<_>>()
        );
        assert_eq!(
            sample.schedule_phases[SchedulePhase::Select as usize],
            Duration::from_micros(210)
        );
        let top = &sample.schedule_top[0];
        assert_eq!(
            (top.actor, top.base, top.package),
            (0x113, Some(0x200), Some(0x300))
        );
    }

    #[test]
    fn skipped_schedule_entries_keep_identity_and_completed_phases() {
        STATE.with_borrow_mut(|s| s.sample = Some(Sample::default()));
        for index in 0..2 {
            let mut entry = ScheduleEntry::new(0xABC, index);
            // Enable just this guard, without mutating process environment or
            // the global configuration shared by parallel tests.
            entry.start = Some(Instant::now());
            entry.phase_start = entry.start;
            if index == 0 {
                continue;
            }
            entry.next(SchedulePhase::Reference);
            entry.base(0xDEF);
            entry.next(SchedulePhase::Select);
            entry.package(Some(0x123));
        }
        let sample = STATE.with_borrow_mut(|s| s.sample.take().unwrap());
        assert_eq!(sample.schedule_top.len(), 2);
        for entry in &sample.schedule_top {
            assert_eq!(entry.actor, 0xABC);
            assert_eq!(entry.total, entry.phases.iter().copied().sum::<Duration>());
            if entry.index == 0 {
                assert_eq!((entry.base, entry.package), (None, None));
                assert_eq!(entry.total, entry.phases[SchedulePhase::Disabled as usize]);
            } else {
                assert_eq!((entry.base, entry.package), (Some(0xDEF), Some(0x123)));
            }
        }
        // Work outside a redraw must not leak into the next sample.
        drop(ScheduleEntry::new(0xBAD, 99));
        STATE.with_borrow(|s| assert!(s.sample.is_none()));
    }
}
