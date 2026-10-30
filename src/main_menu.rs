//! The main menu's side in the engine: the state with no game running and
//! the save Continue loads. The menu itself is `swf_ui::main_menu`; the choices
//! made without a source are in `known_ambiguities/main-menu.md`.

use crate::engine::Engine;
use crate::items::Menu;
use crate::save::SaveListing;

impl Engine {
    /// The main menu up, with no game behind it.
    pub fn enter_main_menu(&mut self) {
        self.no_game = true;
        self.menu = Some(Menu::Main);
    }

    /// The save Continue loads: the newest one that can be loaded.
    pub fn continue_target(&self) -> Option<SaveListing> {
        self.save_list()
            .into_iter()
            .find(|s| !s.corrupt && !s.obsolete)
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
        assert_eq!(engine.menu, Some(Menu::Main));
        assert!(engine.no_game);
        assert_eq!(engine.messages.boxes.len(), 1);
    }
}
