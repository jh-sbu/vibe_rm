//! Synthetic actors, graph timers and collision; no third-party asset fixtures.
use super::*;
use crate::ai::ActorRuntime;
use crate::engine::CellRuntime;
use crate::render::{ActorInstance, CellKey, RenderCell};
use crate::world::{
    behavior::{GraphAnim, ProjectRuntime},
    ragdoll::{RagdollBody, RagdollDesc},
    skeleton::{Bone, Skeleton},
};
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Vec2;
use havok::behavior::runtime::Shared;
use havok::behavior::{
    BehaviorGraph, EventProperty, Generator, Modifier, Project, StartMode, State, Transition,
};
use std::sync::Arc;
const ACTOR: FormId = FormId(0x801);
const OTHER: FormId = FormId(0x806);
const NPC: FormId = FormId(0x802);
const RACE: FormId = FormId(0x803);
const CELL: FormId = FormId(0x804);
const PACKAGE: FormId = FormId(0x805);
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture(immune: bool) -> (Fixture, Engine) {
    let dir = std::env::temp_dir().join(format!(
        "vrm-ragdoll-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let f = Fixture(dir);
    let mut data = [0; 36];
    data[32..36].copy_from_slice(&(if immune { 0x400000u32 } else { 0 }).to_le_bytes());
    let records = [
        Node::top(
            b"RACE",
            vec![RecordBuilder::new(b"RACE", RACE).field(b"DATA", &data)],
        ),
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).form(b"RNAM", RACE)],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", OTHER).form(b"NAME", NPC),
            ],
        ),
        Node::top(b"PACK", vec![RecordBuilder::new(b"PACK", PACKAGE)]),
    ];
    std::fs::write(
        f.0.join("Ragdolls.esp"),
        write_plugin(0, &[], "test", "synthetic knock", 0x806, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Ragdolls.esp");
    let skeleton = Arc::new(Skeleton {
        bones: vec![Bone {
            name: "pelvis".into(),
            parent: None,
            bind: nif::Transform {
                translation: Vec3::Z * 60.0,
                ..Default::default()
            },
        }],
        by_name: std::collections::HashMap::from([("pelvis".into(), 0)]),
        height: 120.0,
        footprint: Vec2::splat(20.0),
        ragdoll: Some(Arc::new(RagdollDesc {
            bodies: vec![RagdollBody {
                bone: 0,
                offset: Mat4::IDENTITY,
                p1: -Vec3::X * 10.0,
                p2: Vec3::X * 10.0,
                radius: 5.0,
                mass: 10.0,
            }],
            joints: vec![],
        })),
    });
    let event = |id| EventProperty {
        event: id,
        payload: None,
    };
    let state = |id, name: &str, g, transitions| State {
        id,
        name: name.into(),
        generator: Some(g),
        transitions,
        enter_events: if id == 2 { vec![event(4)] } else { vec![] },
        exit_events: vec![],
    };
    let mut graph = BehaviorGraph::new(
        "synthetic",
        Some(0),
        vec![
            Generator::StateMachine {
                name: "root".into(),
                start: 0,
                start_variable: None,
                start_mode: StartMode::Default,
                sync_variable: None,
                change_event: None,
                states: vec![
                    state(0, "Root", 1, vec![]),
                    state(
                        1,
                        "FullyRagdoll",
                        2,
                        vec![Transition {
                            event: 2,
                            to_state: 2,
                            ..Default::default()
                        }],
                    ),
                    state(
                        2,
                        "GetUpFromRagdoll",
                        3,
                        vec![Transition {
                            event: 3,
                            to_state: 0,
                            ..Default::default()
                        }],
                    ),
                ],
                wildcards: vec![Transition {
                    event: 0,
                    to_state: 1,
                    ..Default::default()
                }],
            },
            Generator::Other("idle".into()),
            Generator::Wrap {
                name: "down".into(),
                class: "hkbModifierGenerator".into(),
                child: None,
                modifier: Some(0),
            },
            Generator::Wrap {
                name: "up".into(),
                class: "hkbModifierGenerator".into(),
                child: None,
                modifier: Some(2),
            },
        ],
        [
            "Ragdoll",
            "GetUpBegin",
            "GetUpStart",
            "GetUpEnd",
            "AddCharacterControllerToWorld",
            "staggerStart",
        ]
        .map(String::from)
        .to_vec(),
    );
    graph.modifiers = vec![
        Modifier::EventDriven {
            child: Some(1),
            activate: 1,
            deactivate: -1,
            active_by_default: false,
        },
        Modifier::Timer {
            seconds: 0.5,
            alarm: event(2),
        },
        Modifier::List(vec![3, 4]),
        Modifier::Timer {
            seconds: 1.0,
            alarm: event(3),
        },
        Modifier::GetUp(havok::behavior::ragdoll::GetUp {
            enabled: true,
            ground_normal: Vec3::Z,
            duration: 1.0,
            align_duration: 0.25,
            bones: [0, 0, 0],
        }),
    ];
    graph.modifier_bindings = vec![vec![]; 5];
    graph.modifier_property_bindings = vec![vec![]; 5];
    let project = Arc::new(ProjectRuntime {
        shared: Shared::new(Arc::new(Project {
            graphs: vec![("synthetic.hkx".into(), graph)],
            character: None,
        })),
        dir: "meshes/synthetic".into(),
        name: "synthetic".into(),
        files: vec![],
    });
    let transform = Mat4::from_translation(Vec3::X * 200.0);
    let mut a = ActorRuntime::new(
        ACTOR,
        NPC,
        skeleton.clone(),
        transform,
        vec![crate::ai::package::Package::bare(
            PACKAGE,
            crate::ai::package::Behaviour::Travel,
        )],
        0.0,
    );
    a.exec.health = 100.0;
    a.exec.current = Some(0);
    a.exec.begin_run(Some(0), 0.0, false);
    a.graph = Some(GraphAnim::new(
        project,
        "synthetic.nif",
        false,
        &skeleton,
        1,
    ));
    a.capsule = Some(e.physics.add_actor_capsule(a.exec.pos, 1.0, ACTOR));
    let key = CellKey::Interior(CELL);
    e.actor_cells.insert(ACTOR, key);
    let mut runtime = CellRuntime::default();
    runtime.actors.push(a);
    e.cells.insert(key, runtime);
    e.scene.cells.insert(
        key,
        RenderCell {
            actors: vec![ActorInstance {
                meshes: vec![],
                attachments: vec![],
                equipment: vec![],
                held_light: None,
                transform,
                pose: skeleton.model_space(&skeleton.bind_locals()),
                lights: [0xffff; 8],
                radius: 120.0,
                shadow_only: false,
                skin_tone: None,
                weight: 0.0,
                alpha: 1.0,
            }],
            ..Default::default()
        },
    );
    e.physics.world.insert_collider(
        rapier3d::prelude::ColliderBuilder::cuboid(10000.0, 10000.0, 10.0)
            .translation(-Vec3::Z * 10.0)
            .build(),
        None,
    );
    e.player.position = Vec3::Z * (e.physics.player_half_height + e.physics.player_radius);
    (f, e)
}
fn add_other(e: &mut Engine) {
    let a = e.actor_ref(ACTOR).unwrap();
    let skeleton = a.skeleton.clone();
    let project = a.graph.as_ref().unwrap().project().clone();
    let key = e.actor_cells[&ACTOR];
    let transform = Mat4::from_translation(Vec3::X * 300.0);
    let mut other = ActorRuntime::new(OTHER, NPC, skeleton.clone(), transform, vec![], 0.0);
    other.exec.health = 100.0;
    other.graph = Some(GraphAnim::new(
        project,
        "synthetic.nif",
        false,
        &skeleton,
        2,
    ));
    other.capsule = Some(e.physics.add_actor_capsule(other.exec.pos, 1.0, OTHER));
    e.cells.get_mut(&key).unwrap().actors.push(other);
    e.actor_cells.insert(OTHER, key);
    let mut inst = e.scene.cells[&key].actors[0].clone();
    inst.transform = transform;
    e.scene.cells.get_mut(&key).unwrap().actors.push(inst);
}

fn frames(e: &mut Engine, n: usize) {
    for _ in 0..n {
        e.physics.step(1.0 / 60.0);
        e.update_knocks(1.0 / 60.0);
    }
}
#[test]
fn negative_force_reverses_direction_but_still_lifts() {
    let push = push_velocity(Vec3::ZERO, Vec3::X, 10.0);
    let pull = push_velocity(Vec3::ZERO, Vec3::X, -10.0);
    assert!(push.x > 0.0 && pull.x < 0.0);
    assert!(push.z > 0.0 && (pull.z - push.z).abs() < 1e-3);
}
#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn npc_recovers_through_graph_and_keeps_package_run() {
    let (_f, mut e) = fixture(false);
    assert!(e.push_actor_away(PLAYER_REF, ACTOR, 5.0));
    let capsule = e.actor_ref(ACTOR).unwrap().capsule.unwrap();
    assert!(!e.physics.world.colliders[capsule].is_enabled());
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.run.as_ref().unwrap().id,
        PACKAGE
    );
    assert_eq!(e.knocked_state(ACTOR), KnockState::Queued);
    for (func, value) in [(107u16, 1.0f32), (725, 5.0)] {
        let mut data = [0u8; 32];
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data[8..10].copy_from_slice(&func.to_le_bytes());
        let cond = crate::condition::parse(&e.lo.get(NPC).unwrap(), &data).unwrap();
        assert!(crate::condition::evaluate_one(
            &e,
            &cond,
            crate::condition::Context {
                subject: Some(ACTOR),
                ..Default::default()
            }
        ));
    }

    frames(&mut e, 360);
    assert_eq!(e.knocked_state(ACTOR), KnockState::Normal);
    assert!(e.physics.world.colliders[capsule].is_enabled());
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.run.as_ref().unwrap().id,
        PACKAGE
    );
}
#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn death_keeps_bodies_and_constraints_release_after_recovery() {
    let (_f, mut e) = fixture(false);
    add_other(&mut e);
    assert!(!e.add_ball_socket_constraint(
        (ACTOR, "pelvis"),
        (OTHER, "pelvis"),
        Vec3::ZERO,
        Vec3::ZERO
    ));
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    assert!(e.knock_down(OTHER, Vec3::ZERO, KnockCause::Script));
    assert!(e.add_ball_socket_constraint(
        (ACTOR, "pelvis"),
        (OTHER, "pelvis"),
        Vec3::X * 100.0,
        Vec3::ZERO
    ));
    assert_eq!(e.havok_constraints.len(), 1);
    e.request_get_up(ACTOR);
    frames(&mut e, 120);
    assert!(e.havok_constraints.is_empty());
    assert!(!e.remove_havok_constraints((ACTOR, "pelvis"), (OTHER, "pelvis")));
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    frames(&mut e, 90);
    // Two bones on the same body cannot be joined, but a fixed anchor can.
    let bodies = e
        .actor_ref(ACTOR)
        .unwrap()
        .knock
        .as_ref()
        .unwrap()
        .ragdoll
        .bodies
        .clone();
    assert!(e.kill_actor_quietly(ACTOR));
    assert_eq!(
        e.actor_ref(ACTOR)
            .unwrap()
            .ragdoll
            .as_ref()
            .unwrap()
            .0
            .bodies,
        bodies
    );
    assert!(e.actor_ref(ACTOR).unwrap().knock.is_none());
}
#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn immunity_area_threshold_paralysis_and_saved_pose() {
    let (_f, mut e) = fixture(true);
    assert!(!e.push_actor_away(PLAYER_REF, ACTOR, 10.0));
    assert!(!e.is_knocked(ACTOR));
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Paralysis));
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 1.0);
    frames(&mut e, 180);
    assert!(e.is_knocked(ACTOR));
    let saved = crate::save::knock::capture(&e);
    let bodies = saved[0].bodies.clone();
    e.finish_knock(ACTOR);
    e.restore_saved_knocks(saved);
    assert!(
        e.physics
            .ragdoll_bodies(&e.actor_ref(ACTOR).unwrap().knock.as_ref().unwrap().ragdoll)[0]
            .abs_diff_eq(bodies[0], 0.001)
    );
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 0.0);
    frames(&mut e, 240);
    assert!(!e.is_knocked(ACTOR));
    let (_f, mut e) = fixture(false);
    e.knock_area(PLAYER_REF, 0.99, 800.0);
    assert!(!e.is_knocked(ACTOR));
    e.knock_area(PLAYER_REF, 1.0, 800.0);
    assert!(e.is_knocked(ACTOR));
}

#[test]
#[ignore = "requires installed vanilla assets and a headless GPU adapter"]
fn vanilla_third_word_runs_its_push_script_and_wards_stop_it() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    e.enter_interior(FormId(0x1605e), None).unwrap();
    let actor = FormId(0x813ba);
    assert!(e.actor_ref(actor).is_some_and(|a| !a.in_furniture()));
    let shout = e.lo.find_editor_id("VoiceUnrelentingForce3").unwrap();
    assert!(e.apply_item(shout, Some(PLAYER_REF), actor));
    for _ in 0..30 {
        e.update_scripts(1.0 / 60.0);
    }
    assert!(
        e.is_knocked(actor),
        "VoicePushEffectScript must reach PushActorAway"
    );
    let keyword = e.lo.find_editor_id("ImmuneStrongUnrelentingForce").unwrap();
    let base = e.lo.find_editor_id("Arngeir").unwrap();
    let immune = e.create_ref(base, PLAYER_REF, false).unwrap();
    assert!(e.has_keyword(immune, keyword));
    assert!(e.apply_item(shout, Some(PLAYER_REF), immune));
    for _ in 0..30 {
        e.update_scripts(1.0 / 60.0);
    }
    assert!(!e.is_knocked(immune));
    // Entering essential bleedout must not replace the ragdoll graph before
    // its recovery timer can run; the kneeling state follows the get-up.
    assert!(e.knock_down(immune, Vec3::ZERO, KnockCause::Paralysis));
    assert!(e.kill(immune, false, Some(PLAYER_REF)));
    assert!(e.request_get_up(immune));
    frames(&mut e, 420);
    assert!(!e.is_dead(immune));
    assert!(!e.is_knocked(immune));
    e.set_actor_value(PLAYER_REF, esp::actor_value::WARD_POWER, 1000.0);
    e.camera.yaw = 0.0;
    e.actor_mut(actor).unwrap().exec.pos = e.player.position + Vec3::Y * 100.0;
    assert!(!e.apply_item(shout, Some(actor), PLAYER_REF));
    for _ in 0..30 {
        e.update_scripts(1.0 / 60.0);
    }
    assert!(
        !e.is_knocked(PLAYER_REF),
        "ward must stop the effect before its script runs"
    );
    assert!(crate::console::execute(&mut e, "knock player 0")[0].contains("accepted"));
    assert!(e.third_person);
    assert!(!e.jump_during_get_up());
    assert!(e.request_get_up(PLAYER_REF));
    frames(&mut e, 420);
    assert!(!e.is_knocked(PLAYER_REF));
    assert!(!e.third_person);
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn paralysis_during_matching_or_get_up_returns_to_down() {
    for before in [10, 40] {
        let (_f, mut e) = fixture(false);
        assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
        assert!(e.request_get_up(ACTOR));
        frames(&mut e, before);
        assert_eq!(
            e.knocked_state(ACTOR),
            if before == 40 {
                KnockState::GetUp
            } else {
                KnockState::Down
            }
        );
        e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 1.0);
        let bodies = e
            .actor_ref(ACTOR)
            .unwrap()
            .knock
            .as_ref()
            .unwrap()
            .ragdoll
            .bodies
            .clone();
        frames(&mut e, 1);
        assert_eq!(e.knocked_state(ACTOR), KnockState::Down);
        assert!(
            !e.actor_ref(ACTOR)
                .unwrap()
                .knock
                .as_ref()
                .unwrap()
                .getup_requested
        );
        frames(&mut e, 179);
        assert_eq!(e.knocked_state(ACTOR), KnockState::Down);
        let a = e.actor_ref(ACTOR).unwrap();
        assert_eq!(a.knock.as_ref().unwrap().ragdoll.bodies, bodies);
        assert!(!e.physics.world.colliders[a.capsule.unwrap()].is_enabled());
        e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 0.0);
        frames(&mut e, 240);
        assert_eq!(e.knocked_state(ACTOR), KnockState::Normal);
    }
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn force_remove_releases_hold_even_while_paralyzed() {
    let (_f, mut e) = fixture(false);
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 1.0);
    frames(&mut e, 90);
    assert!(!e.request_get_up(ACTOR));
    assert!(
        !e.actor_ref(ACTOR)
            .unwrap()
            .knock
            .as_ref()
            .unwrap()
            .script_hold
    );
    frames(&mut e, 90);
    assert_eq!(e.knocked_state(ACTOR), KnockState::Down);
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 0.0);
    frames(&mut e, 360);
    assert_eq!(e.knocked_state(ACTOR), KnockState::Normal);
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn resurrect_after_knock_returns_graph_to_root() {
    let (_f, mut e) = fixture(false);
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    frames(&mut e, 60);
    assert!(e.kill_actor_quietly(ACTOR));
    e.resurrect(ACTOR);
    assert!(!e.is_dead(ACTOR));
    let skeleton = e.actor_ref(ACTOR).unwrap().skeleton.clone();
    let mut graph = e.actor_mut(ACTOR).unwrap().graph.take().unwrap();
    for _ in 0..300 {
        graph.update(1.0 / 60.0, &e.vfs, &mut e.anims, &skeleton);
    }
    assert_eq!(graph.active_states(), ["Root"]);
}

#[test]
#[ignore = "requires installed vanilla assets and a headless GPU adapter"]
fn player_ragdoll_accepts_constraint() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    e.enter_interior(FormId(0x1605e), None).unwrap();
    let actor = FormId(0x813ba);
    assert!(e.knock_down(actor, Vec3::ZERO, KnockCause::Script));
    assert!(e.knock_down(PLAYER_REF, Vec3::ZERO, KnockCause::Script));
    assert!(e.add_ball_socket_constraint(
        (actor, "NPC L Hand [LHnd]"),
        (PLAYER_REF, "npc l hand [lhnd]"),
        Vec3::ZERO,
        Vec3::ZERO
    ));
    assert!(e.request_get_up(PLAYER_REF));
    frames(&mut e, 40);
    assert_eq!(e.knocked_state(PLAYER_REF), KnockState::GetUp);
    assert!(!e.add_ball_socket_constraint(
        (actor, "NPC L Hand [LHnd]"),
        (PLAYER_REF, "NPC L Hand [LHnd]"),
        Vec3::ZERO,
        Vec3::ZERO
    ));
    let bodies = e.player_knock.as_ref().unwrap().ragdoll.bodies.clone();
    e.set_actor_value(PLAYER_REF, esp::actor_value::PARALYSIS, 1.0);
    frames(&mut e, 120);
    assert_eq!(e.knocked_state(PLAYER_REF), KnockState::Down);
    assert_eq!(e.player_knock.as_ref().unwrap().ragdoll.bodies, bodies);
    assert!(e.havok_constraints.is_empty());
    assert!(!e.request_get_up(PLAYER_REF));
    assert!(!e.player_knock.as_ref().unwrap().script_hold);
    e.set_actor_value(PLAYER_REF, esp::actor_value::PARALYSIS, 0.0);
    frames(&mut e, 900);
    assert_eq!(
        e.knocked_state(PLAYER_REF),
        KnockState::Normal,
        "player recovery: {:?}",
        e.player_body
            .as_ref()
            .unwrap()
            .third_graph()
            .unwrap()
            .active_states()
    );
    assert!(e.havok_constraints.is_empty());
}

#[test]
fn get_up_type_is_swimming_only_with_the_pelvis_under_water() {
    assert_eq!(get_up_type(None, 10.0), 1.0);
    assert_eq!(get_up_type(Some(5.0), 10.0), 1.0);
    assert_eq!(get_up_type(Some(15.0), 10.0), 2.0);
}

fn knock_of(e: &Engine) -> &Knock {
    e.actor_ref(ACTOR).unwrap().knock.as_ref().unwrap()
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn paralysis_holds_the_pose_it_struck_in_not_the_push_pose() {
    let (_f, mut e) = fixture(false);
    assert!(e.knock_down(ACTOR, Vec3::X * 300.0, KnockCause::Script));
    frames(&mut e, 90);
    assert!(knock_of(&e).hold_pose.is_none());
    let skeleton = e.actor_ref(ACTOR).unwrap().skeleton.clone();
    let lying = {
        let k = knock_of(&e);
        k.mapping
            .pose(&skeleton, &e.physics.ragdoll_bodies(&k.ragdoll))
    };
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 1.0);
    frames(&mut e, 1);
    let held = knock_of(&e)
        .hold_pose
        .clone()
        .expect("paralysis holds a pose");
    let bone = e
        .actor_ref(ACTOR)
        .unwrap()
        .skeleton
        .ragdoll
        .as_ref()
        .unwrap()
        .bodies[0]
        .bone;
    assert!(held[bone].abs_diff_eq(lying[bone], 5.0));
    frames(&mut e, 60);
    assert_eq!(knock_of(&e).hold_pose.as_ref(), Some(&held));
    e.set_actor_value(ACTOR, esp::actor_value::PARALYSIS, 0.0);
    frames(&mut e, 1);
    assert!(knock_of(&e).hold_pose.is_none());
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn a_second_drop_after_resting_still_hurts() {
    let (_f, mut e) = fixture(false);
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    frames(&mut e, 120);
    assert!(knock_of(&e).fall_applied);
    let health = e.actor_value(ACTOR, esp::actor_value::HEALTH);
    // Off the ledge it rested on.
    let k = knock_of(&e);
    let bodies: Vec<_> = e
        .physics
        .ragdoll_bodies(&k.ragdoll)
        .iter()
        .map(|m| Mat4::from_translation(Vec3::Z * 800.0) * *m)
        .collect();
    let still = vec![(Vec3::ZERO, Vec3::ZERO); bodies.len()];
    let ragdoll = k.ragdoll.clone();
    e.physics.place_ragdoll(&ragdoll, &bodies, &still);
    frames(&mut e, 240);
    assert!(
        e.is_dead(ACTOR) || e.actor_value(ACTOR, esp::actor_value::HEALTH) < health,
        "the second fall did no damage"
    );
}

#[test]
#[ignore = "requires a headless GPU adapter; synthetic records only"]
fn a_saved_knock_is_dropped_when_its_actor_moved_meanwhile() {
    let (_f, mut e) = fixture(false);
    assert!(e.knock_down(ACTOR, Vec3::ZERO, KnockCause::Script));
    frames(&mut e, 60);
    let saved = crate::save::knock::capture(&e);
    e.finish_knock(ACTOR);
    e.actor_mut(ACTOR).unwrap().exec.pos += Vec3::X * 2000.0;
    e.restore_saved_knocks(saved);
    assert!(!e.is_knocked(ACTOR));
}
