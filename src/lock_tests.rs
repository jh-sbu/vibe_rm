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
const FACTION: FormId = FormId(0x821);
const ACTOR: FormId = FormId(0x822);
const OTHER: FormId = FormId(0x823);
const BASE_FACTION: FormId = FormId(0x824);
const QUEST: FormId = FormId(0x840);
const KEY: FormId = FormId(0x841);
const WAX_KEY: FormId = FormId(0x842);
const CONDITIONAL_WAX_KEY: FormId = FormId(0x843);
const INVALID_KEY_LOCK: FormId = FormId(0x844);
const KEY_QUEST: FormId = FormId(0x845);
const FAILED_KEY_QUEST: FormId = FormId(0x846);
const LOCATION: FormId = FormId(0x847);
const STATIC_BASE: FormId = FormId(0x848);
const STATIC_REF: FormId = FormId(0x849);
/// A load door pair locked (adept) on the inside only, as shops' are.
const SHOP_IN: FormId = FormId(0x850);
const SHOP_OUT: FormId = FormId(0x851);

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
    let mut snam = [0; 8];
    snam[..4].copy_from_slice(&BASE_FACTION.0.to_le_bytes());
    snam[4] = 5;
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
    let mut xloc = [0; 12];
    xloc[4..8].copy_from_slice(&KEY.0.to_le_bytes());
    let mut adept = [0; 12];
    adept[0] = 50;
    let mut invalid_xloc = [0; 12];
    invalid_xloc[4..8].copy_from_slice(&OWNER.0.to_le_bytes());
    let cnto = |item: FormId, count: u32| {
        let mut data = item.0.to_le_bytes().to_vec();
        data.extend_from_slice(&count.to_le_bytes());
        data
    };
    let records = [
        Node::top(b"DOOR", vec![RecordBuilder::new(b"DOOR", FormId(0x830))]),
        Node::top(b"CONT", vec![RecordBuilder::new(b"CONT", FormId(0x831))]),
        Node::top(b"STAT", vec![RecordBuilder::new(b"STAT", STATIC_BASE)]),
        Node::top(b"KEYM", vec![RecordBuilder::new(b"KEYM", KEY)]),
        Node::top(
            b"PERK",
            vec![
                wax_key_perk(WAX_KEY, false),
                wax_key_perk(CONDITIONAL_WAX_KEY, true),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", OWNER).field(b"SNAM", &snam)],
        ),
        Node::top(
            b"FACT",
            vec![
                RecordBuilder::new(b"FACT", FACTION).u32(b"DATA", 0x8000),
                RecordBuilder::new(b"FACT", BASE_FACTION),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", OWNER),
                RecordBuilder::new(b"ACHR", OTHER).form(b"NAME", OWNER),
            ],
        ),
        Node::top(b"LCTN", vec![RecordBuilder::new(b"LCTN", LOCATION)]),
        Node::top(
            b"QUST",
            vec![
                RecordBuilder::new(b"QUST", QUEST)
                    .u32(b"ALST", 0)
                    .form(b"ALFR", ACTOR)
                    .form(b"ALFC", FACTION)
                    .form(b"ALFC", FACTION)
                    .form(b"ALFC", BASE_FACTION)
                    .form(b"ALFC", FormId::NULL)
                    .field(b"ALFC", &[1, 2])
                    .field(b"ALED", &[])
                    .u32(b"ALST", 1)
                    .form(b"ALFR", OTHER)
                    .form(b"ALFC", FACTION)
                    .field(b"ALED", &[])
                    .u32(b"ALLS", 2)
                    .u32(b"FNAM", 2) // Optional, unfilled location alias.
                    .form(b"ALFC", FACTION)
                    .field(b"ALED", &[]),
                RecordBuilder::new(b"QUST", KEY_QUEST)
                    .u32(b"ALST", 10)
                    .form(b"ALFR", ACTOR)
                    .field(b"CNTO", &cnto(KEY, 2))
                    .field(b"CNTO", &[1, 2]) // Malformed entry is ignored.
                    .field(b"CNTO", &cnto(FormId(0xDEAD), 1)) // Missing item form.
                    .field(b"CNTO", &cnto(KEY, u32::MAX)) // Outside the supported count range.
                    .field(b"ALED", &[])
                    .u32(b"ALST", 11)
                    .form(b"ALFR", OTHER)
                    .field(b"CNTO", &cnto(KEY, 1))
                    .field(b"ALED", &[])
                    .u32(b"ALLS", 12)
                    .u32(b"FNAM", 2)
                    .form(b"ALFL", LOCATION)
                    .field(b"CNTO", &cnto(KEY, 99)) // CNTO on a location alias is ignored.
                    .field(b"ALED", &[]),
                RecordBuilder::new(b"QUST", FAILED_KEY_QUEST)
                    .u32(b"ALST", 20)
                    .form(b"ALFR", ACTOR)
                    .field(b"CNTO", &cnto(KEY, 3))
                    .field(b"ALED", &[])
                    .u32(b"ALST", 21)
                    .form(b"ALFR", OTHER)
                    .field(b"CNTO", &cnto(KEY, 4))
                    .field(b"ALED", &[]),
            ],
        ),
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
                        door(SHOP_IN, SHOP_OUT.0).field(b"XLOC", &adept),
                        RecordBuilder::new(b"REFR", CHEST)
                            .u32(b"NAME", 0x831)
                            .field(b"XLOC", &xloc),
                        RecordBuilder::new(b"REFR", INVALID_KEY_LOCK)
                            .u32(b"NAME", 0x831)
                            .field(b"XLOC", &invalid_xloc),
                        RecordBuilder::new(b"REFR", STATIC_REF).form(b"NAME", STATIC_BASE),
                    ],
                ),
                Node::Record(RecordBuilder::new(b"CELL", HALL).field(b"DATA", &[1, 0])),
                cell(HALL, vec![door(DOOR, PARTNER.0), door(SHOP_OUT, SHOP_IN.0)]),
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

fn wax_key_perk(id: FormId, require_unlocked: bool) -> RecordBuilder {
    let mut perk = RecordBuilder::new(b"PERK", id)
        .field(b"DATA", &[0, 0, 1, 1, 0])
        .field(b"PRKE", &[2, 0, 0])
        .field(b"DATA", &[90, 1, 2])
        .field(b"PRKC", &[1]);
    if require_unlocked {
        let mut ctda = [0; 32];
        ctda[4..8].copy_from_slice(&(0.0f32).to_le_bytes());
        ctda[8..10].copy_from_slice(&5u16.to_le_bytes()); // GetLocked
        perk = perk.field(b"CTDA", &ctda);
    }
    perk.field(b"EPFT", &[1])
        .field(b"EPFD", &100.0f32.to_le_bytes())
        .field(b"PRKF", &[])
}

fn finish_pick(e: &mut Engine, lock: FormId) {
    e.lockpick = Some(Lockpick {
        lock,
        then: lock,
        level: 0,
        centre: 0.0,
        sweet: 30.0,
        partial: 10.0,
        durability: 2.0,
        strain: 0.0,
        pick: 0.0,
        turn: 0.99,
        turning: true,
        hold: 1.0,
    });
    e.menu = Some(crate::items::Menu::Lockpick);
    e.update_lockpick(0.01);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn wax_key_rewards_only_after_success_and_respects_entry_point_conditions() {
    let (_dir, mut e) = fixture();
    e.add_perk(PLAYER_REF, WAX_KEY);
    finish_pick(&mut e, CHEST);
    assert!(!e.is_locked(CHEST));
    assert_eq!(e.item_count(PLAYER_REF, KEY), 1);

    // Picking the same lock again cannot duplicate a key already carried.
    e.set_locked(CHEST, true);
    finish_pick(&mut e, CHEST);
    assert_eq!(e.item_count(PLAYER_REF, KEY), 1);

    // An entry-point condition on tab 1 sees the lock and is evaluated after
    // the successful transition has unlocked it.
    e.set_locked(CHEST, true);
    e.remove_perk(PLAYER_REF, WAX_KEY);
    e.remove_item(PLAYER_REF, KEY, 1, None);
    e.add_perk(PLAYER_REF, CONDITIONAL_WAX_KEY);
    e.reward_lock_key(CHEST); // Condition fails while the lock remains locked.
    assert_eq!(e.item_count(PLAYER_REF, KEY), 0);
    finish_pick(&mut e, CHEST);
    assert_eq!(e.item_count(PLAYER_REF, KEY), 1);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn wax_key_does_not_reward_without_perk_or_without_a_key() {
    let (_dir, mut e) = fixture();
    finish_pick(&mut e, CHEST);
    assert_eq!(e.item_count(PLAYER_REF, KEY), 0);

    e.add_perk(PLAYER_REF, WAX_KEY);
    finish_pick(&mut e, DOOR); // This reference has no XLOC key.
    finish_pick(&mut e, INVALID_KEY_LOCK); // XLOC points to an NPC, not a KEYM.
    assert_eq!(e.item_count(PLAYER_REF, KEY), 0);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn wax_key_does_not_reward_failed_cancelled_key_or_script_unlocks() {
    let (_dir, mut e) = fixture();
    e.add_perk(PLAYER_REF, WAX_KEY);

    // A cancelled menu discards the active attempt without opening the lock.
    e.lockpick = Some(Lockpick {
        lock: CHEST,
        then: CHEST,
        level: 0,
        centre: 0.0,
        sweet: 30.0,
        partial: 10.0,
        durability: 1.0,
        strain: 0.0,
        pick: 0.0,
        turn: 0.0,
        turning: false,
        hold: 0.0,
    });
    e.menu = None;
    e.update_lockpick(1.0);
    assert!(e.is_locked(CHEST));
    assert_eq!(e.item_count(PLAYER_REF, KEY), 0);

    // A failed pick reaches the edge of its partial zone, then breaks.
    e.lockpick = Some(Lockpick {
        lock: CHEST,
        then: CHEST,
        level: 0,
        centre: 0.0,
        sweet: 0.0,
        partial: 0.0,
        durability: 0.01,
        strain: 0.0,
        pick: 90.0,
        turn: 0.0,
        turning: true,
        hold: 1.0,
    });
    e.menu = Some(crate::items::Menu::Lockpick);
    e.update_lockpick(0.1);
    assert!(e.is_locked(CHEST));
    assert_eq!(e.item_count(PLAYER_REF, KEY), 0);

    // Neither opening with the key nor script unlocking runs the reward hook.
    e.add_item(PLAYER_REF, KEY, 1);
    assert!(e.player_unlock(CHEST, CHEST));
    assert_eq!(e.item_count(PLAYER_REF, KEY), 1);
    e.set_locked(CHEST, true);
    e.set_locked(CHEST, false);
    assert_eq!(e.item_count(PLAYER_REF, KEY), 1);
}

fn lock(e: &mut Engine, r: FormId, args: &[Value]) {
    let this = e.object_value(r);
    crate::script::natives::call(e, "objectreference", "lock", Some(&this), args);
}

fn in_faction(e: &Engine, actor: FormId) -> bool {
    let mut data = [0; 32];
    data[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    data[8..10].copy_from_slice(&71u16.to_le_bytes()); // GetInFaction == 1
    data[12..16].copy_from_slice(&FACTION.0.to_le_bytes());
    let record = e.lo.get(QUEST).unwrap();
    let condition = crate::condition::parse(&record, &data).unwrap();
    crate::condition::evaluate(
        e,
        &[condition],
        crate::condition::Context {
            subject: Some(actor),
            ..Default::default()
        },
    )
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

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn alias_factions_grant_access_only_while_filled_and_running() {
    let (_dir, mut e) = fixture();
    let specs = e.alias_specs(QUEST);
    assert_eq!(specs[0].factions, [FACTION, BASE_FACTION]); // Reject null/malformed; deduplicate.
    assert!(specs[2].factions.is_empty()); // Reference aliases only.
    e.scripts.owners.insert(DOOR, Some(FACTION));
    e.set_locked(DOOR, true);
    assert!(!e.npc_may_open(ACTOR, DOOR));
    assert!(!in_faction(&e, ACTOR));
    assert_eq!(e.faction_rank(ACTOR, BASE_FACTION), 5);
    let quest = e.object_value(QUEST);
    crate::script::natives::call(&mut e, "quest", "start", Some(&quest), &[]);
    assert!(e.scripts.quests[&QUEST].running);
    assert!(e.npc_may_open(ACTOR, DOOR));
    assert!(e.npc_may_open(OTHER, DOOR));
    assert_eq!(e.faction_rank(ACTOR, FACTION), 0);
    assert!(in_faction(&e, ACTOR));
    assert_eq!(e.faction_rank(ACTOR, BASE_FACTION), 5); // Alias doesn't lower base rank.
    // Membership is per reference: clearing one fill does not change another.
    let alias = Value::Object(
        papyrus::ObjectId::Alias {
            quest: QUEST.0,
            alias: 0,
        },
        "ReferenceAlias".into(),
    );
    crate::script::natives::call(&mut e, "referencealias", "clear", Some(&alias), &[]);
    assert!(!e.npc_may_open(ACTOR, DOOR));
    assert!(!in_faction(&e, ACTOR));
    assert!(e.npc_may_open(OTHER, DOOR));
    let second_alias = Value::Object(
        papyrus::ObjectId::Alias {
            quest: QUEST.0,
            alias: 1,
        },
        "ReferenceAlias".into(),
    );
    let actor = e.object_value(ACTOR);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&second_alias),
        &[actor],
    );
    assert!(e.npc_may_open(ACTOR, DOOR)); // First alias empty; second still supplies membership.
    let other = e.object_value(OTHER);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&second_alias),
        &[other],
    );
    assert!(!e.npc_may_open(ACTOR, DOOR));
    let actor = e.object_value(ACTOR);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&alias),
        &[actor],
    );
    assert!(e.npc_may_open(ACTOR, DOOR));
    // Explicit ranks retain precedence and survive removing the alias overlay.
    e.set_faction_rank(ACTOR, FACTION, 4);
    let file = crate::save::state::Snapshot::capture(&e).write();
    crate::script::natives::call(&mut e, "quest", "stop", Some(&quest), &[]);
    assert_eq!(e.faction_rank(ACTOR, FACTION), 4);
    assert_eq!(e.faction_rank(OTHER, FACTION), -1);
    assert_eq!(e.faction_rank(ACTOR, BASE_FACTION), 5);
    crate::save::state::Snapshot::read(&file)
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert_eq!(e.faction_rank(ACTOR, FACTION), 4);
    assert_eq!(e.faction_rank(OTHER, FACTION), 0);
    // Player base-record queries use the player reference's alias fills.
    let player = e.object_value(PLAYER_REF);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&alias),
        &[player],
    );
    assert!(e.player_in_faction(FACTION, 0));
    e.set_faction_rank(PLAYER_REF, FACTION, -1);
    assert!(!e.player_in_faction(FACTION, 0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn alias_inventory_grants_keys_only_on_successful_new_fills_and_stays_permanent() {
    let (_dir, mut e) = fixture();
    let specs = e.alias_specs(KEY_QUEST);
    assert_eq!(specs[0].inventory, [(KEY, 2), (FormId(0xDEAD), 1)]);
    // Truncated data and a count outside the signed runtime range are ignored.
    assert_eq!(specs[1].inventory, [(KEY, 1)]);
    assert!(specs[2].location);
    assert!(specs[2].inventory.is_empty()); // Location aliases cannot grant inventory.

    // A later required alias fails, so earlier tentative grants are not applied.
    e.set_disabled(OTHER, true);
    let failed_quest = e.object_value(FAILED_KEY_QUEST);
    crate::script::natives::call(&mut e, "quest", "start", Some(&failed_quest), &[]);
    assert!(!e.scripts.quests[&FAILED_KEY_QUEST].running);
    assert_eq!(e.item_count(ACTOR, KEY), 0);
    assert_eq!(e.item_count(OTHER, KEY), 0);
    e.set_disabled(OTHER, false);

    // Without its alias key, the actor cannot pass this locked reference.
    e.scripts.owners.insert(CHEST, Some(FormId(0x829)));
    e.set_locked(CHEST, true);
    assert!(!e.npc_may_open(ACTOR, CHEST));
    let quest = e.object_value(KEY_QUEST);
    crate::script::natives::call(&mut e, "quest", "start", Some(&quest), &[]);
    assert!(e.scripts.quests[&KEY_QUEST].running);
    assert_eq!(e.alias_ref(KEY_QUEST, 10), Some(ACTOR));
    assert_eq!(e.alias_ref(KEY_QUEST, 11), Some(OTHER));
    assert_eq!(e.item_count(ACTOR, KEY), 2);
    assert_eq!(e.item_count(OTHER, KEY), 1);
    assert!(e.npc_may_open(ACTOR, CHEST));

    // Loading an already-filled alias restores inventory and bindings without
    // replaying the grant.
    let saved = crate::save::state::Snapshot::capture(&e).write();
    e.remove_item(ACTOR, KEY, 2, None);
    crate::save::state::Snapshot::read(&saved)
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert_eq!(e.item_count(ACTOR, KEY), 2);
    assert_eq!(e.alias_ref(KEY_QUEST, 10), Some(ACTOR));

    let alias = Value::Object(
        papyrus::ObjectId::Alias {
            quest: KEY_QUEST.0,
            alias: 10,
        },
        "ReferenceAlias".into(),
    );
    let actor = e.object_value(ACTOR);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&alias),
        &[actor.clone()],
    );
    assert_eq!(e.item_count(ACTOR, KEY), 2); // Same-reference force is idempotent.
    crate::script::natives::call(&mut e, "referencealias", "clear", Some(&alias), &[]);
    assert_eq!(e.item_count(ACTOR, KEY), 2); // Leaving the alias does not revoke it.
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcereftoifempty",
        Some(&alias),
        &[actor],
    );
    assert_eq!(e.item_count(ACTOR, KEY), 4); // A clear followed by a fill is new.

    e.stop_quest(KEY_QUEST);
    assert_eq!(e.item_count(ACTOR, KEY), 4);
    assert_eq!(e.item_count(OTHER, KEY), 1);
    crate::script::natives::call(&mut e, "quest", "start", Some(&quest), &[]);
    assert_eq!(e.item_count(ACTOR, KEY), 6);
    assert_eq!(e.item_count(OTHER, KEY), 2);

    // Runtime inventory counts are signed. A grant must saturate at the
    // representable ceiling rather than overflowing on repeated refills.
    e.remove_item(ACTOR, KEY, 6, None);
    e.inventory_mut(ACTOR).add(KEY, i32::MAX);
    crate::script::natives::call(&mut e, "referencealias", "clear", Some(&alias), &[]);
    let actor = e.object_value(ACTOR);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&alias),
        &[actor],
    );
    assert_eq!(e.item_count(ACTOR, KEY), i32::MAX);

    // A static reference is not an inventory recipient even though it has a base.
    let static_ref = e.object_value(STATIC_REF);
    crate::script::natives::call(
        &mut e,
        "referencealias",
        "forcerefto",
        Some(&alias),
        &[static_ref],
    );
    assert_eq!(e.item_count(STATIC_REF, KEY), 0);
}

#[test]
fn a_load_door_pair_shares_one_lock() {
    let (_dir, mut e) = fixture();
    // The outside door has no lock of its own and reports the inside one's.
    assert!(e.is_locked(SHOP_IN) && e.is_locked(SHOP_OUT));
    assert_eq!(e.lock_of(SHOP_OUT).map(|l| l.level), Some(50));
    assert_eq!(
        e.door_lock_for_player(SHOP_OUT, Some(SHOP_IN)),
        Some(SHOP_IN)
    );

    // The console unlocking the outside door (the one clicked) opens the pair.
    crate::console::execute(&mut e, &format!("unlock {SHOP_OUT}"));
    assert!(!e.is_locked(SHOP_IN) && !e.is_locked(SHOP_OUT));
    assert_eq!(e.door_lock_for_player(SHOP_OUT, Some(SHOP_IN)), None);

    // Locking either side locks both, at the pair's level.
    lock(&mut e, SHOP_IN, &[Value::Bool(true)]);
    assert!(e.is_locked(SHOP_OUT));
    assert_eq!(
        e.door_lock_for_player(SHOP_OUT, Some(SHOP_IN)),
        Some(SHOP_IN)
    );
    e.set_lock_level(SHOP_OUT, 75);
    assert_eq!(e.lock_of(SHOP_IN).map(|l| l.level), Some(75));

    // A pair with no lock authored shares one too.
    assert!(!e.is_locked(DOOR) && !e.is_locked(PARTNER));
    e.set_locked(DOOR, true);
    assert!(e.is_locked(PARTNER));
}
