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
}

impl Sample {
    fn add(&mut self, name: &'static str, elapsed: Duration) {
        *self.stages.entry(name).or_default() += elapsed;
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
            log::info!(target: "vibe_rm::diagnostics",
                "frame={sequence} work={:.3}ms gap_before={:.3}ms{detail}",
                elapsed.as_secs_f64() * 1000.0, self.gap.as_secs_f64() * 1000.0);
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
}
