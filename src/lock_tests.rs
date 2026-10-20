//! Generated plugin records only; exercises the Papyrus call and crime access.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::Value;

const ROOM: FormId = FormId(0x800);
const HALL: FormId = FormId(0x801);
const DOOR: FormId = FormId(0x810);
const PARTNER: FormId = FormId(0x811);
const CHEST: FormId = FormId(0x812);
const OWNER: FormId = FormId(0x820);

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        Fixture(std::env::temp_dir().join(format!("vrm-locks-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let door = |id, to| {
        let mut xtel = [0; 32];
        xtel[..4].copy_from_slice(&FormId(to).0.to_le_bytes());
        RecordBuilder::new(b"REFR", id)
            .u32(b"NAME", 0x830)
            .field(b"XTEL", &xtel)
    };
    let cell = |id: FormId, refs: Vec<RecordBuilder>| Node::Group {
        label: id.0.to_le_bytes(),
        group_type: esp::group_type::CELL_CHILDREN,
        children: vec![Node::Group {
            label: id.0.to_le_bytes(),
            group_type: esp::group_type::CELL_TEMPORARY,
            children: refs.into_iter().map(Node::Record).collect(),
        }],
    };
    let records = [
        Node::top(b"DOOR", vec![RecordBuilder::new(b"DOOR", FormId(0x830))]),
        Node::top(b"CONT", vec![RecordBuilder::new(b"CONT", FormId(0x831))]),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", OWNER)]),
        Node::Group {
            label: *b"CELL",
            group_type: 0,
            children: vec![
                Node::Record(
                    RecordBuilder::new(b"CELL", ROOM)
                        .field(b"DATA", &[0x21, 0])
                        .form(b"XOWN", OWNER),
                ),
                cell(
                    ROOM,
                    vec![
                        door(PARTNER, DOOR.0),
                        RecordBuilder::new(b"REFR", CHEST).u32(b"NAME", 0x831),
                    ],
                ),
                Node::Record(RecordBuilder::new(b"CELL", HALL).field(b"DATA", &[1, 0])),
                cell(HALL, vec![door(DOOR, PARTNER.0)]),
            ],
        },
    ];
    std::fs::write(
        dir.0.join("Locks.esp"),
        write_plugin(0, &[], "test", "synthetic locks", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Locks.esp");
    (dir, engine)
}

fn lock(e: &mut Engine, r: FormId, args: &[Value]) {
    let this = e.object_value(r);
    crate::script::natives::call(e, "objectreference", "lock", Some(&this), args);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn owner_lock_changes_access_and_survives_saves() {
    let (_dir, mut e) = fixture();
    e.location = crate::engine::Location::Interior(ROOM);
    assert!(!e.trespassing()); // Authored public room.
    lock(&mut e, DOOR, &[Value::Bool(true), Value::Bool(true)]);
    assert!(e.is_locked(DOOR));
    assert!(e.trespassing());
    assert_eq!(e.scripts.cell_public.get(&ROOM), Some(&false));
    let file = crate::save::state::Snapshot::capture(&e).write();
    lock(&mut e, DOOR, &[Value::Bool(false), Value::Bool(true)]);
    assert!(!e.trespassing());
    crate::save::state::Snapshot::read(&file)
        .unwrap()
        .apply(&mut e)
        .unwrap();
    e.location = crate::engine::Location::Interior(ROOM);
    assert!(e.trespassing());
    assert!(e.is_locked(DOOR));
    // Default as-owner=false changes the lock alone, including omitted args.
    lock(&mut e, DOOR, &[Value::Bool(false)]);
    assert!(e.trespassing());
    lock(&mut e, DOOR, &[]);
    assert!(e.is_locked(DOOR));
    lock(&mut e, DOOR, &[Value::Bool(false), Value::Bool(true)]);
    assert!(!e.trespassing());
    lock(&mut e, CHEST, &[Value::Bool(true), Value::Bool(true)]);
    assert!(e.is_locked(CHEST));
    assert!(!e.trespassing()); // Containers don't change access.
}

#[test]
fn cell_access_section_is_optional_and_validated() {
    use crate::save::state::Snapshot;
    let mut s = Snapshot::default();
    s.cell_public.insert(ROOM, false);
    s.cell_public.insert(HALL, true);
    let mut file = s.write();
    assert_eq!(Snapshot::read(&file).unwrap().cell_public, s.cell_public);
    file.sections.retain(|(tag, _)| tag != b"CPUB");
    assert!(Snapshot::read(&file).unwrap().cell_public.is_empty());
    file.add(b"CPUB", vec![1, 0, 0, 0]);
    assert!(Snapshot::read(&file).is_err());
}
