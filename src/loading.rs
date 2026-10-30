//! Loads behind a loading screen: every cell change the player makes through
//! a load door, fast travel, an arrest or a script move, and every native
//! save load, is one [`LoadRequest`]. Callers check what they refuse first
//! and then queue the request; the next frame starts the loading state
//! ([`Loading`]): the view fades to black, the loading screen comes up, the
//! load runs (synchronously, the screen frozen meanwhile), the screen stays
//! up for a minimum time, goes, and the world fades back in. The game is
//! paused throughout, as the Loading Menu pauses it (CommonLibSSE). The
//! movie's timings are the asset's (`research/ui/loading-screen.md`); the
//! fades and the minimum time are choices (`known_ambiguities/loading-screen.md`).

use anyhow::Result;
use esp::FormId;
use glam::Vec3;

use crate::engine::Engine;

/// The fade to black before the screen and the fade back after it.
pub const FADE_SECONDS: f32 = crate::imagespace::SLEEP_FADE_SECONDS;
/// `loadingmenu.swf`'s `fadeIn`: 19 frames at 30 fps.
pub const SCREEN_IN_SECONDS: f32 = 19.0 / 30.0;
/// `loadingmenu.swf`'s `fadeOut`: 10 frames at 30 fps.
pub const SCREEN_OUT_SECONDS: f32 = 10.0 / 30.0;
/// The least time the screen is up, from its fade in to its fade out.
pub const MIN_SCREEN_SECONDS: f32 = 2.0;

/// Where the loading state is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The view fades to black.
    FadeOut,
    /// The screen fades in over black.
    ScreenIn,
    /// The screen is fully up: the load runs on the next update.
    Load,
    /// Loaded; the screen stays up its minimum time.
    Hold,
    /// The screen fades out, then the state ends and the world fades in.
    ScreenOut,
}

/// A load in progress.
#[derive(Debug, Clone)]
pub struct Loading {
    pub request: LoadRequest,
    pub phase: Phase,
    /// Seconds in this phase.
    pub t: f32,
    /// Seconds the screen has been up (from its fade in).
    pub shown: f32,
    /// The loading screens chosen as the load started.
    pub screens: crate::load_screens::Choice,
    /// The screen whose text was given last, and whether any was.
    pub last_text: Option<usize>,
    pub texts_given: u32,
}

/// Where a load takes the player.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadTarget {
    /// An interior cell, at a position and heading (none: the cell's own
    /// entry point).
    Interior {
        cell: FormId,
        spawn: Option<(Vec3, f32)>,
    },
    /// A worldspace, the player's feet at `feet`, facing `yaw`.
    Exterior { world: FormId, feet: Vec3, yaw: f32 },
    /// A save game, by name.
    Save(String),
}

/// What asked for a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadCause {
    Door,
    FastTravel,
    Script,
    Arrest,
    SaveLoad,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadRequest {
    pub target: LoadTarget,
    pub cause: LoadCause,
}

impl LoadRequest {
    pub fn new(target: LoadTarget, cause: LoadCause) -> Self {
        LoadRequest { target, cause }
    }

    pub fn save(name: String) -> Self {
        LoadRequest::new(LoadTarget::Save(name), LoadCause::SaveLoad)
    }
}

impl Engine {
    /// Queue a load for the next frame. A load already queued or under way
    /// stays: the first request wins.
    pub fn request_load(&mut self, request: LoadRequest) {
        if let Some(busy) = self
            .pending_load
            .as_ref()
            .or(self.loading.as_ref().map(|l| &l.request))
        {
            log::warn!("load {request:?} dropped: {busy:?} comes first");
            return;
        }
        self.pending_load = Some(request);
    }

    /// Start the queued load's loading state, if any. Menus close; from the
    /// main menu (no game, its movie faded out already) the screen comes up
    /// at once.
    pub fn apply_pending_load(&mut self) {
        if self.loading.is_some() {
            return;
        }
        let Some(request) = self.pending_load.take() else {
            return;
        };
        log::info!("loading: {request:?}");
        let screens = self.choose_load_screens();
        if let Some(i) = screens.first {
            log::info!("loading screen {}", self.load_screens()[i].id);
        }
        self.menu = None;
        let phase = if self.no_game {
            Phase::ScreenIn
        } else {
            self.imagespace.start_sleep_fade_out();
            Phase::FadeOut
        };
        self.loading = Some(Loading {
            request,
            phase,
            t: 0.0,
            shown: 0.0,
            screens,
            last_text: None,
            texts_given: 0,
        });
    }

    /// Whether the loading menu shows the player's level: not from the main
    /// menu, with no player yet.
    pub fn loading_shows_level(&self) -> bool {
        !self.no_game
    }

    /// The loading menu's next hint text (none: a blank): the chosen
    /// screen's first, then others from its pool.
    pub fn next_loading_text(&mut self) -> Option<String> {
        let mut l = self.loading.take()?;
        let next = if l.texts_given == 0 {
            l.screens.first
        } else {
            self.later_load_screen(&l.screens.pool, l.last_text)
        };
        l.texts_given += 1;
        l.last_text = next;
        self.loading = Some(l);
        let text = next.map(|i| self.load_screens()[i].text.clone())?;
        Some(self.replace_text_tags_preserving_controls(&text, None))
    }

    /// Whether the loading state is up (the game paused behind it).
    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// The loading state's frame: the fades run on and the phases follow
    /// each other; the world, its scripts and its clocks stand still.
    pub(crate) fn update_loading(&mut self, dt: f32) {
        self.update_sleep_imagespace(dt);
        if let Some(a) = self.audio.as_mut() {
            a.update();
        }
        self.update_audio_director();
        self.update_music();
        let Some(mut l) = self.loading.take() else {
            return;
        };
        l.t += dt;
        if l.phase != Phase::FadeOut {
            l.shown += dt;
        }
        let next = match l.phase {
            Phase::FadeOut if self.imagespace.sleep_fade_is_black() => Some(Phase::ScreenIn),
            Phase::ScreenIn if l.t >= SCREEN_IN_SECONDS => Some(Phase::Load),
            Phase::Load => {
                self.run_load(l.request.clone());
                // A failed load from the main menu has its menu back; one
                // that failed half way ends the session for it.
                if self.menu.is_some() || self.quit_to_main_menu {
                    return;
                }
                Some(Phase::Hold)
            }
            Phase::Hold if l.shown >= MIN_SCREEN_SECONDS => Some(Phase::ScreenOut),
            Phase::ScreenOut if l.t >= SCREEN_OUT_SECONDS => {
                // The world fades back in, running again.
                self.imagespace.set_sleep_fade_black();
                self.imagespace.start_sleep_fade_in();
                self.scene.post = self.imagespace.sleep_post();
                return;
            }
            _ => None,
        };
        if let Some(phase) = next {
            l.phase = phase;
            l.t = 0.0;
        }
        self.loading = Some(l);
    }

    /// Run a queued load through its whole loading state at once (tests).
    #[cfg(test)]
    pub fn complete_load(&mut self) {
        self.apply_pending_load();
        for _ in 0..100 {
            if self.loading.is_none() {
                return;
            }
            self.update_loading(1.0);
        }
        panic!("the loading state didn't end");
    }

    /// The load itself, and what each cause does when it fails.
    fn run_load(&mut self, request: LoadRequest) {
        let result = match &request.target {
            LoadTarget::Save(name) => {
                self.finish_save_load(name.clone());
                return;
            }
            target => self.enter(target),
        };
        if let Err(error) = result {
            log::warn!("{:?} load failed: {error:#}", request.cause);
            if request.cause == LoadCause::FastTravel {
                let text = crate::fast_travel::Refusal::InvalidDestination.message(self);
                self.scripts.notify(text);
            }
        }
    }

    /// Enter a cell target (not a save).
    pub(crate) fn enter(&mut self, target: &LoadTarget) -> Result<()> {
        match *target {
            LoadTarget::Interior { cell, spawn } => self.enter_interior(cell, spawn),
            LoadTarget::Exterior { world, feet, yaw } => self.enter_exterior(world, feet, yaw),
            LoadTarget::Save(_) => unreachable!("saves load through finish_save_load"),
        }
    }

    /// The target a door's destination (`XTEL`) leads to.
    pub fn door_target(&self, dest_door: FormId, pos: Vec3, yaw: f32) -> Result<LoadTarget> {
        let cell = anyhow::Context::context(
            self.lo.cell_of_ref(dest_door),
            "destination door has no cell",
        )?;
        let idx = self.lo.cell(cell).cloned().unwrap_or_default();
        Ok(match idx.world {
            Some(world) => LoadTarget::Exterior {
                world,
                feet: pos,
                yaw,
            },
            None => LoadTarget::Interior {
                cell,
                spawn: Some((pos, yaw)),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::write_plugin;

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vrm-loading-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(dir.join("saves")).unwrap();
        std::fs::write(
            dir.join("Loading.esp"),
            write_plugin(0, &[], "test", "loading state", 0x900, &[]),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&dir, "Loading.esp");
        (Fixture(dir), engine)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_load_fades_out_holds_its_screen_and_fades_back_with_the_game_paused() {
        let (_fixture, mut e) = fixture();
        e.no_game = false;
        e.menu = Some(crate::items::Menu::Journal);
        let hour = e.hour;
        let real = e.scripts.real_time;
        e.pending_save_load = Some("Missing".into());
        e.apply_pending_save_load();
        assert_eq!(e.menu, None, "the journal closes");
        assert!(e.is_loading() && e.menu_up());
        let mut phases = vec![Phase::FadeOut];
        let mut seconds = 0.0;
        while e.is_loading() {
            e.update(Default::default(), 0.05, 20.0);
            seconds += 0.05;
            let phase = e.loading.as_ref().map(|l| l.phase);
            if let Some(p) = phase
                && phases.last() != Some(&p)
            {
                phases.push(p);
            }
            assert!(seconds < 10.0, "stuck in {phase:?}");
        }
        assert_eq!(
            phases,
            [
                Phase::FadeOut,
                Phase::ScreenIn,
                Phase::Load,
                Phase::Hold,
                Phase::ScreenOut
            ]
        );
        // The fade out, the minimum time up and the screen's fade out.
        let least = FADE_SECONDS + MIN_SCREEN_SECONDS + SCREEN_OUT_SECONDS;
        assert!(seconds >= least - 0.1, "{seconds} s");
        assert_eq!(e.hour, hour);
        assert_eq!(e.scripts.real_time, real);
        // The failure is reported, and the world fades back in.
        assert_eq!(e.messages.boxes.len(), 1);
        assert!(e.imagespace.sleep_fade_amount() > 0.9);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn the_first_load_requested_wins() {
        let (_fixture, mut e) = fixture();
        e.request_load(LoadRequest::save("First".into()));
        e.request_load(LoadRequest::save("Second".into()));
        e.apply_pending_load();
        e.request_load(LoadRequest::save("Third".into()));
        assert_eq!(
            e.loading.as_ref().unwrap().request.target,
            LoadTarget::Save("First".into())
        );
        assert!(e.pending_load.is_none());
    }

    /// A save standing in a cell this load order hasn't, in `dir`'s saves.
    fn write_save_in_missing_cell(e: &Engine, name: &str) {
        let mut snapshot = crate::save::state::Snapshot::capture(e);
        snapshot.spot = Some(crate::save::state::Spot {
            location: crate::engine::Location::Interior(FormId(0x00AB_CDEF)),
            feet: Vec3::ZERO,
            yaw: 0.0,
        });
        let path = e.save_directory.join(format!("{name}.vrs"));
        std::fs::write(path, snapshot.write().to_bytes()).unwrap();
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_save_that_cant_load_leaves_the_game_as_it_was() {
        let (fixture, mut e) = fixture();
        e.save_directory = fixture.0.join("saves");
        e.no_game = false;
        e.day = 12;
        write_save_in_missing_cell(&e, "Broken");
        e.day = 30;
        e.pending_save_load = Some("Broken".into());
        e.apply_pending_save_load();
        e.complete_load();
        assert_eq!(e.day, 30, "nothing of the save was applied");
        assert!(!e.no_game && !e.quit_to_main_menu);
        assert_eq!(e.messages.boxes.len(), 1);
        assert!(e.messages.boxes[0].text.contains("Returning to game"));

        // From the main menu, the menu comes back with the menu's message.
        e.messages.boxes.clear();
        e.enter_main_menu();
        e.pending_save_load = Some("Broken".into());
        e.apply_pending_save_load();
        e.complete_load();
        assert_eq!(e.menu, Some(crate::items::Menu::Main));
        assert!(e.messages.boxes[0].text.contains("cannot be loaded"));
    }
}
