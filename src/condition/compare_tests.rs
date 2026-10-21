//! Actor comparison and status conditions (`SameFaction`, `SameRace`, `SameSex`,
//! `GetIsClass`, `GetGold`, `GetHealthPercentage`, `IsOverEncumbered`,
//! `GetPCMiscStat`...) on generated records only. Engine owns a renderer, so
//! these need a headless GPU adapter.

use super::*;
use crate::world::inventory::Inventory;
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};

/// The player's NPC record (the player's own reference is `PLAYER_REF`).
const PC_BASE: FormId = FormId(0x7);
const ALLY: FormId = FormId(0x840);
const FOE: FormId = FormId(0x841);
const ALLY_REF: FormId = FormId(0x850);
const FOE_REF: FormId = FormId(0x851);
const COND_HOLDER: FormId = FormId(0x860);
const STAT_HOLDER: FormId = FormId(0x861);
const FACT_A: FormId = FormId(0x820);
const FACT_B: FormId = FormId(0x821);
const RACE_A: FormId = FormId(0x830);
const RACE_B: FormId = FormId(0x831);
const CLASS_A: FormId = FormId(0x832);
const CLASS_B: FormId = FormId(0x833);
const HEAVY: FormId = FormId(0x870);

const SAME_FACTION: u16 = 42;
const SAME_RACE: u16 = 43;
const SAME_SEX: u16 = 44;
const GET_IS_CLASS: u16 = 68;
const GET_GOLD: u16 = 48;
const GET_IN_FACTION: u16 = 71;
const GET_PC_IN_FACTION: u16 = 132;
const SAME_FACTION_AS_PC: u16 = 133;
const SAME_RACE_AS_PC: u16 = 134;
const SAME_SEX_AS_PC: u16 = 135;
const GET_PC_IS_CLASS: u16 = 129;
const GET_HEALTH_PERCENTAGE: u16 = 430;
const IS_OVER_ENCUMBERED: u16 = 734;
const GET_PC_MISC_STAT: u16 = 312;

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `SNAM` of an NPC: its faction, then the rank (a signed byte).
fn snam(faction: FormId, rank: i8) -> Vec<u8> {
    let mut d = faction.0.to_le_bytes().to_vec();
    d.extend_from_slice(&[rank as u8, 0, 0, 0]);
    d
}

/// An NPC record: `female`, its race, class and factions with their ranks.
fn npc(
    id: FormId,
    female: bool,
    race: FormId,
    class: FormId,
    factions: &[(FormId, i8)],
) -> RecordBuilder {
    let mut acbs = [0; 24];
    acbs[0] = female as u8;
    let mut r = RecordBuilder::new(b"NPC_", id)
        .field(b"ACBS", &acbs)
        .form(b"RNAM", race)
        .form(b"CNAM", class);
    for &(f, rank) in factions {
        r = r.field(b"SNAM", &snam(f, rank));
    }
    r
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-compare-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let reference = |id, base| RecordBuilder::new(b"REFR", id).form(b"NAME", base);
    // The player: male, race A, class A, in faction A and (as a potential
    // follower) faction B at rank -1.
    let pc = npc(
        PC_BASE,
        false,
        RACE_A,
        CLASS_A,
        &[(FACT_A, 0), (FACT_B, -1)],
    );
    // An ally: female, race A, class B, in faction A at rank 1.
    let ally = npc(ALLY, true, RACE_A, CLASS_B, &[(FACT_A, 1)]);
    // A foe: male, race B, class B, only a potential follower of faction B.
    let foe = npc(FOE, false, RACE_B, CLASS_B, &[(FACT_B, -1)]);
    // Heavy: 20 weight.
    let mut heavy = 0i32.to_le_bytes().to_vec();
    heavy.extend_from_slice(&20f32.to_le_bytes());
    let records = [
        Node::top(
            b"FACT",
            vec![
                RecordBuilder::new(b"FACT", FACT_A),
                RecordBuilder::new(b"FACT", FACT_B),
            ],
        ),
        Node::top(
            b"RACE",
            vec![
                RecordBuilder::new(b"RACE", RACE_A),
                RecordBuilder::new(b"RACE", RACE_B),
            ],
        ),
        Node::top(
            b"CLAS",
            vec![
                RecordBuilder::new(b"CLAS", CLASS_A),
                RecordBuilder::new(b"CLAS", CLASS_B),
            ],
        ),
        Node::top(
            b"MISC",
            vec![RecordBuilder::new(b"MISC", HEAVY).field(b"DATA", &heavy)],
        ),
        Node::top(b"NPC_", vec![pc, ally, foe]),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ALLY_REF).form(b"NAME", ALLY),
                RecordBuilder::new(b"ACHR", FOE_REF).form(b"NAME", FOE),
            ],
        ),
        Node::top(
            b"REFR",
            vec![
                reference(COND_HOLDER, FOE),
                // A condition with a string parameter (`CIS1`) for `GetPCMiscStat`.
                reference(STAT_HOLDER, FOE)
                    .field(b"CTDA", &ctda(GET_PC_MISC_STAT, 0, 3.0))
                    .zstring(b"CIS1", "barters"),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Compare.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic comparison conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Compare.esp");
    let mut ally_items = Inventory::default();
    ally_items.add(crate::crime::GOLD, 120);
    e.inventories.insert(ALLY_REF, ally_items);
    e.increment_stat("barters", 3);
    (f, e)
}

/// `func(param) == value` as a CTDA record's bytes.
fn ctda(func: u16, param: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&param.to_le_bytes());
    d
}

/// Whether `func(param) == value` holds for `subject`.
fn holds_value(e: &Engine, subject: FormId, func: u16, param: u32, value: f32) -> bool {
    let rec = e.lo.get(COND_HOLDER).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, param, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

/// Whether `func(param) == 1` holds for `subject`.
fn holds(e: &Engine, subject: FormId, func: u16, param: u32) -> bool {
    holds_value(e, subject, func, param, 1.0)
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn same_faction_leaves_out_potential_followers() {
    let (_f, e) = fixture();
    // The ally and the player share faction A.
    assert!(holds(&e, ALLY_REF, SAME_FACTION, PLAYER_REF.0));
    assert!(holds(&e, ALLY_REF, SAME_FACTION_AS_PC, 0));
    // The foe's only faction is one the player is a potential follower of (rank -1).
    assert!(!holds(&e, FOE_REF, SAME_FACTION, PLAYER_REF.0));
    assert!(!holds(&e, FOE_REF, SAME_FACTION_AS_PC, 0));
    // A reference that is not an actor shares nothing.
    assert!(!holds(&e, COND_HOLDER, SAME_FACTION, PLAYER_REF.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_pc_in_faction_is_the_players_membership() {
    let (_f, e) = fixture();
    assert!(holds(&e, ALLY_REF, GET_PC_IN_FACTION, FACT_A.0));
    // Rank -1 isn't membership, as `GetInFaction`.
    assert!(!holds(&e, ALLY_REF, GET_PC_IN_FACTION, FACT_B.0));
    // The subject doesn't matter: it asks about the player.
    assert!(holds(&e, FOE_REF, GET_PC_IN_FACTION, FACT_A.0));
    // And the same subject's own membership is what GetInFaction asks.
    assert!(holds(&e, ALLY_REF, GET_IN_FACTION, FACT_A.0));
    assert!(!holds(&e, FOE_REF, GET_IN_FACTION, FACT_A.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn same_race_and_sex_compare_with_the_player_or_another_actor() {
    let (_f, e) = fixture();
    assert!(holds(&e, ALLY_REF, SAME_RACE, PLAYER_REF.0));
    assert!(!holds(&e, FOE_REF, SAME_RACE, PLAYER_REF.0));
    assert!(holds(&e, ALLY_REF, SAME_RACE_AS_PC, 0));
    assert!(!holds(&e, FOE_REF, SAME_RACE_AS_PC, 0));
    // The ally is female, the player and the foe male.
    assert!(!holds(&e, ALLY_REF, SAME_SEX, PLAYER_REF.0));
    assert!(holds(&e, FOE_REF, SAME_SEX, PLAYER_REF.0));
    assert!(!holds(&e, ALLY_REF, SAME_SEX_AS_PC, 0));
    assert!(holds(&e, FOE_REF, SAME_SEX_AS_PC, 0));
    assert!(holds(&e, FOE_REF, SAME_SEX, FOE.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_class_asks_the_npcs_class_and_the_players() {
    let (_f, e) = fixture();
    assert!(holds(&e, ALLY_REF, GET_IS_CLASS, CLASS_B.0));
    assert!(!holds(&e, ALLY_REF, GET_IS_CLASS, CLASS_A.0));
    assert!(holds(&e, ALLY_REF, GET_PC_IS_CLASS, CLASS_A.0));
    assert!(!holds(&e, ALLY_REF, GET_PC_IS_CLASS, CLASS_B.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_gold_counts_the_subjects_gold() {
    let (_f, e) = fixture();
    assert!(holds_value(&e, ALLY_REF, GET_GOLD, 0, 120.0));
    assert!(!holds_value(&e, FOE_REF, GET_GOLD, 0, 120.0));
    assert!(holds_value(&e, FOE_REF, GET_GOLD, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_health_percentage_is_the_share_of_the_most() {
    let (_f, mut e) = fixture();
    e.set_actor_value(PLAYER_REF, av::HEALTH, 100.0);
    e.player_health = 25.0;
    assert!(holds_value(&e, PLAYER_REF, GET_HEALTH_PERCENTAGE, 0, 0.25));
    assert!(!holds_value(&e, PLAYER_REF, GET_HEALTH_PERCENTAGE, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_over_encumbered_is_the_players_carry_weight() {
    let (_f, mut e) = fixture();
    e.set_actor_value(PLAYER_REF, av::CARRY_WEIGHT, 10.0);
    let mut carried = Inventory::default();
    carried.add(HEAVY, 1);
    e.inventories.insert(PLAYER_REF, carried);
    assert!(holds(&e, PLAYER_REF, IS_OVER_ENCUMBERED, 0));
    // Carrying less than it weighs in: not over-encumbered.
    e.set_actor_value(PLAYER_REF, av::CARRY_WEIGHT, 25.0);
    assert!(!holds(&e, PLAYER_REF, IS_OVER_ENCUMBERED, 0));
    // Only the player's encumbrance is tracked.
    e.set_actor_value(PLAYER_REF, av::CARRY_WEIGHT, 10.0);
    assert!(!holds(&e, ALLY_REF, IS_OVER_ENCUMBERED, 0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_pc_misc_stat_reads_the_named_journal_statistic() {
    let (_f, e) = fixture();
    let rec = e.lo.get(STAT_HOLDER).unwrap();
    let conditions = crate::condition::parse_all(&rec);
    let subject = crate::condition::Context {
        subject: Some(FOE_REF),
        ..Default::default()
    };
    assert!(crate::condition::evaluate(&e, &conditions, subject));
    // A statistic nobody counted is 0, so it isn't the 3 the condition asks for.
    let mut other = conditions.clone();
    other[0].string_p1 = Some("never counted".into());
    assert!(!crate::condition::evaluate(&e, &other, subject));
}
