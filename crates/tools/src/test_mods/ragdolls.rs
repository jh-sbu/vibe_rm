//! Vanilla-only, manually stepped ragdoll observations. Never installs or runs SE.
use super::{TestMod, own, records, script};
use anyhow::{Context, Result};
use esp::LoadOrder;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use records::spell;
const SOURCE: &str = include_str!("ragdolls/ragdolls.psc");
pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let vanilla = |name: &str| {
        lo.find_editor_id(name)
            .with_context(|| format!("Skyrim.esm {name}"))
    };
    let mut props = vec![
        ("Run".into(), PropertyValue::Object(own(0x801))),
        ("Probe".into(), PropertyValue::Object(own(0x802))),
        (
            "Base".into(),
            PropertyValue::Object(vanilla("EncBandit01Melee1HOrcM")?),
        ),
        (
            "ImmuneBase".into(),
            PropertyValue::Object(vanilla("EncAtronachFlame")?),
        ),
        (
            "MammothBase".into(),
            PropertyValue::Object(vanilla("EncMammothWildNoAggro")?),
        ),
        (
            "CenturionBase".into(),
            PropertyValue::Object(vanilla("EncDwarvenCenturion01")?),
        ),
        ("Paralyze1".into(), PropertyValue::Object(own(0x804))),
        ("Paralyze5".into(), PropertyValue::Object(own(0x805))),
    ];
    let mut effects = Vec::new();
    let mut magic = Vec::new();
    for n in 0..9 {
        for (func, prefix, offset) in [(725, "State", 0x810), (107, "Legacy", 0x820)] {
            let object = offset + n;
            magic.push(records::copy(
                lo,
                "AlchFortifyOneHanded",
                object,
                &format!("VrmRag{prefix}{n}"),
                None,
                |tag, d| {
                    if tag == b"DATA" && d.len() >= 0x58 {
                        d[0x44..0x48].copy_from_slice(&6u32.to_le_bytes());
                        d[0x50..0x54].copy_from_slice(&spell::FIRE_AND_FORGET.to_le_bytes());
                        d[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
                    }
                    if tag == b"CTDA" || tag == b"VMAD" {
                        d.clear();
                    }
                },
            )?);
            props.push((format!("{prefix}{n}"), PropertyValue::Object(own(object))));
            effects.push(
                records::Effect::new(own(object), 1.0, 1).when(records::ctda(
                    func,
                    0,
                    0,
                    records::op::EQ,
                    n as f32,
                    records::run_on::SUBJECT,
                )),
            );
        }
    }
    let probe = records::spell(
        0x802,
        "VrmRagStateProbe",
        spell::SPELL,
        spell::FIRE_AND_FORGET,
        spell::SELF,
        0,
        None,
        &effects,
    );
    let paralysis = vanilla("ParalysisFFAimed")?;
    let paralyses = [(0x804, 1), (0x805, 5)]
        .into_iter()
        .map(|(id, duration)| {
            records::spell(
                id,
                &format!("VrmRagParalyze{duration}"),
                spell::SPELL,
                spell::FIRE_AND_FORGET,
                spell::TARGET_ACTOR,
                0,
                None,
                &[records::Effect::new(paralysis, 1.0, duration)],
            )
        })
        .collect::<Vec<_>>();
    let quest = records::Quest {
        object_id: 0x800,
        edid: "VrmRagdolls",
        flags: 0x11,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: "VrmRagdollsScript".into(),
            properties: props,
        }),
        stages: &[],
        aliases: vec![],
    }
    .record();
    let run = RecordBuilder::new(b"GLOB", own(0x801))
        .zstring(b"EDID", "VrmRagRun")
        .field(b"FNAM", b"s")
        .field(b"FLTV", &0f32.to_le_bytes());
    let mut spells = vec![probe];
    spells.extend(paralyses);
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Living ragdoll observations",
            0x830,
            &[
                Node::top(b"QUST", vec![quest]),
                Node::top(b"GLOB", vec![run]),
                Node::top(b"MGEF", magic),
                Node::top(b"SPEL", spells),
            ],
        ),
        scripts: script::compile(&[SOURCE])?,
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn script_is_vanilla_and_compiles() {
        assert_eq!(super::script::compile(&[super::SOURCE]).unwrap().len(), 1);
    }
}
