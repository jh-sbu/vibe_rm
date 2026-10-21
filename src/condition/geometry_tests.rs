//! Positional condition functions over synthetic references; no game assets.
use super::*;
use crate::ai::schedule::Place;
use crate::world_state::Moved;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Vec3;

const A: FormId = FormId(0x810);
const B: FormId = FormId(0x811);
/// A reference with no record: moved into the world only.
const C: FormId = FormId(0x812);
const QUEST: FormId = FormId(0x820);
const CELL: FormId = FormId(0x830);
const CREATED_BASE: FormId = FormId(0x840);

const GET_POS: u16 = 6;
const GET_ANGLE: u16 = 8;
const GET_STARTING_POS: u16 = 10;
const GET_STARTING_ANGLE: u16 = 11;
const GET_HEADING_ANGLE: u16 = 99;
const GET_RELATIVE_ANGLE: u16 = 584;
const GET_LINE_OF_SIGHT: u16 = 27;
const IS_CLOSER: u16 = 577;
const X: u32 = b'X' as u32;
const Y: u32 = b'Y' as u32;
const Z: u32 = b'Z' as u32;

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `DATA` of a placed reference: position, then Euler angles (radians).
fn placed(id: FormId, pos: [f32; 3], rot: [f32; 3]) -> RecordBuilder {
    let mut data = Vec::new();
    for v in pos.into_iter().chain(rot) {
        data.extend_from_slice(&v.to_le_bytes());
    }
    RecordBuilder::new(b"REFR", id).field(b"DATA", &data)
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        Fixture(std::env::temp_dir().join(format!("vrm-geometry-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let records = [
        Node::top(b"MISC", vec![RecordBuilder::new(b"MISC", CREATED_BASE)]),
        Node::top(
            b"REFR",
            vec![
                placed(
                    A,
                    [100.0, 200.0, 300.0],
                    [0.0, 0.0, std::f32::consts::FRAC_PI_2],
                ),
                placed(B, [0.0, 0.0, 0.0], [0.0; 3]),
            ],
        ),
        Node::top(b"QUST", vec![RecordBuilder::new(b"QUST", QUEST)]),
    ];
    std::fs::write(
        dir.0.join("Geometry.esp"),
        write_plugin(0, &[], "test", "synthetic positions", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Geometry.esp");
    (dir, engine)
}

/// Puts a reference in the world at `pos` facing `rot` (radians), in one cell.
fn move_to(e: &mut Engine, r: FormId, pos: [f32; 3], rot: [f32; 3]) {
    e.world_state.moved.insert(
        r,
        Moved {
            place: Place::Interior(CELL),
            pos: Vec3::from(pos),
            rot: Vec3::from(rot),
        },
    );
}

/// The condition `func(p1, p2) == value` as a CTDA record would hold it.
fn ctda(func: u16, p1: u32, p2: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&p1.to_le_bytes());
    d[16..20].copy_from_slice(&p2.to_le_bytes());
    d
}

fn parsed(e: &Engine, bytes: &[u8]) -> Condition {
    let rec = e.lo.get(QUEST).unwrap();
    parse(&rec, bytes).unwrap()
}

/// Whether `func(p1, p2) == value` holds for `subject` (run on the subject).
fn holds(e: &Engine, subject: FormId, func: u16, p1: u32, p2: u32, value: f32) -> bool {
    let c = parsed(e, &ctda(func, p1, p2, value));
    evaluate(
        e,
        &[c],
        Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
fn get_pos_and_get_angle_read_the_current_state_along_an_axis() {
    let (_dir, mut e) = fixture();
    move_to(
        &mut e,
        A,
        [100.0, 200.0, 300.0],
        [0.0, 0.0, std::f32::consts::FRAC_PI_2],
    );
    assert!(holds(&e, A, GET_POS, Y, 0, 200.0));
    assert!(holds(&e, A, GET_POS, Z, 0, 300.0));
    assert!(!holds(&e, A, GET_POS, X, 0, 200.0));
    // Angles in degrees, as scripts read them.
    assert!(holds(&e, A, GET_ANGLE, Z, 0, 90.0));
    assert!(holds(&e, A, GET_ANGLE, X, 0, 0.0));
    // Not an axis: unsupported, so the condition fails.
    assert!(!holds(&e, A, GET_POS, 87, 0, 100.0));
    assert!(!holds(&e, A, GET_ANGLE, 87, 0, 0.0));
}

#[test]
fn starting_position_and_angle_are_the_editor_placement() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, A, [500.0, 0.0, 0.0], [0.0; 3]);
    assert!(holds(&e, A, GET_POS, X, 0, 500.0));
    assert!(holds(&e, A, GET_STARTING_POS, X, 0, 100.0));
    assert!(holds(&e, A, GET_STARTING_POS, Z, 0, 300.0));
    assert!(holds(&e, A, GET_ANGLE, Z, 0, 0.0));
    assert!(holds(&e, A, GET_STARTING_ANGLE, Z, 0, 90.0));
    // A reference with no record has no placement to start from.
    move_to(&mut e, C, [1.0, 2.0, 3.0], [0.0; 3]);
    assert!(!holds(&e, C, GET_STARTING_POS, X, 0, 1.0));
    assert!(!holds(&e, C, GET_STARTING_ANGLE, Z, 0, 0.0));
}

#[test]
fn starting_transform_of_world_created_refs_is_immutable_and_per_reference() {
    let (_dir, mut e) = fixture();
    move_to(
        &mut e,
        A,
        [12.0, 34.0, 56.0],
        [0.1, 0.2, std::f32::consts::FRAC_PI_2],
    );
    let first = e.create_ref(CREATED_BASE, A, false).unwrap();

    move_to(
        &mut e,
        B,
        [-3.0, 8.0, 13.0],
        [0.0, 0.0, std::f32::consts::PI],
    );
    let second = e.create_ref(CREATED_BASE, B, false).unwrap();
    assert_ne!(first, second);

    assert!(holds(&e, first, GET_STARTING_POS, X, 0, 12.0));
    assert!(holds(&e, first, GET_STARTING_POS, Y, 0, 34.0));
    assert!(holds(&e, first, GET_STARTING_ANGLE, Z, 0, 90.0));
    assert!(holds(&e, second, GET_STARTING_POS, X, 0, -3.0));
    assert!(holds(&e, second, GET_STARTING_ANGLE, Z, 0, 180.0));

    move_to(
        &mut e,
        first,
        [1000.0, 2000.0, 3000.0],
        [0.0, 0.0, std::f32::consts::PI],
    );
    assert!(holds(&e, first, GET_POS, X, 0, 1000.0));
    assert!(holds(&e, first, GET_ANGLE, Z, 0, 180.0));
    assert!(holds(&e, first, GET_STARTING_POS, X, 0, 12.0));
    assert!(holds(&e, first, GET_STARTING_ANGLE, Z, 0, 90.0));

    let dropped = e.create_object_at(
        CREATED_BASE,
        Place::Interior(CELL),
        Vec3::new(70.0, 80.0, 90.0),
        Vec3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2),
        1,
    );
    assert!(holds(&e, dropped, GET_STARTING_POS, Z, 0, 90.0));
    assert!(holds(&e, dropped, GET_STARTING_ANGLE, Z, 0, 90.0));

    let inside = e.create_ref(CREATED_BASE, A, true).unwrap();
    assert!(!holds(&e, inside, GET_STARTING_POS, X, 0, 0.0));
    assert!(!holds(&e, inside, GET_STARTING_ANGLE, Z, 0, 0.0));
}

#[test]
fn heading_angle_is_the_bearing_from_the_subjects_facing() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, B, [100.0, 0.0, 0.0], [0.0; 3]);
    // Facing north (+Y), the target to the east (+X) is 90 degrees to the right.
    assert!(holds(&e, C, GET_HEADING_ANGLE, B.0, 0, 90.0));
    // Turn the subject to face the target: 0.
    move_to(&mut e, C, [0.0; 3], [0.0, 0.0, std::f32::consts::FRAC_PI_2]);
    assert!(holds(&e, C, GET_HEADING_ANGLE, B.0, 0, 0.0));
    // Directly behind: -180 (the range is [-180, 180)).
    move_to(&mut e, B, [0.0, -100.0, 0.0], [0.0; 3]);
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    assert!(holds(&e, C, GET_HEADING_ANGLE, B.0, 0, -180.0));
    // A null reference names nothing to face.
    assert!(!holds(&e, C, GET_HEADING_ANGLE, 0, 0, 0.0));
}

#[test]
fn relative_angle_is_the_references_angle_less_the_subjects() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, B, [0.0; 3], [0.0, 0.0, std::f32::consts::FRAC_PI_2]);
    assert!(holds(&e, C, GET_RELATIVE_ANGLE, B.0, Z, 90.0));
    // The other way round: 270, in [0, 360).
    assert!(holds(&e, B, GET_RELATIVE_ANGLE, C.0, Z, 270.0));
    assert!(!holds(&e, C, GET_RELATIVE_ANGLE, 0, Z, 0.0));
}

#[test]
fn within_distance_reads_the_raw_float_outside_packages() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, B, [0.0, 100.0, 0.0], [0.0; 3]);
    // Second parameter: the distance as float bits.
    let within = |e: &Engine, d: f32| holds(e, C, GET_WITHIN_DISTANCE, B.0, d.to_bits(), 1.0);
    assert!(within(&e, 150.0));
    assert!(within(&e, 100.0));
    assert!(!within(&e, 50.0));
    // Out of the same worldspace: never within.
    e.world_state.moved.get_mut(&B).unwrap().place = Place::Interior(FormId(0x831));
    assert!(!within(&e, 1.0e6));
}

#[test]
fn within_distance_in_packages_reads_the_float_input() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, B, [0.0, 100.0, 0.0], [0.0; 3]);
    let input = |data: Option<PackageData>| {
        let mut c = parsed(&e, &ctda(GET_WITHIN_DISTANCE, B.0, 30, 1.0));
        // The first parameter is a package input, so the second one is too.
        c.p1_input = true;
        c.package_data = data;
        c
    };
    let run = |c: Condition| {
        evaluate(
            &e,
            &[c],
            Context {
                subject: Some(C),
                ..Default::default()
            },
        )
    };
    assert!(run(input(Some(PackageData::Number(150.0)))));
    assert!(!run(input(Some(PackageData::Number(50.0)))));
    // A missing or wrong-kind distance input fails the condition.
    assert!(!run(input(None)));
}

#[test]
fn is_closer_to_a_than_b_compares_the_two_distances() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, A, [10.0, 0.0, 0.0], [0.0; 3]);
    move_to(&mut e, B, [0.0, 100.0, 0.0], [0.0; 3]);
    assert!(holds(&e, C, IS_CLOSER, A.0, B.0, 1.0));
    assert!(!holds(&e, C, IS_CLOSER, B.0, A.0, 1.0));
    // Equally far is not closer.
    move_to(&mut e, B, [10.0, 0.0, 0.0], [0.0; 3]);
    assert!(!holds(&e, C, IS_CLOSER, A.0, B.0, 1.0));
}

#[test]
fn line_of_sight_needs_both_references_in_the_world() {
    let (_dir, mut e) = fixture();
    move_to(&mut e, C, [0.0; 3], [0.0; 3]);
    move_to(&mut e, A, [10.0, 0.0, 0.0], [0.0; 3]);
    // Nothing in the physics world blocks the line.
    assert!(holds(&e, C, GET_LINE_OF_SIGHT, A.0, 0, 1.0));
    // Neither a missing reference nor a null one has a line.
    assert!(!holds(&e, C, GET_LINE_OF_SIGHT, FormId(0x813).0, 0, 1.0));
    assert!(!holds(&e, C, GET_LINE_OF_SIGHT, 0, 0, 1.0));
}
