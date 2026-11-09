//! The `Debug` and `Utility` natives meant for testing: frame-rate capture,
//! god mode, hiding the menus, foot IK, moving the player by name, and the
//! profiling and budget queries, which have nothing behind them here
//! (`research/pex-natives/debug-batch14-utility.md`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};

use papyrus::{NativeResult, Value};

use crate::engine::Engine;

/// Frame times for `Utility.CaptureFrameRate` (the latest) and the
/// `Start/EndFrameRateCapture` window's average, least and most.
#[derive(Default)]
pub struct FrameRates {
    recent: VecDeque<f32>,
    capturing: bool,
    window: Vec<f32>,
}

/// Frame rates `CaptureFrameRate` can report (its string is "up to about
/// 1K characters").
const RECENT: usize = 200;

/// `Debug.SetFootIK(false)` turns every actor's foot placement off.
static FOOT_IK_OFF: AtomicBool = AtomicBool::new(false);

/// Whether `Debug.SetFootIK` left foot IK on.
pub fn foot_ik_on() -> bool {
    !FOOT_IK_OFF.load(Ordering::Relaxed)
}

impl FrameRates {
    /// One frame of `dt` seconds.
    pub fn observe(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let fps = 1.0 / dt;
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(fps);
        if self.capturing {
            self.window.push(fps);
        }
    }

    /// The last `n` frame rates, oldest first, comma separated.
    pub fn capture(&self, n: usize) -> String {
        let skip = self.recent.len().saturating_sub(n);
        self.recent
            .iter()
            .skip(skip)
            .map(|f| format!("{f:.1}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn start(&mut self) {
        self.capturing = true;
        self.window.clear();
    }

    pub fn end(&mut self) {
        self.capturing = false;
    }

    pub fn average(&self) -> f32 {
        if self.window.is_empty() {
            return 0.0;
        }
        self.window.iter().sum::<f32>() / self.window.len() as f32
    }

    pub fn min(&self) -> f32 {
        self.window.iter().copied().reduce(f32::min).unwrap_or(0.0)
    }

    pub fn max(&self) -> f32 {
        self.window.iter().copied().reduce(f32::max).unwrap_or(0.0)
    }
}

/// The natives, or None for one this module doesn't handle.
pub(crate) fn call(
    e: &mut Engine,
    class: &str,
    func: &str,
    args: &[Value],
) -> Option<NativeResult> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let v = |x: Value| Some(NativeResult::Value(x));
    match (class, func) {
        ("utility", "captureframerate") => v(Value::str(
            &e.frame_rates.capture(arg(0).as_int().max(0) as usize),
        )),
        ("utility", "startframeratecapture") => {
            e.frame_rates.start();
            v(Value::None)
        }
        ("utility", "endframeratecapture") => {
            e.frame_rates.end();
            v(Value::None)
        }
        ("utility", "getaverageframerate") => v(Value::Float(e.frame_rates.average())),
        ("utility", "getminframerate") => v(Value::Float(e.frame_rates.min())),
        ("utility", "getmaxframerate") => v(Value::Float(e.frame_rates.max())),
        // Script memory budgets: none are kept.
        ("utility", "getcurrentmemory") | ("utility", "getbudgetcount") => v(Value::Int(0)),
        ("utility", "getcurrentbudget") => v(Value::Int(0)),
        ("utility", "overbudget") => v(Value::Bool(false)),
        ("utility", "getbudgetname") => v(Value::str("")),
        // Profiling writes to the game's logs: nothing here.
        ("debug", "startscriptprofiling")
        | ("debug", "stopscriptprofiling")
        | ("debug", "startstackprofiling")
        | ("debug", "stopstackprofiling")
        | ("form", "startobjectprofiling")
        | ("form", "stopobjectprofiling")
        | ("alias", "startobjectprofiling")
        | ("alias", "stopobjectprofiling")
        | ("activemagiceffect", "startobjectprofiling")
        | ("activemagiceffect", "stopobjectprofiling") => v(Value::None),
        // User logs go to the debug log with the rest (`TraceUser`).
        ("debug", "openuserlog") => v(Value::Bool(true)),
        ("debug", "closeuserlog") => v(Value::None),
        ("debug", "getconfigname") => v(Value::str("")),
        ("debug", "debugchannelnotify") | ("debug", "dbsendplayerposition") => {
            log::debug!("papyrus {func}: {:?}", args);
            v(Value::None)
        }
        ("debug", "dumpaliasdata") => {
            if let Some(q) = arg(0).as_form() {
                log::debug!(
                    "alias data of {}: {:?}",
                    q,
                    e.scripts.quests.get(&esp::FormId(q)).map(|s| &s.aliases)
                );
            }
            v(Value::None)
        }
        ("debug", "showrefposition") => {
            if let Some(r) = arg(0).as_form() {
                let p = e.ref_position(esp::FormId(r));
                e.scripts.notify(format!("{}: {p:?}", esp::FormId(r)));
            }
            v(Value::None)
        }
        // The console's `tcl`, `tai`, `tgm` and `tm`.
        ("debug", "togglecollisions") => {
            e.player.noclip = !e.player.noclip;
            v(Value::None)
        }
        ("debug", "toggleai") => {
            e.ai_enabled = !e.ai_enabled;
            v(Value::None)
        }
        ("debug", "setgodmode") => {
            e.god_mode = arg(0).as_bool();
            v(Value::None)
        }
        ("debug", "togglemenus") => {
            e.menus_hidden = !e.menus_hidden;
            v(Value::None)
        }
        ("debug", "setfootik") => {
            FOOT_IK_OFF.store(!arg(0).as_bool(), Ordering::Relaxed);
            v(Value::None)
        }
        // `coc`: the cell by editor id. The move is made at once, so the
        // waiting form has nothing left to wait for.
        ("debug", "centeroncell") | ("debug", "centeroncellandwait") => {
            let name = arg(0).to_string();
            match e.lo.find_editor_id(&name) {
                Some(cell) if e.lo.tag_of(cell).is_some_and(|t| t.0 == *b"CELL") => {
                    if let Err(why) = e.center_on_cell(cell) {
                        log::warn!("Debug.{func}({name}): {why:#}");
                    }
                }
                _ => log::warn!("Debug.{func}: no cell {name}"),
            }
            v(Value::None)
        }
        // `player.moveto` a reference by editor id; the float returned isn't
        // described (0 here).
        ("debug", "playermovetoandwait") => {
            let name = arg(0).to_string();
            match e.resolve_form(&name) {
                Some(r) => e.queue_player_moveto(r),
                None => log::warn!("Debug.PlayerMoveToAndWait: no reference {name}"),
            }
            v(Value::Float(0.0))
        }
        ("debug", "quitgame") => {
            e.quit_requested = true;
            v(Value::None)
        }
        ("debug", "takescreenshot") => {
            e.screenshot_requests.push(arg(0).to_string());
            v(Value::None)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::FrameRates;

    #[test]
    fn a_capture_window_keeps_its_average_least_and_most() {
        let mut f = FrameRates::default();
        f.observe(1.0 / 30.0);
        f.start();
        for dt in [1.0 / 60.0, 1.0 / 20.0, 1.0 / 40.0] {
            f.observe(dt);
        }
        f.end();
        f.observe(1.0 / 10.0);
        assert!((f.average() - 40.0).abs() < 0.01);
        assert!((f.min() - 20.0).abs() < 0.01);
        assert!((f.max() - 60.0).abs() < 0.01);
        assert_eq!(f.capture(2), "40.0,10.0");
        assert_eq!(f.capture(99).split(',').count(), 5);
    }
}
