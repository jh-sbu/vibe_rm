//! `Weapon.Fire` from a placed object, over generated records only. Engine
//! owns a renderer, so this needs a headless GPU adapter.

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Vec3;

const ARROW: FormId = FormId(0x800);
const BOW: FormId = FormId(0x801);
const STATIC: FormId = FormId(0x802);
const TRAP: FormId = FormId(0x803);

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_trap_fires_the_given_ammo_the_way_it_faces() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-weapon-fire-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut ammo = [0u8; 20];
    ammo[4..8].copy_from_slice(&super::AMMO_NON_BOLT.to_le_bytes());
    ammo[8..12].copy_from_slice(&10f32.to_le_bytes());
    let mut dnam = [0u8; 100];
    dnam[0] = super::ANIM_BOW;
    // Placed at (0, 0, 100), turned a quarter about Z.
    let mut data = [0u8; 24];
    data[8..12].copy_from_slice(&100f32.to_le_bytes());
    data[20..24].copy_from_slice(&std::f32::consts::FRAC_PI_2.to_le_bytes());
    let records = [
        Node::top(
            b"AMMO",
            vec![RecordBuilder::new(b"AMMO", ARROW).field(b"DATA", &ammo)],
        ),
        Node::top(
            b"WEAP",
            vec![RecordBuilder::new(b"WEAP", BOW).field(b"DNAM", &dnam)],
        ),
        Node::top(b"STAT", vec![RecordBuilder::new(b"STAT", STATIC)]),
        Node::top(
            b"REFR",
            vec![
                RecordBuilder::new(b"REFR", TRAP)
                    .form(b"NAME", STATIC)
                    .field(b"DATA", &data),
            ],
        ),
    ];
    std::fs::write(
        dir.join("Fire.esp"),
        write_plugin(0, &[], "test", "synthetic weapon fire", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir, "Fire.esp");
    let _ = std::fs::remove_dir_all(&dir);
    let bow = e.object_value(BOW);
    let trap = e.object_value(TRAP);
    // No ammo given and none worn: nothing flies.
    crate::script::natives::call(&mut e, "weapon", "fire", Some(&bow), &[trap.clone()]);
    assert!(e.projectiles.is_empty());
    let arrow = e.object_value(ARROW);
    crate::script::natives::call(&mut e, "weapon", "fire", Some(&bow), &[trap, arrow]);
    assert_eq!(e.projectiles.len(), 1);
    let p = &e.projectiles[0];
    assert_eq!(p.shooter, TRAP);
    assert_eq!(p.arrow.ammo, ARROW);
    // A quarter turn about Z points +Y (the reference's front) along -X or
    // +X; either way level and sideways.
    let d = p.vel.normalize();
    assert!(d.x.abs() > 0.99 && d.z.abs() < 0.01, "{d:?}");
    assert!((p.pos - Vec3::new(0.0, 0.0, 100.0)).length() < 20.0);
}
