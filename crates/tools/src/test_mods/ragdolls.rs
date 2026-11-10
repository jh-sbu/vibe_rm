//! Vanilla-only, manually stepped ragdoll observations. Never installs or runs SE.
use super::{TestMod, own, records, script};
use anyhow::{Context, Result};
use esp::LoadOrder;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use records::spell;
const SOURCE: &str = include_str!("ragdolls/ragdolls.psc");

fn state_probe_effect(object: u32, edid: &str) -> RecordBuilder {
    // MGEF DATA layout: public xEdit definition, also read by MagicEffect::load.
    // Script archetype without VMAD: a timed marker with no actor-value effect.
    let mut data = [0u8; 0x98];
    data[0..4].copy_from_slice(&0x802u32.to_le_bytes()); // Recover, Hide in UI
    for offset in [0x0c, 0x10, 0x44, 0x58] {
        // skill, resistance, primary/secondary AV
        data[offset..offset + 4].copy_from_slice(&(-1i32).to_le_bytes());
    }
    data[0x40..0x44].copy_from_slice(&1u32.to_le_bytes()); // Script archetype
    data[0x50..0x54].copy_from_slice(&spell::FIRE_AND_FORGET.to_le_bytes());
    data[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
    RecordBuilder::new(b"MGEF", own(object))
        .zstring(b"EDID", edid)
        .field(b"DATA", &data)
}

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
            magic.push(state_probe_effect(object, &format!("VrmRag{prefix}{n}")));
            props.push((format!("{prefix}{n}"), PropertyValue::Object(own(object))));
            effects.push(
                records::Effect::new(own(object), 0.0, 1).when(records::ctda(
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
    fn state_probe_is_a_timed_script_marker_without_actor_values_or_scripts() {
        let bytes = super::state_probe_effect(0x810, "VrmRagState0").to_bytes();
        let record = esp::Record::from_raw(&bytes).unwrap();
        let data = record.get(b"DATA").unwrap();
        let word = |o| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
        assert_eq!(word(0x40), 1, "Script archetype");
        for offset in [0x0c, 0x10, 0x44, 0x58] {
            assert_eq!(
                word(offset),
                u32::MAX,
                "no skill, resistance or actor value"
            );
        }
        assert_eq!(word(0) & 0x200, 0, "duration must remain enabled");
        assert!(record.get(b"VMAD").is_none());
        assert_eq!(word(0x68), 0, "no skill XP");
    }

    #[test]
    fn script_is_vanilla_and_compiles() {
        assert_eq!(super::script::compile(&[super::SOURCE]).unwrap().len(), 1);
    }
}
