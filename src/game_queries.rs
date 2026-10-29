//! Whole-game natives: finding references and actors near a point
//! (`Game.FindClosest...` / `FindRandom...`), the sun's direction and the
//! player looking at it, and the flags scripts set on the game itself
//! (`SetPlayerReportCrime`, `SetInChargen`, `ClearPrison`). Saved: `GAMF`.
//! Choices: `known_ambiguities/remaining-natives.md`.

use esp::FormId;
use glam::Vec3;

use crate::engine::{Engine, PLAYER_REF};

/// How far from the sun's direction the camera may point and still be
/// looking into it: the cosine of 12 degrees (no source).
const SUNGAZE_COS: f32 = 0.978;
/// How far to look for something between the camera and the sun.
const SUN_RAY: f32 = 20_000.0;

/// What `SetInChargen` turned off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Chargen {
    pub no_saving: bool,
    pub no_waiting: bool,
    pub controls_message: bool,
}

#[derive(Default)]
pub struct GameFlags {
    /// `SetPlayerReportCrime(false)`: the player's crimes go unreported.
    pub(crate) crime_unreported: bool,
    pub(crate) chargen: Option<Chargen>,
}

/// Which base forms a search wants.
pub enum Wanted<'a> {
    One(FormId),
    AnyOf(&'a [FormId]),
}

impl Engine {
    /// The loaded actors (the player too) and where they are, none disabled.
    fn actors_in_range(&self, centre: Vec3, radius: f32) -> Vec<(FormId, f32)> {
        let mut out = Vec::new();
        let player = self
            .ref_position(PLAYER_REF)
            .map(|p| (PLAYER_REF, p.distance(centre)));
        out.extend(player.filter(|&(_, d)| d <= radius));
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let d = a.exec.pos.distance(centre);
            if d <= radius && !self.is_disabled(a.ref_id) {
                out.push((a.ref_id, d));
            }
        }
        out
    }

    /// The loaded references of the wanted base objects within `radius`
    /// of a point, with their distances; none disabled.
    fn references_in_range(
        &self,
        wanted: &Wanted<'_>,
        centre: Vec3,
        radius: f32,
    ) -> Vec<(FormId, f32)> {
        let mut out = Vec::new();
        for &r in self.cells.values().flat_map(|rt| &rt.refs) {
            let Some(base) = self.base_of(r) else {
                continue;
            };
            let wanted = match wanted {
                Wanted::One(b) => *b == base,
                Wanted::AnyOf(list) => list.contains(&base),
            };
            if !wanted || self.is_disabled(r) {
                continue;
            }
            let Some(d) = self.ref_position(r).map(|p| p.distance(centre)) else {
                continue;
            };
            if d <= radius {
                out.push((r, d));
            }
        }
        out
    }

    pub fn find_closest_actor(&self, centre: Vec3, radius: f32) -> Option<FormId> {
        closest(self.actors_in_range(centre, radius))
    }

    pub fn find_random_actor(&mut self, centre: Vec3, radius: f32) -> Option<FormId> {
        let found = self.actors_in_range(centre, radius);
        self.pick_random(found)
    }

    pub fn find_closest_reference(
        &self,
        wanted: &Wanted<'_>,
        centre: Vec3,
        radius: f32,
    ) -> Option<FormId> {
        closest(self.references_in_range(wanted, centre, radius))
    }

    pub fn find_random_reference(
        &mut self,
        wanted: &Wanted<'_>,
        centre: Vec3,
        radius: f32,
    ) -> Option<FormId> {
        let found = self.references_in_range(wanted, centre, radius);
        self.pick_random(found)
    }

    fn pick_random(&mut self, mut found: Vec<(FormId, f32)>) -> Option<FormId> {
        if found.is_empty() {
            return None;
        }
        // The same set in the same order whatever the cells' hash order.
        found.sort_by_key(|&(r, _)| r);
        let i = (self.rand() % found.len() as u64) as usize;
        Some(found[i].0)
    }

    /// The direction from the world to the sun (unit; `GetSunPosition*`).
    pub fn sun_direction(&self) -> Vec3 {
        self.scene.env.sun_dir.normalize_or_zero()
    }

    /// `IsPlayerSungazing`: outdoors with the sun up, the camera pointed at
    /// it and nothing in the way.
    pub fn player_sungazing(&self) -> bool {
        if !matches!(self.location, crate::engine::Location::Exterior { .. }) {
            return false;
        }
        let sun = self.sun_direction();
        if sun.z <= 0.0 || self.camera.forward().dot(sun) < SUNGAZE_COS {
            return false;
        }
        self.physics
            .raycast_excluding(self.camera.position, sun, SUN_RAY, PLAYER_REF)
            .is_none()
    }

    /// `SetPlayerReportCrime`.
    pub fn set_player_report_crime(&mut self, report: bool) {
        self.game_flags.crime_unreported = !report;
    }

    /// `ClearPrison`: the player is out of prison as far as the game knows.
    /// Nothing else is undone (belongings stay in their chest).
    pub fn clear_prison(&mut self) {
        self.crime.arrests.jailed = None;
    }

    /// `SetInChargen`.
    pub fn set_in_chargen(&mut self, no_saving: bool, no_waiting: bool, controls_message: bool) {
        self.game_flags.chargen =
            (no_saving || no_waiting || controls_message).then_some(Chargen {
                no_saving,
                no_waiting,
                controls_message,
            });
    }

    pub fn saving_disabled(&self) -> bool {
        self.game_flags.chargen.is_some_and(|c| c.no_saving)
            || matches!(self.menu, Some(crate::items::Menu::Crafting { .. }))
    }

    pub fn waiting_disabled(&self) -> bool {
        self.game_flags.chargen.is_some_and(|c| c.no_waiting)
    }

    pub(crate) fn restore_game_flags(&mut self, crime_unreported: bool, chargen: Option<Chargen>) {
        self.game_flags.crime_unreported = crime_unreported;
        self.game_flags.chargen = chargen;
    }
}

fn closest(found: Vec<(FormId, f32)>) -> Option<FormId> {
    found
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, Value};

    const TREE: FormId = FormId(0x800);
    const ROCK: FormId = FormId(0x801);
    const LIST: FormId = FormId(0x802);
    const NEAR_TREE: FormId = FormId(0x810);
    const FAR_TREE: FormId = FormId(0x811);
    const ROCK_REF: FormId = FormId(0x812);
    const HIDDEN_TREE: FormId = FormId(0x813);

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn placed(id: FormId, base: FormId, x: f32) -> RecordBuilder {
        let mut data = Vec::new();
        for v in [x, 0.0, 0.0, 0.0, 0.0, 0.0] {
            data.extend(v.to_le_bytes());
        }
        RecordBuilder::new(b"REFR", id)
            .form(b"NAME", base)
            .field(b"DATA", &data)
    }

    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-game-queries-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let records = [
            Node::top(
                b"STAT",
                vec![
                    RecordBuilder::new(b"STAT", TREE),
                    RecordBuilder::new(b"STAT", ROCK),
                ],
            ),
            Node::top(
                b"FLST",
                vec![RecordBuilder::new(b"FLST", LIST).form(b"LNAM", ROCK)],
            ),
            Node::top(
                b"REFR",
                vec![
                    placed(NEAR_TREE, TREE, 100.0),
                    placed(FAR_TREE, TREE, 900.0),
                    placed(ROCK_REF, ROCK, 300.0),
                    placed(HIDDEN_TREE, TREE, 50.0),
                ],
            ),
        ];
        std::fs::write(
            f.0.join("GameQueries.esp"),
            write_plugin(0, &[], "test", "synthetic game queries", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&f.0, "GameQueries.esp");
        let rt = e
            .cells
            .entry(crate::render::CellKey::Interior(FormId(0x820)))
            .or_default();
        rt.refs.extend([NEAR_TREE, FAR_TREE, ROCK_REF, HIDDEN_TREE]);
        e.scripts.disabled.insert(HIDDEN_TREE, true);
        (f, e)
    }

    fn game(e: &mut Engine, func: &str, args: &[Value]) -> Value {
        match crate::script::natives::call(e, "game", func, None, args) {
            NativeResult::Value(v) => v,
            _ => panic!("Game.{func} didn't return"),
        }
    }

    fn floats(at: [f32; 3], radius: f32) -> Vec<Value> {
        let mut v: Vec<Value> = at.iter().map(|&f| Value::Float(f)).collect();
        v.push(Value::Float(radius));
        v
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn finders_search_loaded_enabled_references_in_a_sphere() {
        let (_f, mut e) = fixture();
        let tree = e.object_value(TREE);
        let with = |base: Value, tail: Vec<Value>| [vec![base], tail].concat();
        // The closest tree, not the disabled one nearer still.
        let closest = game(
            &mut e,
            "findclosestreferenceoftype",
            &with(tree.clone(), floats([0.0; 3], 2000.0)),
        );
        assert_eq!(closest, e.object_value(NEAR_TREE));
        // Radius limits it; from the far side the far tree is nearest.
        let far = game(
            &mut e,
            "findclosestreferenceoftype",
            &with(tree.clone(), floats([1000.0, 0.0, 0.0], 500.0)),
        );
        assert_eq!(far, e.object_value(FAR_TREE));
        assert_eq!(
            game(
                &mut e,
                "findclosestreferenceoftype",
                &with(tree.clone(), floats([0.0, 5000.0, 0.0], 100.0)),
            ),
            Value::None
        );
        // By a list of bases.
        let list = e.object_value(LIST);
        assert_eq!(
            game(
                &mut e,
                "findclosestreferenceofanytypeinlist",
                &with(list.clone(), floats([0.0; 3], 2000.0)),
            ),
            e.object_value(ROCK_REF)
        );
        // Random picks something eligible, and nothing when nothing is.
        for _ in 0..8 {
            let r = game(
                &mut e,
                "findrandomreferenceoftype",
                &with(tree.clone(), floats([0.0; 3], 2000.0)),
            );
            assert!(r == e.object_value(NEAR_TREE) || r == e.object_value(FAR_TREE));
        }
        assert_eq!(
            game(
                &mut e,
                "findrandomreferenceofanytypeinlist",
                &with(list, floats([0.0, 0.0, 9000.0], 100.0)),
            ),
            Value::None
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn actor_finders_include_the_player_and_loaded_actors() {
        let (_f, mut e) = fixture();
        let key = crate::render::CellKey::Interior(FormId(0x820));
        let mut a = crate::ai::ActorRuntime::new(
            FormId(0x900),
            FormId(2),
            std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
            glam::Mat4::from_translation(Vec3::new(500.0, 0.0, 0.0)),
            Vec::new(),
            0.0,
        );
        a.exec.health = 10.0;
        e.cells.entry(key).or_default().actors.push(a);
        e.player.position = Vec3::new(0.0, 0.0, 0.0);
        let player_at = e.ref_position(PLAYER_REF).unwrap();
        let near_player = floats([player_at.x, player_at.y, player_at.z], 50.0);
        let found = game(&mut e, "findclosestactor", &near_player);
        assert_eq!(found, e.object_value(PLAYER_REF));
        let found = game(&mut e, "findclosestactor", &floats([500.0, 0.0, 0.0], 10.0));
        assert_eq!(found, Value::None, "the actor has no record to name it by");
        assert_eq!(
            e.find_closest_actor(Vec3::new(500.0, 0.0, 0.0), 10.0),
            Some(FormId(0x900))
        );
        assert_eq!(
            e.find_random_actor(Vec3::new(500.0, 0.0, 0.0), 10.0),
            Some(FormId(0x900))
        );
        assert_eq!(
            e.find_closest_actor(Vec3::new(500.0, 900.0, 0.0), 10.0),
            None
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn the_sun_is_a_direction_and_gazing_needs_it_up_ahead_outdoors() {
        let (_f, mut e) = fixture();
        e.scene.env.sun_dir = Vec3::new(0.0, 3.0, 4.0);
        let x = game(&mut e, "getsunpositionx", &[]);
        let y = game(&mut e, "getsunpositiony", &[]);
        let z = game(&mut e, "getsunpositionz", &[]);
        assert_eq!(
            (x, y, z),
            (Value::Float(0.0), Value::Float(0.6), Value::Float(0.8))
        );
        // Indoors there is no sun to look at.
        e.camera.yaw = 0.0;
        e.camera.pitch = 0.9273; // atan(4 / 3)
        assert_eq!(game(&mut e, "isplayersungazing", &[]), Value::Bool(false));
        e.location = crate::engine::Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        };
        assert_eq!(game(&mut e, "isplayersungazing", &[]), Value::Bool(true));
        e.camera.yaw = 1.5;
        assert_eq!(game(&mut e, "isplayersungazing", &[]), Value::Bool(false));
        // Below the horizon it can't be seen.
        e.camera.yaw = 0.0;
        e.scene.env.sun_dir = Vec3::new(0.0, 0.6, -0.8);
        e.camera.pitch = -0.9273;
        assert_eq!(game(&mut e, "isplayersungazing", &[]), Value::Bool(false));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn game_flags_gate_crime_saving_and_waiting_and_prison_clears() {
        let (_f, mut e) = fixture();
        let faction = FormId(0x950);
        game(&mut e, "setplayerreportcrime", &[Value::Bool(false)]);
        assert!(!e.commit_crime(crate::crime::CrimeType::Trespass, None, Some(faction), 0));
        assert!(e.game_flags.crime_unreported);
        game(&mut e, "setplayerreportcrime", &[]);
        assert!(!e.game_flags.crime_unreported);
        game(
            &mut e,
            "setinchargen",
            &[Value::Bool(true), Value::Bool(true), Value::Bool(false)],
        );
        assert!(e.saving_disabled() && e.waiting_disabled());
        assert!(e.save_game("chargen-test").is_err());
        e.request_sleep_wait(None);
        assert!(e.menu.is_none());
        game(
            &mut e,
            "setinchargen",
            &[Value::Bool(false), Value::Bool(false), Value::Bool(false)],
        );
        assert!(!e.saving_disabled() && !e.waiting_disabled());
        assert!(e.game_flags.chargen.is_none());
        // ClearPrison forgets the sentence.
        e.crime.arrests.jailed = Some(crate::arrest::Sentence {
            faction,
            days: 3,
            inside: FormId(0x960),
            owed: Default::default(),
        });
        game(&mut e, "clearprison", &[]);
        assert!(e.crime.arrests.jailed.is_none());
        assert_eq!(
            game(&mut e, "calculatefavorcost", &[Value::Int(25)]),
            Value::Int(25)
        );
    }
}
