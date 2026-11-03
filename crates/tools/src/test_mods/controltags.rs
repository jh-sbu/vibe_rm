//! `test-mod controltags`: what Skyrim SE shows for `[Event]` tags in text
//! (`plans/control-text-tags.md`). Visual only: Papyrus can't read rendered
//! text, so the user takes screenshots. The script is
//! `controltags/controltags.psc`; `research/ui/control-tags.md` ("Manual
//! Skyrim SE observation") has the full protocol.
//!
//! Every token is numbered so a screenshot can be read even where art
//! replaces the tag. Set A is ordinary bindings, set B the edge cases. All
//! B's events are in the shipped map, so no control map needs editing:
//!
//! | # | tag | shipped binding | this engine shows (label / art) |
//! | --- | --- | --- | --- |
//! | 1 | `[Activate]` | E | E / E |
//! | 2 | `[activate]` | case-insensitive match | E / E |
//! | 3 | `[Sprint]` | L-Alt | L-Alt / L-Alt |
//! | 4 | `[Right Attack/Block]` | mouse only, Mouse1 | Mouse1 / Mouse1 |
//! | 5 | `[Favorites]` | Q | Q / Q |
//! | 6 | `[Mouse1]` | a key name, not an event | Mouse1 / Mouse1 |
//! | 7 | `[XButton]` | Item Menus event, `!0,Ready Weapon` (R) | R / R |
//! | 8 | `[Zoom In]` | mouse wheel only | MouseWheelUp / Wheel |
//! | 9 | `[Multi-Screenshot]` | chord L-Ctrl+PrintSrc (no PrintSrc art) | L-Ctrl+PrintSrc / UnknownKey |
//! | 10 | `[Move]` | gamepad only | UnknownKey / UnknownKey |
//! | 11 | `[Optional]` | not a control | kept / kept |
//! | 12 | `[Mouse Move]` | not a name (the key is `MouseMove`) | kept / kept |
//! | 13 | `[Left Thumbstick]` | not a name | kept / kept |
//!
//! Surfaces: a message box (title, text, button), two notifications, the
//! quest's objective (HUD banner and journal), its name and log entry in the
//! journal, and two help messages (the HUD's tutorial hint). Round 2 rebinds
//! Activate to NumPad5, a key with no button art: its label and art, and
//! whether text already in the journal or a hint already up changes.

use super::{TestMod, own, script};
use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};

const SCRIPT: &str = "VrmControlTagsScript";
const SOURCE: &str = include_str!("controltags/controltags.psc");
const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x806;

const A: &str =
    "A: 1[Activate] 2[activate] 3[Sprint] 4[Right Attack/Block] 5[Favorites] 6[Mouse1] 7[XButton]";
const B: &str =
    "B: 8[Zoom In] 9[Multi-Screenshot] 10[Move] 11[Optional] 12[Mouse Move] 13[Left Thumbstick]";

/// The script's message properties: name, object id, whether a box, text.
fn messages() -> [(&'static str, u32, bool, String); 5] {
    [
        ("Box", 0x801, true, format!("Box\n{A}\n{B}")),
        ("NoteA", 0x802, false, format!("Note {A}")),
        ("NoteB", 0x803, false, format!("Note {B}")),
        ("HelpA", 0x804, false, format!("Help {A}")),
        ("HelpB", 0x805, false, format!("Help {B}")),
    ]
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    script::compile(&[SOURCE])
}

pub fn build() -> Result<TestMod> {
    let attached = AttachedScript {
        name: SCRIPT.into(),
        properties: messages()
            .iter()
            .map(|(name, id, ..)| (name.to_string(), PropertyValue::Object(own(*id))))
            .collect(),
    };
    // DNAM: flags (start game enabled, starts enabled), priority, form
    // version, unknown, type 8 (side quest: the journal lists it with its
    // name, objectives and log).
    let mut dnam = 0x0011u16.to_le_bytes().to_vec();
    dnam.extend_from_slice(&[0, 0]);
    dnam.extend_from_slice(&0u32.to_le_bytes());
    dnam.extend_from_slice(&8u32.to_le_bytes());
    let quest = RecordBuilder::new(b"QUST", own(QUEST))
        .zstring(b"EDID", "VrmControlTagsQuest")
        .field(b"VMAD", &vmad(&[attached]))
        .zstring(
            b"FULL",
            "Quest 1[Activate] 9[Multi-Screenshot] 11[Optional]",
        )
        .field(b"DNAM", &dnam)
        .field(b"NEXT", &[])
        // Stage 10's one log entry.
        .field(b"INDX", &[10, 0, 0, 0])
        .field(b"QSDT", &[0])
        .zstring(b"CNAM", &format!("Log\n{A}\n{B}"))
        .field(b"QOBJ", &10u16.to_le_bytes())
        .u32(b"FNAM", 0)
        .zstring(
            b"NNAM",
            "Obj 1[Activate] 8[Zoom In] 9[Multi-Screenshot] 11[Optional]",
        )
        .u32(b"ANAM", 0);
    let messages = messages()
        .into_iter()
        .map(|(name, id, message_box, text)| {
            let mut r = RecordBuilder::new(b"MESG", own(id))
                .zstring(b"EDID", &format!("VrmControlTags{name}"))
                .zstring(b"DESC", &text);
            if message_box {
                r = r.zstring(b"FULL", "Title 1[Activate] 9[Multi-Screenshot]");
            }
            r = r.u32(b"DNAM", u32::from(message_box)).u32(b"TNAM", 10);
            if message_box {
                r = r.zstring(b"ITXT", "OK 1[Activate]");
            }
            r
        })
        .collect();
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Control tags (visual): cqf VrmControlTagsQuest Probe, close the console.",
            NEXT_OBJECT_ID,
            &[
                Node::top(b"QUST", vec![quest]),
                Node::top(b"MESG", messages),
            ],
        ),
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_with_vanilla_natives() {
        let m = super::build().unwrap();
        assert_eq!(m.scripts.len(), 1);
        let p = papyrus::pex::parse(&m.scripts[0].1).unwrap();
        assert_eq!(p.str(p.objects[0].name), super::SCRIPT);
        assert!(super::super::script::mistyped_calls(&p).is_empty());
        assert!(!m.plugin.is_empty());
        assert_eq!(super::A.matches('[').count(), 7);
        assert_eq!(super::B.matches('[').count(), 6);
    }
}
