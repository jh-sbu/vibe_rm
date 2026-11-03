//! Manual vanilla SE probe; never sends animation events or uses SKSE.
//! Reports are bounded to twenty event lines per phase and read from saves.
use anyhow::{Context, Result};
use esp::LoadOrder;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};

use super::records::{self, spell};
use super::{TestMod, own, script};

const SOURCE: &str = include_str!("firstperson/firstperson.psc");

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let effect = records::copy(
        lo,
        "AlchFortifyOneHanded",
        0x802,
        "VrmFpProbeEffect",
        None,
        |tag, d| {
            if tag == b"DATA" && d.len() >= 0x58 {
                d[0x44..0x48].copy_from_slice(&6u32.to_le_bytes());
                d[0x50..0x54].copy_from_slice(&spell::CONSTANT.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
            }
            if tag == b"CTDA" {
                d.clear();
            }
        },
    )?;
    let probe = records::spell(
        0x803,
        "VrmFpProbe",
        spell::ABILITY,
        spell::CONSTANT,
        spell::SELF,
        0,
        None,
        &[records::Effect::new(own(0x802), 3.0, 0).when(records::ctda(
            391,
            0,
            0,
            records::op::EQ,
            1.0,
            records::run_on::SUBJECT,
        ))],
    );
    let quest = records::Quest {
        object_id: 0x800,
        edid: "VrmFirstPerson",
        flags: 0x11,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: "VrmFirstPersonScript".into(),
            properties: vec![
                ("Step".into(), PropertyValue::Object(own(0x801))),
                ("Probe".into(), PropertyValue::Object(own(0x803))),
                ("Chair".into(), PropertyValue::Object(own(0x805))),
            ],
        }),
        stages: &[],
        aliases: Vec::new(),
    }
    .record();
    let step = RecordBuilder::new(b"GLOB", own(0x801))
        .zstring(b"EDID", "VrmFirstPersonStep")
        .field(b"FNAM", b"s")
        .field(b"FLTV", &0f32.to_le_bytes());
    let keyword = lo
        .default_objects()
        .get(b"FFFP")
        .copied()
        .filter(|f| !f.is_null())
        .context("Skyrim.esm FFFP default keyword")?;
    let furniture = records::copy(lo, "CommonChair01", 0x805, "VrmFpChair", None, |tag, d| {
        if tag == b"KWDA" || tag == b"KSIZ" {
            d.clear();
        }
    })?
    .field(b"KSIZ", &1u32.to_le_bytes())
    .form(b"KWDA", keyword);
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "First-person event and camera probe",
            0x806,
            &[
                Node::top(b"QUST", vec![quest]),
                Node::top(b"GLOB", vec![step]),
                Node::top(b"MGEF", vec![effect]),
                Node::top(b"SPEL", vec![probe]),
                Node::top(b"FURN", vec![furniture]),
            ],
        ),
        scripts: script::compile(&[SOURCE])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vanilla_probe_script_compiles() {
        let scripts = script::compile(&[SOURCE]).unwrap();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].0, "VrmFirstPersonScript");
    }
}
