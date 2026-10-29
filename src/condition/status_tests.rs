//! Status conditions (platform, race, items, ownership...) on generated records
//! only. Engine owns a renderer, so these need a headless GPU adapter.

use super::*;
use crate::world::inventory::Inventory;
use esp::writer::{Node, RecordBuilder, write_plugin};

pub(super) const SWORD_REF: FormId = FormId(0x814);
const PLAYABLE_RACE: FormId = FormId(0x830);
const CREATURE_RACE: FormId = FormId(0x831);
const HUMAN: FormId = FormId(0x832);
const BEAST: FormId = FormId(0x833);
const HUMAN_REF: FormId = FormId(0x834);
const BEAST_REF: FormId = FormId(0x835);
const KEYWORD: FormId = FormId(0x840);
const OTHER_KEYWORD: FormId = FormId(0x841);
const TAGGED_A: FormId = FormId(0x842);
const TAGGED_B: FormId = FormId(0x843);
const UNTAGGED: FormId = FormId(0x844);
const SWORD: FormId = FormId(0x850);
const GREATSWORD: FormId = FormId(0x851);
const BOUND_SWORD: FormId = FormId(0x852);
const BOUND_BATTLEAXE: FormId = FormId(0x853);
const BOW: FormId = FormId(0x854);
const GUILD: FormId = FormId(0x860);
const CELL_A: FormId = FormId(0x861);
const CELL_B: FormId = FormId(0x862);
const CELL_C: FormId = FormId(0x863);
const PLAIN_REF: FormId = FormId(0x864);
const GUILD_REF: FormId = FormId(0x865);
const FREE_REF: FormId = FormId(0x866);
const HUMAN_IN_A: FormId = FormId(0x867);
const BEAST_IN_A: FormId = FormId(0x868);
const HUMAN_IN_B: FormId = FormId(0x869);
const HUMAN_IN_C: FormId = FormId(0x86A);
const CRATE: FormId = FormId(0x86B);
const HOLD: FormId = FormId(0x870);
const INN: FormId = FormId(0x871);
const FORT: FormId = FormId(0x872);
const HOLD_KEYWORD: FormId = FormId(0x873);
const LIST_OF_INNS: FormId = FormId(0x874);
const WOLVES: FormId = FormId(0x880);
const HUNTERS: FormId = FormId(0x881);
const BEARS: FormId = FormId(0x882);
const BOSS: FormId = FormId(0x890);
const GUARD_TYPE: FormId = FormId(0x891);
const QUEST: FormId = FormId(0x892);
const REGION: FormId = FormId(0x8A0);
const OTHER_REGION: FormId = FormId(0x8A1);
const ZONE: FormId = FormId(0x8A2);
const OWN_ZONE: FormId = FormId(0x8A3);
const TORCH: FormId = FormId(0x8B0);
const LAMP: FormId = FormId(0x8B1);
const TORCH_REF: FormId = FormId(0x8B2);
const LAMP_REF: FormId = FormId(0x8B3);
const SHIRT: FormId = FormId(0x8B4);
const ROBE: FormId = FormId(0x8B5);
const MAIL: FormId = FormId(0x8B6);
const PLAGUE: FormId = FormId(0x8B7);
const PLAGUE_EFFECT: FormId = FormId(0x8B8);
const HIGH_REF: FormId = FormId(0x8B9);

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
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-status-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let race = |id, flags: u32| {
        let mut data = [0u8; 36];
        data[32..36].copy_from_slice(&flags.to_le_bytes());
        RecordBuilder::new(b"RACE", id).field(b"DATA", &data)
    };
    let npc = |id, race| {
        RecordBuilder::new(b"NPC_", id)
            .field(b"ACBS", &[0; 24])
            .form(b"RNAM", race)
    };
    // `DNAM`: the animation type, and the bound weapon flag among flags2.
    let weapon = |id, anim: u8, bound: bool| {
        let mut dnam = [0u8; 100];
        dnam[0] = anim;
        dnam[40..44].copy_from_slice(&(if bound { 0x2000u32 } else { 0 }).to_le_bytes());
        RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam)
    };
    let actor = |id, base| RecordBuilder::new(b"ACHR", id).form(b"NAME", base);
    // An interior cell with its (temporary) references.
    // `XNAM`: the other faction, a modifier and the group combat reaction.
    let xnam = |other: FormId, reaction: u32| {
        let mut d = other.0.to_le_bytes().to_vec();
        d.extend_from_slice(&0i32.to_le_bytes());
        d.extend_from_slice(&reaction.to_le_bytes());
        d
    };
    // `ACSR`: static references with their ref types (and a grid cell each).
    let acsr = |refs: &[(FormId, FormId)]| {
        refs.iter()
            .flat_map(|&(ty, r)| [ty.0, r.0, 0, 0])
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<u8>>()
    };
    // `DATA` of a placed reference: its position, then its rotation.
    let position = |x: f32, y: f32, z: f32| {
        [x, y, z, 0.0, 0.0, 0.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    // A light's `DATA`: flags at 12 (0x2: can be carried).
    let light = |id, flags: u32| {
        let mut data = [0u8; 48];
        data[12..16].copy_from_slice(&flags.to_le_bytes());
        RecordBuilder::new(b"LIGH", id).field(b"DATA", &data)
    };
    // Worn armor: its slots and type in `BOD2`, its value in `DATA`.
    let armor = |id, kind: u32, value: i32| {
        let mut bod2 = [0u8; 8];
        bod2[4..8].copy_from_slice(&kind.to_le_bytes());
        let mut data = value.to_le_bytes().to_vec();
        data.extend_from_slice(&1.0f32.to_le_bytes());
        RecordBuilder::new(b"ARMO", id)
            .field(b"BOD2", &bod2)
            .field(b"DATA", &data)
    };
    // A disease: a constant value-modifying effect.
    let plague = {
        let mut mgef = [0; 0x6c];
        mgef[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        mgef[0x40..0x44].copy_from_slice(&crate::magic::archetype::VALUE_MODIFIER.to_le_bytes());
        mgef[0x44..0x48].copy_from_slice(&esp::actor_value::SNEAK.to_le_bytes());
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&1u32.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&5.0f32.to_le_bytes());
        (
            RecordBuilder::new(b"MGEF", PLAGUE_EFFECT).field(b"DATA", &mgef),
            RecordBuilder::new(b"SPEL", PLAGUE)
                .field(b"SPIT", &spit)
                .u32(b"EFID", PLAGUE_EFFECT.0)
                .field(b"EFIT", &efit),
        )
    };
    let locations = [(CELL_A, INN), (CELL_B, FORT)];
    let cell = |id: FormId, owner: Option<FormId>, refs: Vec<RecordBuilder>| {
        let mut record = RecordBuilder::new(b"CELL", id).field(b"DATA", &[1, 0]);
        if let Some(&(_, location)) = locations.iter().find(|&&(c, _)| c == id) {
            record = record.form(b"XLCN", location);
        }
        if let Some(owner) = owner {
            record = record.form(b"XOWN", owner);
        }
        if id == CELL_A {
            record = record.form(b"XCLR", REGION);
        }
        if id == CELL_B {
            record = record.form(b"XEZN", ZONE);
        }
        [
            Node::Record(record),
            Node::Group {
                label: id.0.to_le_bytes(),
                group_type: esp::group_type::CELL_CHILDREN,
                children: vec![Node::Group {
                    label: id.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_TEMPORARY,
                    children: refs.into_iter().map(Node::Record).collect(),
                }],
            },
        ]
    };
    let placed = |id, base| RecordBuilder::new(b"REFR", id).form(b"NAME", base);
    let mut snam = [0u8; 8];
    snam[..4].copy_from_slice(&GUILD.0.to_le_bytes());
    let mut cells = Vec::new();
    cells.extend(cell(
        CELL_A,
        Some(HUMAN),
        vec![
            placed(PLAIN_REF, CRATE).field(b"XSCL", &1.5f32.to_le_bytes()),
            placed(GUILD_REF, CRATE)
                .form(b"XOWN", GUILD)
                .form(b"XEZN", OWN_ZONE),
            RecordBuilder::new(b"ACHR", HUMAN_IN_A).form(b"NAME", HUMAN),
            RecordBuilder::new(b"ACHR", BEAST_IN_A).form(b"NAME", BEAST),
        ],
    ));
    cells.extend(cell(
        CELL_B,
        None,
        vec![
            placed(FREE_REF, CRATE),
            // Fifty units above the origin.
            placed(HIGH_REF, CRATE).field(b"DATA", &position(0.0, 0.0, 50.0)),
            RecordBuilder::new(b"ACHR", HUMAN_IN_B).form(b"NAME", HUMAN),
        ],
    ));
    cells.extend(cell(
        CELL_C,
        Some(GUILD),
        vec![
            RecordBuilder::new(b"ACHR", HUMAN_IN_C).form(b"NAME", HUMAN),
            placed(TORCH_REF, TORCH),
            placed(LAMP_REF, LAMP),
        ],
    ));
    let records = [
        Node::Group {
            label: *b"CELL",
            group_type: esp::group_type::TOP,
            children: vec![Node::Group {
                label: 8i32.to_le_bytes(),
                group_type: esp::group_type::INTERIOR_BLOCK,
                children: vec![Node::Group {
                    label: 2i32.to_le_bytes(),
                    group_type: esp::group_type::INTERIOR_SUB_BLOCK,
                    children: cells,
                }],
            }],
        },
        // Hold > inn, and hold > fort; the hold has the keyword.
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", HOLD).form(b"KWDA", HOLD_KEYWORD),
                // The inn's two actors are bosses; the fort's is a guard.
                RecordBuilder::new(b"LCTN", INN)
                    .form(b"PNAM", HOLD)
                    .field(b"ACSR", &acsr(&[(BOSS, HUMAN_IN_A), (BOSS, BEAST_IN_A)])),
                RecordBuilder::new(b"LCTN", FORT)
                    .form(b"PNAM", HOLD)
                    .field(b"ACSR", &acsr(&[(GUARD_TYPE, HUMAN_IN_B)])),
            ],
        ),
        Node::top(
            b"FLST",
            vec![RecordBuilder::new(b"FLST", LIST_OF_INNS).form(b"LNAM", INN)],
        ),
        Node::top(
            b"FACT",
            vec![
                RecordBuilder::new(b"FACT", GUILD),
                // Wolves hate hunters and befriend bears; hunters list nothing.
                RecordBuilder::new(b"FACT", WOLVES)
                    .field(b"XNAM", &xnam(HUNTERS, 1))
                    .field(b"XNAM", &xnam(BEARS, 3)),
                RecordBuilder::new(b"FACT", HUNTERS),
                RecordBuilder::new(b"FACT", BEARS),
            ],
        ),
        Node::top(b"MISC", vec![RecordBuilder::new(b"MISC", CRATE)]),
        Node::top(
            b"RACE",
            vec![
                race(PLAYABLE_RACE, 0x1),
                // `ActorTypeCreature`.
                race(CREATURE_RACE, 0).form(b"KWDA", FormId(0x13795)),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![
                npc(HUMAN, PLAYABLE_RACE).field(b"SNAM", &snam),
                npc(BEAST, CREATURE_RACE),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![actor(HUMAN_REF, HUMAN), actor(BEAST_REF, BEAST)],
        ),
        Node::top(
            b"KYWD",
            vec![
                RecordBuilder::new(b"KYWD", KEYWORD),
                RecordBuilder::new(b"KYWD", OTHER_KEYWORD),
                RecordBuilder::new(b"KYWD", HOLD_KEYWORD),
            ],
        ),
        Node::top(
            b"MISC",
            vec![
                RecordBuilder::new(b"MISC", TAGGED_A).form(b"KWDA", KEYWORD),
                RecordBuilder::new(b"MISC", TAGGED_B).form(b"KWDA", KEYWORD),
                RecordBuilder::new(b"MISC", UNTAGGED).form(b"KWDA", OTHER_KEYWORD),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![
                weapon(SWORD, 1, false),
                weapon(GREATSWORD, 5, false),
                weapon(BOUND_SWORD, 1, true),
                weapon(BOUND_BATTLEAXE, 6, true),
                weapon(BOW, 7, false),
            ],
        ),
        Node::top(b"LIGH", vec![light(TORCH, 0xB), light(LAMP, 0x9)]),
        Node::top(
            b"ARMO",
            vec![armor(SHIRT, 2, 25), armor(ROBE, 2, 40), armor(MAIL, 1, 500)],
        ),
        Node::top(b"MGEF", vec![plague.0]),
        Node::top(b"SPEL", vec![plague.1]),
        Node::top(b"REFR", vec![RecordBuilder::new(b"REFR", SWORD_REF)]),
    ];
    std::fs::write(
        f.0.join("Status.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic status conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Status.esp");
    let mut bag = Inventory::default();
    bag.add(TAGGED_A, 2);
    bag.add(TAGGED_B, 3);
    bag.add(UNTAGGED, 7);
    e.inventories.insert(HUMAN_REF, bag);
    (f, e)
}

/// `func(param) == value` as a CTDA record's bytes.
fn ctda(func: u16, param: u32, value: f32) -> [u8; 32] {
    ctda2(func, param, 0, value)
}

/// As `ctda`, with a second parameter.
fn ctda2(func: u16, param: u32, second: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[16..20].copy_from_slice(&second.to_le_bytes());
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&param.to_le_bytes());
    d
}

/// Whether `func(param) == value` holds for `subject`.
fn is(e: &Engine, subject: FormId, func: u16, param: u32, value: f32) -> bool {
    is2(e, subject, func, param, 0, value)
}

/// As `is`, with a second parameter.
fn is2(e: &Engine, subject: FormId, func: u16, param: u32, second: u32, value: f32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda2(func, param, second, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn unmounted_actor_passes_riding_mount_zero_without_relaxing_unknown_conditions() {
    let (_fixture, e) = fixture();
    for subject in [PLAYER_REF, HUMAN_REF, BEAST_REF] {
        assert!(is(&e, subject, 327, 0, 0.));
        assert!(!is(&e, subject, 327, 0, 1.));
        assert!(!is(&e, subject, u16::MAX, 0, 0.));
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn the_platform_is_a_pc_with_gore() {
    let (_f, e) = fixture();
    assert!(is(&e, PLAYER_REF, 521, 0, 1.0)); // IsWin32
    assert!(is(&e, PLAYER_REF, 309, 0, 0.0)); // IsXBox
    assert!(is(&e, PLAYER_REF, 520, 0, 0.0)); // IsPS3
    assert!(is(&e, PLAYER_REF, 547, 0, 0.0)); // IsGoreDisabled
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_is_playable_race_reads_the_races_playable_flag() {
    let (_f, e) = fixture();
    assert!(is(&e, HUMAN_REF, 254, 0, 1.0));
    assert!(!is(&e, BEAST_REF, 254, 0, 1.0));
    // Nothing to ask about: no race, not playable.
    assert!(!is(&e, SWORD_REF, 254, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_keyword_item_count_totals_the_carried_items_with_the_keyword() {
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 465, KEYWORD.0, 5.0));
    assert!(is(&e, HUMAN_REF, 465, OTHER_KEYWORD.0, 7.0));
    // A stack taken away stops counting; a keyword nothing has is zero.
    e.inventories
        .get_mut(&HUMAN_REF)
        .unwrap()
        .remove_split(TAGGED_A, 2, None);
    assert!(is(&e, HUMAN_REF, 465, KEYWORD.0, 3.0));
    assert!(is(&e, HUMAN_REF, 465, 0x845, 0.0));
    assert!(is(&e, BEAST_REF, 465, KEYWORD.0, 0.0));
}

/// `actor` holds `item` in `hand`.
fn hold(e: &mut Engine, actor: FormId, item: FormId, hands: &[usize]) {
    let lo = &e.lo;
    let inv = e.inventories.entry(actor).or_default();
    inv.add(item, 1);
    inv.hold(lo, item, hands);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn two_handed_weapons_are_the_two_handed_animation_types() {
    use crate::spells::RIGHT;
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
    hold(&mut e, HUMAN_REF, SWORD, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
    hold(&mut e, HUMAN_REF, BOUND_BATTLEAXE, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 1.0));
    hold(&mut e, HUMAN_REF, GREATSWORD, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 1.0));
    hold(&mut e, HUMAN_REF, BOW, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn bound_weapons_are_found_in_the_hand_the_source_names() {
    use crate::spells::{LEFT, RIGHT};
    let (_f, mut e) = fixture();
    hold(&mut e, HUMAN_REF, BOUND_SWORD, &[RIGHT]);
    hold(&mut e, HUMAN_REF, SWORD, &[LEFT]);
    // Sources: 0 the left hand, 1 the right, 2 the voice.
    assert!(is(&e, HUMAN_REF, 706, 1, 1.0));
    assert!(is(&e, HUMAN_REF, 706, 0, 0.0));
    assert!(is(&e, HUMAN_REF, 706, 2, 0.0));
    assert!(is(&e, BEAST_REF, 706, 1, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_owner_asks_who_owns_the_reference() {
    let (_f, mut e) = fixture();
    // Owned through its cell, then by its own record, which wins.
    assert!(is(&e, PLAIN_REF, 278, HUMAN.0, 1.0));
    assert!(!is(&e, PLAIN_REF, 278, GUILD.0, 1.0));
    assert!(is(&e, GUILD_REF, 278, GUILD.0, 1.0));
    assert!(!is(&e, GUILD_REF, 278, HUMAN.0, 1.0));
    // Unowned, and no owner named means the player.
    assert!(!is(&e, FREE_REF, 278, HUMAN.0, 1.0));
    assert!(!is(&e, FREE_REF, 278, 0, 1.0));
    e.scripts.owners.insert(FREE_REF, Some(FormId(0x7)));
    assert!(is(&e, FREE_REF, 278, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_cell_owner_asks_who_owns_the_cell() {
    let (_f, e) = fixture();
    assert!(is2(&e, HUMAN_REF, 280, CELL_A.0, HUMAN.0, 1.0));
    assert!(!is2(&e, HUMAN_REF, 280, CELL_A.0, GUILD.0, 1.0));
    assert!(is2(&e, HUMAN_REF, 280, CELL_C.0, GUILD.0, 1.0));
    assert!(!is2(&e, HUMAN_REF, 280, CELL_B.0, HUMAN.0, 1.0));
    assert!(!is2(&e, HUMAN_REF, 280, CELL_B.0, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_in_my_owned_cell_follows_the_actor_or_its_faction() {
    let (_f, e) = fixture();
    assert!(is(&e, HUMAN_IN_A, 146, 0, 1.0));
    assert!(!is(&e, BEAST_IN_A, 146, 0, 1.0));
    assert!(!is(&e, HUMAN_IN_B, 146, 0, 1.0));
    // A cell the faction owns is its members'.
    assert!(is(&e, HUMAN_IN_C, 146, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_xp_for_next_level_is_the_levels_whole_cost() {
    let (_f, mut e) = fixture();
    // fXPLevelUpBase 75 and fXPLevelUpMult 25 at level 5.
    e.skills.level = 5;
    e.skills.level_xp = 40.0;
    assert!(is(&e, PLAYER_REF, 530, 0, 200.0));
    // A level up waiting makes the next one cost for the level after it.
    e.skills.pending = 1;
    assert!(is(&e, PLAYER_REF, 530, 0, 225.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_pc_sleeping_follows_the_hours_passing_in_bed() {
    let (_f, mut e) = fixture();
    assert!(is(&e, PLAYER_REF, 175, 0, 0.0));
    // Waiting is not sleeping.
    e.menu = Some(crate::items::Menu::SleepWait { sleeping: false });
    e.start_waiting(1);
    assert!(is(&e, PLAYER_REF, 175, 0, 0.0));
    e.menu = Some(crate::items::Menu::SleepWait { sleeping: true });
    e.start_waiting(1);
    assert!(is(&e, PLAYER_REF, 175, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_pc_1st_person_follows_the_camera() {
    let (_f, mut e) = fixture();
    e.third_person = false;
    assert!(is(&e, PLAYER_REF, 391, 0, 1.0));
    e.third_person = true;
    assert!(is(&e, PLAYER_REF, 391, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn same_location_as_a_reference_compares_the_parents_with_the_keyword() {
    let (_f, e) = fixture();
    // An inn and a fort are different places, in the same hold.
    assert!(is2(&e, PLAIN_REF, 180, FREE_REF.0, HOLD_KEYWORD.0, 1.0));
    assert!(is2(&e, PLAIN_REF, 603, FREE_REF.0, HOLD_KEYWORD.0, 1.0));
    assert!(!is2(&e, PLAIN_REF, 180, FREE_REF.0, OTHER_KEYWORD.0, 1.0));
    assert!(!is2(&e, PLAIN_REF, 603, 0, HOLD_KEYWORD.0, 1.0));
    // The actors in the inn share it with a crate there.
    assert!(is2(&e, PLAIN_REF, 603, HUMAN_IN_A.0, HOLD_KEYWORD.0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn location_form_lists_and_loaded_locations() {
    let (_f, mut e) = fixture();
    assert!(is(&e, PLAIN_REF, 444, LIST_OF_INNS.0, 1.0));
    assert!(!is(&e, FREE_REF, 444, LIST_OF_INNS.0, 1.0));
    // Nothing is loaded until a cell is.
    assert!(is(&e, PLAIN_REF, 625, HOLD.0, 0.0));
    e.enter_interior(CELL_A, None).unwrap();
    assert!(is(&e, PLAIN_REF, 625, INN.0, 1.0));
    assert!(is(&e, PLAIN_REF, 625, HOLD.0, 1.0));
    assert!(is(&e, PLAIN_REF, 625, FORT.0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_faction_combat_reaction_reads_the_first_factions_list() {
    let (_f, e) = fixture();
    let reaction =
        |from: FormId, to: FormId, want: f32| is2(&e, PLAYER_REF, 410, from.0, to.0, want);
    assert!(reaction(WOLVES, HUNTERS, 1.0));
    assert!(reaction(WOLVES, BEARS, 3.0));
    // Not mutual unless the other lists the first, and unlisted is neutral.
    assert!(reaction(HUNTERS, WOLVES, 0.0));
    assert!(reaction(WOLVES, GUILD, 0.0));
    // A faction is its own ally.
    assert!(reaction(HUNTERS, HUNTERS, 2.0));
}

/// Whether `func(param, second) == value` holds for the player, in a quest's
/// context (so its aliases resolve).
fn is_in_quest(e: &Engine, func: u16, param: u32, second: u32, value: f32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda2(func, param, second, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(PLAYER_REF),
            quest: Some(QUEST),
            ..Default::default()
        },
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn ref_type_counts_split_a_locations_living_and_dead() {
    let (_f, mut e) = fixture();
    // 591 dead, 592 alive: the inn's two bosses, then its parent's (none in the
    // hold itself, and the fort's guard isn't a boss).
    assert!(is2(&e, PLAYER_REF, 592, INN.0, BOSS.0, 2.0));
    assert!(is2(&e, PLAYER_REF, 591, INN.0, BOSS.0, 0.0));
    assert!(is2(&e, PLAYER_REF, 592, FORT.0, BOSS.0, 0.0));
    // The hold counts what is inside it.
    assert!(is2(&e, PLAYER_REF, 592, HOLD.0, BOSS.0, 2.0));
    assert!(is2(&e, PLAYER_REF, 592, HOLD.0, GUARD_TYPE.0, 1.0));
    // A killed one moves over, loaded or not.
    let place = crate::ai::schedule::Place::Interior(CELL_A);
    e.world_state
        .dead
        .insert(HUMAN_IN_A, (place, glam::Vec3::ZERO));
    assert!(is2(&e, PLAYER_REF, 592, INN.0, BOSS.0, 1.0));
    assert!(is2(&e, PLAYER_REF, 591, INN.0, BOSS.0, 1.0));
    assert!(is2(&e, PLAYER_REF, 591, HOLD.0, BOSS.0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn location_alias_forms_read_the_aliased_location() {
    let (_f, mut e) = fixture();
    // 601 alive, 600 dead, 608 keyword data: through alias 3, which holds the inn.
    assert!(!is_in_quest(&e, 601, 3, BOSS.0, 2.0));
    e.scripts
        .quests
        .entry(QUEST)
        .or_default()
        .aliases
        .insert(3, INN);
    assert!(is_in_quest(&e, 601, 3, BOSS.0, 2.0));
    assert!(is_in_quest(&e, 600, 3, BOSS.0, 0.0));
    e.location_keyword_data.insert((INN, KEYWORD), 7.0);
    assert!(is_in_quest(&e, 608, 3, KEYWORD.0, 7.0));
    assert!(is_in_quest(&e, 608, 3, OTHER_KEYWORD.0, 0.0));
    // An empty alias holds no location, so nothing is in it.
    assert!(is_in_quest(&e, 601, 4, BOSS.0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_player_in_region_reads_the_players_cells_regions() {
    let (_f, mut e) = fixture();
    // Nowhere in particular: in no region.
    assert!(is(&e, PLAYER_REF, 117, REGION.0, 0.0));
    e.enter_interior(CELL_A, None).unwrap();
    assert!(is(&e, PLAYER_REF, 117, REGION.0, 1.0));
    assert!(is(&e, PLAYER_REF, 117, OTHER_REGION.0, 0.0));
    // It is the player's cell, whoever is asked.
    assert!(is(&e, HUMAN_IN_B, 117, REGION.0, 1.0));
    e.enter_interior(CELL_B, None).unwrap();
    assert!(is(&e, PLAYER_REF, 117, REGION.0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_in_zone_reads_the_references_zone_else_its_cells() {
    let (_f, mut e) = fixture();
    // A reference's own zone beats its cell's; cell B's other occupants and
    // references use the cell's, and cell A's own none.
    assert!(is(&e, GUILD_REF, 445, OWN_ZONE.0, 1.0));
    assert!(is(&e, GUILD_REF, 445, ZONE.0, 0.0));
    assert!(is(&e, FREE_REF, 445, ZONE.0, 1.0));
    assert!(is(&e, HUMAN_IN_B, 445, ZONE.0, 1.0));
    assert!(is(&e, PLAIN_REF, 445, ZONE.0, 0.0));
    // The player's is the cell they are in.
    e.enter_interior(CELL_B, None).unwrap();
    assert!(is(&e, PLAYER_REF, 445, ZONE.0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn menu_mode_and_time_passing_follow_the_pausing_menus() {
    let (_f, mut e) = fixture();
    assert!(is(&e, PLAYER_REF, 36, 0, 0.0)); // MenuMode
    assert!(is(&e, PLAYER_REF, 265, 0, 1.0)); // IsTimePassing
    e.menu = Some(crate::items::Menu::SleepWait { sleeping: false });
    assert!(is(&e, PLAYER_REF, 36, 0, 1.0));
    assert!(is(&e, PLAYER_REF, 265, 0, 0.0));
    // Waiting passes the time from inside the menu.
    e.start_waiting(1);
    assert!(is(&e, PLAYER_REF, 265, 0, 1.0));
    // A menu number is unsupported: it fails whatever it is compared with.
    assert!(!is(&e, PLAYER_REF, 36, 1002, 1.0));
    assert!(!is(&e, PLAYER_REF, 36, 1002, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_player_controls_disabled_asks_about_the_selected_controls() {
    let (_f, mut e) = fixture();
    let asks =
        |e: &Engine, movement, fighting, want| is2(e, PLAYER_REF, 98, movement, fighting, want);
    assert!(asks(&e, 0, 0, 0.0));
    assert!(asks(&e, 1, 0, 0.0));
    e.disabled_controls.movement = true;
    assert!(asks(&e, 0, 0, 1.0));
    assert!(asks(&e, 1, 0, 1.0));
    assert!(asks(&e, 0, 1, 0.0));
    // Both selected: both must be disabled.
    assert!(asks(&e, 1, 1, 0.0));
    e.disabled_controls.fighting = true;
    assert!(asks(&e, 1, 1, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn real_time_conditions_follow_the_clock() {
    let (_f, mut e) = fixture();
    e.scripts.wall_time = 5400.0;
    assert!(is(&e, PLAYER_REF, 716, 0, 1.5)); // GetRealHoursPassed
    e.frame_seconds = 0.25;
    assert!(is(&e, PLAYER_REF, 12, 0, 0.25)); // GetSecondsPassed
}

/// As `is`, in a context that names the idle's (or activation's) target.
fn is_towards(e: &Engine, subject: FormId, target: FormId, func: u16, value: f32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, 0, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            target: Some(target),
            ..Default::default()
        },
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn creatures_and_scale_come_from_the_race_and_the_reference() {
    let (_f, e) = fixture();
    assert!(is(&e, BEAST_REF, 64, 0, 1.0)); // GetIsCreature
    assert!(is(&e, HUMAN_REF, 64, 0, 0.0));
    // A placed reference's scale; an unscaled one is 1.
    assert!(is(&e, PLAIN_REF, 24, 0, 1.5)); // GetScale
    assert!(is(&e, FREE_REF, 24, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn faction_relation_is_the_strongest_reaction_between_the_actors_factions() {
    let (_f, mut e) = fixture();
    // The wolf (BEAST_IN_A) and a bear (BEAST_REF) have no factions of their own.
    e.set_faction_rank(BEAST_IN_A, WOLVES, 4);
    e.set_faction_rank(BEAST_REF, BEARS, 0);
    let relation = |e: &Engine, from: FormId, to: u32, want: f32| is(e, from, 449, to, want);
    // Wolves hate hunters (the guild's humans are not) and befriend bears; a
    // bear that is also a wolf is a friend still, the strongest of its factions.
    e.set_faction_rank(HUMAN_IN_B, HUNTERS, 0);
    assert!(relation(&e, BEAST_IN_A, HUMAN_IN_B.0, 1.0));
    assert!(relation(&e, BEAST_IN_A, HUMAN_IN_C.0, 0.0));
    assert!(relation(&e, BEAST_IN_A, BEAST_REF.0, 3.0));
    e.set_faction_rank(BEAST_REF, WOLVES, 1);
    assert!(relation(&e, BEAST_IN_A, BEAST_REF.0, 3.0));
    // Hunters list nothing, so they are neutral to wolves.
    assert!(relation(&e, HUMAN_IN_B, BEAST_IN_A.0, 0.0));
    // No actor: the player, who is in no faction here.
    assert!(relation(&e, BEAST_IN_A, 0, 0.0));
    // The difference in ranks, with -1 for a non-member.
    let difference = |a: FormId, b: FormId, want: f32| is2(&e, a, 60, WOLVES.0, b.0, want);
    assert!(difference(BEAST_IN_A, BEAST_REF, 3.0));
    assert!(difference(BEAST_IN_A, HUMAN_IN_C, 5.0));
    assert!(difference(HUMAN_IN_C, BEAST_IN_A, -5.0));
    assert!(difference(HUMAN_IN_C, HUMAN_IN_B, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn weapon_anim_type_follows_the_weapon_in_hand() {
    use crate::spells::{LEFT, RIGHT};
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 108, 0, 0.0)); // hand to hand
    hold(&mut e, HUMAN_REF, BOW, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 108, 0, 7.0));
    hold(&mut e, HUMAN_REF, GREATSWORD, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 108, 0, 5.0));
    // Only a left-hand weapon is still the animation type.
    hold(&mut e, BEAST_REF, SWORD, &[LEFT]);
    assert!(is(&e, BEAST_REF, 108, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn carryable_objects_are_items_and_carried_lights() {
    let (_f, e) = fixture();
    assert!(is(&e, PLAIN_REF, 487, 0, 1.0)); // a crate
    assert!(is(&e, TORCH_REF, 487, 0, 1.0));
    assert!(!is(&e, LAMP_REF, 487, 0, 1.0));
    assert!(!is(&e, HUMAN_IN_A, 487, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn heights_are_the_other_references_less_the_subjects() {
    let (_f, e) = fixture();
    // 720 asks about the reference named; 680 about the idle's target.
    assert!(is(&e, PLAIN_REF, 720, HIGH_REF.0, 50.0));
    assert!(is(&e, HIGH_REF, 720, PLAIN_REF.0, -50.0));
    assert!(is_towards(&e, PLAIN_REF, HIGH_REF, 680, 50.0));
    // With no target there is nothing to measure.
    assert!(!is(&e, PLAIN_REF, 680, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn clothing_value_totals_the_worn_clothes_only() {
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 41, 0, 0.0)); // GetClothingValue
    let inv = e.inventories.entry(HUMAN_REF).or_default();
    for item in [SHIRT, ROBE, MAIL] {
        inv.add(item, 1);
        inv.wear(item);
    }
    assert!(is(&e, HUMAN_REF, 41, 0, 65.0));
    // Carried but not worn doesn't count.
    e.inventories.get_mut(&HUMAN_REF).unwrap().take_off(ROBE);
    assert!(is(&e, HUMAN_REF, 41, 0, 25.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_disease_is_a_disease_effect_on_the_subject() {
    let (_f, mut e) = fixture();
    assert!(is(&e, PLAYER_REF, 39, 0, 0.0)); // GetDisease
    e.apply_item(PLAGUE, None, PLAYER_REF);
    assert!(is(&e, PLAYER_REF, 39, 0, 1.0));
    assert!(is(&e, HUMAN_REF, 39, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn cell_parameter_torch_and_cannibal_conditions() {
    use crate::spells::LEFT;
    let (_f, mut e) = fixture();
    // GetInCellParam(cell, reference).
    assert!(is2(&e, PLAIN_REF, 230, CELL_A.0, HUMAN_IN_A.0, 1.0));
    assert!(is2(&e, PLAIN_REF, 230, CELL_B.0, HUMAN_IN_A.0, 0.0));
    // IsActorUsingATorch: a torch in the hand; nobody eats corpses yet.
    assert!(is(&e, HUMAN_REF, 306, 0, 0.0));
    hold(&mut e, HUMAN_REF, TORCH, &[LEFT]);
    assert!(is(&e, HUMAN_REF, 306, 0, 1.0));
    assert!(is(&e, HUMAN_REF, 227, 0, 0.0)); // GetCannibal
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_running_follows_the_players_gait_and_nobody_is_unconscious_standing() {
    let (_f, mut e) = fixture();
    assert!(is(&e, PLAYER_REF, 287, 0, 0.0)); // IsRunning
    e.player.moving = true;
    e.player.running = true;
    assert!(is(&e, PLAYER_REF, 287, 0, 1.0));
    e.player.moving = false;
    assert!(is(&e, PLAYER_REF, 287, 0, 0.0));
    assert!(is(&e, HUMAN_REF, 242, 0, 0.0)); // GetUnconscious
}
