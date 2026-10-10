//! Keyboard and gamepad input for the menus, as the game would pass it on.
//!
//! The menus' CLIK `InputDelegate` listens to `Key` and maps Flash key codes to
//! navigation: 38/40/37/39 the arrows, 13 ENTER, 27 ESCAPE, 9 TAB (with shift
//! SHIFT_TAB), 8 BACK, 36/35/33/34 home/end/page up/page down, and Scaleform's
//! gamepad codes 96..=107 GAMEPAD_A, B, X, Y, L1, R1, L2, R2, L3, R3, START,
//! BACK. Which keys the game sends for its "Menu Mode" events comes from
//! Interface/Controls/PC/controlmap.txt: Up/Down/Left/Right are Forward/Back/
//! Strafe Left/Strafe Right (W/S/A/D), Accept is Activate (E), Cancel is Tween
//! Menu or Pause (Tab, Esc); on the gamepad Accept is A, Cancel B, and the d-pad
//! and left stick navigate. How the game turns those events into Flash key
//! codes isn't public; here each becomes the CLIK code of the same meaning,
//! and the letter keys also go through as themselves (the message box takes
//! Y, N and A as shortcuts for Yes, No and Yes to All).
//!
//! Accept is Enter (13) from the gamepad too: CLIK buttons only take ENTER,
//! and Scaleform presses the focused button on Enter (Ruffle patched to do so
//! without a focus highlight), which is how the message box accepts — it
//! empties its buttons' keyboard `handlePress`. Cancel from the gamepad is
//! GAMEPAD_B (97), which the message box checks along with ESCAPE and TAB.
//!
//! While an input text field has the focus (the game's text input mode, which
//! a menu asks for with `SetAllowTextInput`), keys go as themselves
//! (`text_mode_key`), typed characters as `PlayerEvent::TextInput` and editing
//! keys as `PlayerEvent::TextControl` (`text_control`).

use std::collections::HashMap;

use ruffle_core::events::{
    GamepadButton, KeyCode, KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey,
};
use winit::keyboard::KeyCode as Key;

/// Gamepad buttons as Scaleform's key codes (the d-pad as the arrows).
pub fn gamepad_key_codes() -> HashMap<GamepadButton, KeyCode> {
    use GamepadButton as B;
    [
        (B::South, 13),         // A: Accept, as Enter (GAMEPAD_A is 96)
        (B::East, 97),          // B
        (B::West, 98),          // X
        (B::North, 99),         // Y
        (B::LeftTrigger, 100),  // L1 (left bumper)
        (B::RightTrigger, 101), // R1
        (B::LeftTrigger2, 102), // L2 (left trigger)
        (B::RightTrigger2, 103),
        (B::Start, 106),
        (B::Select, 107), // BACK
        (B::DPadUp, 38),
        (B::DPadDown, 40),
        (B::DPadLeft, 37),
        (B::DPadRight, 39),
    ]
    .into_iter()
    .map(|(b, c)| (b, KeyCode::from_code(c)))
    .collect()
}

/// A keyboard key as the menu should see it: the menu-mode event's CLIK key
/// when the key is bound to one, then the key itself when it's a letter.
pub fn keys(code: Key) -> Vec<KeyDescriptor> {
    let mut keys: Vec<KeyDescriptor> = key(code).into_iter().collect();
    if let (Some(c), Some(first)) = (letter_or_digit(code), keys.first())
        && first.logical_key != LogicalKey::Character(c)
    {
        keys.push(descriptor(LogicalKey::Character(c)));
    }
    keys
}

fn descriptor(logical_key: LogicalKey) -> KeyDescriptor {
    KeyDescriptor {
        physical_key: PhysicalKey::Unknown,
        logical_key,
        key_location: KeyLocation::Standard,
    }
}

/// A keyboard key's first meaning for the menu: its menu-mode event as the CLIK
/// key of the same meaning, else the key itself.
pub fn key(code: Key) -> Option<KeyDescriptor> {
    let named = |n| LogicalKey::Named(n);
    let logical = match code {
        // Menu Mode: Up / Down / Left / Right (Forward, Back, Strafe Left/Right)
        Key::KeyW | Key::ArrowUp => named(NamedKey::ArrowUp),
        Key::KeyS | Key::ArrowDown => named(NamedKey::ArrowDown),
        Key::KeyA | Key::ArrowLeft => named(NamedKey::ArrowLeft),
        Key::KeyD | Key::ArrowRight => named(NamedKey::ArrowRight),
        // Accept (Activate)
        Key::KeyE | Key::Enter | Key::NumpadEnter => named(NamedKey::Enter),
        // Cancel (Tween Menu, Pause)
        Key::Tab => named(NamedKey::Tab),
        Key::Escape => named(NamedKey::Escape),
        Key::ShiftLeft | Key::ShiftRight => named(NamedKey::Shift),
        Key::Backspace => named(NamedKey::Backspace),
        Key::Home => named(NamedKey::Home),
        Key::End => named(NamedKey::End),
        Key::PageUp => named(NamedKey::PageUp),
        Key::PageDown => named(NamedKey::PageDown),
        Key::Space => LogicalKey::Character(' '),
        code => LogicalKey::Character(letter_or_digit(code)?),
    };
    Some(descriptor(logical))
}

fn letter_or_digit(code: Key) -> Option<char> {
    let name = format!("{code:?}");
    let c = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))?;
    let mut chars = c.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

/// A key while an input text field has the focus (the game's text input mode,
/// `SetAllowTextInput`): as itself, no menu-mode translation, so typing "e"
/// isn't Accept. Enter, Esc, Tab and the arrows still reach the menu.
pub fn text_mode_key(code: Key) -> Option<KeyDescriptor> {
    let named = |n| Some(descriptor(LogicalKey::Named(n)));
    match code {
        Key::ArrowUp => named(NamedKey::ArrowUp),
        Key::ArrowDown => named(NamedKey::ArrowDown),
        Key::ArrowLeft => named(NamedKey::ArrowLeft),
        Key::ArrowRight => named(NamedKey::ArrowRight),
        Key::Enter | Key::NumpadEnter => named(NamedKey::Enter),
        Key::Tab => named(NamedKey::Tab),
        Key::Escape => named(NamedKey::Escape),
        Key::ShiftLeft | Key::ShiftRight => named(NamedKey::Shift),
        Key::ControlLeft | Key::ControlRight => named(NamedKey::Control),
        Key::Backspace => named(NamedKey::Backspace),
        Key::Delete => named(NamedKey::Delete),
        Key::Home => named(NamedKey::Home),
        Key::End => named(NamedKey::End),
        Key::Space => Some(descriptor(LogicalKey::Character(' '))),
        code => letter_or_digit(code).map(|c| descriptor(LogicalKey::Character(c))),
    }
}

/// The editing a key does in a focused text field.
pub fn text_control(
    code: Key,
    shift: bool,
    ctrl: bool,
) -> Option<ruffle_core::events::TextControlCode> {
    use ruffle_core::events::TextControlCode as T;
    Some(match (code, shift, ctrl) {
        (Key::Backspace, _, false) => T::Backspace,
        (Key::Backspace, _, true) => T::BackspaceWord,
        (Key::Delete, _, false) => T::Delete,
        (Key::Delete, _, true) => T::DeleteWord,
        (Key::ArrowLeft, false, false) => T::MoveLeft,
        (Key::ArrowLeft, false, true) => T::MoveLeftWord,
        (Key::ArrowLeft, true, false) => T::SelectLeft,
        (Key::ArrowLeft, true, true) => T::SelectLeftWord,
        (Key::ArrowRight, false, false) => T::MoveRight,
        (Key::ArrowRight, false, true) => T::MoveRightWord,
        (Key::ArrowRight, true, false) => T::SelectRight,
        (Key::ArrowRight, true, true) => T::SelectRightWord,
        (Key::Home, false, _) => T::MoveLeftLine,
        (Key::Home, true, _) => T::SelectLeftLine,
        (Key::End, false, _) => T::MoveRightLine,
        (Key::End, true, _) => T::SelectRightLine,
        (Key::KeyA, _, true) => T::SelectAll,
        (Key::KeyC, _, true) => T::Copy,
        (Key::KeyV, _, true) => T::Paste,
        (Key::KeyX, _, true) => T::Cut,
        _ => return None,
    })
}
