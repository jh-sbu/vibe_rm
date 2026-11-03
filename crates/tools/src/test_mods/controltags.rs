//! Visual-only vanilla SE probe; rendered text cannot be read back by Papyrus.
//! Run `cqf VrmControlTagsQuest Probe` before and after rebinding; see the research note.
use super::script::Code;
use super::{TestMod, own};
use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use papyrus::build::{Arg, ScriptBuilder, auto_var, string};

const SCRIPT: &str = "VrmControlTagsScript";
const TAGS: &str = "[Activate] [Sprint] [Right Attack/Block] [Favorites] [Mouse1] [XButton]";
const HELP_EVENT: &str = "VrmControlTagsProbe";

fn script() -> Result<Vec<u8>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for name in ["Box", "Notification", "Help"] {
        s.auto_property(name, "Message");
    }
    let mut c = Code::new("None");
    // Let the user close the console before any of the three surfaces appears.
    c.global("Utility", "Wait", [5f32.into()]);
    let zeros = || std::array::from_fn::<_, 9, _>(|_| 0f32.into());
    c.method("Message", auto_var("Box"), "Show", zeros());
    c.global("Utility", "Wait", [2f32.into()]);
    c.method("Message", auto_var("Notification"), "Show", zeros());
    c.global("Utility", "Wait", [8f32.into()]);
    c.global("Message", "ResetHelpMessage", [string(HELP_EVENT)]);
    c.method(
        "Message",
        auto_var("Help"),
        "ShowAsHelpMessage",
        [string(HELP_EVENT), 20f32.into(), 0f32.into(), 1.into()],
    );
    s.function("Probe", c.finish(Arg::None));
    Ok(papyrus::pex::write(&s.build()?)?)
}

pub fn build() -> Result<TestMod> {
    let attached = AttachedScript {
        name: SCRIPT.into(),
        properties: ["Box", "Notification", "Help"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name.into(), PropertyValue::Object(own(0x801 + i as u32))))
            .collect(),
    };
    let mut dnam = 0x0011u16.to_le_bytes().to_vec();
    dnam.extend_from_slice(&[0; 10]);
    let quest = RecordBuilder::new(b"QUST", own(0x800))
        .zstring(b"EDID", "VrmControlTagsQuest")
        .field(b"VMAD", &vmad(&[attached]))
        .zstring(b"FULL", "Control tags visual probe")
        .field(b"DNAM", &dnam)
        .field(b"NEXT", &[])
        .u32(b"ANAM", 0);
    let messages = ["Box", "Notification", "Help"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let mut r = RecordBuilder::new(b"MESG", own(0x801 + i as u32))
                .zstring(b"EDID", &format!("VrmControlTags{name}"))
                .zstring(b"DESC", &format!("{name}: {TAGS}"))
                .u32(b"DNAM", u32::from(i == 0))
                .u32(b"TNAM", 10);
            if i == 0 {
                r = r
                    .zstring(b"FULL", "Control tags probe")
                    .zstring(b"ITXT", "Continue");
            }
            r
        })
        .collect();
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Manual control-tag screenshots; cqf VrmControlTagsQuest Probe, close console.",
            0x804,
            &[
                Node::top(b"QUST", vec![quest]),
                Node::top(b"MESG", messages),
            ],
        ),
        scripts: vec![(SCRIPT.into(), script()?)],
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_with_vanilla_natives() {
        let m = super::build().unwrap();
        let p = papyrus::pex::parse(&m.scripts[0].1).unwrap();
        assert_eq!(p.str(p.objects[0].name), super::SCRIPT);
        assert!(super::super::script::mistyped_calls(&p).is_empty());
        assert!(!m.plugin.is_empty());
        assert_eq!(super::TAGS.matches('[').count(), 6);
    }
}
