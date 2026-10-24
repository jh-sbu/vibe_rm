//! Sounds a script started with `Sound.Play`: an instance id for each, so
//! `StopInstance`, `SetInstanceVolume` and `PlayAndWait` can reach the voice.

use crate::audio::VoiceId;
use crate::engine::Engine;
use esp::FormId;
use glam::Vec3;
use std::collections::HashMap;

/// How long a `PlayAndWait` waits when the clip's length isn't known.
const WAIT_CAP: f32 = 60.0;

struct Instance {
    /// `None` without an audio device, or a clip that couldn't be started.
    voice: Option<VoiceId>,
    /// The descriptor's gain for this playing; `SetInstanceVolume` scales it.
    gain: f32,
    looping: bool,
    /// Length of one playing in seconds, when the clip was decoded.
    duration: Option<f32>,
}

pub(crate) struct ScriptSounds {
    next: i32,
    instances: HashMap<i32, Instance>,
    /// What scripts did to sound categories, by form id; kept for the session
    /// (inference: the audio mixer's settings aren't saved) and sent to the
    /// mixer when they change.
    categories: HashMap<u32, crate::audio::CategoryState>,
    categories_dirty: bool,
}

impl Default for ScriptSounds {
    fn default() -> Self {
        Self {
            next: 1,
            instances: HashMap::new(),
            categories: HashMap::new(),
            categories_dirty: false,
        }
    }
}

/// The key a `PlayAndWait` on instance `id` waits for.
fn wait_key(id: i32) -> u64 {
    0x5E00_0000_0000_0000 | id as u32 as u64
}

impl Engine {
    /// Papyrus `Sound.Play`: start the descriptor at `at` and return its
    /// instance id, 0 when there is no such descriptor. Ids are not reused.
    pub(crate) fn play_sound_instance(&mut self, sound: FormId, at: Vec3) -> i32 {
        let Some(desc) = self.sound_desc(sound) else {
            return 0;
        };
        let i = (self.rand() % desc.files.len() as u64) as usize;
        let v = self.roll_variation(&desc);
        let gain = desc.volume * v.gain;
        let (voice, duration) = match self.audio.as_mut() {
            Some(a) => (
                a.play_pitched(
                    &self.vfs,
                    &desc.files[i],
                    gain,
                    desc.looping,
                    v.pitch,
                    desc.positional.then_some(at),
                    desc.falloff,
                ),
                a.clip_duration(&desc.files[i])
                    .map(|d| d / v.pitch.max(0.01)),
            ),
            None => (None, None),
        };
        self.tag_voice(voice, &desc.categories);
        let id = self.script_sounds.next;
        self.script_sounds.next += 1;
        self.script_sounds.instances.insert(
            id,
            Instance {
                voice,
                gain,
                looping: desc.looping,
                duration,
            },
        );
        id
    }

    /// Papyrus `Sound.StopInstance`. An unknown or ended id does nothing.
    pub(crate) fn stop_sound_instance(&mut self, id: i32) {
        if let Some(inst) = self.script_sounds.instances.remove(&id)
            && let (Some(v), Some(a)) = (inst.voice, self.audio.as_ref())
        {
            a.stop(v);
        }
        self.vm.signal(wait_key(id));
    }

    /// Papyrus `Sound.SetInstanceVolume`: `volume` times the playing's gain.
    pub(crate) fn set_sound_instance_volume(&mut self, id: i32, volume: f32) {
        if let Some(inst) = self.script_sounds.instances.get(&id)
            && let (Some(v), Some(a)) = (inst.voice, self.audio.as_mut())
        {
            a.set_volume(v, inst.gain * volume.max(0.0));
        }
    }

    /// What a `PlayAndWait` on `id` waits on and for how long, or `None` when
    /// the sound is already over (no device, or a clip that never started).
    pub(crate) fn sound_instance_wait(&self, id: i32) -> Option<(u64, f32)> {
        let inst = self.script_sounds.instances.get(&id)?;
        inst.voice?;
        // A loop never ends by itself; the cap keeps the script going.
        let timeout = match inst.duration {
            Some(d) if !inst.looping => d + 0.5,
            _ => WAIT_CAP,
        };
        Some((wait_key(id), timeout))
    }

    /// Drop the instances that ended and wake the scripts waiting on them.
    /// What a script does to a sound category (`SoundCategory.Mute`...): any
    /// SNCT, reaching the sounds in it and below it.
    pub fn change_sound_category(
        &mut self,
        category: FormId,
        change: impl FnOnce(&mut crate::audio::CategoryState),
    ) {
        if self.lo.tag_of(category).map(|t| t.0) != Some(*b"SNCT") {
            return;
        }
        let state = self.script_sounds.categories.entry(category.0).or_default();
        change(state);
        self.script_sounds.categories_dirty = true;
    }

    /// The state scripts left a sound category in.
    pub fn sound_category(&self, category: FormId) -> crate::audio::CategoryState {
        self.script_sounds
            .categories
            .get(&category.0)
            .copied()
            .unwrap_or_default()
    }

    /// Put sounds that play in the categories their descriptor names.
    pub(crate) fn tag_voice(&mut self, voice: Option<VoiceId>, categories: &[u32]) {
        if let (Some(v), Some(a)) = (voice, self.audio.as_mut()) {
            a.set_voice_categories(v, categories);
        }
    }

    pub(crate) fn update_script_sounds(&mut self) {
        if self.script_sounds.categories_dirty
            && let Some(a) = self.audio.as_ref()
        {
            for (&c, &state) in &self.script_sounds.categories {
                a.set_category(c, state);
            }
            self.script_sounds.categories_dirty = false;
        }
        let ended: Vec<i32> = self
            .script_sounds
            .instances
            .iter()
            .filter(|(_, inst)| match (inst.voice, self.audio.as_ref()) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => true,
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ended {
            self.script_sounds.instances.remove(&id);
            self.vm.signal(wait_key(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn engine() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-script-sounds-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let records = vec![Node::top(
            b"SNDR",
            vec![
                RecordBuilder::new(b"SNDR", FormId(0x800)).zstring(b"ANAM", "a.wav"),
                RecordBuilder::new(b"SNDR", FormId(0x801)).zstring(b"ANAM", "b.wav"),
                RecordBuilder::new(b"SNDR", FormId(0x802)),
            ],
        )];
        std::fs::write(
            f.0.join("Sounds.esp"),
            write_plugin(0, &[], "test", "synthetic sounds", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "Sounds.esp");
        (f, e)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn instances_get_increasing_ids_even_without_a_device() {
        let (_f, mut e) = engine();
        assert_eq!(e.play_sound_instance(FormId(0x800), Vec3::ZERO), 1);
        assert_eq!(e.play_sound_instance(FormId(0x801), Vec3::ZERO), 2);
        // No files, or no record at all: nothing played.
        assert_eq!(e.play_sound_instance(FormId(0x802), Vec3::ZERO), 0);
        assert_eq!(e.play_sound_instance(FormId(0x999), Vec3::ZERO), 0);
        // Stopping and changing the volume of ended or unknown ids are no-ops.
        e.stop_sound_instance(1);
        e.stop_sound_instance(77);
        e.set_sound_instance_volume(2, 0.5);
        // Without a voice there is nothing to wait for, and the update prunes.
        assert!(e.sound_instance_wait(2).is_none());
        e.update_script_sounds();
        assert!(e.script_sounds.instances.is_empty());
        // Ids keep counting after the table empties.
        assert_eq!(e.play_sound_instance(FormId(0x800), Vec3::ZERO), 3);
    }

    #[test]
    fn wait_keys_are_per_instance() {
        assert_ne!(wait_key(1), wait_key(2));
    }
}
