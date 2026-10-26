use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!(
        "vrm-spell-fx-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut data = vec![0; 400];
    for (offset, value) in [
        (0x14, 0.25f32),
        (0x18, 1.0),
        (0x1C, 0.5),
        (0x54, 1.0),
        (0x158, 1.0),
    ] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut mgef = vec![0; 0x8C];
    for offset in [0x18, 0x20, 0x24, 0x5C, 0x60, 0x6C, 0x74, 0x78, 0x7C] {
        mgef[offset..offset + 4].copy_from_slice(&0x800u32.to_le_bytes());
    }
    mgef[0x70..0x74].copy_from_slice(&1.75f32.to_le_bytes());
    let groups = [
        Node::top(
            b"EFSH",
            vec![RecordBuilder::new(b"EFSH", FormId(0x800)).field(b"DATA", &data)],
        ),
        Node::top(
            b"RFCT",
            vec![
                RecordBuilder::new(b"RFCT", FormId(0x801))
                    .field(b"DATA", &[0, 0, 0, 0, 0, 8, 0, 0, 4, 0, 0, 0]),
            ],
        ),
        Node::top(
            b"MGEF",
            vec![
                RecordBuilder::new(b"MGEF", FormId(0x802)).field(b"DATA", &mgef),
                RecordBuilder::new(b"MGEF", FormId(0x803)).field(b"DATA", &mgef[..0x6C]),
            ],
        ),
    ];
    std::fs::write(
        dir.join("Fx.esp"),
        write_plugin(0, &[], "test", "synthetic", 0x900, &groups),
    )
    .unwrap();
    std::fs::write(
        dir.join("Empty.esp"),
        write_plugin(0, &[], "test", "", 0x800, &[]),
    )
    .unwrap();
    Fixture(dir)
}
#[test]
fn mgef_visual_fields_remap_and_optional_tail_stays_null() {
    let f = fixture();
    let lo = esp::LoadOrder::load(&f.0, &["Empty.esp".into(), "Fx.esp".into()]).unwrap();
    let effect = crate::magic::MagicEffect::load_from(&lo, FormId(0x01000802)).unwrap();
    for id in [
        effect.casting_light,
        effect.hit_shader,
        effect.enchant_shader,
        effect.casting_art,
        effect.hit_art,
        effect.dual_cast.0,
        effect.enchant_art,
        effect.hit_visuals,
        effect.enchant_visuals,
    ] {
        assert_eq!(id, FormId(0x01000800));
    }
    assert_eq!(effect.dual_cast.1, 1.75);
    let old = crate::magic::MagicEffect::load_from(&lo, FormId(0x01000803)).unwrap();
    assert_eq!(old.dual_cast, (FormId::NULL, 0.0));
    assert_eq!(old.enchant_art, FormId::NULL);
}
#[test]
#[ignore = "requires a headless GPU adapter"]
fn script_natives_play_refresh_and_stop_shader_instances() {
    let f = fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let shader = engine.object_value(FormId(0x800));
    let visual = engine.object_value(FormId(0x801));
    let target = engine.object_value(PLAYER_REF);
    crate::script::natives::call(
        &mut engine,
        "effectshader",
        "play",
        Some(&shader),
        &[target.clone(), papyrus::Value::Float(-1.0)],
    );
    let key = (PLAYER_REF, FormId(0x800), 0);
    assert!(engine.spell_fx.shaders[&key].duration.is_infinite());
    crate::script::natives::call(
        &mut engine,
        "effectshader",
        "play",
        Some(&shader),
        &[target.clone(), papyrus::Value::Float(2.0)],
    );
    assert_eq!(engine.spell_fx.shaders.len(), 1);
    assert_eq!(engine.spell_fx.shaders[&key].duration, 2.0);
    crate::script::natives::call(
        &mut engine,
        "effectshader",
        "stop",
        Some(&shader),
        &[target.clone()],
    );
    assert_eq!(engine.spell_fx.shaders[&key].stopped, Some(0.0));
    crate::script::natives::call(
        &mut engine,
        "visualeffect",
        "play",
        Some(&visual),
        &[target.clone(), papyrus::Value::Float(-1.0)],
    );
    let visual_key = (PLAYER_REF, FormId(0x801), 0);
    assert_eq!(engine.spell_fx.shaders.len(), 2);
    assert!(engine.spell_fx.shaders[&key].stopped.is_some());
    assert_eq!(engine.spell_fx.shaders[&visual_key].stopped, None);
    crate::script::natives::call(
        &mut engine,
        "visualeffect",
        "stop",
        Some(&visual),
        &[target],
    );
    assert!(engine.spell_fx.shaders[&visual_key].stopped.is_some());
}
#[test]
fn shader_lifetime_fades_after_dispel_and_one_shot_expires() {
    let f = fixture();
    let lo = esp::LoadOrder::load(&f.0, &["Fx.esp".into()]).unwrap();
    let shader = records::EffectShader::load(&lo, FormId(0x800)).unwrap();
    let mut persistent = shaders::ShaderEffect::new(shader.clone(), f32::INFINITY, true);
    assert!(persistent.advance(1.0, true));
    assert!(persistent.stopped.is_none());
    assert!(persistent.advance(0.0, false));
    assert_eq!(persistent.stopped, Some(1.0));
    assert!(!persistent.advance(0.6, false));
    let mut once = shaders::ShaderEffect::new(shader, 0.1, false);
    assert!(once.advance(0.1, true));
    assert!(once.stopped.is_some());
    assert!(!once.advance(0.6, true));
}
