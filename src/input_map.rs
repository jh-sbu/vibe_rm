//! Main Gameplay user events and the original controlmap column format.
use std::{
    collections::HashSet,
    io,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use winit::keyboard::KeyCode as K;
pub type Shared = Arc<RwLock<InputMap>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Key(K),
    Mouse(u32),
}
#[derive(Clone, Debug)]
pub struct Event {
    pub name: String,
    pub columns: [String; 3],
    pub remappable: [bool; 3],
    pub flags: String,
}
#[derive(Clone, Debug, Default)]
pub struct InputMap {
    pub contexts: Vec<Vec<Event>>,
    defaults: String,
    pub generation: u64,
}
impl InputMap {
    pub fn parse(text: &str) -> Self {
        let mut contexts = vec![Vec::<Event>::new()];
        let mut boundary = false;
        for line in text.lines().map(str::trim) {
            if line.is_empty() {
                if !contexts.last().unwrap().is_empty() {
                    boundary = true;
                }
                continue;
            }
            if line.starts_with("//") {
                continue;
            }
            let cols: Vec<_> = line
                .split('\t')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if cols.len() < 3 {
                continue;
            }
            if boundary {
                contexts.push(Vec::new());
                boundary = false;
            }
            if contexts.last().unwrap().iter().any(|e| e.name == cols[0]) {
                continue;
            }
            contexts.last_mut().unwrap().push(Event {
                name: cols[0].into(),
                columns: std::array::from_fn(|i| cols.get(i + 1).unwrap_or(&"0xff").to_string()),
                remappable: std::array::from_fn(|i| cols.get(i + 4) == Some(&"1")),
                flags: cols.get(7).unwrap_or(&"").to_string(),
            });
        }
        Self {
            contexts,
            defaults: text.into(),
            generation: 0,
        }
    }
    pub fn load(vfs: &vfs::Vfs) -> Shared {
        let default = vfs
            .read("interface/controls/pc/controlmap.txt")
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let mut map = Self::parse(&default);
        if let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) {
            let custom = Self::parse(&text);
            for changed in custom.events() {
                if let Some(event) = map.contexts[0].iter_mut().find(|e| e.name == changed.name) {
                    for d in 0..2 {
                        if event.remappable[d] {
                            event.columns[d] = changed.columns[d].clone();
                        }
                    }
                }
            }
        }
        Arc::new(RwLock::new(map))
    }
    pub fn events(&self) -> &[Event] {
        self.contexts.first().map_or(&[], Vec::as_slice)
    }
    pub fn bindings(&self, event: &str, device: usize) -> Vec<Vec<u32>> {
        self.resolve(0, event, device, 0)
    }
    fn resolve(&self, context: usize, event: &str, device: usize, depth: u8) -> Vec<Vec<u32>> {
        if depth > 8 || device >= 3 {
            return Vec::new();
        }
        let Some(row) = self
            .contexts
            .get(context)
            .and_then(|rows| rows.iter().find(|e| e.name == event))
        else {
            return Vec::new();
        };
        let raw = &row.columns[device];
        if raw.starts_with('!') {
            return raw
                .split(",!")
                .filter_map(|s| s.trim_start_matches('!').split_once(','))
                .flat_map(|(ctx, name)| {
                    ctx.parse::<usize>()
                        .ok()
                        .map_or(Vec::new(), |ctx| self.resolve(ctx, name, device, depth + 1))
                })
                .collect();
        }
        raw.split(',')
            .filter_map(|alternative| {
                let chord: Option<Vec<_>> = alternative
                    .split('+')
                    .map(|s| {
                        u32::from_str_radix(s.trim().trim_start_matches("0x"), 16)
                            .ok()
                            .filter(|n| *n != 0xff)
                    })
                    .collect();
                chord.filter(|c| !c.is_empty())
            })
            .collect()
    }
    pub fn first(&self, event: &str, device: usize) -> Option<u32> {
        self.bindings(event, device).first()?.last().copied()
    }
    pub fn held(&self, event: &str, keys: &HashSet<K>, mouse: &HashSet<u32>) -> bool {
        self.bindings(event, 0).iter().any(|chord| {
            chord
                .iter()
                .all(|c| key_code(*c).is_some_and(|k| keys.contains(&k)))
        }) || self
            .bindings(event, 1)
            .iter()
            .any(|chord| chord.iter().all(|c| mouse.contains(c)))
    }
    /// Releasing the final key ends its action even when a chord's modifier
    /// was released first. This avoids a stuck Activate/attack/voice hold.
    pub fn released(&self, event: &str, button: Button) -> bool {
        let (device, code) = match button {
            Button::Key(key) => (0, scan_code(key)),
            Button::Mouse(code) => (1, Some(code)),
        };
        code.is_some_and(|code| {
            self.bindings(event, device)
                .iter()
                .any(|chord| chord.last() == Some(&code))
        })
    }
    pub fn triggered(
        &self,
        event: &str,
        button: Button,
        keys: &HashSet<K>,
        mouse: &HashSet<u32>,
    ) -> bool {
        let (device, code) = match button {
            Button::Key(k) => (0, scan_code(k)),
            Button::Mouse(n) => (1, Some(n)),
        };
        code.is_some_and(|code| {
            self.bindings(event, device).iter().any(|c| {
                c.last() == Some(&code)
                    && c.iter().all(|n| {
                        *n == code
                            || if device == 0 {
                                key_code(*n).is_some_and(|k| keys.contains(&k))
                            } else {
                                mouse.contains(n)
                            }
                    })
            })
        })
    }
    pub fn encode(&self) -> String {
        let mut out = String::new();
        for context in &self.contexts {
            for e in context {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    e.name,
                    e.columns[0],
                    e.columns[1],
                    e.columns[2],
                    e.remappable[0] as u8,
                    e.remappable[1] as u8,
                    e.remappable[2] as u8,
                    e.flags
                ));
            }
            out.push('\n');
        }
        out
    }
    pub fn reset(&mut self) {
        let generation = self.generation + 1;
        *self = Self::parse(&self.defaults.clone());
        self.generation = generation;
    }
    pub fn save(&self) -> io::Result<()> {
        if let Some(path) = path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&tmp, self.encode())?;
            std::fs::rename(tmp, path)?;
        }
        Ok(())
    }
}
pub fn path() -> Option<PathBuf> {
    std::env::var_os("VRM_CONTROLS")
        .map(PathBuf::from)
        .or_else(|| crate::settings::config_dir().map(|p| p.join("controlmap_custom.txt")))
}
/// DirectInput scan codes are platform independent; winit's native scancodes
/// are not (Linux evdev differs from the game's PC controlmap).
const KEYS: &[(u32, K)] = &[
    (0x01, K::Escape),
    (0x02, K::Digit1),
    (0x03, K::Digit2),
    (0x04, K::Digit3),
    (0x05, K::Digit4),
    (0x06, K::Digit5),
    (0x07, K::Digit6),
    (0x08, K::Digit7),
    (0x09, K::Digit8),
    (0x0a, K::Digit9),
    (0x0b, K::Digit0),
    (0x0c, K::Minus),
    (0x0d, K::Equal),
    (0x0e, K::Backspace),
    (0x0f, K::Tab),
    (0x10, K::KeyQ),
    (0x11, K::KeyW),
    (0x12, K::KeyE),
    (0x13, K::KeyR),
    (0x14, K::KeyT),
    (0x15, K::KeyY),
    (0x16, K::KeyU),
    (0x17, K::KeyI),
    (0x18, K::KeyO),
    (0x19, K::KeyP),
    (0x1a, K::BracketLeft),
    (0x1b, K::BracketRight),
    (0x1c, K::Enter),
    (0x1d, K::ControlLeft),
    (0x1e, K::KeyA),
    (0x1f, K::KeyS),
    (0x20, K::KeyD),
    (0x21, K::KeyF),
    (0x22, K::KeyG),
    (0x23, K::KeyH),
    (0x24, K::KeyJ),
    (0x25, K::KeyK),
    (0x26, K::KeyL),
    (0x27, K::Semicolon),
    (0x28, K::Quote),
    (0x29, K::Backquote),
    (0x2a, K::ShiftLeft),
    (0x2b, K::Backslash),
    (0x2c, K::KeyZ),
    (0x2d, K::KeyX),
    (0x2e, K::KeyC),
    (0x2f, K::KeyV),
    (0x30, K::KeyB),
    (0x31, K::KeyN),
    (0x32, K::KeyM),
    (0x33, K::Comma),
    (0x34, K::Period),
    (0x35, K::Slash),
    (0x36, K::ShiftRight),
    (0x37, K::NumpadMultiply),
    (0x38, K::AltLeft),
    (0x39, K::Space),
    (0x3a, K::CapsLock),
    (0x3b, K::F1),
    (0x3c, K::F2),
    (0x3d, K::F3),
    (0x3e, K::F4),
    (0x3f, K::F5),
    (0x40, K::F6),
    (0x41, K::F7),
    (0x42, K::F8),
    (0x43, K::F9),
    (0x44, K::F10),
    (0x45, K::NumLock),
    (0x46, K::ScrollLock),
    (0x47, K::Numpad7),
    (0x48, K::Numpad8),
    (0x49, K::Numpad9),
    (0x4a, K::NumpadSubtract),
    (0x4b, K::Numpad4),
    (0x4c, K::Numpad5),
    (0x4d, K::Numpad6),
    (0x4e, K::NumpadAdd),
    (0x4f, K::Numpad1),
    (0x50, K::Numpad2),
    (0x51, K::Numpad3),
    (0x52, K::Numpad0),
    (0x53, K::NumpadDecimal),
    (0x56, K::IntlBackslash),
    (0x57, K::F11),
    (0x58, K::F12),
    (0x9c, K::NumpadEnter),
    (0x9d, K::ControlRight),
    (0xb5, K::NumpadDivide),
    (0xb7, K::PrintScreen),
    (0xb8, K::AltRight),
    (0xc5, K::Pause),
    (0xc7, K::Home),
    (0xc8, K::ArrowUp),
    (0xc9, K::PageUp),
    (0xcb, K::ArrowLeft),
    (0xcd, K::ArrowRight),
    (0xcf, K::End),
    (0xd0, K::ArrowDown),
    (0xd1, K::PageDown),
    (0xd2, K::Insert),
    (0xd3, K::Delete),
    (0xdb, K::SuperLeft),
    (0xdc, K::SuperRight),
];
pub fn scan_code(key: K) -> Option<u32> {
    KEYS.iter().find(|(_, k)| *k == key).map(|(n, _)| *n)
}
pub fn key_code(code: u32) -> Option<K> {
    KEYS.iter().find(|(n, _)| *n == code).map(|(_, k)| *k)
}
pub fn mouse_code(button: winit::event::MouseButton) -> Option<u32> {
    use winit::event::MouseButton::*;
    match button {
        Left => Some(0),
        Right => Some(1),
        Middle => Some(2),
        Back => Some(3),
        Forward => Some(4),
        Other(n) if n < 8 => Some(n as u32),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternatives_references_chords_and_round_trip() {
        let m = InputMap::parse(
            "Activate\t0x12,0x1d+0x22\t0xff\t0x1000\t1\t1\t1\n\nAccept\t!0,Activate\t!0,Activate\t0xff\t0\t0\t0\nCancel\t!0,Activate,!0,Activate\t0xff\t0xff\t0\t0\t0\n",
        );
        assert_eq!(
            m.resolve(1, "Accept", 0, 0),
            vec![vec![0x12], vec![0x1d, 0x22]]
        );
        assert_eq!(m.resolve(1, "Cancel", 0, 0).len(), 4);
        assert!(!m.triggered(
            "Activate",
            Button::Key(K::KeyG),
            &HashSet::new(),
            &HashSet::new()
        ));
        assert!(m.triggered(
            "Activate",
            Button::Key(K::KeyG),
            &[K::ControlLeft].into(),
            &HashSet::new()
        ));
        let n = InputMap::parse(&m.encode());
        assert_eq!(n.bindings("Activate", 0), m.bindings("Activate", 0));
        assert_eq!(n.events()[0].columns[2], "0x1000");
    }
}
impl InputMap {
    pub fn remap(&mut self, event: &str, button: Button) -> bool {
        let (device, code) = match button {
            Button::Key(key) => {
                if matches!(key, K::Escape | K::Backquote | K::F3 | K::KeyN) {
                    return false;
                }
                (0, scan_code(key))
            }
            Button::Mouse(code) => (1, (code <= 9).then_some(code)),
        };
        let Some(code) = code else {
            return false;
        };
        let Some(index) = self
            .events()
            .iter()
            .position(|e| e.name == event && e.remappable[device])
        else {
            return false;
        };
        let conflicts: Vec<_> = self
            .events()
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                *i != index
                    && e.remappable[device]
                    && self
                        .bindings(&e.name, device)
                        .iter()
                        .any(|c| c.last() == Some(&code))
            })
            .map(|(i, _)| i)
            .collect();
        for i in conflicts {
            self.contexts[0][i].columns[device] = "0xff".into();
        }
        self.contexts[0][index].columns[0] = "0xff".into();
        self.contexts[0][index].columns[1] = "0xff".into();
        self.contexts[0][index].columns[device] = format!("0x{code:x}");
        self.generation += 1;
        true
    }
}
#[cfg(test)]
mod remap_tests {
    use super::*;
    #[test]
    fn capture_reserved_conflict_and_custom_round_trip() {
        let mut m = InputMap::parse(
            "Activate\t0x12\t0xff\t0x1000\t1\t1\t1\nJump\t0x39\t0xff\t0x8000\t1\t1\t1\n",
        );
        assert!(!m.remap("Activate", Button::Key(K::Escape)));
        assert!(m.remap("Activate", Button::Key(K::Space)));
        assert_eq!(m.first("Jump", 0), None);
        assert!(m.triggered(
            "Activate",
            Button::Key(K::Space),
            &HashSet::new(),
            &HashSet::new()
        ));
        let b = InputMap::parse(&m.encode());
        assert_eq!(b.first("Activate", 0), Some(0x39));
        assert_eq!(b.events()[0].columns[2], "0x1000");
        m.reset();
        assert_eq!(m.first("Activate", 0), Some(0x12));
    }
}

#[cfg(test)]
mod release_tests {
    use super::*;
    #[test]
    fn chord_release_does_not_require_a_still_held_modifier() {
        let map = InputMap::parse("Activate\t0x1d+0x22\t0xff\t0xff\t1\t1\t0\n");
        assert!(!map.triggered(
            "Activate",
            Button::Key(K::KeyG),
            &HashSet::new(),
            &HashSet::new()
        ));
        assert!(map.released("Activate", Button::Key(K::KeyG)));
        assert!(!map.released("Activate", Button::Key(K::ControlLeft)));
    }
}
