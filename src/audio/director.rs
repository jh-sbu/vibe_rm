//! Device-free audio policy. The engine supplies state; the mixer follows targets.
use super::CategoryState;
use crate::world::sound::AudioDefaults;
use std::collections::HashMap;

// Skyrim_Default.ini [Audio]; see research/audio/defaults.md.
pub const MENU_FADE_OUT: f32 = 3.0;
pub const MENU_FADE_IN: f32 = 1.0;
pub const MUSIC_DUCK: f32 = 6.0;
pub const MUSIC_UNDUCK: f32 = 8.0;

/// Incoming MUSC WNAM owns the fade. Abrupt types bypass it.
pub fn music_fade(flags: u32, seconds: f32) -> f32 {
    if flags & crate::music::ABRUPT_TRANSITION != 0 {
        0.0
    } else if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub menu: bool,
    pub loading: bool,
    pub dialogue: bool,
    pub stats: bool,
    pub submerged: bool,
    pub combat: bool,
    pub dead: bool,
    pub space: Option<esp::FormId>,
    pub reverb: Option<esp::FormId>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub state: CategoryState,
    pub seconds: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MusicCommand {
    Add(esp::FormId),
    Remove(esp::FormId),
}
pub struct Director {
    defaults: AudioDefaults,
    pub inputs: Inputs,
    pub targets: HashMap<u32, Target>,
    pub submerged_categories: Vec<u32>,
    pub loops: Vec<esp::FormId>,
    pub music_commands: Vec<MusicCommand>,
    pub reverb: Option<esp::FormId>,
}
impl Director {
    pub fn new(defaults: AudioDefaults) -> Self {
        Self {
            defaults,
            inputs: Inputs::default(),
            targets: HashMap::new(),
            submerged_categories: Vec::new(),
            loops: Vec::new(),
            music_commands: Vec::new(),
            reverb: None,
        }
    }
    pub fn update(&mut self, inputs: Inputs, scripts: &HashMap<u32, CategoryState>) {
        self.music_commands.clear();
        for (key, active, before) in [
            (b"SSSC", inputs.stats, self.inputs.stats),
            (
                b"BTMS",
                inputs.combat && !inputs.dead,
                self.inputs.combat && !self.inputs.dead,
            ),
            (b"DTMS", inputs.dead, self.inputs.dead),
        ] {
            if active != before
                && let Some(id) = self.defaults.get(key)
            {
                self.music_commands.push(if active {
                    MusicCommand::Add(id)
                } else {
                    MusicCommand::Remove(id)
                });
            }
        }
        self.reverb = inputs.reverb.or_else(|| self.defaults.get(b"RVBT"));
        self.inputs = inputs;
        self.loops.clear();
        if inputs.submerged
            && let Some(sound) = self.defaults.get(b"UWLS")
        {
            self.loops.push(sound);
        }
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
        self.gain(b"SMSC", if inputs.stats { 0.0 } else { 1.0 }, 0.005);
        self.gain(
            b"SFDC",
            if inputs.dialogue {
                10f32.powf(-6.0 / 20.0)
            } else {
                1.0
            },
            if inputs.dialogue {
                MENU_FADE_OUT
            } else {
                MENU_FADE_IN
            },
        );
        for &id in &self.submerged_categories {
            let t = self.targets.entry(id).or_insert(Target {
                state: CategoryState::default(),
                seconds: 0.15,
            });
            if inputs.submerged {
                t.state.volume = 0.0;
            }
            t.seconds = 0.15;
        }
    }

    fn gain(&mut self, key: &[u8; 4], gain: f32, seconds: f32) {
        if let Some(id) = self.defaults.get(key) {
            let t = self.targets.entry(id.0).or_insert(Target {
                state: CategoryState::default(),
                seconds,
            });
            t.state.volume *= gain;
            t.seconds = seconds;
        }
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
        let space = self.acoustic_space();
        let reverb = space
            .and_then(|id| self.audio_spaces.records.get(&id))
            .and_then(|s| s.reverb);
        let input = Inputs {
            space,
            reverb,
            menu: self.in_menu_mode(),
            loading: false,
            dialogue: self.conversation.is_some(),
            stats: self.menu == Some(crate::items::Menu::Skills),
            combat: self.cells.values().flat_map(|rt| &rt.actors).any(|a| {
                !a.dead
                    && a.exec
                        .combat
                        .as_ref()
                        .is_some_and(|c| c.target == crate::engine::PLAYER_REF)
            }),
            dead: self.player_dead(),
            submerged: self
                .water_level(self.camera.position)
                .is_some_and(|h| self.camera.position.z < h),
        };
        self.audio_director
            .update(input, &self.script_sounds.categories);
        for command in &self.audio_director.music_commands {
            match *command {
                MusicCommand::Add(id) => {
                    self.music.queue.add(id, false);
                }
                MusicCommand::Remove(id) => self.music.queue.remove_engine(id),
            }
        }

        let wanted = self.audio_director.loops.clone();
        self.ambient_voices.retain(|sound, voice| {
            if wanted.contains(sound) {
                return self.audio.as_ref().is_some_and(|a| a.is_playing(*voice));
            }
            if let Some(a) = &self.audio {
                a.stop_with_fade(*voice, 0.15);
            }
            false
        });
        for sound in wanted {
            if self.ambient_voices.contains_key(&sound) {
                continue;
            }
            let Some(desc) = self.sound_desc(sound) else {
                continue;
            };
            let variation = self.roll_variation(&desc);
            let mut params = desc.params(variation, None, true);
            params.position = None;
            params.fade_in = 0.15;
            if let Some(a) = &mut self.audio
                && let Some(id) = a.play(&self.vfs, &desc.files[0], params)
            {
                self.ambient_voices.insert(sound, id);
            }
        }

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
    fn combat_and_death_default_music_commands_are_edges() {
        let mut d = Director::new(AudioDefaults::from_entries([
            (*b"BTMS", FormId(10)),
            (*b"DTMS", FormId(11)),
        ]));
        let input = Inputs {
            combat: true,
            ..Default::default()
        };
        d.update(input, &HashMap::new());
        assert_eq!(d.music_commands, [MusicCommand::Add(FormId(10))]);
        d.update(input, &HashMap::new());
        assert!(d.music_commands.is_empty());
        d.update(
            Inputs {
                dead: true,
                ..input
            },
            &HashMap::new(),
        );
        assert_eq!(
            d.music_commands,
            [
                MusicCommand::Remove(FormId(10)),
                MusicCommand::Add(FormId(11))
            ]
        );
    }
    #[test]
    fn incoming_music_type_controls_fade_and_abrupt_types_cut() {
        assert_eq!(music_fade(0, 9.0), 9.0);
        assert_eq!(music_fade(crate::music::ABRUPT_TRANSITION, 9.0), 0.0);
    }
    #[test]
    fn submerged_targets_mute_only_flagged_categories_and_request_the_loop() {
        let mut d = Director::new(AudioDefaults::from_entries([(*b"UWLS", FormId(9))]));
        d.submerged_categories = vec![7];
        d.update(
            Inputs {
                submerged: true,
                ..Default::default()
            },
            &HashMap::new(),
        );
        assert_eq!(d.targets[&7].state.volume, 0.0);
        assert_eq!(d.targets[&7].seconds, 0.15);
        assert_eq!(d.loops, [FormId(9)]);
        d.update(Inputs::default(), &HashMap::new());
        assert_eq!(d.targets[&7].state.volume, 1.0);
        assert!(d.loops.is_empty());
    }
    #[test]
    fn stats_mutes_region_ambience_and_restores_it() {
        let mut d = Director::new(AudioDefaults::from_entries([(*b"SMSC", FormId(4))]));
        d.update(
            Inputs {
                stats: true,
                ..Default::default()
            },
            &HashMap::new(),
        );
        assert_eq!(d.targets[&4].state.volume, 0.0);
        d.update(Inputs::default(), &HashMap::new());
        assert_eq!(d.targets[&4].state.volume, 1.0);
    }
    #[test]
    fn only_player_conversation_ducks_the_dialogue_fade_category() {
        let mut d = Director::new(AudioDefaults::from_entries([(*b"SFDC", FormId(4))]));
        d.update(
            Inputs {
                dialogue: true,
                ..Default::default()
            },
            &HashMap::new(),
        );
        assert!((d.targets[&4].state.volume - 0.501187).abs() < 1e-5);
        assert_eq!(d.targets[&4].seconds, 3.0);
        d.update(Inputs::default(), &HashMap::new());
        assert_eq!(d.targets[&4].state.volume, 1.0);
        assert_eq!(d.targets[&4].seconds, 1.0);
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
