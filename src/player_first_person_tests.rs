use super::*;

fn first_rig(graph: GraphAnim) -> FirstPerson {
    FirstPerson {
        skeleton: Arc::new(Skeleton::default()),
        graph,
        rigid: Vec::new(),
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
