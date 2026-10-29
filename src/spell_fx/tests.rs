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
    let mut projectile = [0u8; 84];
    projectile[76..80].copy_from_slice(&0.5f32.to_le_bytes());
    projectile[80..84].copy_from_slice(&0.05f32.to_le_bytes());
    let groups = [
        Node::top(
            b"PROJ",
            vec![
                RecordBuilder::new(b"PROJ", FormId(0x804))
                    .field(b"DATA", &projectile)
                    .zstring(b"MODL", "missing-stream.nif"),
            ],
        ),
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

#[test]
fn stream_start_uses_requested_hand_and_preserves_supplied_fallback() {
    use crate::casting::CastSource;
    let supplied = Vec3::new(1.0, 2.0, 30.0);
    let mut frame = ActorFrame {
        transform: Mat4::from_translation(Vec3::X * 100.0),
        bones: HashMap::from([
            (
                "NPC L MagicNode [LMag]".into(),
                Mat4::from_translation(Vec3::Y),
            ),
            (
                "NPC R MagicNode [RMag]".into(),
                Mat4::from_translation(-Vec3::Y),
            ),
        ])
        .into(),
    };
    assert_eq!(
        stream_start(Some(&frame), CastSource::Left, supplied),
        Vec3::X * 100.0 + Vec3::Y
    );
    assert_eq!(
        stream_start(Some(&frame), CastSource::Right, supplied),
        Vec3::X * 100.0 - Vec3::Y
    );
    for source in [CastSource::Dual, CastSource::Other] {
        assert_eq!(stream_start(Some(&frame), source, supplied), supplied);
    }
    std::sync::Arc::make_mut(&mut frame.bones).clear();
    for source in [CastSource::Left, CastSource::Right] {
        assert_eq!(stream_start(Some(&frame), source, supplied), supplied);
        assert_eq!(stream_start(None, source, supplied), supplied);
    }
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn held_streams_refresh_per_hand_and_expire_without_relaunching() {
    use crate::casting::CastSource;
    // Not the player, whose streams follow the camera's aim.
    const NPC: FormId = FormId(0x0100_0950);
    let f = fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let proj = FormId(0x804);
    let (left, right) = (
        (NPC, proj, CastSource::Left),
        (NPC, proj, CastSource::Right),
    );
    engine.request_spell_stream(NPC, proj, Vec3::X, Vec3::Y, 384.0, 768.0, CastSource::Left);
    engine.request_spell_stream(
        NPC,
        proj,
        -Vec3::X,
        Vec3::Y,
        900.0,
        768.0,
        CastSource::Right,
    );
    assert_eq!(engine.spell_fx.streams.len(), 2);
    assert_eq!(engine.spell_fx.streams[&left].length, 0.5);
    assert_eq!(engine.spell_fx.streams[&right].length, 1.0);
    engine.update_stream_fx(0.2);
    engine.request_spell_stream(NPC, proj, Vec3::Z, Vec3::X, 0.0, 768.0, CastSource::Left);
    let streams = &engine.spell_fx.streams;
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[&left].from, Vec3::Z);
    assert_eq!(streams[&left].dir, Vec3::X);
    assert_eq!(streams[&left].length, 0.0);
    assert_eq!(streams[&right].from, -Vec3::X);
    assert!(streams[&right].remaining < streams[&left].remaining);
    engine.update_stream_fx(0.15);
    assert_eq!(engine.spell_fx.streams.len(), 1);
    assert!(engine.spell_fx.streams.contains_key(&left));
    engine.update_stream_fx(0.2);
    assert!(engine.spell_fx.streams.is_empty());
    assert!(engine.spell_fx.waves.is_empty());
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn cone_waves_travel_their_range_then_stop_where_they_are() {
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    engine.spell_fx.waves.push(ConeWave {
        effect: lifecycle_effect(1.0),
        velocity: Vec3::X * 100.0,
        left: 150.0,
    });
    engine.update_stream_fx(1.0);
    assert_eq!(engine.spell_fx.waves[0].effect.transform.w_axis.x, 100.0);
    engine.update_stream_fx(1.0);
    assert!(engine.spell_fx.waves.is_empty());
    let tail = engine.spell_fx.tails.last().unwrap();
    assert!(tail.stopped);
    assert_eq!(tail.transform.w_axis.x, 150.0);
    engine.update_spell_fx(1.0);
    assert_eq!(engine.spell_fx.tails[0].transform.w_axis.x, 150.0);
}

#[test]
fn actor_pose_cache_reuses_maps_and_invalidates_without_changing_live_frames() {
    use crate::world::skeleton::{Bone, Skeleton};
    use std::sync::Arc;
    let skeleton = Arc::new(Skeleton {
        bones: vec![Bone {
            name: "hand".into(),
            parent: None,
            bind: nif::Transform::default(),
        }],
        ..Default::default()
    });
    let mut cache = CachedActorPose::default();
    let first = cache.bones(&skeleton, &[Mat4::IDENTITY]);
    let same = cache.bones(&skeleton, &[Mat4::IDENTITY]);
    assert!(Arc::ptr_eq(&first, &same));
    let moved = Mat4::from_translation(Vec3::X);
    let changed = cache.bones(&skeleton, &[moved]);
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(first["hand"], Mat4::IDENTITY);
    assert_eq!(changed["hand"], moved);

    let mut replacement = (*skeleton).clone();
    replacement.bones[0].name = "other".into();
    let renamed = cache.bones(&Arc::new(replacement), &[moved]);
    assert!(!renamed.contains_key("hand"));
    assert_eq!(renamed["other"], moved);
    let truncated = cache.bones(&skeleton, &[]);
    assert!(truncated.is_empty());
}

#[test]
fn drawn_enchant_targets_only_its_blade_and_respects_flag_context() {
    use crate::render::{ActorInstance, GpuModel, membrane::shader_target};
    use std::sync::Arc;
    let model = |path: &str| {
        Arc::new(GpuModel {
            path: path.into(),
            parts: Vec::new(),
            skinned: Vec::new(),
            particles: Vec::new(),
            bound_center: Vec3::ZERO,
            bound_radius: 1.0,
        })
    };
    let blade = model("meshes/weapons/sword.nif#blade");
    let mut actor = ActorInstance {
        meshes: Vec::new(),
        attachments: vec![(model("meshes/armor/helmet.nif"), 0, Mat4::IDENTITY)],
        equipment: vec![
            (blade.clone(), 1, Mat4::IDENTITY),
            (blade.clone(), 2, Mat4::IDENTITY),
            (model("meshes/weapons/sword.nif#scb"), 0, Mat4::IDENTITY),
        ],
        held_light: None,
        transform: Mat4::IDENTITY,
        pose: vec![Mat4::IDENTITY; 3],
        lights: [0; 8],
        radius: 1.0,
        shadow_only: false,
        skin_tone: None,
        weight: 0.5,
    };
    let target = shaders::WeaponTarget {
        form: FormId(0x900),
        hand: crate::spells::RIGHT,
        path: blade.path.clone(),
        bone: 1,
    };
    for flags in [0, 1 << 10] {
        let selected = shader_target(&actor, flags, Some(&target));
        assert!(selected.meshes.is_empty());
        assert_eq!(selected.attachments.len(), 1);
        assert_eq!(selected.attachments[0].1, 1);
        assert!(Arc::ptr_eq(&selected.attachments[0].0, &blade));
    }
    assert!(
        shader_target(&actor, 1 << 5, Some(&target))
            .attachments
            .is_empty()
    );
    let body_only = shader_target(&actor, 1 << 10, None);
    assert_eq!(body_only.attachments.len(), 1);
    assert!(body_only.attachments[0].0.path.contains("helmet"));
    assert_eq!(shader_target(&actor, 0, None).attachments.len(), 4);
    let mut changed = target.clone();
    changed.form = FormId(0x901);
    assert_ne!(target, changed);
    actor.equipment[0] = (model("meshes/weapons/sword.nif"), 0, Mat4::IDENTITY);
    assert!(
        shader_target(&actor, 0, Some(&target))
            .attachments
            .is_empty()
    );
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn membrane_materials_reuse_gpu_bindings_while_uniforms_advance() {
    let f = fixture();
    let engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let shader = records::EffectShader::load(&engine.lo, FormId(0x800)).unwrap();
    let first = crate::render::membrane::material(&shader, 0.0, None);
    let material = engine.renderer.create_material(&first);
    let binding = material.bind_group.clone();
    let later = crate::render::membrane::material(&shader, 0.2, None);
    engine.renderer.update_membrane_material(&material, &later);
    assert_eq!(material.bind_group, binding);
}

fn lifecycle_fixture() -> Fixture {
    let f = fixture();
    let lo = esp::LoadOrder::load(&f.0, &["Fx.esp".into()]).unwrap();
    let shader = lo
        .get(FormId(0x800))
        .unwrap()
        .get(b"DATA")
        .unwrap()
        .to_vec();
    let mut persistent = vec![0u8; 0x8C];
    persistent[..4].copy_from_slice(&crate::magic::flags::FX_PERSIST.to_le_bytes());
    persistent[0x20..0x24].copy_from_slice(&0x800u32.to_le_bytes());
    persistent[0x60..0x64].copy_from_slice(&0x810u32.to_le_bytes());
    let mut once = persistent.clone();
    once[..4].fill(0);
    let mut light = vec![0u8; 32];
    light[4..8].copy_from_slice(&128u32.to_le_bytes());
    light[8..11].copy_from_slice(&[255, 128, 64]);
    std::fs::write(
        f.0.join("Fx.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "lifecycle",
            0x900,
            &[
                Node::top(
                    b"EFSH",
                    vec![RecordBuilder::new(b"EFSH", FormId(0x800)).field(b"DATA", &shader)],
                ),
                Node::top(
                    b"MGEF",
                    vec![
                        RecordBuilder::new(b"MGEF", FormId(0x802)).field(b"DATA", &persistent),
                        RecordBuilder::new(b"MGEF", FormId(0x803)).field(b"DATA", &once),
                    ],
                ),
                Node::top(
                    b"LIGH",
                    vec![RecordBuilder::new(b"LIGH", FormId(0x820)).field(b"DATA", &light)],
                ),
            ],
        ),
    )
    .unwrap();
    f
}

fn lifecycle_hit(id: u64, target: FormId, persistent: bool) -> crate::magic::ActiveEffect {
    crate::magic::ActiveEffect {
        id,
        target,
        caster: Some(PLAYER_REF),
        item: FormId::NULL,
        equipment: None,
        index: 0,
        effect: FormId(if persistent { 0x802 } else { 0x803 }),
        magnitude: 1.0,
        duration: 10.0,
        elapsed: 0.0,
        active: true,
        held: [0.0; 2],
        scripted: false,
        done: false,
        quiet: false,
        at: None,
        summoned: None,
        dual: false,
        channelled: false,
    }
}

fn lifecycle_effect(length: f32) -> EffectInstance {
    let anim = crate::world::loader::ModelAnim {
        parts: Vec::new(),
        sequences: vec![crate::render::model::Sequence {
            name: "Idle".into(),
            cycle: 0,
            start: 0.0,
            stop: length,
            frequency: 1.0,
            channels: Vec::new(),
        }],
        controllers: Vec::new(),
        start: 0.0,
        graph: None,
    };
    EffectInstance::from_parts(
        Mat4::IDENTITY,
        None,
        Some(std::sync::Arc::new(anim)),
        Vec::new(),
        f32::INFINITY,
    )
}

fn lifecycle_art(owners: Vec<u64>, persistent: bool) -> ArtEffect {
    ArtEffect {
        effect: lifecycle_effect(0.2),
        default_node: "MagicEffectsNode".into(),
        owners,
        persistent,
        flags: 4,
        facing: None,
        scale: 1.0,
        stopping_age: None,
    }
}

fn load_lifecycle_target(engine: &mut Engine, target: FormId) {
    let model = std::sync::Arc::new(crate::render::GpuModel {
        path: "synthetic.nif".into(),
        parts: Vec::new(),
        skinned: Vec::new(),
        particles: Vec::new(),
        bound_center: Vec3::ZERO,
        bound_radius: 1.0,
    });
    let mut instance = crate::render::Instance::new(model, Mat4::IDENTITY);
    instance.ref_id = target.0;
    engine
        .scene
        .cells
        .entry(crate::render::CellKey::Interior(FormId(0x900)))
        .or_default()
        .instances
        .push(instance);
}

#[test]
fn art_one_shot_uses_actual_subsecond_length_without_minimum_or_mesh_tail() {
    let mut fx = lifecycle_effect(0.2);
    fx.advance(0.19);
    assert!(!art_one_shot_done(&fx));
    fx.advance(0.02);
    assert!(art_one_shot_done(&fx));
    std::sync::Arc::get_mut(fx.anim.as_mut().unwrap())
        .unwrap()
        .sequences[0]
        .frequency = -2.0;
    fx.age = 0.1;
    assert!(art_one_shot_done(&fx));
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn hit_observation_survives_unload_and_only_persistent_shaders_resume() {
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let target = FormId(0x901);
    load_lifecycle_target(&mut engine, target);
    engine.magic.effects = vec![
        lifecycle_hit(1, target, false),
        lifecycle_hit(2, target, true),
    ];
    engine.update_spell_fx(0.1);
    let once = (target, FormId(0x800), 1);
    let persistent = (target, FormId(0x800), 2);
    assert_eq!(engine.spell_fx.shaders.len(), 2);
    engine.update_spell_fx(0.1);
    assert_eq!(engine.spell_fx.shaders[&persistent].age, 0.2);
    engine.scene.cells.clear();
    engine.update_spell_fx(0.1);
    assert!(engine.spell_fx.shaders.is_empty());
    assert_eq!(engine.spell_fx.observed.len(), 2);
    load_lifecycle_target(&mut engine, target);
    engine.update_spell_fx(0.1);
    assert!(!engine.spell_fx.shaders.contains_key(&once));
    assert!(engine.spell_fx.shaders.contains_key(&persistent));
    engine.magic.effects[1].done = true;
    engine.update_spell_fx(0.0);
    assert!(engine.spell_fx.shaders[&persistent].stopped.is_some());
    assert!(!engine.spell_fx.observed.contains(&2));
    engine.update_spell_fx(0.6);
    assert!(engine.spell_fx.shaders.is_empty());
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn unavailable_new_hits_are_observed_without_deferred_one_shot_replay() {
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let target = FormId(0x901);
    engine.magic.effects = vec![
        lifecycle_hit(1, target, false),
        lifecycle_hit(2, target, true),
    ];
    engine.spell_fx.hits.push(engine.magic.effects[0].clone());
    engine.update_spell_fx(0.0);
    assert_eq!(engine.spell_fx.observed.len(), 2);
    load_lifecycle_target(&mut engine, target);
    engine.update_spell_fx(0.0);
    assert_eq!(engine.spell_fx.shaders.len(), 1);
    assert!(
        engine
            .spell_fx
            .shaders
            .contains_key(&(target, FormId(0x800), 2))
    );
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn hit_art_shared_owners_dispel_one_shot_completion_refresh_and_summon_unload() {
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let target = FormId(0x901);
    load_lifecycle_target(&mut engine, target);
    let key = (target, ArtKey::Hit(FormId(0x810)));
    let mut summon = lifecycle_hit(1, target, true);
    summon.summoned = Some(FormId(0x902));
    engine.magic.effects = vec![summon, lifecycle_hit(2, target, true)];
    engine
        .spell_fx
        .art
        .insert(key, lifecycle_art(vec![1], true));
    engine.update_spell_fx(0.1);
    assert_eq!(engine.spell_fx.art[&key].owners, vec![1, 2]);
    engine.scene.cells.clear();
    engine.update_spell_fx(0.0);
    assert!(!engine.spell_fx.art.contains_key(&key));
    load_lifecycle_target(&mut engine, target);
    engine
        .spell_fx
        .art
        .insert(key, lifecycle_art(Vec::new(), true));
    engine.update_spell_fx(0.1);
    assert_eq!(engine.spell_fx.art[&key].owners, vec![1, 2]);
    engine.magic.effects[0].done = true;
    engine.update_spell_fx(0.1);
    assert_eq!(engine.spell_fx.art[&key].owners, vec![2]);
    assert!(!engine.spell_fx.art[&key].effect.stopped);
    engine.magic.effects[1].active = false;
    engine.update_spell_fx(0.0);
    assert!(!engine.spell_fx.art.contains_key(&key));
    assert_eq!(engine.spell_fx.tails.len(), 1);
    let stopped = &engine.spell_fx.tails[0];
    assert!(stopped.stopped && stopped.finished);
    assert_eq!(stopped.duration, 0.2);
    assert_eq!(stopped.fade_duration, 3.0);
    engine.scene.cells.clear();
    engine.update_spell_fx(0.0);
    assert!(engine.spell_fx.art.is_empty());
    assert!(engine.spell_fx.tails[0].stopped);
    // Finish the retained persistent fade before checking the next one-shot's tails.
    engine.update_spell_fx(3.1);
    load_lifecycle_target(&mut engine, target);
    engine.magic.effects = vec![lifecycle_hit(3, target, false)];
    engine
        .spell_fx
        .art
        .insert(key, lifecycle_art(vec![3], false));
    engine.spell_fx.observed.insert(3);
    engine.update_spell_fx(0.21);
    assert!(!engine.spell_fx.art.contains_key(&key));
    assert!(
        engine
            .spell_fx
            .tails
            .iter()
            .all(|fx| fx.finished && fx.fade_duration == 0.0)
    );
    engine.update_spell_fx(0.1);
    assert!(!engine.spell_fx.art.contains_key(&key));
    engine
        .spell_fx
        .art
        .insert(key, lifecycle_art(vec![3], false));
    engine.magic.effects.push(lifecycle_hit(4, target, false));
    engine.update_spell_fx(0.0);
    assert!(!engine.spell_fx.art.contains_key(&key));
    assert!(engine.spell_fx.observed.contains(&4));
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn projectile_pose_light_and_stop_follow_motion_and_lifetime_end() {
    use crate::ai::archery::{Arrow, Projectile};
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let mut fx = lifecycle_effect(1.0);
    fx.light = FormId(0x820);
    fx.light_attach = Vec3::Y * 2.0;
    engine.projectiles.push(Projectile {
        shooter: PLAYER_REF,
        arrow: Arrow {
            ammo: FormId::NULL,
            projectile: FormId::NULL,
            damage: 0.0,
            speed: 10.0,
            gravity: 0.0,
            range: 100.0,
            model: None,
        },
        pos: Vec3::splat(1000.0),
        vel: Vec3::X * 10.0,
        damage: 0.0,
        model: None,
        fx: Some(fx),
        age: 0.0,
        stuck: None,
        dir: Vec3::X,
        spell: None,
        dual: false,
        poison: None,
        lifetime: 10.0,
    });
    engine.update_projectiles(0.1);
    engine.update_spell_fx(0.0);
    let projectile = &engine.projectiles[0];
    let fx = projectile.fx.as_ref().unwrap();
    assert!(
        fx.transform
            .transform_vector3(Vec3::Y)
            .abs_diff_eq(Vec3::X, 1e-5)
    );
    assert_eq!(fx.transform.w_axis.truncate(), projectile.pos);
    let at = fx.transform.transform_point3(fx.light_attach);
    assert_eq!(
        engine.spell_fx.lights[0].pos_radius,
        [at.x, at.y, at.z, 128.0]
    );
    engine.projectiles[0].vel = Vec3::Z * 10.0;
    engine.update_projectiles(0.1);
    engine.update_spell_fx(0.0);
    assert!(
        engine.projectiles[0]
            .fx
            .as_ref()
            .unwrap()
            .transform
            .transform_vector3(Vec3::Y)
            .abs_diff_eq(Vec3::Z, 1e-5)
    );
    engine.projectiles[0].lifetime = 0.0;
    engine.update_projectiles(0.1);
    assert!(engine.projectiles.is_empty());
    assert_eq!(engine.spell_fx.tails.len(), 1);
    assert!(engine.spell_fx.tails[0].stopped);
    engine.update_spell_fx(0.0);
    assert!(engine.spell_fx.lights.is_empty());
}

#[test]
fn sequence_less_art_uses_controller_span_and_stop_uses_same_lifetime() {
    let mut fx = lifecycle_effect(0.2);
    let anim = std::sync::Arc::get_mut(fx.anim.as_mut().unwrap()).unwrap();
    anim.sequences.clear();
    anim.start = 5.0;
    anim.controllers = vec![crate::render::model::NodeChannel {
        node: "synthetic".into(),
        block: 0,
        timing: nif::anim::Timing {
            flags: 12,
            frequency: -2.0,
            phase: 0.0,
            start: 5.0,
            stop: 5.5,
        },
        drive: crate::render::model::NodeDrive::Visibility {
            value: true,
            keys: None,
        },
    }];
    assert_eq!(fx.one_shot_duration(), 0.25);
    fx.advance(0.24);
    assert!(!art_one_shot_done(&fx));
    fx.stop();
    assert!(!fx.finished);
    fx.stopped = false;
    fx.duration = f32::INFINITY;
    fx.advance(0.02);
    assert!(art_one_shot_done(&fx));
    fx.stop();
    assert!(fx.finished);

    let mut fx = lifecycle_effect(0.2);
    fx.advance(0.21);
    fx.stop();
    assert!(fx.finished);
}

#[test]
fn sequence_less_art_uses_material_and_particle_controller_lifetimes() {
    let timing = nif::anim::Timing {
        flags: 1, // APP_INIT controllers start with the effect.
        frequency: -2.0,
        phase: 0.0,
        start: 5.0,
        stop: 5.5,
    };
    let controllers = [
        nif::Block::ShaderController(nif::anim::ShaderController {
            next: nif::Ref(-1),
            timing,
            interpolator: nif::Ref(-1),
            variable: 5,
            color: false,
            lighting: false,
        }),
        nif::Block::AlphaTestController(nif::anim::AlphaTestController {
            next: nif::Ref(-1),
            timing,
            interpolator: nif::Ref(-1),
        }),
        nif::Block::ParticleController(nif::psys::ParticleController {
            next: nif::Ref(-1),
            timing,
            interpolator: nif::Ref(-1),
            modifier: String::new(),
            kind: nif::psys::ControllerKind::Update,
        }),
    ];
    for controller in controllers {
        let mut fx = lifecycle_effect(0.2);
        let anim = std::sync::Arc::get_mut(fx.anim.as_mut().unwrap()).unwrap();
        anim.sequences.clear();
        assert!(anim.controllers.is_empty());
        let mut nif = nif::Nif {
            header: nif::Header {
                version: 0x14020007,
                user_version: 12,
                bs_version: 100,
                block_types: Vec::new(),
                block_type_index: Vec::new(),
                block_sizes: Vec::new(),
                block_offsets: Vec::new(),
                strings: Vec::new(),
            },
            blocks: vec![controller],
            roots: Vec::new(),
        };
        fx.configure(&nif);
        assert_eq!(fx.one_shot_duration(), 0.25);
        fx.advance(0.24);
        assert!(!art_one_shot_done(&fx));
        fx.stop();
        assert!(!fx.finished);
        fx.stopped = false;
        fx.duration = f32::INFINITY;
        fx.advance(0.02);
        assert!(art_one_shot_done(&fx));
        fx.stop();
        assert!(fx.finished);

        // Inactive and sequence-owned controllers do not extend standalone art.
        for flags in [0, 1 | 0x20] {
            let t = match &mut nif.blocks[0] {
                nif::Block::ShaderController(c) => &mut c.timing,
                nif::Block::AlphaTestController(c) => &mut c.timing,
                nif::Block::ParticleController(c) => &mut c.timing,
                _ => unreachable!(),
            };
            t.flags = flags;
            fx.configure(&nif);
            assert_eq!(fx.one_shot_duration(), 1.0);
        }
    }
}

#[test]
fn static_art_fallback_handles_missing_empty_and_unusable_animation() {
    for span in [None, Some(0.0), Some(f32::INFINITY), Some(f32::NAN)] {
        let mut fx = lifecycle_effect(0.2);
        let anim = std::sync::Arc::get_mut(fx.anim.as_mut().unwrap()).unwrap();
        anim.sequences.clear();
        if let Some(stop) = span {
            anim.controllers.push(crate::render::model::NodeChannel {
                node: "synthetic".into(),
                block: 0,
                timing: nif::anim::Timing {
                    flags: 12,
                    frequency: 1.0,
                    phase: 0.0,
                    start: 0.0,
                    stop,
                },
                drive: crate::render::model::NodeDrive::Visibility {
                    value: true,
                    keys: None,
                },
            });
        }
        assert_eq!(fx.one_shot_duration(), 1.0);
        fx.advance(0.99);
        assert!(!art_one_shot_done(&fx));
        fx.advance(0.02);
        assert!(art_one_shot_done(&fx));
        fx.stop();
        assert!(fx.finished);
        fx.anim = None;
        assert_eq!(fx.one_shot_duration(), 1.0);
    }
}

#[test]
#[ignore = "requires a headless GPU adapter"]
fn static_hit_art_draws_on_first_update_and_expires_after_fallback() {
    let f = lifecycle_fixture();
    let mut engine = crate::stats::action_tests::headless_engine(&f.0, "Fx.esp");
    let target = FormId(0x901);
    load_lifecycle_target(&mut engine, target);
    let model = std::sync::Arc::new(engine.renderer.upload_model(
        &crate::render::model::CpuModel {
            meshes: vec![crate::render::model::CpuMesh {
                vertices: vec![bytemuck::Zeroable::zeroed(); 3],
                indices: vec![0, 1, 2],
                material: Default::default(),
                bound_center: Vec3::ZERO,
                bound_radius: 1.0,
            }],
            skinned: Vec::new(),
            bound_center: Vec3::ZERO,
            bound_radius: 1.0,
            animated: Vec::new(),
            sequences: Vec::new(),
            particles: Vec::new(),
            addons: Vec::new(),
        },
    ));
    let key = (target, ArtKey::Hit(FormId(0x810)));
    engine.magic.effects = vec![lifecycle_hit(1, target, false)];
    engine.spell_fx.observed.insert(1);
    let mut art = lifecycle_art(vec![1], false);
    art.effect = EffectInstance::from_parts(
        Mat4::IDENTITY,
        Some(crate::render::Instance::new(model.clone(), Mat4::IDENTITY)),
        None,
        Vec::new(),
        f32::INFINITY,
    );
    engine.spell_fx.art.insert(key, art);
    engine.update_spell_fx(0.016);
    assert!(engine.spell_fx.art.contains_key(&key));
    assert!(
        engine
            .scene
            .dynamic
            .iter()
            .any(|i| std::sync::Arc::ptr_eq(&i.model, &model))
    );
    engine.scene.dynamic.clear();
    engine.update_spell_fx(1.0);
    assert!(!engine.spell_fx.art.contains_key(&key));
    assert!(
        !engine
            .scene
            .dynamic
            .iter()
            .any(|i| std::sync::Arc::ptr_eq(&i.model, &model))
    );
}

#[test]
fn model_caches_are_bounded_and_reuse_existing_entries() {
    let mut fx = SpellFx::default();
    let mut shared = None;
    for i in 0..MODEL_CACHE_CAPACITY * 3 {
        let path = format!("model{i}.nif");
        assert!(cached_model(&mut fx.model_nifs, &path, || None).is_none());
        let nodes = cached_model(&mut fx.model_nodes.borrow_mut(), &path, || {
            std::sync::Arc::new(HashMap::new())
        })
        .clone();
        let reused = cached_model(&mut fx.model_nodes.borrow_mut(), &path, || {
            panic!("cached nodes were reloaded")
        })
        .clone();
        assert!(std::sync::Arc::ptr_eq(&nodes, &reused));
        cached_model(&mut fx.model_nifs, &path, || {
            panic!("cached NIF was reloaded")
        });
        assert!(fx.model_nifs.len() <= MODEL_CACHE_CAPACITY);
        assert!(fx.model_nodes.borrow().len() <= MODEL_CACHE_CAPACITY);
        shared = Some(nodes);
    }
    assert_eq!(fx.model_nifs.len(), MODEL_CACHE_CAPACITY);
    assert_eq!(fx.model_nodes.borrow().len(), MODEL_CACHE_CAPACITY);
    assert!(shared.unwrap().is_empty());
}

#[test]
fn actor_pose_pruning_retains_loaded_pose_storage() {
    let fx = SpellFx::default();
    let loaded = FormId(0x901);
    let unloaded = FormId(0x902);
    let bones = std::sync::Arc::new(HashMap::from([("bone".into(), Mat4::IDENTITY)]));
    fx.actor_poses.borrow_mut().insert(
        loaded,
        CachedActorPose {
            bones: bones.clone(),
            ..Default::default()
        },
    );
    fx.actor_poses
        .borrow_mut()
        .insert(unloaded, Default::default());
    fx.prune_actor_poses(&[loaded].into_iter().collect());
    let poses = fx.actor_poses.borrow();
    assert_eq!(poses.len(), 1);
    assert!(std::sync::Arc::ptr_eq(&poses[&loaded].bones, &bones));
    drop(poses);
    fx.prune_actor_poses(&Default::default());
    assert!(fx.actor_poses.borrow().is_empty());
}

#[test]
fn unchanged_active_effects_are_not_reenqueued_and_only_persistent_hits_resume() {
    let mut fx = SpellFx::default();
    let target = FormId(0x901);
    let mut effects = vec![
        lifecycle_hit(1, target, false),
        lifecycle_hit(2, target, true),
    ];
    let available = [target]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    fx.queue_active_hits(&effects, available.clone());
    assert_eq!(fx.hits.iter().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2]);
    fx.observed.extend([1, 2]);
    fx.persistent_hits.insert(2, target);
    fx.hits.clear();
    fx.queue_active_hits(&effects, available.clone());
    assert!(fx.hits.is_empty());
    fx.hits.push(effects[0].clone());
    fx.hits.push(effects[1].clone());
    fx.queue_active_hits(&effects, available.clone());
    assert!(fx.hits.is_empty());
    fx.queue_active_hits(&effects, Default::default());
    assert!(fx.hits.is_empty());
    fx.queue_active_hits(&effects, available.clone());
    assert_eq!(fx.hits.iter().map(|x| x.id).collect::<Vec<_>>(), vec![2]);
    fx.hits.clear();
    fx.queue_active_hits(&effects, available.clone());
    assert!(fx.hits.is_empty());
    effects.push(lifecycle_hit(3, target, false));
    fx.queue_active_hits(&effects, available.clone());
    assert_eq!(fx.hits.iter().map(|x| x.id).collect::<Vec<_>>(), vec![3]);
    fx.hits.clear();
    effects.clear();
    fx.queue_active_hits(&effects, Default::default());
    assert!(fx.observed.is_empty());
    assert!(fx.persistent_hits.is_empty());
    assert!(fx.available_targets.is_empty());
}
