//! `test-mod pex`: whether Skyrim SE runs a script that `papyrus::build`
//! assembles and `papyrus::pex::write` writes, attached by a VMAD that
//! `esp::writer::vmad` writes. No ambiguity in the game itself; it checks
//! our writers before other test mods rely on them.
//!
//! A start game enabled quest, `VrmPexTestQuest`, carries
//! `VrmPexTestScript` with an object property (`Player`, the player) and an
//! int property (`Answer`, 42). On `OnInit` it shows a message box (and
//! traces it to the Papyrus log) built from a method call, a cast, string
//! concatenation and branches on both properties:
//!
//! | in game                                    | reading                         |
//! | ------------------------------------------ | ------------------------------- |
//! | "vibe_rm PEX test: level N, answer 42"     | the script and its VMAD work    |
//! | "... Player property not set ..."          | the script runs; VMAD objects aren't read |
//! | no ", answer 42"                           | VMAD scalars aren't read        |
//! | no message                                 | the script didn't run: see the Papyrus log |
//!
//! Install the `.esp` and `Scripts/VrmPexTestScript.pex` beside it; the
//! message shows on loading any save or starting a new game.
//!
//! Result in Skyrim SE (2026-10-12): the script and its VMAD work.

use anyhow::Result;
use esp::FormId;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, discard, string};
use papyrus::pex::op;

use super::{TestMod, own};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x801;
const SCRIPT: &str = "VrmPexTestScript";
/// The player's reference: a fixed form every game has.
const PLAYER_REF: FormId = FormId(0x14);

fn script() -> Result<Vec<u8>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    let player = s.auto_property("Player", "Actor");
    let answer = s.auto_property("Answer", "Int");

    let mut f = FunctionBuilder::new("None");
    let text = f.local("text", "String");
    let test = f.local("::temp0", "Bool");
    let level = f.local("::temp1", "Int");
    let level_text = f.local("::temp2", "String");
    let no_actor = f.local("::temp3", "Actor");
    let (filled, check_answer, show) = (f.label(), f.label(), f.label());
    f.assign(text.clone(), string("vibe_rm PEX test: "))
        // `if Player == None`, comparing as the compiler does: with None
        // cast to the property's type.
        .cast(no_actor.clone(), Arg::None)
        .op(op::CMP_EQ, [test.clone(), player.clone(), no_actor])
        .jmpf(test.clone(), filled)
        .strcat(
            text.clone(),
            text.clone(),
            string("Player property not set"),
        )
        .jmp(check_answer)
        .bind(filled)
        .call_method(player, "GetLevel", level.clone(), [])
        .cast(level_text.clone(), level)
        .strcat(text.clone(), text.clone(), string("level "))
        .strcat(text.clone(), text.clone(), level_text)
        .bind(check_answer)
        .op(op::CMP_EQ, [test.clone(), answer, 42.into()])
        .jmpf(test, show)
        .strcat(text.clone(), text.clone(), string(", answer 42"))
        .bind(show)
        .call_static("Debug", "MessageBox", discard(), [text.clone()])
        .call_static("Debug", "Trace", discard(), [text, 0.into()])
        .ret(Arg::None);
    s.function("OnInit", f);
    Ok(papyrus::pex::write(&s.build()?)?)
}

pub fn build() -> Result<TestMod> {
    let attached = AttachedScript {
        name: SCRIPT.into(),
        properties: vec![
            ("Player".into(), PropertyValue::Object(PLAYER_REF)),
            ("Answer".into(), PropertyValue::Int(42)),
        ],
    };
    // DNAM: flags (start game enabled and starts enabled, as vanilla
    // start game enabled quests), priority, form version, unknown, type
    // (none).
    let mut dnam = 0x0011u16.to_le_bytes().to_vec();
    dnam.extend_from_slice(&[0, 0]);
    dnam.extend_from_slice(&0u32.to_le_bytes());
    dnam.extend_from_slice(&0u32.to_le_bytes());
    let quest = RecordBuilder::new(b"QUST", own(QUEST))
        .zstring(b"EDID", "VrmPexTestQuest")
        .field(b"VMAD", &vmad(&[attached]))
        .zstring(b"FULL", "vibe_rm PEX Test")
        .field(b"DNAM", &dnam)
        .field(b"NEXT", &[])
        .u32(b"ANAM", 0); // next alias id
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "A quest whose script vibe_rm wrote: a message box on loading.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", vec![quest])],
    );
    Ok(TestMod {
        plugin,
        scripts: vec![(SCRIPT.into(), script()?)],
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn script_builds() {
        let bytes = super::script().unwrap();
        let p = papyrus::pex::parse(&bytes).unwrap();
        assert_eq!(p.str(p.objects[0].name), super::SCRIPT);
    }
}
