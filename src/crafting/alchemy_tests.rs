use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
const HEAL: FormId = FormId(0x800);
const DAMAGE: FormId = FormId(0x801);
const BREATH: FormId = FormId(0x802);
const A: FormId = FormId(0x810);
const B: FormId = FormId(0x811);
const C: FormId = FormId(0x812);
const D: FormId = FormId(0x813);

fn condition(function: u16, parameter: u32, value: f32) -> [u8; 32] {
    let mut c = [0u8; 32];
    c[4..8].copy_from_slice(&value.to_le_bytes());
    c[8..10].copy_from_slice(&function.to_le_bytes());
    c[12..16].copy_from_slice(&parameter.to_le_bytes());
    c
}
fn perk(id: u32, point: u8, function: u8, value: f32, conditions: &[[u8; 32]]) -> RecordBuilder {
    let mut r = RecordBuilder::new(b"PERK", FormId(id))
        .field(b"PRKE", &[2, 0, 0])
        .field(b"DATA", &[point, function, 1])
        .field(b"PRKC", &[0]);
    for c in conditions {
        r = r.field(b"CTDA", c);
    }
    r.field(b"EPFT", &[1])
        .f32(b"EPFD", value)
        .field(b"PRKF", &[])
}
fn fixture() -> (std::path::PathBuf, Engine) {
    let dir = std::env::temp_dir().join(format!(
        "vrm-alchemy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mgef = |id: FormId, name: &str, flags: u32, cost: f32, keywords: &[u32]| {
        let mut data = [0u8; 0x98];
        data[..4].copy_from_slice(&flags.to_le_bytes());
        data[4..8].copy_from_slice(&cost.to_le_bytes());
        data[0x0c..0x10].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&esp::actor_value::HEALTH.to_le_bytes());
        RecordBuilder::new(b"MGEF", id)
            .zstring(b"FULL", name)
            .field(b"DATA", &data)
            .field(
                b"KWDA",
                &keywords
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect::<Vec<_>>(),
            )
    };
    let ingredient = |id: FormId, effects: &[(FormId, f32, u32)]| {
        let mut r = RecordBuilder::new(b"INGR", id)
            .zstring(b"FULL", "Ingredient")
            .field(
                b"DATA",
                &[1u32.to_le_bytes(), 0.1f32.to_le_bytes()].concat(),
            );
        for &(effect, mag, duration) in effects {
            r = r.form(b"EFID", effect).field(
                b"EFIT",
                &[
                    mag.to_le_bytes(),
                    0u32.to_le_bytes(),
                    duration.to_le_bytes(),
                ]
                .concat(),
            );
        }
        r
    };
    let nodes = [
        Node::top(
            b"MGEF",
            vec![
                mgef(
                    HEAL,
                    "Restore Health",
                    flags::NO_DURATION | flags::POWER_AFFECTS_MAGNITUDE,
                    0.5,
                    &[0x840, 0x841],
                ),
                mgef(
                    DAMAGE,
                    "Damage Health",
                    flags::HOSTILE | flags::DETRIMENTAL | flags::POWER_AFFECTS_MAGNITUDE,
                    3.,
                    &[0x842],
                ),
                mgef(
                    BREATH,
                    "Waterbreathing",
                    flags::NO_MAGNITUDE | flags::POWER_AFFECTS_DURATION,
                    4.5,
                    &[0x841],
                ),
            ],
        ),
        Node::top(
            b"INGR",
            vec![
                ingredient(A, &[(HEAL, 5., 0), (DAMAGE, 2., 1)]),
                ingredient(B, &[(HEAL, 5., 0), (DAMAGE, 2., 1)]),
                ingredient(C, &[(DAMAGE, 2., 1), (BREATH, 0., 5)]),
                ingredient(D, &[(BREATH, 0., 5)]),
            ],
        ),
        Node::top(
            b"PERK",
            vec![
                perk(0x820, 66, 3, 2., &[]),
                perk(0x821, 66, 3, 1.25, &[condition(501, 0x840, 1.)]),
                perk(
                    0x822,
                    66,
                    3,
                    1.25,
                    &[condition(501, 0x841, 1.), condition(500, 0, 0.)],
                ),
                perk(
                    0x823,
                    66,
                    3,
                    1.25,
                    &[condition(501, 0x842, 1.), condition(500, 0, 1.)],
                ),
                perk(0x824, 73, 1, 1., &[]),
            ],
        ),
    ];
    std::fs::write(
        dir.join("Alchemy.esp"),
        write_plugin(0, &[], "test", "synthetic alchemy", 0x900, &nodes),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir, "Alchemy.esp");
    for f in [A, B, C, D] {
        e.inventory_mut(PLAYER_REF).add(f, 10);
    }
    (dir, e)
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_potion_strength_flags_perk_queries_purity_and_names() {
    let (dir, mut e) = fixture();
    for (skill, mag) in [(15., 22.), (50., 25.), (100., 30.)] {
        e.set_actor_value(PLAYER_REF, esp::actor_value::ALCHEMY, skill);
        let result = e.potion_result(&[A, B]).unwrap();
        assert_eq!(
            result
                .effects
                .iter()
                .find(|ef| ef.effect == HEAL)
                .unwrap()
                .magnitude,
            mag
        );
        assert!(!result.poison);
        assert_eq!(result.name, "Potion of Restore Health");
    }
    e.add_perk(PLAYER_REF, FormId(0x820));
    e.add_perk(PLAYER_REF, FormId(0x821));
    e.add_perk(PLAYER_REF, FormId(0x822));
    let result = e.potion_result(&[A, B]).unwrap();
    assert_eq!(
        result
            .effects
            .iter()
            .find(|ef| ef.effect == HEAL)
            .unwrap()
            .magnitude,
        94.
    );
    e.add_perk(PLAYER_REF, FormId(0x823));
    let poison = e.potion_result(&[A, C]).unwrap();
    assert!(poison.poison);
    assert_eq!(poison.name, "Poison of Damage Health");
    assert_eq!(poison.effects[0].magnitude, 30.);
    e.add_perk(PLAYER_REF, FormId(0x824));
    assert_eq!(e.potion_result(&[A, B]).unwrap().effects.len(), 1);
    let breath = e.potion_result(&[C, D]).unwrap();
    assert_eq!(breath.effects[0].effect, BREATH);
    assert_eq!(breath.effects[0].magnitude, 0.);
    assert_eq!(breath.effects[0].duration, 75);
    assert!(e.crafting.alchemy_query.get().is_none());
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}
