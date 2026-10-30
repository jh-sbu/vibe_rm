//! Loading screens (`LSCR`): their text, conditions and model, and which
//! one a load shows. The rules UESP gives ("If a condition is satisfied,
//! that image *will* appear ... If no conditions are satisfied, one of the
//! remaining load screens is selected at random") leave open what is
//! evaluated where; the choices are in `known_ambiguities/loading-screen.md`.
//!
//! The random numbers come from a stream of their own (`Engine::
//! load_screen_rng`), never saved and never touching the gameplay or
//! condition streams a save restores: `GetRandomPercent` in a screen's
//! conditions reads it in place of the condition stream.

use esp::FormId;
use glam::Vec3;

use crate::condition::{self, Condition};
use crate::engine::{Engine, PLAYER_REF};

/// `LSCR` record flag 10 (xEdit): "Displays In Main Menu".
const DISPLAYS_IN_MAIN_MENU: u32 = 1 << 10;

/// One loading screen.
#[derive(Debug, Clone)]
pub struct LoadScreen {
    pub id: FormId,
    pub text: String,
    pub conditions: Vec<Condition>,
    /// `NNAM`: the model's `STAT`.
    pub model: Option<FormId>,
    /// `SNAM`.
    pub scale: f32,
    /// `RNAM`, degrees about X, Y and Z.
    pub rotation: [i16; 3],
    /// `ONAM`: the range the model may be turned through, degrees.
    pub turn: (i16, i16),
    /// `XNAM`.
    pub offset: Vec3,
    /// `MOD2`: the camera path's model.
    pub camera: Option<String>,
    pub main_menu: bool,
}

impl LoadScreen {
    pub fn parse(e: &Engine, id: FormId) -> Option<LoadScreen> {
        let rec = e.lo.get(id)?;
        let i16_at = |d: &[u8], i: usize| {
            d.get(i * 2..i * 2 + 2)
                .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
        };
        let f32_at = |d: &[u8], i: usize| {
            d.get(i * 4..i * 4 + 4)
                .map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
        };
        let rnam = rec.get(b"RNAM").unwrap_or_default();
        let onam = rec.get(b"ONAM");
        let xnam = rec.get(b"XNAM").unwrap_or_default();
        Some(LoadScreen {
            id,
            text: rec
                .get(b"DESC")
                .map(|d| e.lo.lstring(&rec, d))
                .unwrap_or_default(),
            conditions: condition::parse_all(&rec),
            model: rec
                .get(b"NNAM")
                .filter(|d| d.len() >= 4)
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))))
                .filter(|f| !f.is_null()),
            scale: rec.get(b"SNAM").map_or(1.0, |d| f32_at(d, 0)).max(0.01),
            rotation: [i16_at(rnam, 0), i16_at(rnam, 1), i16_at(rnam, 2)],
            turn: onam.map_or((-180, 180), |d| (i16_at(d, 0), i16_at(d, 1))),
            offset: Vec3::new(f32_at(xnam, 0), f32_at(xnam, 1), f32_at(xnam, 2)),
            camera: rec
                .get(b"MOD2")
                .map(|d| String::from_utf8_lossy(d.split(|&b| b == 0).next().unwrap_or(d)).into())
                .filter(|p: &String| !p.is_empty()),
            main_menu: rec.flags() & DISPLAYS_IN_MAIN_MENU != 0,
        })
    }
}

/// The screens a load picks from, chosen as it starts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    /// The screen shown: its model, and the first text.
    pub first: Option<usize>,
    /// The screens later texts come from.
    pub pool: Vec<usize>,
}

/// A draw below `n` from a xorshift stream.
pub fn roll(state: &mut u64, n: usize) -> usize {
    let mut x = (*state).max(1);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) % n.max(1) as u64) as usize
}

/// Picks the screens: from the main menu, the ones flagged for it if any
/// (else the unconditioned ones); in game, one of those whose conditions
/// pass (`passing[i]`) if any, else one without conditions. Later texts come
/// from the passing and the unconditioned ones together.
pub fn choose(
    screens: &[LoadScreen],
    from_main_menu: bool,
    passing: &[bool],
    rng: &mut u64,
) -> Choice {
    let indices = |keep: &dyn Fn(usize) -> bool| -> Vec<usize> {
        (0..screens.len()).filter(|&i| keep(i)).collect()
    };
    let unconditioned = indices(&|i| screens[i].conditions.is_empty());
    let (first_pool, pool) = if from_main_menu {
        let flagged = indices(&|i| screens[i].main_menu);
        if flagged.is_empty() {
            (unconditioned.clone(), unconditioned)
        } else {
            (flagged.clone(), flagged)
        }
    } else {
        let passed = indices(&|i| !screens[i].conditions.is_empty() && passing[i]);
        let pool = indices(&|i| screens[i].conditions.is_empty() || passing[i]);
        if passed.is_empty() {
            (unconditioned, pool)
        } else {
            (passed, pool)
        }
    };
    let first = (!first_pool.is_empty()).then(|| first_pool[roll(rng, first_pool.len())]);
    Choice { first, pool }
}

impl Engine {
    /// Every loading screen in the load order, read once.
    pub fn load_screens(&self) -> &[LoadScreen] {
        self.load_screen_cache.get_or_init(|| {
            self.lo
                .ids_of_type(b"LSCR")
                .iter()
                .filter_map(|&id| LoadScreen::parse(self, id))
                .collect()
        })
    }

    /// The screens for a load starting now, their conditions run on the
    /// player where they stand. `GetRandomPercent` draws from the loading
    /// screens' stream; the condition stream is left as it was.
    pub(crate) fn choose_load_screens(&mut self) -> Choice {
        let from_main_menu = self.no_game;
        let saved = self.cond_rng.get();
        self.cond_rng.set(self.load_screen_rng);
        let ctx = condition::Context {
            subject: Some(PLAYER_REF),
            ..Default::default()
        };
        let passing: Vec<bool> = self
            .load_screens()
            .iter()
            .map(|s| !from_main_menu && condition::evaluate(self, &s.conditions, ctx))
            .collect();
        self.load_screen_rng = self.cond_rng.get();
        self.cond_rng.set(saved);
        let mut rng = self.load_screen_rng;
        let choice = choose(self.load_screens(), from_main_menu, &passing, &mut rng);
        self.load_screen_rng = rng;
        choice
    }

    /// A later text: from the pool, not the one just shown if another is
    /// there.
    pub(crate) fn later_load_screen(
        &mut self,
        pool: &[usize],
        last: Option<usize>,
    ) -> Option<usize> {
        let others: Vec<usize> = pool.iter().copied().filter(|&i| Some(i) != last).collect();
        let from = if others.is_empty() { pool } else { &others[..] };
        if from.is_empty() {
            return None;
        }
        let i = roll(&mut self.load_screen_rng, from.len());
        Some(from[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    fn screen(conditions: usize, main_menu: bool) -> LoadScreen {
        LoadScreen {
            id: FormId::NULL,
            text: String::new(),
            conditions: vec![Condition::default(); conditions],
            model: None,
            scale: 1.0,
            rotation: [0; 3],
            turn: (-180, 180),
            offset: Vec3::ZERO,
            camera: None,
            main_menu,
        }
    }

    #[test]
    fn a_passing_conditioned_screen_comes_first() {
        let screens = [screen(0, false), screen(1, false), screen(2, false)];
        let mut rng = 7;
        for _ in 0..20 {
            let c = choose(&screens, false, &[true, true, false], &mut rng);
            assert_eq!(c.first, Some(1));
            assert_eq!(c.pool, vec![0, 1]);
        }
    }

    #[test]
    fn with_none_passing_an_unconditioned_screen_is_drawn() {
        let screens = [screen(0, false), screen(1, false), screen(0, false)];
        let mut rng = 7;
        let mut seen = [false; 3];
        for _ in 0..50 {
            let c = choose(&screens, false, &[true, false, true], &mut rng);
            seen[c.first.unwrap()] = true;
            assert_eq!(c.pool, vec![0, 2]);
        }
        assert_eq!(seen, [true, false, true]);
    }

    #[test]
    fn none_eligible_gives_no_screen() {
        let screens = [screen(1, false)];
        let c = choose(&screens, false, &[false], &mut 7);
        assert_eq!(c, Choice::default());
        assert_eq!(choose(&[], false, &[], &mut 7), Choice::default());
    }

    #[test]
    fn the_main_menu_takes_its_flagged_screens_or_else_unconditioned_ones() {
        let screens = [screen(0, false), screen(1, true), screen(1, false)];
        let c = choose(&screens, true, &[false; 3], &mut 7);
        assert_eq!((c.first, c.pool), (Some(1), vec![1]));
        let screens = [screen(0, false), screen(1, false)];
        let c = choose(&screens, true, &[true, true], &mut 7);
        assert_eq!((c.first, c.pool), (Some(0), vec![0]));
    }

    /// `func(p1) <op> value`, run on the subject.
    fn ctda(func: u16, p1: u32, op: u8, value: f32) -> [u8; 32] {
        let mut d = [0; 32];
        d[0] = op << 5;
        d[4..8].copy_from_slice(&value.to_le_bytes());
        d[8..10].copy_from_slice(&func.to_le_bytes());
        d[12..16].copy_from_slice(&p1.to_le_bytes());
        d
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn screens_are_chosen_by_their_conditions_without_touching_the_saved_streams() {
        const GLOBAL: u32 = 0x900;
        const GET_RANDOM_PERCENT: u16 = 77;
        const GET_GLOBAL_VALUE: u16 = 74;
        let lscr = |id: u32, text: &str, conditions: &[[u8; 32]]| {
            conditions.iter().fold(
                RecordBuilder::new(b"LSCR", FormId(id))
                    .zstring(b"EDID", text)
                    .zstring(b"DESC", text),
                |r, c| r.field(b"CTDA", c),
            )
        };
        let plugin = write_plugin(
            0,
            &[],
            "test",
            "loading screens",
            0x910,
            &[
                Node::top(
                    b"GLOB",
                    vec![
                        RecordBuilder::new(b"GLOB", FormId(GLOBAL))
                            .zstring(b"EDID", "TestGlobal")
                            .field(b"FNAM", b"f")
                            .field(b"FLTV", &0f32.to_le_bytes()),
                    ],
                ),
                Node::top(
                    b"LSCR",
                    vec![
                        lscr(0x901, "Plain", &[]),
                        // GetRandomPercent < 0: never.
                        lscr(0x902, "Never", &[ctda(GET_RANDOM_PERCENT, 0, 4, 0.0)]),
                        lscr(0x903, "Global", &[ctda(GET_GLOBAL_VALUE, GLOBAL, 0, 1.0)]),
                        // GetRandomPercent >= 0: always.
                        lscr(0x904, "Always", &[ctda(GET_RANDOM_PERCENT, 0, 3, 0.0)]),
                    ],
                ),
            ],
        );
        let dir = std::env::temp_dir().join(format!(
            "vrm-load-screens-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Screens.esp"), plugin).unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Screens.esp");
        e.no_game = false;
        assert_eq!(e.load_screens().len(), 4);
        let text = |e: &Engine, c: &Choice| e.load_screens()[c.first.unwrap()].text.clone();
        let streams = (e.rng, e.cond_rng.get());
        let c = e.choose_load_screens();
        assert_eq!(text(&e, &c), "Always");
        assert_eq!((e.rng, e.cond_rng.get()), streams, "the saved streams");
        // The global now passes too: either conditioned screen may come.
        let global = e.lo.find_editor_id("TestGlobal").unwrap();
        e.scripts.globals.insert(global, 1.0);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..40 {
            let c = e.choose_load_screens();
            seen.insert(text(&e, &c));
            assert_eq!(c.pool.len(), 3, "Plain, Global, Always");
        }
        assert_eq!(seen, ["Always".to_string(), "Global".into()].into());
        // From the main menu, no conditions are run: the plain screen.
        e.no_game = true;
        let c = e.choose_load_screens();
        assert_eq!(text(&e, &c), "Plain");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
