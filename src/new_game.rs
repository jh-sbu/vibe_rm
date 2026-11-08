//! A new game, from the main menu's New: one load behind the loading screen
//! that starts the game's quests and puts the player wherever the opening
//! sends them. The start-game-enabled quests start, and with them the
//! opening, `MQ101`, which no flag or script starts. Their startup stages
//! run before the world loads; the opening's (stage 0, then 10) moves the
//! player to its start marker, and the load enters that marker's space.
//! Without an opening, or with one that moves no one, the player starts
//! where the developer launch puts them. The choices are in
//! `known_ambiguities/new-game.md`.

use anyhow::{Context, Result};
use esp::FormId;
use glam::Vec3;

use crate::engine::Engine;
use crate::loading::{LoadCause, LoadRequest, LoadTarget};

/// The opening quest, `MQ101` ("Unbound"), in `Skyrim.esm`.
const OPENING_QUEST: u32 = 0x0003_372B;

/// Where a new game puts the player when nothing moves them: the developer
/// launch's start, the middle of Tamriel's cell (4, -12) by Riverwood, on
/// the ground.
const FALLBACK_WORLD: &str = "Tamriel";
const FALLBACK_GRID: (i32, i32) = (4, -12);

impl Engine {
    /// Start a new game: the load runs from the next frame.
    pub fn request_new_game(&mut self) {
        self.request_load(LoadRequest::new(LoadTarget::NewGame, LoadCause::NewGame));
    }

    /// The opening quest, when `Skyrim.esm` is loaded and has it.
    pub(crate) fn opening_quest(&self) -> Option<FormId> {
        let id = FormId(OPENING_QUEST);
        let (plugin, _) = self.lo.origin(id)?;
        (plugin.eq_ignore_ascii_case("Skyrim.esm") && self.lo.tag_of(id)?.0 == *b"QUST")
            .then_some(id)
    }

    /// The new game's load: the quests start, their startup stages run, and
    /// the player enters the space the scripts moved them to.
    pub(crate) fn begin_new_game(&mut self) -> Result<()> {
        self.no_game = false;
        self.start_game_enabled_quests();
        if let Some(q) = self.opening_quest() {
            log::info!("starting the opening, {q}");
            self.start_quest(q);
            self.init_started_quests();
        }
        // The startup stages, before any world is loaded. A script's move of
        // the player becomes the load target.
        self.update_scripts(0.0);
        let target = match self.pending_load.take() {
            Some(LoadRequest {
                target: target @ (LoadTarget::Interior { .. } | LoadTarget::Exterior { .. }),
                ..
            }) => target,
            other => {
                if let Some(request) = other {
                    log::warn!("new game: {request:?} isn't a place to start");
                }
                self.fallback_start()?
            }
        };
        log::info!("new game starts at {target:?}");
        self.enter(&target)
    }

    fn fallback_start(&self) -> Result<LoadTarget> {
        let world = self
            .resolve_form(FALLBACK_WORLD)
            .with_context(|| format!("no worldspace {FALLBACK_WORLD} to start in"))?;
        let (x, y) = FALLBACK_GRID;
        let size = crate::world::terrain::CELL_SIZE;
        Ok(LoadTarget::Exterior {
            world,
            feet: Vec3::new((x as f32 + 0.5) * size, (y as f32 + 0.5) * size, -100_000.0),
            yaw: 0.0,
        })
    }

    /// A new game that couldn't start: its quests already ran, so the
    /// session ends for the main menu, which says so.
    pub(crate) fn new_game_failed(&mut self, error: &anyhow::Error) {
        log::warn!("new game failed: {error:#}");
        self.main_menu_notice = Some("The new game couldn't start.".into());
        self.quit_to_main_menu = true;
        self.imagespace.set_sleep_fade_black();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Location;
    use esp::group_type as g;
    use esp::writer::{Node, RecordBuilder as R, write_plugin};

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const GAME_HOUR: FormId = FormId(0x800);
    const OTHER_GLOBAL: FormId = FormId(0x801);
    const TAMRIEL: FormId = FormId(0x900);
    const QUEST: FormId = FormId(0x910);

    /// A plugin with the clock's `GameHour`, another global, a worldspace
    /// called Tamriel and a start-game-enabled quest; no opening.
    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vrm-new-game-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(dir.join("saves")).unwrap();
        let global = |id, name| {
            R::new(b"GLOB", id)
                .zstring(b"EDID", name)
                .field(b"FNAM", b"f")
                .f32(b"FLTV", 3.0)
        };
        let nodes = vec![
            Node::top(
                b"GLOB",
                vec![global(GAME_HOUR, "GameHour"), global(OTHER_GLOBAL, "Other")],
            ),
            Node::top(
                b"QUST",
                vec![
                    R::new(b"QUST", QUEST)
                        .zstring(b"EDID", "StartsWithTheGame")
                        .field(b"DNAM", &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                ],
            ),
            Node::Group {
                label: *b"WRLD",
                group_type: g::TOP,
                children: vec![
                    Node::Record(
                        R::new(b"WRLD", TAMRIEL)
                            .zstring(b"EDID", FALLBACK_WORLD)
                            .field(b"DATA", &[0]),
                    ),
                    Node::Group {
                        label: TAMRIEL.0.to_le_bytes(),
                        group_type: g::WORLD_CHILDREN,
                        children: vec![Node::Record(
                            R::new(b"CELL", FormId(0x920))
                                .flags(0x400)
                                .field(b"DATA", &[0, 0]),
                        )],
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("NewGame.esp"),
            write_plugin(0, &[], "test", "new game", 0xA00, &nodes),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&dir, "NewGame.esp");
        (Fixture(dir), engine)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn setting_game_hour_sets_the_clock() {
        let (_fixture, mut e) = fixture();
        e.day = 3;
        e.hour = 12.0;
        e.set_global_value(GAME_HOUR, 7.0);
        assert_eq!((e.day, e.hour), (3, 7.0));
        assert_eq!(e.global_value(GAME_HOUR), 7.0);
        // The clock goes on: the global follows it.
        e.hour = 8.5;
        assert_eq!(e.global_value(GAME_HOUR), 8.5);
        // Past midnight, into the next day; before it, into the last.
        e.set_global_value(GAME_HOUR, 26.0);
        assert_eq!((e.day, e.hour), (4, 2.0));
        e.set_global_value(GAME_HOUR, -1.0);
        assert_eq!((e.day, e.hour), (3, 23.0));
        // A value saved for the clock's global doesn't shadow it.
        e.scripts.globals.insert(GAME_HOUR, 5.0);
        assert_eq!(e.global_value(GAME_HOUR), 23.0);
        // Other globals keep the value set.
        e.set_global_value(OTHER_GLOBAL, 9.0);
        assert_eq!(e.global_value(OTHER_GLOBAL), 9.0);
        assert_eq!((e.day, e.hour), (3, 23.0));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_new_game_without_an_opening_starts_its_quests_at_the_fallback() {
        let (_fixture, mut e) = fixture();
        e.enter_main_menu();
        assert_eq!(e.opening_quest(), None, "no Skyrim.esm");
        e.menu = None;
        e.request_new_game();
        e.complete_load();
        assert!(!e.no_game);
        assert!(!e.quit_to_main_menu);
        assert!(e.scripts.quests.get(&QUEST).is_some_and(|q| q.running));
        assert_eq!(
            e.location,
            Location::Exterior {
                world: TAMRIEL,
                center: FALLBACK_GRID
            }
        );
    }
}
