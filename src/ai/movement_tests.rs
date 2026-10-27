//! Locomotion regressions using an empty world and synthetic actor state.

use super::*;
use std::collections::HashMap;

fn actor() -> ActorRuntime {
    let mut a = ActorRuntime::new(
        FormId(1),
        FormId(2),
        Arc::new(Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    let movement = |walk, run| MoveSpeeds {
        walk,
        run,
        turn_walk: 4.0,
        turn_run: 4.0,
        turn_moving: 4.0,
    };
    a.moves = Some((movement(100.0, 300.0), 0.0));
    a.sneak_moves = Some((movement(50.0, 150.0), 2.0));
    a.exec.goal = Some(Goal::travel(Vec3::Y * 1000.0));
    a
}

fn with_world(test: impl FnOnce(&mut World<'_>)) {
    let nav = nav::NavWorld::default();
    let mut furniture = FurnitureWorld::default();
    let vfs = vfs::Vfs::new("/nonexistent-vrm-movement-fixture", &[]);
    let mut anims = crate::world::animation::AnimationLibrary::default();
    let mut behaviors = crate::world::animation::BehaviorLibrary::default();
    let mut world = World {
        nav: &nav,
        furniture: &mut furniture,
        clips: Clips {
            vfs: &vfs,
            anims: &mut anims,
            behaviors: &mut behaviors,
        },
        may_use: &|_, _, _| false,
        claimed: &HashMap::new(),
        patrols: &HashMap::new(),
        targets: &HashMap::new(),
        holds: &|_, _| false,
        rand: &mut || 1,
    };
    test(&mut world);
}

#[test]
fn armor_scales_package_gaits_and_combat_without_changing_animation_speeds() {
    let mut a = actor();
    assert_eq!(a.move_speed(), 100.0);
    a.armor_speed_mult = crate::encumbrance::armor_speed(115.0, 115.0, 0.3);
    for (gait, speed) in [
        (package::Gait::Walk, 70.0),
        (package::Gait::FastWalk, 105.0),
        (package::Gait::Jog, 140.0),
        (package::Gait::Run, 210.0),
    ] {
        a.exec.goal.as_mut().unwrap().gait = gait;
        assert_eq!(a.move_speed(), speed);
    }
    assert_eq!(a.run_speed(), 210.0);
    assert_eq!(a.walk_speed(), 100.0);

    a.exec.goal.as_mut().unwrap().sneak = true;
    assert_eq!(a.move_speed(), 105.0);
    a.exec.goal.as_mut().unwrap().gait = package::Gait::Walk;
    assert_eq!(a.move_speed(), 35.0);
    a.exec.goal.as_mut().unwrap().sneak = false;
    a.hurry = true;
    assert_eq!(a.move_speed(), 210.0);

    // Fallback locomotion (no MOVT or root-motion data) is slowed once too.
    a.moves = None;
    assert_eq!(a.run_speed(), WALK_SPEED * 3.0 * 0.7);
    assert_eq!(a.walk_speed(), WALK_SPEED);
}

#[test]
fn armor_slows_actual_package_steps_and_combat_chases() {
    let mut a = actor();
    let destination = Vec3::Y * 1000.0;
    a.state = State::Walk {
        path: vec![destination],
        next: 0,
        budget: 10.0,
        to_seat: false,
    };
    a.armor_speed_mult = 0.7;
    with_world(|w| {
        assert!(a.step(1.0, w));
        assert_eq!(a.exec.pos, Vec3::Y * 70.0);
        assert_eq!(a.speed(), 70.0);

        a.exec.pos = Vec3::ZERO;
        a.exec.combat = Some(combat::Combat::new(FormId(3)));
        a.chase(1.0, w, destination, a.run_speed());
        assert_eq!(a.exec.pos, Vec3::Y * 210.0);
        assert_eq!(a.speed(), 210.0);
        assert!(matches!(a.gait(), crate::world::footsteps::Gait::Run));
    });
}

#[test]
fn scheduled_travel_heading_turns_at_movement_rate_and_cancels_on_halt() {
    let mut a = actor();
    let target = std::f32::consts::FRAC_PI_2;
    a.turn_after_arrival = Some(target);
    with_world(|w| {
        a.step(0.1, w);
        assert!((a.exec.heading - 0.4).abs() < 1e-5);
        assert!(a.turn_after_arrival.is_some());
        a.step(0.3, w);
        assert!((a.exec.heading - target).abs() < 1e-5);
        assert!(a.turn_after_arrival.is_none());

        a.turn_after_arrival = Some(0.0);
        a.halt(1.0);
        assert!(a.turn_after_arrival.is_none());

        a.turn_after_arrival = Some(0.0);
        a.exec.goal.as_mut().unwrap().behaviour = Behaviour::Sandbox;
        a.state = State::Idle(1.0);
        a.step(0.1, w);
        assert!(a.turn_after_arrival.is_none());

        a.exec.goal.as_mut().unwrap().behaviour = Behaviour::Travel;
        a.turn_after_arrival = Some(0.0);
        a.face = Some((FormId(4), false));
        a.step(0.1, w);
        assert!(a.turn_after_arrival.is_none());
        a.face = None;
        a.turn_after_arrival = Some(0.0);
        a.interrupt(&mut FurnitureWorld::default());
        assert!(a.turn_after_arrival.is_none());
    });
}

#[test]
fn arrival_heading_waits_for_initial_package_evaluation() {
    let mut a = actor();
    let target = std::f32::consts::FRAC_PI_2;
    a.exec.goal = None;
    a.exec.current = None;
    a.exec.next_eval = 0.5;
    a.turn_after_arrival = Some(target);
    with_world(|w| {
        // A newly spawned actor may spend its stagger with no goal yet. It
        // must neither turn early nor discard the schedule's pending heading.
        a.step(0.1, w);
        assert_eq!(a.exec.heading, 0.0);
        assert_eq!(a.turn_after_arrival, Some(target));

        // This is the same reconciliation used when evaluate_packages adopts
        // the first applicable package (which does not interrupt the actor).
        a.exec.goal = Some(Goal::travel(Vec3::Y * 1000.0));
        a.turn_after_arrival = arrival_turn_after_package_eval(a.turn_after_arrival, a.exec.goal);
        a.step(0.1, w);
        assert!((a.exec.heading - 0.4).abs() < 1e-5);
        assert!(a.turn_after_arrival.is_some());

        // At an actual evaluation with no applicable package, the target is
        // discarded instead of surviving indefinitely while goal-less.
        assert_eq!(arrival_turn_after_package_eval(Some(target), None), None);
    });
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn equipped_armor_draw_state_and_perks_refresh_npc_movement() {
    use crate::world::inventory::Inventory;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("vrm-npc-armor-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&fixture.0).unwrap();
    let armor = FormId(0x800);
    let perk = FormId(0x801);
    let mut data = [0; 8];
    data[4..].copy_from_slice(&50.0f32.to_le_bytes());
    let records = [
        Node::top(
            b"ARMO",
            vec![RecordBuilder::new(b"ARMO", armor).field(b"DATA", &data)],
        ),
        Node::top(
            b"PERK",
            vec![
                RecordBuilder::new(b"PERK", perk)
                    .field(b"PRKE", &[2, 0, 0])
                    .field(b"DATA", &[crate::perks::ep::MOD_ARMOR_WEIGHT, 1, 2])
                    .field(b"EPFT", &[1])
                    .f32(b"EPFD", 0.0)
                    .field(b"PRKF", &[]),
            ],
        ),
        Node::top(
            b"GMST",
            vec![
                RecordBuilder::new(b"GMST", FormId(0x802))
                    .zstring(b"EDID", "fMoveWeightMax")
                    .f32(b"DATA", 100.0),
                RecordBuilder::new(b"GMST", FormId(0x803))
                    .zstring(b"EDID", "fMoveEncumEffect")
                    .f32(b"DATA", 0.4),
                RecordBuilder::new(b"GMST", FormId(0x804))
                    .zstring(b"EDID", "fMoveEncumEffectNoWeapon")
                    .f32(b"DATA", 0.2),
            ],
        ),
    ];
    std::fs::write(
        fixture.0.join("Armor.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic NPC armor movement",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&fixture.0, "Armor.esp");
    let a = actor();
    let npc = a.ref_id;
    let key = crate::render::CellKey::Interior(FormId(0x810));
    e.actor_cells.insert(npc, key);
    e.cells.entry(key).or_default().actors.push(a);
    let mut inv = Inventory::default();
    inv.add(armor, 5); // Carried copies do not slow an actor.
    e.inventories.insert(npc, inv);
    let speed = |e: &Engine| e.cells[&key].actors[0].move_speed();

    e.update_actor_encumbrance();
    assert_eq!(speed(&e), 100.0);
    e.inventories.get_mut(&npc).unwrap().wear(armor);
    e.update_actor_encumbrance();
    assert_eq!(e.worn_armor_weight(npc), 50.0);
    assert_eq!(speed(&e), 90.0);
    e.cells.get_mut(&key).unwrap().actors[0].drawn = true;
    e.update_actor_encumbrance();
    assert_eq!(speed(&e), 80.0);
    assert!(e.add_perk(npc, perk));
    e.update_actor_encumbrance();
    assert_eq!(speed(&e), 100.0);
    e.remove_perk(npc, perk);
    e.update_actor_encumbrance();
    assert_eq!(speed(&e), 80.0);
    e.inventories.get_mut(&npc).unwrap().take_off(armor);
    e.update_actor_encumbrance();
    assert_eq!(speed(&e), 100.0);
}

#[test]
fn an_archer_backs_off_facing_its_target() {
    let mut a = actor();
    a.exec.combat = Some(combat::Combat::new(FormId(3)));
    a.exec.combat.as_mut().unwrap().away_to = Some(-Vec3::Y * 600.0);
    let before = a.exec.pos;
    with_world(|w| {
        a.archer_step(0.1, w, Vec3::Y * 100.0, 100.0);
    });
    let velocity = a.exec.pos - before;
    assert!(velocity.y < 0.0);
    assert!(a.exec.heading.abs() < 1e-5);
    assert_eq!(
        crate::world::movement::direction(a.exec.heading, velocity),
        Some(0.5)
    );
}

#[test]
fn dueling_circle_keeps_distance_and_reports_sideways_motion() {
    let mut a = actor();
    a.exec.pos = -Vec3::Y * 110.0;
    a.exec.combat = Some(combat::Combat::new(FormId(3)));
    a.exec.combat.as_mut().unwrap().maneuver = 3.0;
    let initial = a.exec.pos;
    with_world(|w| {
        for _ in 0..30 {
            let before = a.exec.pos;
            assert!(a.combat_maneuver(0.05, w, Vec3::ZERO, false));
            assert!((a.exec.pos.length() - 110.0).abs() < 0.01);
            let d = crate::world::movement::direction(a.exec.heading, a.exec.pos - before).unwrap();
            assert!((d - 0.25).abs().min((d - 0.75).abs()) < 0.02);
        }
    });
    assert!(initial.distance(a.exec.pos) > 60.0);
}
