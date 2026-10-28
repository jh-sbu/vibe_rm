//! Device-free audio policy. The engine supplies state; the mixer follows targets.
use super::CategoryState;
use crate::world::sound::AudioDefaults;
use std::collections::HashMap;

// Skyrim_Default.ini [Audio]; see research/audio/defaults.md.
pub const MENU_FADE_OUT: f32 = 3.0;
pub const MENU_FADE_IN: f32 = 1.0;
pub const MUSIC_DUCK: f32 = 6.0;
pub const MUSIC_UNDUCK: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub menu: bool,
    pub loading: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub state: CategoryState,
    pub seconds: f32,
}
pub struct Director {
    defaults: AudioDefaults,
    pub inputs: Inputs,
    pub targets: HashMap<u32, Target>,
}
impl Director {
    pub fn new(defaults: AudioDefaults) -> Self {
        Self {
            defaults,
            inputs: Inputs::default(),
            targets: HashMap::new(),
        }
    }
    pub fn update(&mut self, inputs: Inputs, scripts: &HashMap<u32, CategoryState>) {
        self.inputs = inputs;
        self.targets.clear();
        for (&id, &state) in scripts {
            self.targets.insert(
                id,
                Target {
                    state,
                    seconds: 0.005,
                },
            );
        }
        self.pause(b"PIMC", inputs.menu, 0.0);
        self.pause(
            b"PDMC",
            inputs.menu,
            if inputs.menu {
                MENU_FADE_OUT
            } else {
                MENU_FADE_IN
            },
        );
        self.pause(b"PDLC", inputs.loading, 0.0);
    }
    fn pause(&mut self, key: &[u8; 4], paused: bool, seconds: f32) {
        if let Some(id) = self.defaults.get(key) {
            let t = self.targets.entry(id.0).or_insert(Target {
                state: CategoryState::default(),
                seconds,
            });
            t.state.paused |= paused;
            t.seconds = seconds;
        }
    }
}
impl crate::engine::Engine {
    pub(crate) fn update_audio_director(&mut self) {
        let input = Inputs {
            menu: self.in_menu_mode(),
            loading: false,
        };
        self.audio_director
            .update(input, &self.script_sounds.categories);
        if let Some(audio) = &self.audio {
            for (&id, target) in &self.audio_director.targets {
                audio.set_category(id, target.state, target.seconds);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use esp::FormId;
    fn director() -> Director {
        Director::new(AudioDefaults::from_entries([
            (*b"PIMC", FormId(1)),
            (*b"PDMC", FormId(2)),
            (*b"PDLC", FormId(3)),
        ]))
    }
    #[test]
    fn menu_targets_pause_immediately_or_after_the_ini_fade() {
        let mut d = director();
        d.update(
            Inputs {
                menu: true,
                ..Default::default()
            },
            &HashMap::new(),
        );
        assert!(d.targets[&1].state.paused);
        assert_eq!(d.targets[&1].seconds, 0.0);
        assert!(d.targets[&2].state.paused);
        assert_eq!(d.targets[&2].seconds, 3.0);
        assert!(!d.targets[&3].state.paused);
        d.update(Inputs::default(), &HashMap::new());
        assert!(!d.targets[&2].state.paused);
        assert_eq!(d.targets[&2].seconds, 1.0);
        d.update(
            Inputs {
                loading: true,
                ..Default::default()
            },
            &HashMap::new(),
        );
        assert!(d.targets[&3].state.paused);
    }
    #[test]
    fn scripts_compose_with_policy_instead_of_overwriting_it() {
        let mut d = director();
        d.update(
            Inputs {
                menu: true,
                ..Default::default()
            },
            &[(
                2,
                CategoryState {
                    frequency: 0.5,
                    volume: 0.4,
                    ..Default::default()
                },
            )]
            .into(),
        );
        assert!(d.targets[&2].state.paused);
        assert_eq!(d.targets[&2].state.volume, 0.4);
        assert_eq!(d.targets[&2].state.frequency, 0.5);
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn pausing_menu_reaches_director_without_an_audio_device() {
        let dir = std::env::temp_dir().join(format!("vrm-audio-menu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Audio.esp"),
            esp::writer::write_plugin(0, &[], "test", "empty", 0x800, &[]),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Audio.esp");
        e.menu = Some(crate::items::Menu::Inventory);
        e.update(crate::player::MoveInput::default(), 0.016, 1.0);
        assert!(e.audio.is_none());
        assert!(e.audio_director.inputs.menu);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
