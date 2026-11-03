use super::*;

fn first_rig(graph: GraphAnim) -> FirstPerson {
    FirstPerson {
        skeleton: Arc::new(Skeleton::default()),
        graph,
        rigid: Vec::new(),
        particles: Default::default(),
        weapon_out: false,
        ready: false,
        instance: crate::render::ActorInstance {
            meshes: Vec::new(),
            attachments: Vec::new(),
            equipment: Vec::new(),
            held_light: None,
            transform: Mat4::IDENTITY,
            pose: Vec::new(),
            lights: [0xffff; 8],
            radius: 1.0,
            shadow_only: false,
            skin_tone: None,
            weight: 0.5,
        },
    }
}

fn notifications(event: &str) -> GraphAnim {
    use havok::behavior::{BehaviorGraph, EventProperty, Generator, Project, State, Transition};
    let state = |id, enter| State {
        id,
        name: format!("s{id}"),
        generator: None,
        transitions: if id == 0 {
            vec![Transition::new(0, 1, None)]
        } else {
            Vec::new()
        },
        enter_events: if enter {
            vec![EventProperty {
                event: 1,
                payload: None,
            }]
        } else {
            Vec::new()
        },
        exit_events: Vec::new(),
    };
    let graph = BehaviorGraph::new(
        "notifications",
        Some(0),
        vec![Generator::StateMachine {
            name: "Root".into(),
            start: 0,
            start_variable: None,
            start_mode: Default::default(),
            sync_variable: None,
            change_event: None,
            states: vec![state(0, false), state(1, true)],
            wildcards: Vec::new(),
        }],
        vec!["tick".into(), event.into()],
    );
    let project = Arc::new(crate::world::behavior::ProjectRuntime {
        shared: havok::behavior::runtime::Shared::new(Arc::new(Project {
            graphs: vec![("test".into(), graph)],
            character: None,
        })),
        dir: "test".into(),
        name: "test".into(),
        files: Vec::new(),
    });
    GraphAnim::new(project, "test", false, &Skeleton::default(), 1)
}

#[test]
fn only_visible_graph_supplies_hits_arrows_steps_and_script_notifications() {
    let vfs = vfs::Vfs::new("/tmp/vrm-first-person-no-assets", &[]);
    let mut anims = crate::world::animation::AnimationLibrary::default();
    for (first, third, view, hit, arrow, foot) in [
        ("HitFrame", "arrowRelease", false, true, false, false),
        ("HitFrame", "arrowRelease", true, false, true, false),
        ("arrowRelease", "HitFrame", false, false, true, false),
        ("FootLeft", "HitFrame", false, false, false, true),
    ] {
        let mut body = PlayerBody::new(Arc::new(Skeleton::default()), 0.0);
        body.graph = Some(notifications(third));
        body.first = Some(first_rig(notifications(first)));
        body.set_view(view);
        body.sample(0.0, &vfs, &mut anims);
        body.script_events.clear();
        body.step_events.clear();
        assert!(body.send_event("tick", &vfs, &mut anims));
        body.sample(0.1, &vfs, &mut anims);
        assert_eq!(body.combat_events.hit_frames != 0, hit);
        assert_eq!(body.combat_events.arrow_release, arrow);
        assert_eq!(body.step_events.contains(&"footleft".into()), foot);
        let expected = if view { third } else { first };
        assert!(
            body.script_events.iter().all(|e| e == expected),
            "{:?}",
            body.script_events
        );
        body.set_view(!view);
        body.combat_events = PlayerCombatEvents::default();
        body.sample(0.1, &vfs, &mut anims);
        assert_eq!(body.combat_events.hit_frames, 0);
        assert!(!body.combat_events.arrow_release);
    }
}

#[test]
fn hidden_rig_draws_and_sheathes_and_view_switch_preserves_one_pending_shot() {
    let vfs = vfs::Vfs::new("/tmp/vrm-first-person-no-assets", &[]);
    let mut anims = crate::world::animation::AnimationLibrary::default();
    let mut body = super::tests::equipment_body();
    body.first = Some(first_rig(super::tests::equipment_body().graph.unwrap()));
    body.sample(0.0, &vfs, &mut anims);
    assert!(body.send_event("WeapEquip", &vfs, &mut anims));
    body.drawn = true;
    body.sample(0.0, &vfs, &mut anims);
    assert!(body.weapon_out && body.first.as_ref().unwrap().weapon_out);
    body.bow.drawing = true;
    assert!(body.bow.release(0.75, || true));
    body.set_view(false);
    assert!(body.bow.shot.is_some());
    assert_eq!(body.bow.update(true, true, 0.0), Some(0.75));
    body.set_view(true);
    assert_eq!(body.bow.update(true, true, 0.0), None);
    assert!(body.send_event("Unequip", &vfs, &mut anims));
    body.sample(0.0, &vfs, &mut anims);
    assert!(body.send_event("SheathDone", &vfs, &mut anims));
    body.sample(0.0, &vfs, &mut anims);
    assert!(!body.weapon_out && !body.first.as_ref().unwrap().weapon_out);
}

#[test]
fn pitched_rig_anchors_camera_at_eye_with_yaw_scale_and_crouch() {
    use crate::world::skeleton::Bone;
    let bones = [
        ("root", None, Vec3::ZERO),
        ("Camera1st [Cam1]", Some(0), Vec3::Z * 120.5),
        ("NPC LookNode [Look]", Some(0), Vec3::Z * 120.5),
        ("hand", Some(2), Vec3::new(12.0, 30.0, -18.0)),
    ]
    .into_iter()
    .map(|(name, parent, translation)| Bone {
        name: name.into(),
        parent,
        bind: nif::Transform {
            translation,
            ..Default::default()
        },
    })
    .collect::<Vec<_>>();
    let skeleton = Skeleton {
        by_name: bones
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name.to_ascii_lowercase(), i))
            .collect(),
        bones,
        ..Default::default()
    };
    let bind = skeleton.model_space(&skeleton.bind_locals());
    let mut rig = first_rig(notifications("HitFrame"));
    rig.skeleton = Arc::new(skeleton);
    for scale in [0.95, 1.0, 1.08] {
        for pitch in [-1.4, 0.0, 1.4] {
            for drop in [0.0, 25.0] {
                let eye = Vec3::new(100.0, -600.0, 130.0 - drop);
                rig.instance.pose = bind.clone();
                rig.place(eye, 0.8, pitch, scale);
                let camera = rig
                    .instance
                    .transform
                    .transform_point3(rig.instance.pose[1].w_axis.truncate());
                assert!(camera.distance(eye) < 0.0001);
                let look = rig
                    .instance
                    .transform
                    .transform_point3(rig.instance.pose[2].w_axis.truncate());
                assert!(look.distance(eye) < 0.0001);
                let delta = rig.instance.pose[3].w_axis.truncate() - bind[3].w_axis.truncate();
                assert_eq!(delta.length() > 1.0, pitch != 0.0);
            }
        }
    }
}

#[test]
fn first_person_instance_is_excluded_from_world_queries_and_has_its_own_projection() {
    let mut scene = crate::render::Scene::default();
    scene.first_person = Some(first_rig(notifications("HitFrame")).instance);
    assert_eq!(scene.actors().count(), 0);
    assert!(scene.pick(Vec3::ZERO, Vec3::Y).is_empty());
    let camera = crate::render::Camera {
        position: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 1.0,
        projection: crate::render::Projection::Perspective,
    };
    let p = camera.first_person_proj(16.0 / 9.0);
    let near = p.project_point3(Vec3::new(0.0, 0.0, -1.0));
    assert!((near.z - 1.0).abs() < 0.0001);
    assert_ne!(p, camera.proj(16.0 / 9.0));
}

#[test]
fn wheel_crosses_view_boundary_and_preserves_saved_distance_until_scrolling_out() {
    assert_eq!(zoom_view(false, 300.0, -30.0), (false, 300.0));
    assert_eq!(zoom_view(false, 300.0, 30.0), (true, THIRD_PERSON_MIN));
    assert_eq!(
        zoom_view(true, THIRD_PERSON_MIN, -30.0),
        (false, THIRD_PERSON_MIN)
    );
    assert_eq!(
        zoom_view(true, THIRD_PERSON_MIN + 30.0, -30.0),
        (true, THIRD_PERSON_MIN)
    );
    assert_eq!(zoom_view(true, 300.0, 30.0), (true, 330.0));
    assert_eq!(
        zoom_view(true, THIRD_PERSON_MAX, 30.0),
        (true, THIRD_PERSON_MAX)
    );
    assert_eq!(zoom_view(false, 300.0, f32::NAN), (false, 300.0));
}

#[test]
#[ignore = "requires installed Skyrim SE assets and a headless GPU adapter"]
fn first_person_draw_binds_the_character_rig_and_puts_the_sword_in_hand() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    let sword = e.lo.find_editor_id("IronSword").unwrap();
    e.inventory_mut(PLAYER_REF).add(sword, 1);
    e.equip_item(PLAYER_REF, sword, true).unwrap();
    e.player.grounded = true;
    e.update_player_body(Vec3::ZERO, 0.0);
    assert!(e.player_draw_weapon(true));
    for _ in 0..120 {
        e.update_player_body(Vec3::ZERO, 1.0 / 60.0);
    }
    let first = e.player_body.as_ref().unwrap().first.as_ref().unwrap();
    assert!(first.ready && first.weapon_out);
    assert!(!first.instance.meshes.is_empty());
    assert!(!first.instance.equipment.is_empty());
    let bind = first.skeleton.model_space(&first.skeleton.bind_locals());
    let hand = first.skeleton.find("NPC R Hand [RHnd]").unwrap();
    assert!(
        first.instance.pose[hand]
            .w_axis
            .truncate()
            .distance(bind[hand].w_axis.truncate())
            > 10.0
    );
    let at = e.player_node("WEAPON").unwrap().w_axis.truncate();
    assert!(
        (at - e.player.eye()).dot(e.camera.forward()) > 1.0,
        "{at:?}"
    );
}

#[test]
#[ignore = "requires installed Skyrim SE assets and a headless GPU adapter"]
fn drawn_bow_keeps_arm_pose_with_short_bone_masks_and_releases_once_across_view_switch() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    let bow = e.lo.find_editor_id("LongBow").unwrap();
    let arrow = e.lo.find_editor_id("IronArrow").unwrap();
    e.inventory_mut(PLAYER_REF).add(bow, 1);
    e.inventory_mut(PLAYER_REF).add(arrow, 20);
    e.equip_item(PLAYER_REF, bow, true).unwrap();
    e.player.grounded = true;
    e.update_player_body(Vec3::ZERO, 0.0);
    assert!(e.player_draw_weapon(true));
    for _ in 0..120 {
        e.update_player_body(Vec3::ZERO, 1.0 / 60.0);
    }
    assert!(e.player_begin_bow_draw());
    for _ in 0..90 {
        e.update_player_body(Vec3::ZERO, 1.0 / 60.0);
    }
    let held = e.player_node("SHIELD").unwrap().w_axis.truncate();
    assert!(
        (held - e.player.eye()).dot(e.camera.forward()) > 15.0,
        "{held:?}"
    );
    e.player_release_bow(1.5);
    e.set_third_person(true);
    for _ in 0..120 {
        e.update_player_body(Vec3::ZERO, 1.0 / 60.0);
    }
    assert_eq!(e.inventories[&PLAYER_REF].count(arrow), 19);
    e.set_third_person(false);
    for _ in 0..30 {
        e.update_player_body(Vec3::ZERO, 1.0 / 60.0);
    }
    assert_eq!(e.inventories[&PLAYER_REF].count(arrow), 19);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn camera_locks_and_forced_furniture_restore_the_previous_view() {
    use esp::writer::{Node, RecordBuilder, write_plugin};
    let dir = std::env::temp_dir().join(format!("vrm-first-person-camera-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let groups = [
        Node::top(b"KYWD", vec![RecordBuilder::new(b"KYWD", FormId(0x800))]),
        Node::top(
            b"DOBJ",
            vec![
                RecordBuilder::new(b"DOBJ", FormId(0x31))
                    .field(b"DNAM", &[b'F', b'F', b'F', b'P', 0, 8, 0, 0]),
            ],
        ),
        Node::top(
            b"FURN",
            vec![
                RecordBuilder::new(b"FURN", FormId(0x801))
                    .field(b"WBDT", &[1, 0xff])
                    .form(b"KWDA", FormId(0x800)),
            ],
        ),
        Node::top(
            b"REFR",
            vec![RecordBuilder::new(b"REFR", FormId(0x802)).form(b"NAME", FormId(0x801))],
        ),
    ];
    std::fs::write(
        dir.join("Camera.esp"),
        write_plugin(0, &[], "test", "camera", 0x900, &groups),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir, "Camera.esp");
    for pov in [0, 1] {
        e.set_third_person(false);
        let mut flags = [false; 8];
        flags[2] = true;
        e.disabled_controls.apply(true, flags, pov);
        e.toggle_player_view();
        e.zoom_third_person(30.0);
        assert!(!e.third_person);
        e.set_third_person(true); // Script-forced view bypasses input locks.
        e.toggle_player_view();
        e.zoom_third_person(-1000.0);
        assert!(e.third_person);
        e.disabled_controls.apply(false, flags, pov);
    }
    e.third_person_distance = 300.0;
    assert!(e.open_crafting_station(FormId(0x802)));
    assert!(!e.third_person);
    assert_eq!(e.first_person_furniture, Some((FormId(0x802), true)));
    e.toggle_player_view();
    e.zoom_third_person(30.0);
    e.set_third_person(true);
    assert!(!e.third_person);
    assert_eq!(e.third_person_distance, 300.0);
    e.menu = None;
    e.update_furniture_view();
    assert!(e.third_person);
    assert!(e.first_person_furniture.is_none());
    e.set_third_person(false);
    assert!(e.open_crafting_station(FormId(0x802)));
    e.menu = None;
    e.update_furniture_view();
    assert!(!e.third_person);
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}
