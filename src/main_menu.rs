//! The main menu's side in the engine: the state with no game running, its
//! title track, the save Continue loads, and Quit to Main Menu: a fade to
//! black, then a teardown that builds a fresh engine from the old one's
//! plugins, renderer, audio device, settings and control bindings. The menu itself is `swf_ui::main_menu`; the choices
//! made without a source are in `known_ambiguities/main-menu.md`.

use crate::engine::Engine;
use crate::items::Menu;
use crate::save::SaveListing;

/// What a new session is built with, besides the old engine's parts: the
/// launch's time of day, forced weather and load radius.
pub struct SessionOptions {
    pub hour: f32,
    pub weather: Option<String>,
    pub radius: i32,
}

/// The title track the main menu plays (UESP Skyrim:Music: "Dragonborn").
const MAIN_MENU_MUSIC: &str = "music/special/mus_maintheme.xwm";

impl Engine {
    /// The main menu up, with no game behind it, and its title track.
    pub fn enter_main_menu(&mut self) {
        self.no_game = true;
        self.menu = Some(Menu::Main);
        self.start_main_menu_music();
    }

    /// The title track ("Dragonborn", `mus_maintheme` per UESP), looping in
    /// the music category, unless it is already playing.
    pub(crate) fn start_main_menu_music(&mut self) {
        let playing = |e: &Engine, v| e.audio.as_ref().is_some_and(|a| a.is_playing(v));
        if self.main_menu_music.is_some_and(|v| playing(self, v)) {
            return;
        }
        let mut params = crate::world::sound::default_params(
            &self.lo,
            &self.audio_defaults,
            b"MDSC",
            b"----",
            None,
        );
        params.looping = true;
        self.main_menu_music = self
            .audio
            .as_mut()
            .and_then(|a| a.play(&self.vfs, MAIN_MENU_MUSIC, params));
    }

    /// Quit to Main Menu, confirmed in the journal: the journal closes and
    /// the screen fades to black before the session ends.
    pub fn request_quit_to_main_menu(&mut self) {
        self.menu = None;
        self.quit_to_main_menu = true;
        self.imagespace.start_sleep_fade_out();
    }

    /// Whether the fade before Quit to Main Menu's teardown is done.
    pub fn main_menu_ready(&self) -> bool {
        self.quit_to_main_menu && self.imagespace.sleep_fade_is_black()
    }

    /// The save Continue loads: the newest one that can be loaded.
    pub fn continue_target(&self) -> Option<SaveListing> {
        self.save_list()
            .into_iter()
            .find(|s| !s.corrupt && !s.obsolete)
    }

    /// Quit to Main Menu: the session's pending saves are written, then a new
    /// engine replaces this one, at the main menu. The plugins (their runtime
    /// records cleared), the renderer, the audio device (silenced), the
    /// settings, the control bindings and the save folder carry over;
    /// everything of the game itself starts over.
    pub fn into_main_menu(mut self, options: SessionOptions) -> Engine {
        self.flush_save_requests();
        let notice = self.main_menu_notice.take();
        self.discard_conversation_for_load();
        self.unload_all();
        let Engine {
            mut lo,
            vfs,
            renderer,
            mut audio,
            input_map,
            settings,
            save_directory,
            ..
        } = self;
        lo.clear_runtime_records();
        if let Some(audio) = audio.as_mut() {
            audio.stop_all();
        }
        let mut engine = Engine::new(
            lo,
            vfs,
            renderer,
            options.hour,
            options.weather,
            options.radius,
        );
        engine.audio = audio;
        engine.input_map = input_map;
        engine.settings = settings;
        engine.save_directory = save_directory;
        engine.refresh_audio_settings();
        engine.sync_settings();
        engine.enter_main_menu();
        if let Some(text) = notice {
            engine.debug_message_box(text);
        }
        engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::FormId;
    use esp::writer::{RecordBuilder, write_plugin};

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
        let fixture = Fixture(
            std::env::temp_dir().join(format!("vrm-main-menu-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir_all(fixture.0.join("saves")).unwrap();
        std::fs::write(
            fixture.0.join("Menu.esp"),
            write_plugin(0, &[], "test", "main menu sessions", 0x900, &[]),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&fixture.0, "Menu.esp");
        (fixture, engine)
    }

    fn options() -> SessionOptions {
        SessionOptions {
            hour: 12.0,
            weather: None,
            radius: 0,
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn quitting_to_the_main_menu_starts_the_game_over() {
        let (fixture, mut engine) = fixture();
        let input = engine.input_map.clone();
        for cycle in 0..3 {
            // A game in progress, as far as the engine's state goes.
            engine.no_game = false;
            engine.day = 40;
            engine.scripts.real_time = 1000.0;
            engine.menu = Some(Menu::Journal);
            engine.debug_message_box("left up".into());
            let potion = RecordBuilder::new(b"ALCH", FormId(0xFF00_0800)).to_bytes();
            engine.lo.insert_runtime_record(&potion).unwrap();
            engine.settings.set("bInvertYValues", 1.0).unwrap();
            engine.request_quit_to_main_menu();
            assert!(!engine.main_menu_ready(), "the fade comes first");
            engine.update_sleep_imagespace(crate::imagespace::SLEEP_FADE_SECONDS);
            assert!(engine.main_menu_ready());

            engine = engine.into_main_menu(options());
            assert_eq!(engine.menu, Some(Menu::Main), "cycle {cycle}");
            assert!(engine.no_game);
            assert!(!engine.quit_to_main_menu);
            assert_ne!(engine.day, 40);
            assert_eq!(engine.scripts.real_time, 0.0);
            assert!(engine.messages.boxes.is_empty());
            assert!(!engine.imagespace.sleep_fade_is_black());
            assert!(engine.lo.get(FormId(0xFF00_0800)).is_none());
            // Kept: the player's settings, bindings and save folder.
            assert!(engine.settings.enabled("bInvertYValues"));
            assert!(std::sync::Arc::ptr_eq(&engine.input_map, &input));
            assert_eq!(engine.save_directory, fixture.0.join("saves"));
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn continue_loads_the_newest_save_that_can_be_loaded() {
        let (fixture, mut engine) = fixture();
        assert!(engine.continue_target().is_none());
        engine.save_game("Older").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        engine.save_game("Newer").unwrap();
        assert_eq!(engine.continue_target().unwrap().name, "Newer");
        // A newer file that can't be read is passed over.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(fixture.0.join("saves/Broken.vrs"), b"not a save").unwrap();
        assert!(engine.save_list().iter().any(|s| s.corrupt));
        assert_eq!(engine.continue_target().unwrap().name, "Newer");
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_failed_load_from_the_main_menu_returns_to_it() {
        let (_fixture, mut engine) = fixture();
        engine.enter_main_menu();
        engine.menu = None;
        engine.pending_save_load = Some("Missing".into());
        engine.apply_pending_save_load();
        engine.complete_load();
        assert_eq!(engine.menu, Some(Menu::Main));
        assert!(engine.no_game);
        assert_eq!(engine.messages.boxes.len(), 1);
    }
}
