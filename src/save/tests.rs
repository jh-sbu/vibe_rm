//! Saving and loading through a real `Engine` (a headless GPU adapter) with
//! generated records only.

use esp::FormId;
use esp::group_type;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};

use super::codec::Writer;
use super::file::SaveFile;
use super::state::{Snapshot, Spot};
use crate::ai::schedule::Place;
use crate::engine::{Engine, Location, PLAYER_REF};
use crate::weather::SavedWeather;
use crate::world::destruction::DestructionState;
use crate::world::inventory::{Inventory, LEFT, RIGHT};
use crate::world_state::{Moved, Wounds};

const SWORD: FormId = FormId(0x800);

#[test]
fn crafting_forms_round_trip_and_legacy_saves_have_none() {
    let id = FormId(0xFF00_0800);
    let record = RecordBuilder::new(b"ALCH", id)
        .zstring(b"FULL", "Potion of Testing")
        .to_bytes();
    let s = Snapshot {
        forms: vec![record.clone()],
        ..Default::default()
    };
    let file = s.write();
    let loaded = Snapshot::read(&file).unwrap();
    assert_eq!(loaded.forms, vec![record]);
    let mut lo = esp::LoadOrder::load(std::env::temp_dir(), &[]).unwrap();
    lo.insert_runtime_record(&loaded.forms[0]).unwrap();
    assert_eq!(
        crate::world::inventory::item_info(&lo, id).unwrap().name,
        "Potion of Testing"
    );
    let mut legacy = file.clone();
    legacy.sections.retain(|(t, _)| t != b"FORM");
    assert!(Snapshot::read(&legacy).unwrap().forms.is_empty());
    let mut duplicate = file;
    duplicate.add(b"FORM", vec![0; 4]);
    assert!(Snapshot::read(&duplicate).is_err());
    let bad = Snapshot {
        forms: vec![vec![0; 24]],
        ..Default::default()
    }
    .write();
    assert!(Snapshot::read(&bad).is_err());
}

#[test]
fn crafting_collection_keeps_inventory_and_transitive_forms() {
    let potion = FormId(0xFF00_0800);
    let enchantment = FormId(0xFF00_0801);
    let orphan = FormId(0xFF00_0802);
    let forms = vec![
        RecordBuilder::new(b"ALCH", potion)
            .form(b"EFID", enchantment)
            .to_bytes(),
        RecordBuilder::new(b"ENCH", enchantment).to_bytes(),
        RecordBuilder::new(b"ALCH", orphan).to_bytes(),
    ];
    let mut s = Snapshot {
        forms,
        ..Default::default()
    };
    let mut inv = Inventory::default();
    inv.add(potion, 1);
    s.inventories.insert(PLAYER_REF, inv);
    s.collect_forms();
    let ids: Vec<_> = s
        .forms
        .iter()
        .map(|b| esp::RecordHeader::parse(b).form_id)
        .collect();
    assert_eq!(ids, vec![potion, enchantment]);
    s.inventories.clear();
    s.collect_forms();
    assert!(s.forms.is_empty());
}
const SHIELD: FormId = FormId(0x801);
const GEM: FormId = FormId(0x802);
const WEATHER: FormId = FormId(0x890);
const WEATHER_OTHER: FormId = FormId(0x891);
const GLOBAL: FormId = FormId(0x810);
const QUEST: FormId = FormId(0x820);
const PERK: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x840);
const CELL: FormId = FormId(0x850);
const STORY_FIRST: FormId = FormId(0x860);
const STORY_SECOND: FormId = FormId(0x861);
const COOLDOWN: FormId = FormId(0x862);
const RANK_EVENT: FormId = FormId(0x863);
const LOCATION_EVENT: FormId = FormId(0x864);
const MESSAGE_WAIT_A: FormId = FormId(0x8A0);
const MESSAGE_WAIT_B: FormId = FormId(0x8A1);
const ANIMATION_WAIT_OBJECT: FormId = FormId(0x8A2);
const TIMER_SCENE: FormId = FormId(0x8B0);
const TIMER_SCENE_QUEST: FormId = FormId(0x8B1);
const PACKAGE_SCENE: FormId = FormId(0x8B2);
const SCENE_PACKAGE: FormId = FormId(0x8B3);
const SCENE_ACTION_ALIAS: u32 = 5;
const DIALOGUE_SCENE: FormId = FormId(0x8B4);
const SCENE_ACTOR: FormId = FormId(0x8B5);
const SCENE_TOPIC: FormId = FormId(0x8B6);
const SCENE_INFO: FormId = FormId(0x8B7);
const OTHER_TOPIC: FormId = FormId(0x8B8);
const OTHER_INFO: FormId = FormId(0x8B9);
const LOOP_DIALOGUE_SCENE: FormId = FormId(0x8BA);
const RESTART_SCENE_A: FormId = FormId(0x8BB);
const RESTART_SCENE_B: FormId = FormId(0x8BC);
const FORCE_GREET_TEMPLATE: FormId = FormId(0x8BD);
const HEADTRACK_SCENE: FormId = FormId(0x8BE);
const TWO_PHASE_TIMER_SCENE: FormId = FormId(0x8BF);
const ROLLED_SPELL_ACTOR: FormId = FormId(0x8C0);
const ROLLED_SPELL_LIST: FormId = FormId(0x8C1);
const EMPTY_ROLL_ACTOR: FormId = FormId(0x8C2);
const EMPTY_ROLL_LIST: FormId = FormId(0x8C3);
const ROLL_CHANCE_GLOBAL: FormId = FormId(0x8C4);
const SPELL_AT_LEVEL_1: FormId = FormId(0x8C5);
const SPELL_AT_LEVEL_10: FormId = FormId(0x8C6);
const EMPTY_LIST_SPELL: FormId = FormId(0x8C7);
const DUPLICATE_ROLL_ACTOR: FormId = FormId(0x8C8);
const DUPLICATE_ROLL_LIST: FormId = FormId(0x8C9);
const DUPLICATE_ROLL_SPELL: FormId = FormId(0x8CA);
const USAGE_SPELL_A: FormId = FormId(0x8D0);
const USAGE_SPELL_B: FormId = FormId(0x8D1);
const USAGE_SHOUT_A: FormId = FormId(0x8D2);
const USAGE_SHOUT_B: FormId = FormId(0x8D3);
const GREATER_POWER_A: FormId = FormId(0x8A4);
const GREATER_POWER_B: FormId = FormId(0x8A5);
const LESSER_POWER: FormId = FormId(0x8A6);
const FORTIFY_CARRY: FormId = FormId(0x8E0);
const FORTIFY_CARRY_SPELL: FormId = FormId(0x8E1);
const FORTIFY_CARRY_ABILITY: FormId = FormId(0x8E2);
const SCRIPTED_EFFECT: FormId = FormId(0x8E3);
const SCRIPTED_SPELL: FormId = FormId(0x8E4);
const TAPERING_DRAIN: FormId = FormId(0x8E5);
const TAPERING_SPELL: FormId = FormId(0x8E6);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn attach_scene_test_actor(e: &mut Engine) {
    let cell = crate::render::CellKey::Interior(CELL);
    let actor = crate::ai::ActorRuntime::new(
        SCENE_ACTOR,
        ACTOR,
        std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    e.cells.entry(cell).or_default().actors.push(actor);
    e.actor_cells.insert(SCENE_ACTOR, cell);
}

fn ready_scene_force_greet_actor(e: &mut Engine) {
    let packages = e.scenes.packages[&SCENE_ACTOR].2.clone();
    let actor = e.actor_mut(SCENE_ACTOR).unwrap();
    actor.exec.packages = packages;
    actor.exec.current = Some(0);
    let mut goal = crate::ai::Goal::travel(Vec3::ZERO);
    goal.behaviour = crate::ai::package::Behaviour::ForceGreet;
    actor.exec.goal = Some(goal);
    actor.exec.greeting = true;
}

fn engine(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-save-{name}-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let spell_record = |id: FormId, spell_type: u32, name: &str| {
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&spell_type.to_le_bytes());
        spit[16..20].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        RecordBuilder::new(b"SPEL", id)
            .zstring(b"FULL", name)
            .field(b"SPIT", &spit)
    };
    let mut scene_never_done = [0; 24];
    scene_never_done[4..8].copy_from_slice(&1.0f32.to_bits().to_le_bytes());
    scene_never_done[8..10].copy_from_slice(&59u16.to_le_bytes()); // GetStageDone
    scene_never_done[12..16].copy_from_slice(&TIMER_SCENE_QUEST.0.to_le_bytes());
    scene_never_done[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut leveled_acbs = [0; 14];
    leveled_acbs[0..4].copy_from_slice(&0x80u32.to_le_bytes()); // PC level multiplier
    leveled_acbs[8..10].copy_from_slice(&1000u16.to_le_bytes());
    leveled_acbs[10..12].copy_from_slice(&1u16.to_le_bytes());
    leveled_acbs[12..14].copy_from_slice(&60u16.to_le_bytes());
    let lvlo = |level: u16, spell: FormId| {
        let mut data = [0; 12];
        data[0..2].copy_from_slice(&level.to_le_bytes());
        data[4..8].copy_from_slice(&spell.0.to_le_bytes());
        data[8..10].copy_from_slice(&1u16.to_le_bytes());
        data
    };
    // Fortify Carry Weight (value modifier, Recover), timed and constant.
    let mut fortify = [0; 0x98];
    fortify[0..4].copy_from_slice(&crate::magic::flags::RECOVER.to_le_bytes());
    for o in [0x0C, 0x10, 0x58] {
        fortify[o..o + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    fortify[0x44..0x48].copy_from_slice(&esp::actor_value::CARRY_WEIGHT.to_le_bytes());
    let fortify_spell = |id: FormId, spell_type: u32, casting: u32, duration: u32| {
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&spell_type.to_le_bytes());
        spit[16..20].copy_from_slice(&casting.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&25.0f32.to_le_bytes());
        efit[8..12].copy_from_slice(&duration.to_le_bytes());
        RecordBuilder::new(b"SPEL", id)
            .field(b"SPIT", &spit)
            .u32(b"EFID", FORTIFY_CARRY.0)
            .field(b"EFIT", &efit)
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![
                RecordBuilder::new(b"MGEF", FORTIFY_CARRY).field(b"DATA", &fortify),
                // Drain Carry Weight at once, tapering at its full rate for 2 s.
                RecordBuilder::new(b"MGEF", TAPERING_DRAIN).field(b"DATA", &{
                    let mut d = fortify;
                    d[0..4].copy_from_slice(&crate::magic::flags::DETRIMENTAL.to_le_bytes());
                    d[0x1C..0x20].copy_from_slice(&1.0f32.to_le_bytes());
                    d[0x38..0x3C].copy_from_slice(&2.0f32.to_le_bytes());
                    d
                }),
                RecordBuilder::new(b"MGEF", SCRIPTED_EFFECT)
                    .field(
                        b"VMAD",
                        &esp::writer::vmad(&[esp::writer::AttachedScript {
                            name: "VrmEffectSaveTest".into(),
                            properties: vec![],
                        }]),
                    )
                    .field(b"DATA", &{
                        let mut d = [0; 0x98];
                        for o in [0x0C, 0x10, 0x44, 0x58] {
                            d[o..o + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                        }
                        d[0x40..0x44]
                            .copy_from_slice(&crate::magic::archetype::SCRIPT.to_le_bytes());
                        d
                    }),
            ],
        ),
        Node::top(
            b"IMAD",
            vec![
                RecordBuilder::new(b"IMAD", FormId(0x8D4))
                    .field(b"BNAM", &[0, 0, 0, 0, 0, 0, 32, 65]), // blur 10
                RecordBuilder::new(b"IMAD", FormId(0x8D5))
                    .field(b"BNAM", &[0, 0, 0, 0, 0, 0, 160, 65]), // blur 20
            ],
        ),
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", ACTOR),
                RecordBuilder::new(b"NPC_", FormId(7)),
                RecordBuilder::new(b"NPC_", ROLLED_SPELL_ACTOR)
                    .field(b"ACBS", &leveled_acbs)
                    .u32(b"SPLO", ROLLED_SPELL_LIST.0),
                RecordBuilder::new(b"NPC_", EMPTY_ROLL_ACTOR)
                    .field(b"ACBS", &leveled_acbs)
                    .u32(b"SPLO", EMPTY_ROLL_LIST.0),
                RecordBuilder::new(b"NPC_", DUPLICATE_ROLL_ACTOR)
                    .field(b"ACBS", &leveled_acbs)
                    .u32(b"SPLO", DUPLICATE_ROLL_LIST.0),
            ],
        ),
        Node::top(
            b"LVSP",
            vec![
                RecordBuilder::new(b"LVSP", ROLLED_SPELL_LIST)
                    .field(b"LVLD", &[0])
                    .field(b"LVLF", &[0])
                    .field(b"LVLO", &lvlo(1, SPELL_AT_LEVEL_1))
                    .field(b"LVLO", &lvlo(10, SPELL_AT_LEVEL_10)),
                RecordBuilder::new(b"LVSP", EMPTY_ROLL_LIST)
                    .field(b"LVLD", &[0])
                    .field(b"LVLF", &[0])
                    .form(b"LVLG", ROLL_CHANCE_GLOBAL)
                    .field(b"LVLO", &lvlo(1, EMPTY_LIST_SPELL)),
                RecordBuilder::new(b"LVSP", DUPLICATE_ROLL_LIST)
                    .field(b"LVLD", &[0])
                    .field(b"LVLF", &[0x04])
                    .field(b"LVLO", &lvlo(1, DUPLICATE_ROLL_SPELL))
                    .field(b"LVLO", &lvlo(1, DUPLICATE_ROLL_SPELL)),
            ],
        ),
        Node::top(
            b"GLOB",
            vec![
                RecordBuilder::new(b"GLOB", ROLL_CHANCE_GLOBAL)
                    .field(b"FLTV", &100.0f32.to_le_bytes()),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", SCENE_ACTOR).form(b"NAME", ACTOR)],
        ),
        Node::top(
            b"QUST",
            [
                STORY_FIRST,
                STORY_SECOND,
                COOLDOWN,
                RANK_EVENT,
                LOCATION_EVENT,
                TIMER_SCENE_QUEST,
            ]
            .into_iter()
            .map(|id| RecordBuilder::new(b"QUST", id))
            .collect(),
        ),
        Node::top(
            b"SCEN",
            vec![
                RecordBuilder::new(b"SCEN", TIMER_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Timer phase")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "Wait")
                    .u32(b"INAM", 7)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .field(b"SNAM", &1.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "Long wait")
                    .u32(b"INAM", 9)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .field(b"SNAM", &5.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", PACKAGE_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Package phase")
                    .field(b"NEXT", &[])
                    .field(b"CTDA", &scene_never_done)
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &1u16.to_le_bytes())
                    .zstring(b"NAM0", "Run package")
                    .u32(b"ALID", SCENE_ACTION_ALIAS)
                    .u32(b"INAM", 13)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .u32(b"PNAM", SCENE_PACKAGE.0)
                    .field(b"ANAM", &[])
                    .field(b"ANAM", &1u16.to_le_bytes())
                    .zstring(b"NAM0", "Optional package actor")
                    .u32(b"ALID", SCENE_ACTION_ALIAS + 1)
                    .u32(b"INAM", 14)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .u32(b"PNAM", SCENE_PACKAGE.0)
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", DIALOGUE_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Dialogue phase")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &0u16.to_le_bytes())
                    .zstring(b"NAM0", "Speak")
                    .u32(b"ALID", SCENE_ACTION_ALIAS)
                    .u32(b"INAM", 17)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .form(b"DATA", SCENE_TOPIC)
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", LOOP_DIALOGUE_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Looping dialogue phase")
                    .field(b"NEXT", &[])
                    .field(b"CTDA", &scene_never_done)
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &0u16.to_le_bytes())
                    .zstring(b"NAM0", "Repeat line")
                    .u32(b"ALID", SCENE_ACTION_ALIAS)
                    .u32(b"INAM", 18)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .u32(b"FNAM", 1 << 16)
                    .field(b"DMIN", &5.0f32.to_le_bytes())
                    .field(b"DMAX", &5.0f32.to_le_bytes())
                    .form(b"DATA", SCENE_TOPIC)
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", HEADTRACK_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Headtracking phase")
                    .field(b"NEXT", &[])
                    .field(b"CTDA", &scene_never_done)
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &0u16.to_le_bytes())
                    .zstring(b"NAM0", "Headtrack only")
                    .u32(b"ALID", SCENE_ACTION_ALIAS)
                    .u32(b"INAM", 21)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .form(b"DATA", FormId::NULL)
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", TWO_PHASE_TIMER_SCENE)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "First phase")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Second phase")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "First timer")
                    .u32(b"INAM", 23)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .field(b"SNAM", &1.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "Second timer")
                    .u32(b"INAM", 24)
                    .u32(b"SNAM", 1)
                    .u32(b"ENAM", 1)
                    .field(b"SNAM", &100.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", RESTART_SCENE_A)
                    .u32(b"FNAM", 0x8)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Restart A")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "Short timer A")
                    .u32(b"INAM", 19)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .field(b"SNAM", &1.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
                RecordBuilder::new(b"SCEN", RESTART_SCENE_B)
                    .u32(b"FNAM", 0x8)
                    .field(b"HNAM", &[])
                    .zstring(b"NAM0", "Restart B")
                    .field(b"NEXT", &[])
                    .field(b"NEXT", &[])
                    .field(b"HNAM", &[])
                    .field(b"ANAM", &2u16.to_le_bytes())
                    .zstring(b"NAM0", "Short timer B")
                    .u32(b"INAM", 20)
                    .u32(b"SNAM", 0)
                    .u32(b"ENAM", 0)
                    .field(b"SNAM", &1.0f32.to_le_bytes())
                    .field(b"ANAM", &[])
                    .u32(b"PNAM", TIMER_SCENE_QUEST.0),
            ],
        ),
        Node::Group {
            label: *b"DIAL",
            group_type: 0,
            children: vec![
                Node::Record(
                    RecordBuilder::new(b"DIAL", SCENE_TOPIC)
                        .zstring(b"FULL", "Scene topic")
                        .form(b"QNAM", TIMER_SCENE_QUEST)
                        .field(b"SNAM", b"HELO"),
                ),
                Node::Group {
                    label: SCENE_TOPIC.0.to_le_bytes(),
                    group_type: 7,
                    children: vec![Node::Record({
                        let mut current = [0; 24];
                        current[12] = 3;
                        let mut queued = [0; 24];
                        queued[12] = 3; // Duplicate response numbers are deliberate.
                        RecordBuilder::new(b"INFO", SCENE_INFO)
                            .field(b"TRDT", &current)
                            .zstring(b"NAM1", "The saved line.")
                            .field(b"TRDT", &queued)
                            .zstring(b"NAM1", "The queued line.")
                    })],
                },
                Node::Record(
                    RecordBuilder::new(b"DIAL", OTHER_TOPIC)
                        .zstring(b"FULL", "Other topic")
                        .form(b"QNAM", TIMER_SCENE_QUEST),
                ),
                Node::Group {
                    label: OTHER_TOPIC.0.to_le_bytes(),
                    group_type: 7,
                    children: vec![Node::Record({
                        let mut response = [0; 24];
                        response[12] = 3;
                        RecordBuilder::new(b"INFO", OTHER_INFO)
                            .field(b"TRDT", &response)
                            .zstring(b"NAM1", "Wrong topic line.")
                    })],
                },
            ],
        },
        Node::top(b"PACK", {
            let mut pkcu = [0; 8];
            pkcu[4..8].copy_from_slice(&FORCE_GREET_TEMPLATE.0.to_le_bytes());
            vec![
                RecordBuilder::new(b"PACK", SCENE_PACKAGE)
                    .zstring(b"EDID", "ScenePackage")
                    .field(b"PKCU", &pkcu),
                RecordBuilder::new(b"PACK", FORCE_GREET_TEMPLATE).zstring(b"EDID", "ForceGreet"),
            ]
        }),
        Node::top(
            b"SHOU",
            vec![
                RecordBuilder::new(b"SHOU", USAGE_SHOUT_A).zstring(b"FULL", "Usage Shout A"),
                RecordBuilder::new(b"SHOU", USAGE_SHOUT_B).zstring(b"FULL", "Usage Shout B"),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell_record(
                    USAGE_SPELL_A,
                    crate::magic::spell_type::SPELL,
                    "Usage Spell A",
                ),
                spell_record(
                    USAGE_SPELL_B,
                    crate::magic::spell_type::SPELL,
                    "Usage Spell B",
                ),
                spell_record(
                    GREATER_POWER_A,
                    crate::magic::spell_type::POWER,
                    "Greater A",
                ),
                spell_record(
                    GREATER_POWER_B,
                    crate::magic::spell_type::POWER,
                    "Greater B",
                ),
                spell_record(
                    LESSER_POWER,
                    crate::magic::spell_type::LESSER_POWER,
                    "Lesser",
                ),
                fortify_spell(FORTIFY_CARRY_SPELL, crate::magic::spell_type::SPELL, 1, 30),
                {
                    let mut spit = [0; 36];
                    spit[16..20].copy_from_slice(&1u32.to_le_bytes());
                    let mut efit = [0; 12];
                    efit[0..4].copy_from_slice(&25.0f32.to_le_bytes());
                    RecordBuilder::new(b"SPEL", TAPERING_SPELL)
                        .field(b"SPIT", &spit)
                        .u32(b"EFID", TAPERING_DRAIN.0)
                        .field(b"EFIT", &efit)
                },
                {
                    let mut spit = [0; 36];
                    spit[16..20].copy_from_slice(&1u32.to_le_bytes());
                    let mut efit = [0; 12];
                    efit[8..12].copy_from_slice(&30u32.to_le_bytes());
                    RecordBuilder::new(b"SPEL", SCRIPTED_SPELL)
                        .field(b"SPIT", &spit)
                        .u32(b"EFID", SCRIPTED_EFFECT.0)
                        .field(b"EFIT", &efit)
                },
                fortify_spell(
                    FORTIFY_CARRY_ABILITY,
                    crate::magic::spell_type::ABILITY,
                    0,
                    0,
                ),
            ],
        ),
        Node::top(
            b"SMEN",
            vec![
                RecordBuilder::new(b"SMEN", FormId(0x870)).field(b"ENAM", b"LOCK"),
                RecordBuilder::new(b"SMEN", FormId(0x871)).field(b"ENAM", b"SCPT"),
                RecordBuilder::new(b"SMEN", FormId(0x872)).field(b"ENAM", b"CHRR"),
                RecordBuilder::new(b"SMEN", FormId(0x873)).field(b"ENAM", b"CLOC"),
            ],
        ),
        Node::top(
            b"SMQN",
            vec![
                RecordBuilder::new(b"SMQN", FormId(0x880))
                    .u32(b"PNAM", 0x870)
                    .field(b"DNAM", &[0, 0, 1, 0])
                    .u32(b"NNAM", STORY_FIRST.0)
                    .u32(b"NNAM", STORY_SECOND.0),
                RecordBuilder::new(b"SMQN", FormId(0x881))
                    .u32(b"PNAM", 0x871)
                    .u32(b"NNAM", COOLDOWN.0)
                    .field(b"RNAM", &24.0f32.to_le_bytes()),
                RecordBuilder::new(b"SMQN", FormId(0x882))
                    .u32(b"PNAM", 0x872)
                    .u32(b"NNAM", RANK_EVENT.0),
                RecordBuilder::new(b"SMQN", FormId(0x883))
                    .u32(b"PNAM", 0x873)
                    .u32(b"NNAM", LOCATION_EVENT.0),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![{
                let mut dnam = [0; 100];
                dnam[0] = 1;
                RecordBuilder::new(b"WEAP", SWORD)
                    .zstring(b"FULL", "Save Test Sword")
                    .field(b"DNAM", &dnam)
            }],
        ),
        Node::top(
            b"WTHR",
            [WEATHER, WEATHER_OTHER]
                .into_iter()
                .map(|id| RecordBuilder::new(b"WTHR", id))
                .collect(),
        ),
        Node::top(
            b"MESG",
            vec![
                RecordBuilder::new(b"MESG", MESSAGE_WAIT_A)
                    .zstring(b"FULL", "First title")
                    .zstring(b"DESC", "First question")
                    .field(b"DNAM", &[1])
                    .zstring(b"ITXT", "No")
                    .zstring(b"ITXT", "Yes"),
                RecordBuilder::new(b"MESG", MESSAGE_WAIT_B)
                    .zstring(b"FULL", "Second title")
                    .zstring(b"DESC", "Second question")
                    .field(b"DNAM", &[1])
                    .zstring(b"ITXT", "A")
                    .zstring(b"ITXT", "B"),
            ],
        ),
        Node::top(b"ARMO", vec![RecordBuilder::new(b"ARMO", SHIELD)]),
        Node::top(
            b"SLGM",
            vec![
                RecordBuilder::new(b"SLGM", GEM)
                    .field(b"SOUL", &[0])
                    .field(b"SLCP", &[1]),
            ],
        ),
        // An empty interior cell (object id 0x850: block 8, sub-block 2).
        Node::Group {
            label: *b"CELL",
            group_type: group_type::TOP,
            children: vec![Node::Group {
                label: 8i32.to_le_bytes(),
                group_type: group_type::INTERIOR_BLOCK,
                children: vec![Node::Group {
                    label: 2i32.to_le_bytes(),
                    group_type: group_type::INTERIOR_SUB_BLOCK,
                    children: vec![Node::Record(
                        RecordBuilder::new(b"CELL", CELL)
                            .zstring(b"EDID", "SaveCell")
                            .field(b"DATA", &[1, 0]),
                    )],
                }],
            }],
        },
    ];
    std::fs::write(
        f.0.join("Save.esp"),
        write_plugin(0, &[], "test", "synthetic saves", 0x900, &records),
    )
    .unwrap();
    // A script that sets `count` to 1, waits ten seconds and sets it to 2.
    std::fs::create_dir(f.0.join("scripts")).unwrap();
    let mut script = papyrus::build::ScriptBuilder::new("VrmSaveTest", "Quest");
    let count = script.variable("count", "Int", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.assign(count.clone(), papyrus::build::Arg::Int(1));
    on_init.call_static(
        "Utility",
        "Wait",
        papyrus::build::discard(),
        [papyrus::build::Arg::Float(10.0)],
    );
    on_init.assign(count, papyrus::build::Arg::Int(2));
    on_init.ret(papyrus::build::Arg::None);
    script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmSaveTest.pex"),
        papyrus::pex::write(&script.build().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/Utility.pex"),
        papyrus::build::native_class(
            "Utility",
            "",
            &[("Wait", "None", &[("afSeconds", "Float")], true)],
        )
        .unwrap(),
    )
    .unwrap();
    let mut message_script = papyrus::build::ScriptBuilder::new("VrmMessageWaitTest", "Message");
    let result = message_script.variable("result", "Int", papyrus::build::Arg::None);
    let done = message_script.variable("done", "Int", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.call_method(papyrus::build::id("self"), "Show", result, []);
    on_init.assign(done, 1.into());
    on_init.ret(papyrus::build::Arg::None);
    message_script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmMessageWaitTest.pex"),
        papyrus::pex::write(&message_script.build().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/Message.pex"),
        papyrus::build::native_class("Message", "Form", &[("Show", "Int", &[], false)]).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/ObjectReference.pex"),
        papyrus::build::native_class(
            "ObjectReference",
            "Form",
            &[(
                "PlayAnimationAndWait",
                "Bool",
                &[("asEventName", "String"), ("asWaitForEvent", "String")],
                false,
            )],
        )
        .unwrap(),
    )
    .unwrap();
    let mut animation_script =
        papyrus::build::ScriptBuilder::new("VrmAnimationWaitTest", "ObjectReference");
    let started = animation_script.variable("started", "Bool", papyrus::build::Arg::None);
    let done = animation_script.variable("done", "Bool", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.assign(started, true.into());
    on_init.call_method(
        papyrus::build::id("self"),
        "PlayAnimationAndWait",
        papyrus::build::discard(),
        [
            papyrus::build::string("Start"),
            papyrus::build::string("Done"),
        ],
    );
    on_init.assign(done, true.into());
    on_init.ret(papyrus::build::Arg::None);
    animation_script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmAnimationWaitTest.pex"),
        papyrus::pex::write(&animation_script.build().unwrap()).unwrap(),
    )
    .unwrap();
    // An effect script counting its starts and noting its finish.
    let mut effect_script =
        papyrus::build::ScriptBuilder::new("VrmEffectSaveTest", "ActiveMagicEffect");
    let starts = effect_script.variable("starts", "Int", papyrus::build::Arg::None);
    let finished = effect_script.variable("finished", "Int", papyrus::build::Arg::None);
    let mut on_start = papyrus::build::FunctionBuilder::new("None");
    on_start.param("akTarget", "Actor");
    on_start.param("akCaster", "Actor");
    on_start.op(1, [starts.clone(), starts, papyrus::build::Arg::Int(1)]); // iadd
    on_start.ret(papyrus::build::Arg::None);
    effect_script.function("OnEffectStart", on_start);
    let mut on_finish = papyrus::build::FunctionBuilder::new("None");
    on_finish.param("akTarget", "Actor");
    on_finish.param("akCaster", "Actor");
    on_finish.assign(finished, papyrus::build::Arg::Int(1));
    on_finish.ret(papyrus::build::Arg::None);
    effect_script.function("OnEffectFinish", on_finish);
    std::fs::write(
        f.0.join("scripts/VrmEffectSaveTest.pex"),
        papyrus::pex::write(&effect_script.build().unwrap()).unwrap(),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Save.esp");
    (f, e)
}

fn use_power(e: &mut Engine, power: FormId) {
    e.equip_spell(PLAYER_REF, power, crate::world::inventory::VOICE);
    e.player_use_power();
}

/// Change a bit of everything saves keep.
fn play(e: &mut Engine) {
    e.day = 12;
    e.hour = 17.5;
    e.scripts.real_time = 321.5;
    e.scripts.wall_time = 400.25;
    e.scripts.globals.insert(GLOBAL, 3.5);
    let q = e.scripts.quests.entry(QUEST).or_default();
    q.running = true;
    q.stage = 30;
    q.done.extend([10, 20, 30]);
    q.objectives_displayed.insert(1);
    q.objectives_completed.insert(0);
    q.objective_texts.insert(1, "Find the sword".into());
    q.log.push("It was lost.".into());
    q.active = true;
    q.aliases.insert(2, ACTOR);
    q.filled_at.insert(2, 7);
    q.event = Some(crate::story::StoryEvent {
        code: *b"CLOC",
        refs: [ACTOR, FormId::NULL],
        values: [1.0, 2.0],
        ..Default::default()
    });
    e.scripts.stats.insert("Locks Picked".into(), 4);
    e.scripts.weapon_usage.record(SWORD);
    e.scripts.weapon_usage.record(SWORD);
    e.set_weather(WEATHER, false, false);
    e.force_weather(WEATHER_OTHER, true);
    let quest = papyrus::ObjectId::Form(QUEST.0);
    e.scripts.initialized.insert(quest);
    e.scripts.timers.push(crate::script::Timer {
        at: 340.0,
        game_time: false,
        obj: quest,
        script: "VrmSaveTest".into(),
        event: "OnUpdate",
        repeat: None,
    });
    e.scripts.sleep_listeners.push(quest);
    e.scripts
        .inventory_filters
        .insert(quest, [SWORD].into_iter().collect());
    e.scripts.actor_values.insert(
        (PLAYER_REF, 7),
        crate::actor_values::Modifiers {
            base: Some(40.0),
            permanent: 2.0,
            temporary: -1.0,
            damage: -3.0,
        },
    );
    e.scripts
        .other_actor_values
        .insert((ACTOR, "Aggression".into()), 2.0);
    e.scripts.looted.insert(FormId(0x900));
    e.scripts.disabled.insert(FormId(0x901), true);
    e.scripts.locked.insert(FormId(0x902), false);
    e.scripts.lock_levels.insert(FormId(0x902), 50);
    e.scripts.owners.insert(FormId(0x903), None);
    e.scripts.owners.insert(FormId(0x904), Some(ACTOR));
    e.scripts
        .faction_changes
        .entry(ACTOR)
        .or_default()
        .insert(FormId(0x905), -1);

    let mut inv = Inventory::default();
    inv.add(SWORD, 2);
    inv.add(SHIELD, 1);
    inv.add_owned(GEM, Some(ACTOR), 3);
    inv.put_soul(GEM, 4);
    inv.set_charge(SWORD, 12.0, 50.0);
    inv.poison(SWORD, FormId(0x906), 2);
    inv.hold(&e.lo, SWORD, &[RIGHT]);
    inv.hold(&e.lo, SWORD, &[LEFT]);
    e.inventories.insert(PLAYER_REF, inv);
    let mut chest = Inventory::default();
    chest.add(SWORD, 1);
    e.inventories.insert(FormId(0x907), chest);

    let ws = &mut e.world_state;
    ws.alerted.insert(ACTOR);
    ws.dead.insert(
        ACTOR,
        (
            Place::Exterior(FormId(0x3C), (-2, 7)),
            Vec3::new(1.0, 2.0, 3.0),
        ),
    );
    ws.open_doors.insert(FormId(0x908));
    ws.actor_open_doors.insert(FormId(0x9A0));
    ws.body_poses.insert(
        ACTOR,
        vec![Mat4::from_translation(Vec3::new(5.0, 6.0, 7.0))],
    );
    e.away.insert(
        ACTOR,
        crate::ai::exec::ActorExec::wounded(Wounds {
            health: 10.0,
            max: 50.0,
            rate: 0.01,
            at: 99.0,
        }),
    );
    ws.moved.insert(
        FormId(0x909),
        Moved {
            place: Place::Interior(CELL),
            pos: Vec3::ONE,
            rot: Vec3::new(0.0, 0.0, 1.5),
        },
    );
    ws.item_charges.insert(FormId(0x90A), vec![1.0, 2.0]);
    ws.broken_joints.insert(FormId(0x90B), vec![0, 3]);
    ws.learned_effects.insert((FormId(0x90C), 2));
    e.created_refs.refs.insert(
        FormId(0xFF00_0800),
        crate::created::Created {
            base: SWORD,
            position: Vec3::new(9.0, 8.0, 7.0),
            rotation: Vec3::ZERO,
            origin: Some(crate::created::Origin {
                position: Vec3::new(7.0, 8.0, 9.0),
                rotation: Vec3::new(0.2, 0.3, 0.4),
            }),
            place: Some(Place::Interior(CELL)),
            container: None,
            location: Some(FormId(0x90D)),
            actor: false,
            count: 3,
            owner: Some(FormId(0x91A)),
        },
    );
    e.created_refs.next = 0xFF00_0801;

    e.player_health = 66.0;
    e.player_stamina = 55.0;
    e.skills.xp[3] = 12.5;
    e.skills.legendary[1] = 2;
    e.skills.level = 9;
    e.skills.perk_points = 4;
    e.skills.read_books.insert(FormId(0x90E));
    e.perks.added.insert(PLAYER_REF, vec![(PERK, 2)]);
    e.perks.removed.entry(ACTOR).or_default().insert(PERK);
    e.magic.added.insert(PLAYER_REF, vec![FormId(0x90F)]);
    e.shouts.known.insert(FormId(0x910));
    e.shouts.unlocked.insert(FormId(0x910));
    e.favorites = vec![SWORD, GEM];
    e.hotkeys[2] = Some(SWORD);
    e.infos_said.insert(FormId(0x911));
    e.talked_to_pc.insert(ACTOR);
    e.crime.bounties.insert(
        FormId(0x912),
        crate::crime::Bounty {
            violent: 25,
            nonviolent: 15,
            infamy_violent: 25,
            infamy_nonviolent: 20,
        },
    );
    e.crime.stolen_value.insert(FormId(0x912), (5, 6));
    e.restocks.stocked.insert(FormId(0x907), 48.0);
    e.story.restore(crate::story::StoryState {
        last_run: [(STORY_FIRST, 200.0)].into(),
        ran: [(FormId(0x880), [STORY_FIRST].into())].into(),
        next_social: [(ACTOR, 333.0)].into(),
        last_location: Some(None),
        found_bodies: [(ACTOR, FormId(0x999))].into(),
        next_body_check: 322.0,
    });
    e.relationships
        .restore_ranks([((ACTOR, FormId(0x999)), -3)].into());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_saved_game_loads_into_another_engine_unchanged() {
    let (_a, mut a) = engine("a");
    play(&mut a);
    let saved = Snapshot::capture(&a).write();
    let bytes = saved.to_bytes();
    assert_eq!(
        Snapshot::capture(&a).write().to_bytes(),
        bytes,
        "deterministic"
    );

    let (_b, mut b) = engine("b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    // Report the specific section if byte-stable recapture ever regresses.
    let recaptured = Snapshot::capture(&b).write();
    for (tag, payload) in &saved.sections {
        assert_eq!(
            recaptured.section(tag),
            Some(payload.as_slice()),
            "section {:?}",
            std::str::from_utf8(tag)
        );
    }
    assert_eq!(recaptured.to_bytes(), bytes);

    assert_eq!((b.day, b.hour), (12, 17.5));
    assert_eq!(b.scripts.globals[&GLOBAL], 3.5);
    let q = &b.scripts.quests[&QUEST];
    assert_eq!(q.stage, 30);
    assert_eq!(q.aliases[&2], ACTOR);
    assert_eq!(q.event.as_ref().unwrap().code, *b"CLOC");
    assert_eq!(b.query_stat("Locks Picked"), 4);
    assert_eq!(
        b.stat("Favorite Weapon"),
        crate::stats::StatValue::Name("Save Test Sword".into())
    );
    assert_eq!(b.query_stat("Favorite Weapon"), 0);
    // The forced weather is on, overriding; releasing it brings back the one
    // it replaced.
    assert_eq!(b.current_weather(), Some(WEATHER_OTHER));
    assert!(b.weather.overridden);
    b.release_weather_override();
    assert_eq!(b.current_weather(), Some(WEATHER));
    let inv = &b.inventories[&PLAYER_REF];
    assert_eq!(inv.count(SWORD), 2);
    assert_eq!(inv.hands().0, [Some(SWORD), Some(SWORD), None]);
    assert!(inv.is_equipped(SWORD));
    assert_eq!(inv.souls, vec![(GEM, 4, 1)]);
    assert_eq!(inv.charges.len(), 1);
    assert!(b.has_perk(PLAYER_REF, PERK));
    assert_eq!(b.skills.level, 9);
    assert_eq!(b.hotkeys[2], Some(SWORD));
    assert_eq!(b.crime.bounties[&FormId(0x912)].total(), 40);
    assert!(b.world_state.dead.contains_key(&ACTOR));
    assert!(b.world_state.actor_open_doors.contains(&FormId(0x9A0)));
    assert_eq!(b.created_refs.refs.len(), 1);
    assert_eq!(b.created_refs.next, 0xFF00_0801);
    assert_eq!(
        b.created_refs.refs[&FormId(0xFF00_0800)].owner,
        Some(FormId(0x91A))
    );
    assert_eq!(
        b.created_refs.refs[&FormId(0xFF00_0800)].origin,
        a.created_refs.refs[&FormId(0xFF00_0800)].origin
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic leveled spell records only"]
fn rolled_leveled_spells_survive_level_and_global_changes_after_load() {
    let (_a_fixture, mut a) = engine("rolled-spells-a");
    a.skills.level = 1;
    a.scripts.globals.insert(ROLL_CHANCE_GLOBAL, 100.0);
    assert_eq!(a.actor_spells(ROLLED_SPELL_ACTOR), vec![SPELL_AT_LEVEL_1]);
    assert!(a.actor_spells(EMPTY_ROLL_ACTOR).is_empty());
    assert_eq!(
        a.actor_spells(DUPLICATE_ROLL_ACTOR),
        vec![DUPLICATE_ROLL_SPELL]
    );
    let saved = Snapshot::capture(&a);
    assert_eq!(saved.rolled_spells.len(), 3);
    assert_eq!(
        saved
            .rolled_spells
            .iter()
            .find(|entry| entry.actor == EMPTY_ROLL_ACTOR)
            .unwrap()
            .spells,
        Vec::<FormId>::new()
    );
    assert_eq!(
        saved
            .rolled_spells
            .iter()
            .find(|entry| entry.actor == DUPLICATE_ROLL_ACTOR)
            .unwrap()
            .spells,
        vec![DUPLICATE_ROLL_SPELL, DUPLICATE_ROLL_SPELL],
        "the cache stores the raw resolver result before actor-spell deduplication"
    );
    let bytes = saved.write().to_bytes();

    let (_b_fixture, mut b) = engine("rolled-spells-b");
    b.skills.level = 15;
    b.scripts.globals.insert(ROLL_CHANCE_GLOBAL, 0.0);
    assert_eq!(b.actor_spells(ROLLED_SPELL_ACTOR), vec![SPELL_AT_LEVEL_10]);
    assert_eq!(b.actor_spells(EMPTY_ROLL_ACTOR), vec![EMPTY_LIST_SPELL]);

    let mut wrong_list = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    wrong_list.rolled_spells[0].list = SWORD;
    assert!(wrong_list.apply(&mut b).is_err());
    assert_eq!(b.actor_spells(ROLLED_SPELL_ACTOR), vec![SPELL_AT_LEVEL_10]);
    assert_eq!(b.actor_spells(EMPTY_ROLL_ACTOR), vec![EMPTY_LIST_SPELL]);

    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    b.skills.level = 15;
    b.scripts.globals.insert(ROLL_CHANCE_GLOBAL, 0.0);
    assert_eq!(
        b.actor_spells(ROLLED_SPELL_ACTOR),
        vec![SPELL_AT_LEVEL_1],
        "the saved roll is retained instead of using the new actor level"
    );
    assert!(
        b.actor_spells(EMPTY_ROLL_ACTOR).is_empty(),
        "a saved empty roll is not rerolled after the chance global changes"
    );
    assert_eq!(
        b.actor_spells(DUPLICATE_ROLL_ACTOR),
        vec![DUPLICATE_ROLL_SPELL]
    );

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"RLSP");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    b.skills.level = 15;
    b.scripts.globals.insert(ROLL_CHANCE_GLOBAL, 0.0);
    assert_eq!(
        b.actor_spells(ROLLED_SPELL_ACTOR),
        vec![SPELL_AT_LEVEL_10],
        "legacy loads clear the old roll and use the current level"
    );
    assert_eq!(
        b.actor_spells(EMPTY_ROLL_ACTOR),
        vec![EMPTY_LIST_SPELL],
        "legacy loads clear saved empty results too"
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_replaces_the_state_it_finds_and_refuses_other_plugins() {
    let (_a, mut a) = engine("a");
    play(&mut a);
    let dir = std::env::temp_dir().join(format!("vrm-save-files-{}", std::process::id()));
    let path = dir.join("one.vrs");
    a.save_game_to(&path).unwrap();
    assert!(!path.with_extension("tmp").exists());

    // Loading drops what the other game had.
    let (_b, mut b) = engine("b");
    b.scripts.globals.insert(FormId(0x999), 1.0);
    b.inventories.insert(FormId(0x998), Inventory::default());
    b.load_game_from(&path).unwrap();
    assert!(!b.scripts.globals.contains_key(&FormId(0x999)));
    assert!(!b.inventories.contains_key(&FormId(0x998)));
    assert_eq!(b.scripts.globals[&GLOBAL], 3.5);

    // A damaged file changes nothing.
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();
    let (_c, mut c) = engine("c");
    assert!(c.load_game_from(&path).is_err());
    assert!(c.scripts.globals.is_empty());

    // Another load order is refused.
    let mut snapshot = Snapshot::capture(&a);
    snapshot.plugins.push("Other.esp".into());
    let err = snapshot.apply(&mut c).unwrap_err();
    assert!(err.to_string().contains("other plugins"));
    assert!(c.scripts.globals.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn script_save_requests_are_written_on_the_next_update_and_deduplicated() {
    let (_a, mut a) = engine("a");
    a.request_save(false);
    a.request_save(false);
    a.request_save(true);
    assert_eq!(a.save_requests.len(), 2);
}

#[test]
fn quicksaves_choose_the_next_append_only_number() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-names-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("quicksave 1.vrs"), b"one").unwrap();
    std::fs::write(dir.join("quicksave 4.vrs"), b"four").unwrap();
    std::fs::write(dir.join("quicksave latest.vrs"), b"malformed").unwrap();
    std::fs::write(dir.join("autosave.vrs"), b"auto").unwrap();
    std::fs::write(dir.join("quicksave.vrs"), b"legacy").unwrap();
    let unreadable_dir = dir.join("not-a-directory");
    std::fs::write(&unreadable_dir, b"file").unwrap();

    assert_eq!(super::next_quicksave_name_in(&dir).unwrap(), "quicksave 5");
    assert!(super::next_quicksave_name_in(&unreadable_dir).is_err());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn quicksave_number_overflow_is_reported() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-overflow-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("quicksave 18446744073709551615.vrs"), b"max").unwrap();

    assert!(super::next_quicksave_name_in(&dir).is_err());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_quicksave_write_leaves_the_slot_available() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let name = super::next_quicksave_name_in(&dir).unwrap();
    let path = super::save_path_in(&dir, &name).unwrap();
    std::fs::create_dir(path.with_extension("tmp")).unwrap();

    assert!(super::write_save_bytes(&path, b"snapshot").is_err());
    assert!(!path.exists());
    assert_eq!(super::next_quicksave_name_in(&dir).unwrap(), name);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn flushed_quicksaves_keep_snapshots_and_other_save_slots() {
    let (fixture, mut engine) = engine("flush");
    let dir = fixture.0.join("saves");
    std::fs::create_dir(&dir).unwrap();
    let read_day = |name: &str| {
        let bytes = std::fs::read(super::save_path_in(&dir, name).unwrap()).unwrap();
        Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
            .unwrap()
            .day
    };

    engine.day = 11;
    engine.request_save(false);
    engine.flush_save_requests_in(&dir);
    engine.day = 12;
    engine.request_save(false);
    engine.flush_save_requests_in(&dir);
    assert_eq!(read_day("quicksave 1"), 11);
    assert_eq!(read_day("quicksave 2"), 12);

    engine.day = 20;
    engine.save_game_to(&dir.join("manual.vrs")).unwrap();
    engine.day = 21;
    engine.request_save(true);
    engine.flush_save_requests_in(&dir);
    engine.day = 22;
    engine.request_save(true);
    engine.flush_save_requests_in(&dir);
    assert_eq!(read_day("autosave1"), 21);
    assert_eq!(read_day("autosave2"), 22);
    assert_eq!(read_day("manual"), 20);
    assert_eq!(read_day("quicksave 1"), 11);
    assert_eq!(read_day("quicksave 2"), 12);
}

#[test]
fn where_the_player_stood_survives_the_file() {
    let mut s = Snapshot::default();
    s.spot = Some(Spot {
        location: Location::Exterior {
            world: FormId(0x3C),
            center: (-3, 4),
        },
        feet: Vec3::new(10.0, 20.0, 30.0),
        yaw: 1.25,
    });
    s.hotkeys[7] = Some(FormId(5));
    let back = Snapshot::read(&s.write()).unwrap();
    assert_eq!(back.spot, s.spot);
    assert_eq!(back.hotkeys[7], Some(FormId(5)));
    assert!(back.favorites.is_empty());
}

#[test]
fn a_file_missing_a_section_is_refused() {
    let mut file = Snapshot::default().write();
    file.sections.retain(|(t, _)| t != b"GLOB");
    let err = Snapshot::read(&file).err().unwrap();
    assert!(err.0.contains("GLOB"), "{err}");
}

fn destruction_payload(entries: &[(FormId, f64, i32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (reference, health, stage) in entries {
        w.form(*reference);
        w.f64(*health);
        w.i32(*stage);
    }
    w.0
}

fn file_with_destruction_payload(payload: Vec<u8>) -> SaveFile {
    let mut file = Snapshot::default().write();
    let (_, data) = file
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"DSTR")
        .expect("Snapshot::write emits DSTR");
    *data = payload;
    file
}

#[test]
fn destructible_health_save_round_trip_is_stable_and_legacy_section_is_optional() {
    let untouched_threshold = FormId(0xA10);
    let destroyed = FormId(0xA11);
    let mut a = Snapshot::default();
    a.destruction.insert(
        untouched_threshold,
        DestructionState {
            health: 99.5,
            stage: -1,
        },
    );
    a.destruction.insert(
        destroyed,
        DestructionState {
            health: 0.0,
            stage: 0,
        },
    );
    let mut b = Snapshot::default();
    b.destruction.insert(
        destroyed,
        DestructionState {
            health: 0.0,
            stage: 0,
        },
    );
    b.destruction.insert(
        untouched_threshold,
        DestructionState {
            health: 99.5,
            stage: -1,
        },
    );
    let bytes = a.write().to_bytes();
    assert_eq!(
        bytes,
        b.write().to_bytes(),
        "DSTR entries sort by reference"
    );
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.destruction, a.destruction);

    let mut old = Snapshot::default().write();
    old.sections.retain(|(tag, _)| tag != b"DSTR");
    assert!(Snapshot::read(&old).unwrap().destruction.is_empty());
}

#[test]
fn malformed_destructible_health_sections_are_refused() {
    let valid = destruction_payload(&[(FormId(0xA10), 50.0, 0)]);
    let mut truncated = valid.clone();
    truncated.pop();
    let malformed = [
        truncated,
        destruction_payload(&[(FormId(0xA10), f64::NAN, 0)]),
        destruction_payload(&[(FormId(0xA10), f64::INFINITY, 0)]),
        destruction_payload(&[(FormId(0xA10), -1.0, 0)]),
        destruction_payload(&[(FormId(0xA10), u32::MAX as f64 + 1.0, 0)]),
        destruction_payload(&[(FormId(0xA10), 50.0, -2)]),
        destruction_payload(&[(FormId(0xA10), 50.0, 256)]),
        destruction_payload(&[(FormId::NULL, 50.0, 0)]),
        destruction_payload(&[(FormId(0xA10), 50.0, 0), (FormId(0xA10), 40.0, 1)]),
    ];
    for payload in malformed {
        assert!(
            Snapshot::read(&file_with_destruction_payload(payload)).is_err(),
            "malformed DSTR payload should be rejected"
        );
    }

    let mut trailing = destruction_payload(&[]);
    trailing.push(0);
    assert!(Snapshot::read(&file_with_destruction_payload(trailing)).is_err());
}

fn origin_payload(entries: &[(FormId, Vec3, Vec3)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (id, position, rotation) in entries {
        w.form(*id);
        w.vec3(*position);
        w.vec3(*rotation);
    }
    w.0
}

#[test]
fn created_reference_origins_round_trip_in_an_optional_section() {
    let id = FormId(0xFF00_0800);
    let origin = crate::created::Origin {
        position: Vec3::new(1.0, 2.0, 3.0),
        rotation: Vec3::new(0.1, 0.2, 0.3),
    };
    let mut s = Snapshot::default();
    s.created.insert(
        id,
        crate::created::Created {
            base: SWORD,
            position: Vec3::new(10.0, 20.0, 30.0),
            rotation: Vec3::new(1.0, 2.0, 3.0),
            origin: Some(origin),
            place: Some(Place::Interior(CELL)),
            container: None,
            location: None,
            actor: false,
            count: 1,
            owner: None,
        },
    );

    let file = s.write();
    assert!(file.section(b"ORIG").is_some());
    // Current WRLD data retains its existing layout and current transform.
    let back = Snapshot::read(&file).unwrap();
    let restored = &back.created[&id];
    assert_eq!(restored.position, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(restored.rotation, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(restored.origin, Some(origin));

    // Older saves have no ORIG section: the created reference still loads,
    // but its original transform remains unknown instead of being guessed.
    let mut old = file;
    old.sections.retain(|(tag, _)| tag != b"ORIG");
    let old = Snapshot::read(&old).unwrap();
    assert_eq!(old.created[&id].position, restored.position);
    assert_eq!(old.created[&id].origin, None);
}

#[test]
fn malformed_created_reference_origin_sections_are_refused() {
    let id = FormId(0xFF00_0800);
    let mut s = Snapshot::default();
    s.created.insert(
        id,
        crate::created::Created {
            base: SWORD,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            origin: None,
            place: Some(Place::Interior(CELL)),
            container: None,
            location: None,
            actor: false,
            count: 1,
            owner: None,
        },
    );
    let valid = s.write();

    for payload in [
        vec![1, 0, 0, 0, 0], // one entry, cut short
        origin_payload(&[(id, Vec3::ZERO, Vec3::ZERO), (id, Vec3::ONE, Vec3::ONE)]),
        origin_payload(&[(FormId(0xDEAD), Vec3::ZERO, Vec3::ZERO)]),
        origin_payload(&[(id, Vec3::splat(f32::NAN), Vec3::ZERO)]),
    ] {
        let mut file = valid.clone();
        file.add(b"ORIG", payload);
        assert!(Snapshot::read(&file).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_goes_to_where_the_player_stood() {
    let (_a, mut a) = engine("a");
    a.enter_interior(CELL, Some((Vec3::new(100.0, 200.0, 0.0), 1.5)))
        .unwrap();
    let feet = a.player_feet();
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("b");
    assert_eq!(b.location, Location::Nowhere);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(b.location, Location::Interior(CELL));
    assert!(
        (b.player_feet() - feet).length() < 1.0,
        "{:?} {:?}",
        b.player_feet(),
        feet
    );
    assert!((b.camera.yaw - 1.5).abs() < 1e-4);
}

/// Run the engine's VM to `now`.
fn run_vm(e: &mut Engine, now: f64, start: Option<papyrus::ObjectId>) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        if let Some(obj) = start {
            assert!(vm.attach(&mut host, obj, "VrmSaveTest", &[]));
            vm.send_event(&mut host, obj, "OnInit", vec![]);
        }
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

fn run_message_vm(e: &mut Engine, now: f64, messages: &[FormId]) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        for message in messages {
            let obj = papyrus::ObjectId::Form(message.0);
            assert!(vm.attach(&mut host, obj, "VrmMessageWaitTest", &[]));
            vm.send_event(&mut host, obj, "OnInit", vec![]);
        }
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

fn install_animation_wait_graph(e: &mut Engine, sender: FormId) {
    use crate::world::behavior::{GraphAnim, ProjectRuntime};
    use havok::behavior::runtime::Shared;
    use havok::behavior::{BehaviorGraph, ClipMode, Generator, Project};
    use std::sync::Arc;

    let graph = BehaviorGraph::new(
        "SyntheticWait",
        Some(0),
        vec![Generator::Clip {
            name: "Start".into(),
            animation: "Start".into(),
            mode: ClipMode::Looping,
            speed: 1.0,
            enforced_duration: 0.0,
            user_fraction: 0.0,
            triggers: vec![],
            crop_start: 0.0,
            crop_end: 0.0,
            start_time: 0.0,
        }],
        vec!["Start".into(), "Done".into()],
    );
    let project = Arc::new(ProjectRuntime {
        shared: Shared::new(Arc::new(Project {
            graphs: vec![("SyntheticWait".into(), graph)],
            character: None,
        })),
        dir: "synthetic".into(),
        name: "wait".into(),
        files: vec![],
    });
    let skeleton = Arc::new(crate::world::skeleton::Skeleton::default());
    let graph = GraphAnim::new(project, "synthetic", false, &skeleton, 1);
    let cell = crate::render::CellKey::Interior(CELL);
    e.cells
        .entry(cell)
        .or_default()
        .animated
        .push(crate::world::animated::test_graph_object(sender, graph));
}

fn run_animation_wait_vm(e: &mut Engine, now: f64, start_count: usize) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        let obj = papyrus::ObjectId::Form(ANIMATION_WAIT_OBJECT.0);
        assert!(vm.attach(&mut host, obj, "VrmAnimationWaitTest", &[]));
        for _ in 0..start_count {
            assert!(vm.send_event(&mut host, obj, "OnInit", vec![]) > 0);
        }
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_script_waiting_when_saved_finishes_after_loading() {
    let (_a, mut a) = engine("a");
    let quest = papyrus::ObjectId::Form(QUEST.0);
    run_vm(&mut a, 0.0, Some(quest));
    a.scripts.real_time = 3.0;
    run_vm(&mut a, 3.0, None);
    assert_eq!(a.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(b.vm.thread_count(), 1);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    run_vm(&mut b, 9.0, None);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    run_vm(&mut b, 10.5, None);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(2)));
    // The same script attached again (a cell loading) keeps what it had.
    let mut vm = std::mem::take(&mut b.vm);
    {
        let mut host = crate::script::EngineHost { engine: &mut b };
        assert!(vm.attach(&mut host, quest, "VrmSaveTest", &[]));
    }
    b.vm = vm;
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(2)));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records and PEX only"]
fn message_show_waits_restore_and_answer_on_the_original_filtered_thread() {
    use crate::messages::MessageBox;

    let (_a, mut a) = engine("message-wait-a");
    run_message_vm(&mut a, 0.0, &[MESSAGE_WAIT_A, MESSAGE_WAIT_B]);
    assert_eq!(a.messages.boxes.len(), 2);
    assert_eq!(a.messages.boxes[0].title, "First title");
    assert_eq!(a.messages.boxes[1].title, "Second title");
    assert_eq!(a.messages.next_key, 2);
    assert_eq!(a.vm.thread_count(), 2);
    assert_eq!(
        a.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(0))
    );
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("message-wait-b");
    b.messages.boxes.push_back(MessageBox {
        title: "stale".into(),
        text: "stale".into(),
        buttons: vec![(0, "Okay".into())],
        key: None,
        action: None,
    });
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(
        b.messages.boxes.len(),
        2,
        "saved boxes replace stale UI state"
    );
    assert_eq!(
        b.messages.boxes[0].buttons,
        vec![(0, "No".into()), (1, "Yes".into())]
    );
    assert_eq!(b.messages.next_key, 2);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(0)),
        "restored waits must not run until a button is answered"
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(0))
    );

    b.choose_message_button(1);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "result"),
        Some(papyrus::Value::Int(1))
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(1))
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(0)),
        "the second filtered thread remains suspended"
    );
    b.choose_message_button(0);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(1))
    );
    assert!(b.messages.boxes.is_empty());
    assert_eq!(b.messages.next_key, 2);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic graph and PEX only"]
fn animation_event_waits_restore_semantically_and_resume_on_matching_event() {
    let (_a, mut a) = engine("animation-wait-a");
    install_animation_wait_graph(&mut a, ANIMATION_WAIT_OBJECT);
    run_animation_wait_vm(&mut a, 0.0, 2);
    let object = papyrus::ObjectId::Form(ANIMATION_WAIT_OBJECT.0);
    assert_eq!(a.vm.thread_count(), 2);
    assert_eq!(
        a.vm.get_var(object, "started"),
        Some(papyrus::Value::Bool(true))
    );
    assert_eq!(
        a.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );
    assert_eq!(a.anim_events.waiting().len(), 1);

    let bytes = Snapshot::capture(&a).write().to_bytes();
    let (_b, mut b) = engine("animation-wait-b");
    install_animation_wait_graph(&mut b, ANIMATION_WAIT_OBJECT);
    b.anim_events.wait_for(FormId(0x8A3), "Stale");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert!(
        b.anim_events
            .waiting()
            .iter()
            .all(|(_, sender, _)| *sender == ANIMATION_WAIT_OBJECT)
    );
    assert_eq!(b.vm.thread_count(), 2);

    // A second roundtrip must retain semantic metadata after importing the VM.
    let bytes = Snapshot::capture(&b).write().to_bytes();
    let (_c, mut c) = engine("animation-wait-c");
    install_animation_wait_graph(&mut c, ANIMATION_WAIT_OBJECT);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut c)
        .unwrap();
    assert_eq!(c.anim_events.waiting().len(), 1);
    assert_eq!(c.vm.thread_count(), 2);
    run_vm(&mut c, 0.0, None);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );

    c.raise_anim_events(FormId(0x8A3), &["Done".into()]);
    c.raise_anim_events(ANIMATION_WAIT_OBJECT, &["Wrong".into()]);
    run_vm(&mut c, 0.0, None);
    assert_eq!(c.vm.thread_count(), 2);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );

    c.raise_anim_events(ANIMATION_WAIT_OBJECT, &["dOnE".into()]);
    run_vm(&mut c, 0.0, None);
    assert_eq!(c.vm.thread_count(), 0);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(true))
    );
    assert!(c.anim_events.waiting().is_empty());

    // The existing timed fallback remains the result when the event never comes.
    let (_d, mut d) = engine("animation-wait-timeout");
    install_animation_wait_graph(&mut d, ANIMATION_WAIT_OBJECT);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut d)
        .unwrap();
    run_vm(&mut d, 10.5, None);
    assert_eq!(d.vm.thread_count(), 0);
    assert_eq!(
        d.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(true))
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn greater_power_cooldowns_survive_load_and_expire_after_a_game_day() {
    let (_a, mut a) = engine("power-cooldown-a");
    a.day = 5;
    a.hour = 6.0;
    use_power(&mut a, GREATER_POWER_A);
    use_power(&mut a, LESSER_POWER);
    let saved = Snapshot::capture(&a);
    assert_eq!(saved.power_uses.len(), 1);
    assert!(saved.power_uses.contains_key(&GREATER_POWER_A));
    assert!(!saved.power_uses.contains_key(&LESSER_POWER));
    let bytes = saved.write().to_bytes();

    let (_b, mut b) = engine("power-cooldown-b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    b.scripts.notifications.clear();
    use_power(&mut b, GREATER_POWER_B);
    assert!(
        b.scripts.notifications.is_empty(),
        "an unused power is independent"
    );
    use_power(&mut b, GREATER_POWER_A);
    use_power(&mut b, GREATER_POWER_B);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "both daily powers are blocked"
    );
    use_power(&mut b, LESSER_POWER);
    use_power(&mut b, LESSER_POWER);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "lesser powers have no daily cooldown"
    );
    assert!(!b.casting.power_uses().contains_key(&LESSER_POWER));

    b.day = 6;
    b.hour = 6.0;
    use_power(&mut b, GREATER_POWER_A);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "the cooldown expires at 24 hours"
    );
    use_power(&mut b, GREATER_POWER_A);
    assert_eq!(
        b.scripts.notifications.len(),
        3,
        "use starts a new daily cooldown"
    );

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"PWCD");
    let (_c, mut c) = engine("power-cooldown-legacy");
    c.day = 5;
    c.hour = 6.0;
    use_power(&mut c, GREATER_POWER_A);
    c.scripts.notifications.clear();
    Snapshot::read(&legacy).unwrap().apply(&mut c).unwrap();
    c.scripts.notifications.clear();
    use_power(&mut c, GREATER_POWER_A);
    assert!(
        c.scripts.notifications.is_empty(),
        "legacy save absence replaces stale destination cooldowns"
    );

    let mut bad = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    bad.power_uses.clear();
    bad.power_uses.insert(SWORD, 5.0);
    let (_d, mut d) = engine("power-cooldown-invalid-form");
    d.day = 99;
    assert!(
        bad.apply(&mut d).is_err(),
        "a non-power record cannot own a cooldown"
    );
    assert_eq!(
        d.day, 99,
        "validation rejects the save before mutating the engine"
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn spent_souls_restore_across_load_and_legacy_saves_clear_them() {
    let (_a, mut a) = engine("spent-soul-a");
    a.world_state
        .dead
        .insert(ACTOR, (Place::Interior(CELL), Vec3::ZERO));
    a.world_state
        .dead
        .insert(FormId(7), (Place::Interior(CELL), Vec3::ONE));
    assert!(
        !a.trap_soul(PLAYER_REF, FormId(7)),
        "failed capture with no gem is retryable"
    );
    a.inventory_mut(PLAYER_REF).add(GEM, 4);
    assert!(a.trap_soul(PLAYER_REF, ACTOR));
    assert!(a.trap_soul(PLAYER_REF, FormId(7)));
    assert_eq!(a.magic.souls_taken, [ACTOR, FormId(7)].into());
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("spent-soul-b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    b.inventory_mut(PLAYER_REF).add(GEM, 1);
    let gems = b.inventory_mut(PLAYER_REF).count(GEM);
    assert!(
        !b.trap_soul(PLAYER_REF, ACTOR),
        "a spent body remains spent"
    );
    assert_eq!(b.inventory_mut(PLAYER_REF).count(GEM), gems);
    assert!(!b.trap_soul(PLAYER_REF, FormId(7)));
    assert!(b.magic.souls_taken.contains(&FormId(7)));

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"SOUL");
    let (_c, mut c) = engine("spent-soul-legacy");
    c.magic.souls_taken.insert(ACTOR);
    Snapshot::read(&legacy).unwrap().apply(&mut c).unwrap();
    assert!(
        c.magic.souls_taken.is_empty(),
        "legacy absence replaces stale state"
    );
    assert!(c.trap_soul(PLAYER_REF, ACTOR));
}

/// Every runtime field is encoded without requiring a renderer or game assets.
#[test]
fn story_and_relationship_sections_are_deterministic_and_optional() {
    let mut s = Snapshot::default();
    s.story.last_run.insert(STORY_FIRST, 42.5);
    s.story
        .ran
        .insert(FormId(0x880), [STORY_FIRST, STORY_SECOND].into());
    s.story.next_social.insert(ACTOR, 100.25);
    s.story.last_location = Some(Some(CELL));
    s.story.found_bodies.insert((ACTOR, FormId(0x999)));
    s.story.found_bodies.insert((FormId(0x999), ACTOR));
    s.story.next_body_check = 101.5;
    s.relationship_ranks.insert((FormId(7), ACTOR), -4);
    s.relationship_ranks.insert((ACTOR, FormId(0x999)), 4);
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.story, s.story);
    assert_eq!(read.relationship_ranks, s.relationship_ranks);
    assert_eq!(read.write().to_bytes(), bytes);
    // Distinguish never observed, observed no location, and observed location.
    for location in [None, Some(None), Some(Some(CELL))] {
        s.story.last_location = location;
        assert_eq!(
            Snapshot::read(&s.write()).unwrap().story.last_location,
            location
        );
    }
    let mut old = s.write();
    old.sections
        .retain(|(tag, _)| tag != b"STOR" && tag != b"RELA");
    let read = Snapshot::read(&old).unwrap();
    assert_eq!(read.story, Default::default());
    assert!(read.relationship_ranks.is_empty());
}

#[test]
fn timer_scene_section_round_trips_and_legacy_absence_clears() {
    let mut s = Snapshot::default();
    s.scenes.push(crate::scene::SavedScene {
        scene: FormId(0x8B0),
        phase: 2,
        timers: vec![
            crate::scene::SavedSceneTimer {
                action: 3,
                elapsed: Some(7.25),
            },
            crate::scene::SavedSceneTimer {
                action: 9,
                elapsed: None,
            },
        ],
    });
    let valid = s.write();
    let bytes = valid.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scenes, s.scenes);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SCNE");
    assert!(Snapshot::read(&legacy).unwrap().scenes.is_empty());

    let mut duplicate = valid.clone();
    duplicate.add(b"SCNE", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = s;
    malformed.scenes[0].timers[0].elapsed = Some(f64::NAN);
    assert!(Snapshot::read(&malformed.write()).is_err());
}

#[test]
fn scene_lifecycle_section_round_trips_and_rejects_bad_state_codes() {
    let mut s = Snapshot::default();
    s.scene_lifecycle.push(crate::scene::SavedSceneLifecycle {
        scene: FormId(0x8B0),
        phase: Some(2),
        begun: true,
        stopping: false,
        paused: true,
        actions: vec![
            crate::scene::SavedSceneAction {
                action: 3,
                state: crate::scene::SavedActionState::Done,
                elapsed: None,
            },
            crate::scene::SavedSceneAction {
                action: 9,
                state: crate::scene::SavedActionState::Running,
                elapsed: Some(7.25),
            },
        ],
    });
    let valid = s.write();
    let bytes = valid.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_lifecycle, s.scene_lifecycle);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SCLF");
    assert!(Snapshot::read(&legacy).unwrap().scene_lifecycle.is_empty());

    let mut malformed = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(FormId(0x8B0));
    w.bool(false);
    w.bool(true);
    w.bool(false);
    w.bool(false);
    w.len(1);
    w.u32(3);
    w.u8(9);
    w.bool(false);
    malformed.add(b"SCLF", w.0);
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn scene_binding_section_round_trips_and_clears_legacy_bindings() {
    let mut s = Snapshot::default();
    s.scene_bindings.extend([
        crate::scene::SavedSceneBinding {
            scene: TIMER_SCENE,
            action: 9,
            actor: Some(FormId(7)),
            package_active: false,
            elapsed: None,
        },
        crate::scene::SavedSceneBinding {
            scene: PACKAGE_SCENE,
            action: 13,
            actor: Some(ACTOR),
            package_active: true,
            elapsed: Some(2.5),
        },
        crate::scene::SavedSceneBinding {
            scene: PACKAGE_SCENE,
            action: 14,
            actor: None,
            package_active: false,
            elapsed: None,
        },
    ]);
    let file = s.write();
    let bytes = file.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_bindings, s.scene_bindings);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SCAB");
    assert!(Snapshot::read(&legacy).unwrap().scene_bindings.is_empty());

    let mut duplicate = file.clone();
    duplicate.add(b"SCAB", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(PACKAGE_SCENE);
    w.u32(13);
    w.bool(true);
    w.form(FormId::NULL);
    w.bool(false);
    w.bool(false);
    malformed.add(b"SCAB", w.0);
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn scene_dialogue_section_round_trips_positions_and_rejects_bad_suffixes() {
    let mut s = Snapshot::default();
    s.scene_dialogue.push(crate::scene::SavedSceneDialogue {
        scene: DIALOGUE_SCENE,
        action: 17,
        topic: SCENE_TOPIC,
        info: SCENE_INFO,
        current_response: 3,
        queued_responses: vec![4, 5],
        remaining: 1.25,
    });
    let file = s.write();
    let bytes = file.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_dialogue, s.scene_dialogue);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SDLG");
    assert!(Snapshot::read(&legacy).unwrap().scene_dialogue.is_empty());

    let mut duplicate = file.clone();
    duplicate.add(b"SDLG", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = file;
    let payload = &mut malformed
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SDLG")
        .unwrap()
        .1;
    // The second queued position must follow the first contiguously.
    payload[32..36].copy_from_slice(&9u32.to_le_bytes());
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn scene_repeat_section_round_trips_deadlines_and_rejects_invalid_entries() {
    let mut s = Snapshot::default();
    s.scene_repeats.push(crate::scene::SavedSceneRepeat {
        scene: LOOP_DIALOGUE_SCENE,
        action: 18,
        remaining: 4.25,
    });
    let file = s.write();
    let bytes = file.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_repeats, s.scene_repeats);
    assert_eq!(read.write().to_bytes(), bytes);

    s.scene_repeats[0].remaining = 0.0;
    let zero_deadline =
        Snapshot::read(&SaveFile::from_bytes(&s.write().to_bytes()).unwrap()).unwrap();
    assert_eq!(zero_deadline.scene_repeats[0].remaining, 0.0);

    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SLRP");
    assert!(Snapshot::read(&legacy).unwrap().scene_repeats.is_empty());

    let mut duplicate = file.clone();
    duplicate.add(b"SLRP", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = file;
    let payload = &mut malformed
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SLRP")
        .unwrap()
        .1;
    payload[12..20].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn scene_restart_section_preserves_queue_order_duplicates_and_legacy_clearing() {
    let mut s = Snapshot::default();
    s.scene_restarts = vec![RESTART_SCENE_B, RESTART_SCENE_A, RESTART_SCENE_B];
    let file = s.write();
    let bytes = file.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_restarts, s.scene_restarts);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SRSQ");
    assert!(Snapshot::read(&legacy).unwrap().scene_restarts.is_empty());

    let mut duplicate = file.clone();
    duplicate.add(b"SRSQ", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(FormId::NULL);
    malformed.add(b"SRSQ", w.0);
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn scene_force_greet_section_round_trips_and_rejects_bad_markers() {
    let mut s = Snapshot::default();
    s.scene_force_greeted.push(SCENE_ACTOR);
    let file = s.write();
    let bytes = file.to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.scene_force_greeted, s.scene_force_greeted);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SFGT");
    assert!(
        Snapshot::read(&legacy)
            .unwrap()
            .scene_force_greeted
            .is_empty()
    );

    let mut duplicate = file.clone();
    duplicate.add(b"SFGT", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());

    let mut malformed = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(FormId::NULL);
    malformed.add(b"SFGT", w.0);
    assert!(Snapshot::read(&malformed).is_err());
}

#[test]
fn rolled_spell_section_is_deterministic_and_preserves_empty_ordered_results() {
    use crate::magic::SavedRolledSpell;

    let first = SavedRolledSpell {
        actor: FormId(0x20),
        list: FormId(0x40),
        spells: vec![FormId(0x90), FormId(0x90), FormId(0x91)],
    };
    let empty = SavedRolledSpell {
        actor: FormId(0x10),
        list: FormId(0x30),
        spells: vec![],
    };
    let mut a = Snapshot::default();
    a.rolled_spells = vec![first.clone(), empty.clone()];
    let mut b = Snapshot::default();
    b.rolled_spells = vec![empty.clone(), first.clone()];
    let bytes = a.write().to_bytes();
    assert_eq!(
        b.write().to_bytes(),
        bytes,
        "record map order is deterministic"
    );

    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(
        read.rolled_spells,
        vec![empty.clone(), first.clone()],
        "empty result, order and duplicates survive"
    );
    assert_eq!(read.write().to_bytes(), bytes);

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"RLSP");
    assert!(Snapshot::read(&legacy).unwrap().rolled_spells.is_empty());

    let mut duplicate = Snapshot::default();
    duplicate.rolled_spells = vec![first.clone(), first.clone()];
    assert!(Snapshot::read(&SaveFile::from_bytes(&duplicate.write().to_bytes()).unwrap()).is_err());

    for malformed_entry in [
        SavedRolledSpell {
            actor: FormId::NULL,
            list: FormId(0x30),
            spells: vec![],
        },
        SavedRolledSpell {
            actor: FormId(0x10),
            list: FormId::NULL,
            spells: vec![],
        },
        SavedRolledSpell {
            actor: FormId(0x10),
            list: FormId(0x30),
            spells: vec![FormId::NULL],
        },
    ] {
        let mut malformed = Snapshot::default();
        malformed.rolled_spells.push(malformed_entry);
        assert!(
            Snapshot::read(&SaveFile::from_bytes(&malformed.write().to_bytes()).unwrap()).is_err()
        );
    }

    let mut duplicate_section = SaveFile::from_bytes(&bytes).unwrap();
    duplicate_section.add(b"RLSP", vec![]);
    assert!(Snapshot::read(&duplicate_section).is_err());

    let mut oversized = Snapshot::default().write();
    oversized.add(b"RLSP", u32::MAX.to_le_bytes().to_vec());
    assert!(Snapshot::read(&oversized).is_err());

    let mut truncated = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(FormId(0x10));
    w.form(FormId(0x30));
    w.len(u32::MAX as usize);
    truncated.add(b"RLSP", w.0);
    assert!(Snapshot::read(&truncated).is_err());
}

#[test]
fn ward_hit_section_is_deterministic_bounded_and_legacy_clearing() {
    let actor_a = FormId(0x20);
    let actor_b = FormId(0x10);
    let mut a = Snapshot::default();
    a.ward_states.insert(actor_a, (2, 0.25));
    a.ward_states.insert(actor_b, (1, 0.75));
    let bytes = a.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.ward_states, a.ward_states);
    assert_eq!(
        read.write().to_bytes(),
        bytes,
        "keys encode in sorted order"
    );

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"WSTA");
    assert!(Snapshot::read(&legacy).unwrap().ward_states.is_empty());

    let mut duplicate_section = SaveFile::from_bytes(&bytes).unwrap();
    duplicate_section.add(b"WSTA", vec![]);
    assert!(Snapshot::read(&duplicate_section).is_err());

    let mut unsorted = Writer::default();
    unsorted.len(2);
    for actor in [actor_a, actor_b] {
        unsorted.form(actor);
        unsorted.i32(1);
        unsorted.f32(0.5);
    }
    let mut malformed = Snapshot::default().write();
    malformed.add(b"WSTA", unsorted.0);
    assert!(Snapshot::read(&malformed).is_err());

    for (actor, state, remaining) in [
        (FormId::NULL, 1, 0.5),
        (actor_b, 0, 0.5),
        (actor_b, 3, 0.5),
        (actor_b, 1, 0.0),
        (actor_b, 2, f32::NAN),
    ] {
        let mut w = Writer::default();
        w.len(1);
        w.form(actor);
        w.i32(state);
        w.f32(remaining);
        let mut malformed = Snapshot::default().write();
        malformed.add(b"WSTA", w.0);
        assert!(Snapshot::read(&malformed).is_err());
    }

    let mut truncated = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(actor_b);
    w.i32(1);
    truncated.add(b"WSTA", w.0);
    assert!(Snapshot::read(&truncated).is_err());

    let mut trailing = Snapshot::default().write();
    let mut w = Writer::default();
    w.len(1);
    w.form(actor_b);
    w.i32(1);
    w.f32(0.5);
    w.u8(0);
    trailing.add(b"WSTA", w.0);
    assert!(Snapshot::read(&trailing).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn timer_scene_load_resumes_elapsed_time_and_replaces_legacy_runs() {
    let (_fixture, mut e) = engine("scene-continuity");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(TIMER_SCENE, false));
    let before_first_update = Snapshot::capture(&e).write();
    let before_first_update = Snapshot::read(&before_first_update).unwrap();
    assert_eq!(before_first_update.scene_lifecycle[0].phase, None);
    assert!(!before_first_update.scene_lifecycle[0].begun);
    before_first_update.apply(&mut e).unwrap();
    e.update_scenes();

    e.scripts.real_time = 13.0;
    e.update_scenes();
    let file = Snapshot::capture(&e).write();
    let saved = Snapshot::read(&file).unwrap();
    assert_eq!(saved.scenes[0].phase, 0);
    assert!(saved.scene_lifecycle[0].begun);
    assert_eq!(saved.scene_lifecycle[0].phase, Some(0));
    assert_eq!(
        saved.scene_lifecycle[0].actions[0].state,
        crate::scene::SavedActionState::Done
    );
    assert_eq!(saved.scene_lifecycle[0].actions[1].elapsed, Some(3.0));
    let mut item1_format = file.clone();
    item1_format.sections.retain(|(tag, _)| {
        !matches!(
            tag,
            b"SCLF" | b"SCAB" | b"SDLG" | b"SLRP" | b"SRSQ" | b"SFGT"
        )
    });
    Snapshot::read(&item1_format)
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert!(e.is_scene_action_complete(TIMER_SCENE, 7));
    Snapshot::read(&file).unwrap().apply(&mut e).unwrap();
    assert!(e.is_scene_playing(TIMER_SCENE));
    assert!(e.is_scene_action_complete(TIMER_SCENE, 7));

    e.scripts.real_time = 14.9;
    e.update_scenes();
    assert!(e.is_scene_playing(TIMER_SCENE), "timer restarted on load");
    e.scripts.real_time = 15.1;
    e.update_scenes();
    assert!(!e.is_scene_playing(TIMER_SCENE));

    e.scripts.real_time = 20.0;
    assert!(e.start_scene(TIMER_SCENE, false));
    e.update_scenes();
    e.stop_scene(TIMER_SCENE);
    let legacy = Snapshot::capture(&e).write();
    let stopped = Snapshot::read(&legacy).unwrap();
    assert!(stopped.scene_lifecycle[0].stopping);
    stopped.apply(&mut e).unwrap();
    assert_eq!(e.describe_scenes().len(), 1);
    e.update_scenes();
    assert!(
        e.describe_scenes().is_empty(),
        "stopping scene resumed after load"
    );

    // An absent lifecycle section still clears any stale run.
    e.scripts.real_time = 30.0;
    assert!(e.start_scene(TIMER_SCENE, false));
    e.update_scenes();
    let mut legacy = Snapshot::capture(&e).write();
    legacy.sections.retain(|(tag, _)| {
        !matches!(
            tag,
            b"SCNE" | b"SCLF" | b"SCAB" | b"SDLG" | b"SLRP" | b"SRSQ" | b"SFGT"
        )
    });
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(
        !e.is_scene_playing(TIMER_SCENE),
        "legacy load kept a stale run"
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn package_scene_load_restores_bindings_and_owned_packages_without_restarting() {
    let (_fixture, mut e) = engine("scene-package-continuity");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, ACTOR);
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(PACKAGE_SCENE, false));
    let before_dispatch = Snapshot::capture(&e);
    assert!(!before_dispatch.scene_lifecycle[0].begun);
    assert_eq!(before_dispatch.scene_lifecycle[0].phase, None);
    let before_dispatch =
        Snapshot::read(&SaveFile::from_bytes(&before_dispatch.write().to_bytes()).unwrap())
            .unwrap();
    before_dispatch.apply(&mut e).unwrap();
    assert!(
        e.scenes.packages.is_empty(),
        "package was assigned before dispatch"
    );

    // Entering the scene's first phase dispatches the package action.
    e.update_scenes();
    let ownership = e.scenes.packages.get(&ACTOR).expect("scene owns package");
    assert_eq!((ownership.0, ownership.1), (PACKAGE_SCENE, 13));
    assert!(
        ownership
            .2
            .iter()
            .any(|package| package.id == SCENE_PACKAGE)
    );
    e.scripts.real_time = 10.5;

    let mut saved = Snapshot::capture(&e);
    let active = saved
        .scene_bindings
        .iter()
        .find(|binding| binding.action == 13)
        .unwrap();
    assert_eq!(active.actor, Some(ACTOR));
    assert!(active.package_active);
    assert_eq!(active.elapsed, Some(0.5));
    let empty = saved
        .scene_bindings
        .iter()
        .find(|binding| binding.action == 14)
        .unwrap();
    assert_eq!(empty.actor, None);
    assert!(!empty.package_active);

    // Alias 5 now points elsewhere; action 13 must retain its captured actor.
    // Alias 6 is filled after save; action 14 must retain its captured None.
    saved
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .aliases
        .insert(SCENE_ACTION_ALIAS, FormId(7));
    saved
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .aliases
        .insert(SCENE_ACTION_ALIAS + 1, FormId(7));
    let file = SaveFile::from_bytes(&saved.write().to_bytes()).unwrap();
    let loaded = Snapshot::read(&file).unwrap();
    e.scenes.packages.clear();
    loaded.apply(&mut e).unwrap();
    assert_eq!(
        e.alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS),
        Some(FormId(7))
    );
    assert_eq!(
        e.alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS + 1),
        Some(FormId(7))
    );
    let ownership = e.scenes.packages.get(&ACTOR).expect("saved owner restored");
    assert_eq!((ownership.0, ownership.1), (PACKAGE_SCENE, 13));
    assert!(
        ownership
            .2
            .iter()
            .any(|package| package.id == SCENE_PACKAGE)
    );
    assert!(e.scenes.packages.get(&FormId(7)).is_none());

    let restored = Snapshot::capture(&e);
    assert_eq!(restored.scene_lifecycle[0].phase, Some(0));
    assert!(restored.scene_lifecycle[0].begun);
    assert_eq!(
        restored
            .scene_bindings
            .iter()
            .find(|binding| binding.action == 13)
            .unwrap()
            .elapsed,
        Some(0.5),
        "restoring ownership must not reset the package action clock"
    );
    assert_eq!(
        restored
            .scene_bindings
            .iter()
            .find(|binding| binding.action == 14)
            .unwrap()
            .actor,
        None,
        "a saved empty actor binding must not re-resolve a later alias fill"
    );

    e.update_scenes();
    assert_eq!(
        e.scenes
            .packages
            .get(&ACTOR)
            .map(|(scene, action, _)| (*scene, *action)),
        Some((PACKAGE_SCENE, 13))
    );
    assert_eq!(
        Snapshot::capture(&e)
            .scene_bindings
            .iter()
            .find(|binding| binding.action == 13)
            .unwrap()
            .elapsed,
        Some(0.5),
        "the resumed update must keep package ownership's elapsed time"
    );

    // Package actions can be Done while a separate completion condition keeps
    // the phase alive; ownership must last until the phase actually ends.
    let mut completed = Snapshot::read(&file).unwrap();
    completed.scene_lifecycle[0]
        .actions
        .iter_mut()
        .find(|action| action.action == 13)
        .unwrap()
        .state = crate::scene::SavedActionState::Done;
    completed.apply(&mut e).unwrap();
    assert!(e.is_scene_playing(PACKAGE_SCENE));
    assert_eq!(
        e.scenes
            .packages
            .get(&ACTOR)
            .map(|(scene, action, _)| (*scene, *action)),
        Some((PACKAGE_SCENE, 13))
    );
    e.update_scenes();
    assert!(
        e.is_scene_playing(PACKAGE_SCENE),
        "unsatisfied phase completion condition must keep the scene and package alive"
    );
    assert!(e.scenes.packages.contains_key(&ACTOR));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic dialogue records only"]
fn dialogue_scene_load_resumes_selected_info_and_remaining_response_suffix() {
    let (_fixture, mut e) = engine("scene-dialogue-continuity");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, SCENE_ACTOR);
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(DIALOGUE_SCENE, false));
    e.update_scenes();

    let mut saved = Snapshot::capture(&e);
    assert_eq!(saved.scene_dialogue.len(), 1);
    assert_eq!(saved.scene_dialogue[0].scene, DIALOGUE_SCENE);
    assert_eq!(saved.scene_dialogue[0].action, 17);
    assert_eq!(saved.scene_dialogue[0].topic, SCENE_TOPIC);
    assert_eq!(saved.scene_dialogue[0].info, SCENE_INFO);
    assert_eq!(saved.scene_dialogue[0].current_response, 0);
    assert_eq!(saved.scene_dialogue[0].queued_responses, vec![1]);
    assert_eq!(saved.scene_dialogue[0].remaining, 2.0);
    e.scripts.real_time = 10.5;
    saved = Snapshot::capture(&e);
    assert_eq!(saved.scene_dialogue[0].remaining, 1.5);
    let bytes = saved.write().to_bytes();
    let file = SaveFile::from_bytes(&bytes).unwrap();

    // This INFO exists but belongs to another topic. Rejection occurs before
    // the live scene is unloaded or replaced.
    let mut wrong_topic = Snapshot::read(&file).unwrap();
    wrong_topic.scene_dialogue[0].info = OTHER_INFO;
    assert!(wrong_topic.apply(&mut e).is_err());
    assert!(e.is_scene_playing(DIALOGUE_SCENE));
    assert_eq!(Snapshot::capture(&e).scene_dialogue, saved.scene_dialogue);

    let loaded = Snapshot::read(&file).unwrap();
    loaded.apply(&mut e).unwrap();
    assert_eq!(Snapshot::capture(&e).scene_dialogue, saved.scene_dialogue);
    attach_scene_test_actor(&mut e);

    e.scripts.real_time = 11.5;
    e.update_scenes();
    assert_eq!(Snapshot::capture(&e).scene_dialogue[0].remaining, 0.5);
    e.scripts.real_time = 12.1;
    e.update_scenes();
    let next = Snapshot::capture(&e).scene_dialogue;
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].current_response, 1);
    assert!(next[0].queued_responses.is_empty());
    assert_eq!(next[0].remaining, 2.0);

    e.scripts.real_time = 14.2;
    e.update_scenes();
    assert!(!e.is_scene_playing(DIALOGUE_SCENE));
    assert!(Snapshot::capture(&e).scene_dialogue.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn looping_scene_dialogue_load_preserves_repeat_deadline_across_roundtrips() {
    let (_fixture, mut e) = engine("scene-repeat-continuity");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, SCENE_ACTOR);
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 20.0;
    assert!(e.start_scene(LOOP_DIALOGUE_SCENE, false));
    e.update_scenes();
    assert_eq!(Snapshot::capture(&e).scene_dialogue.len(), 1);

    // Finish both responses; the loop's fixed five-second wait is then selected.
    e.scripts.real_time = 22.5;
    e.update_scenes();
    assert_eq!(Snapshot::capture(&e).scene_dialogue[0].current_response, 1);
    e.scripts.real_time = 24.6;
    e.update_scenes();
    let saved = Snapshot::capture(&e);
    assert!(saved.scene_dialogue.is_empty());
    assert_eq!(saved.scene_repeats.len(), 1);
    assert_eq!(saved.scene_repeats[0].scene, LOOP_DIALOGUE_SCENE);
    assert_eq!(saved.scene_repeats[0].action, 18);
    assert_eq!(saved.scene_repeats[0].remaining, 5.0);

    let mut non_looping =
        Snapshot::read(&SaveFile::from_bytes(&saved.write().to_bytes()).unwrap()).unwrap();
    non_looping
        .scene_lifecycle
        .push(crate::scene::SavedSceneLifecycle {
            scene: DIALOGUE_SCENE,
            phase: Some(0),
            begun: true,
            stopping: false,
            paused: false,
            actions: vec![crate::scene::SavedSceneAction {
                action: 17,
                state: crate::scene::SavedActionState::Running,
                elapsed: None,
            }],
        });
    non_looping.scene_lifecycle.sort_by_key(|scene| scene.scene);
    non_looping
        .scene_bindings
        .push(crate::scene::SavedSceneBinding {
            scene: DIALOGUE_SCENE,
            action: 17,
            actor: Some(SCENE_ACTOR),
            package_active: false,
            elapsed: None,
        });
    non_looping
        .scene_bindings
        .sort_by_key(|binding| (binding.scene, binding.action));
    non_looping.scene_repeats[0].scene = DIALOGUE_SCENE;
    non_looping.scene_repeats[0].action = 17;
    assert!(non_looping.apply(&mut e).is_err());
    assert!(e.is_scene_playing(LOOP_DIALOGUE_SCENE));
    assert_eq!(Snapshot::capture(&e).scene_repeats, saved.scene_repeats);

    let bytes = saved.write().to_bytes();
    let mut overflowing_deadline = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    overflowing_deadline.real_time = f64::MAX;
    overflowing_deadline.scene_repeats[0].remaining = f64::MAX;
    assert!(overflowing_deadline.apply(&mut e).is_err());
    assert!(e.is_scene_playing(LOOP_DIALOGUE_SCENE));
    assert_eq!(Snapshot::capture(&e).scene_repeats, saved.scene_repeats);

    let loaded = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    loaded.apply(&mut e).unwrap();
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 26.6;
    e.update_scenes();
    let saved_again = Snapshot::capture(&e);
    assert!((saved_again.scene_repeats[0].remaining - 3.0).abs() < 0.0001);

    // A second load must retain the original deadline rather than choose a new
    // random interval or immediately begin the next INFO.
    let bytes = saved_again.write().to_bytes();
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 29.5;
    e.update_scenes();
    assert!(Snapshot::capture(&e).scene_dialogue.is_empty());
    assert!((Snapshot::capture(&e).scene_repeats[0].remaining - 0.1).abs() < 0.0001);

    // A due deadline is saved as zero and becomes eligible on the next update.
    e.scripts.real_time = 30.0;
    let zero_deadline = Snapshot::capture(&e);
    assert_eq!(zero_deadline.scene_repeats[0].remaining, 0.0);
    Snapshot::read(&SaveFile::from_bytes(&zero_deadline.write().to_bytes()).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    attach_scene_test_actor(&mut e);
    e.update_scenes();
    let resumed = Snapshot::capture(&e);
    assert_eq!(resumed.scene_repeats.len(), 0);
    assert_eq!(resumed.scene_dialogue.len(), 1);
    assert_eq!(resumed.scene_dialogue[0].current_response, 0);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn pending_scene_restarts_load_in_order_and_start_on_next_update() {
    let (_fixture, mut e) = engine("scene-restart-queue");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.scripts.real_time = 20.0;
    assert!(e.start_scene(RESTART_SCENE_A, false));
    assert!(e.start_scene(RESTART_SCENE_B, false));
    e.update_scenes();
    e.scripts.real_time = 21.1;
    e.update_scenes();

    let saved = Snapshot::capture(&e);
    assert_eq!(saved.scene_restarts, vec![RESTART_SCENE_A, RESTART_SCENE_B]);
    assert!(!e.is_scene_playing(RESTART_SCENE_A));
    assert!(!e.is_scene_playing(RESTART_SCENE_B));

    let mut invalid =
        Snapshot::read(&SaveFile::from_bytes(&saved.write().to_bytes()).unwrap()).unwrap();
    invalid.scene_restarts[0] = FormId(0x8BD);
    assert!(invalid.apply(&mut e).is_err());
    assert_eq!(Snapshot::capture(&e).scene_restarts, saved.scene_restarts);
    assert!(!e.is_scene_playing(RESTART_SCENE_A));

    let save_bytes = saved.write().to_bytes();
    Snapshot::read(&SaveFile::from_bytes(&save_bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert_eq!(Snapshot::capture(&e).scene_restarts, saved.scene_restarts);
    assert!(!e.is_scene_playing(RESTART_SCENE_A));
    assert!(!e.is_scene_playing(RESTART_SCENE_B));

    let mut legacy_file = SaveFile::from_bytes(&save_bytes).unwrap();
    legacy_file.sections.retain(|(tag, _)| tag != b"SRSQ");
    Snapshot::read(&legacy_file).unwrap().apply(&mut e).unwrap();
    assert!(Snapshot::capture(&e).scene_restarts.is_empty());
    assert!(!e.is_scene_playing(RESTART_SCENE_A));
    Snapshot::read(&SaveFile::from_bytes(&save_bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();

    // Loading only restores the pending queue. The normal scene update consumes
    // it, starts both scenes in order, and leaves their one-second timers live.
    e.update_scenes();
    assert!(e.is_scene_playing(RESTART_SCENE_A));
    assert!(e.is_scene_playing(RESTART_SCENE_B));
    assert!(Snapshot::capture(&e).scene_restarts.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn scene_force_greet_load_discards_conversation_and_preserves_completion() {
    let (_fixture, mut e) = engine("scene-force-greet-continuity");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, SCENE_ACTOR);
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(PACKAGE_SCENE, false));
    e.update_scenes();
    ready_scene_force_greet_actor(&mut e);
    assert!(e.scene_owns_force_greet_package(SCENE_ACTOR));

    // A force-greet marker is set when the scene-owned greet conversation
    // starts. The conversation itself is intentionally not in the save.
    e.update_force_greets(0.0);
    assert!(e.scenes.force_greeted.contains(&SCENE_ACTOR));
    assert!(e.conversation.is_some());
    let saved = Snapshot::capture(&e);
    assert_eq!(saved.scene_force_greeted, vec![SCENE_ACTOR]);
    let save_bytes = saved.write().to_bytes();

    let mut bad_marker = Snapshot::read(&SaveFile::from_bytes(&save_bytes).unwrap()).unwrap();
    bad_marker.scene_force_greeted[0] = ACTOR;
    assert!(bad_marker.apply(&mut e).is_err());
    assert!(
        e.conversation.is_some(),
        "rejected load cleared live dialogue"
    );
    assert!(e.scenes.force_greeted.contains(&SCENE_ACTOR));

    Snapshot::read(&SaveFile::from_bytes(&save_bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert!(e.conversation.is_none(), "transient dialogue was resumed");
    assert!(e.scenes.force_greeted.contains(&SCENE_ACTOR));

    // Missing marker data replaces stale completion state on legacy loads.
    let mut legacy = SaveFile::from_bytes(&save_bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"SFGT");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.scenes.force_greeted.is_empty());
    Snapshot::read(&SaveFile::from_bytes(&save_bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert!(e.conversation.is_none());
    assert!(e.scenes.force_greeted.contains(&SCENE_ACTOR));

    attach_scene_test_actor(&mut e);
    ready_scene_force_greet_actor(&mut e);
    e.update_scenes();
    assert!(e.is_scene_action_complete(PACKAGE_SCENE, 13));
    assert!(e.is_scene_playing(PACKAGE_SCENE));
    assert!(e.conversation.is_none());

    // The marker prevents a fresh ForceGreet package evaluation from reopening
    // the greeting after loading an in-progress conversation as completed.
    e.update_force_greets(0.0);
    assert!(e.conversation.is_none());
    assert!(e.scenes.force_greeted.contains(&SCENE_ACTOR));

    let completed = Snapshot::capture(&e);
    assert_eq!(
        completed.scene_lifecycle[0]
            .actions
            .iter()
            .find(|action| action.action == 13)
            .unwrap()
            .state,
        crate::scene::SavedActionState::Done
    );
    completed.apply(&mut e).unwrap();
    attach_scene_test_actor(&mut e);
    ready_scene_force_greet_actor(&mut e);
    e.update_scenes();
    assert!(e.is_scene_action_complete(PACKAGE_SCENE, 13));
    e.update_force_greets(0.0);
    assert!(e.conversation.is_none());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn invalid_scene_snapshots_are_rejected_before_live_state_changes() {
    let (_fixture, mut e) = engine("scene-validation-transaction");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, SCENE_ACTOR);
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(PACKAGE_SCENE, false));
    e.update_scenes();
    ready_scene_force_greet_actor(&mut e);
    e.update_force_greets(0.0);
    assert!(e.conversation.is_some());

    let bytes = Snapshot::capture(&e).write().to_bytes();
    e.scripts.real_time = 77.0;
    e.scripts.quests.get_mut(&TIMER_SCENE_QUEST).unwrap().stage = 42;
    let before_vm = format!("{:?}", e.vm.export());
    let before_lifecycle = Snapshot::capture(&e).scene_lifecycle;
    let before_bindings = Snapshot::capture(&e).scene_bindings;
    let before_markers = e.scenes.force_greeted.clone();

    let corruptions: Vec<(&str, Box<dyn Fn(&mut Snapshot)>)> = vec![
        ("non-finite clock", Box::new(|s| s.real_time = f64::NAN)),
        (
            "phase outside definition",
            Box::new(|s| s.scene_lifecycle[0].phase = Some(u32::MAX)),
        ),
        (
            "unknown action id",
            Box::new(|s| s.scene_lifecycle[0].actions[0].action = u32::MAX),
        ),
        (
            "duplicate package owner",
            Box::new(|s| {
                let binding = s
                    .scene_bindings
                    .iter_mut()
                    .find(|binding| binding.action == 14)
                    .unwrap();
                binding.actor = Some(SCENE_ACTOR);
                binding.package_active = true;
                binding.elapsed = Some(0.0);
            }),
        ),
        (
            "invalid package start time",
            Box::new(|s| {
                s.scene_bindings
                    .iter_mut()
                    .find(|binding| binding.action == 13)
                    .unwrap()
                    .elapsed = Some(f64::MAX);
            }),
        ),
    ];

    for (label, corrupt) in corruptions {
        let mut snapshot = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
        corrupt(&mut snapshot);
        assert!(snapshot.apply(&mut e).is_err(), "accepted {label}");
        assert_eq!(e.scripts.real_time, 77.0, "changed clock for {label}");
        assert_eq!(
            e.scripts.quests[&TIMER_SCENE_QUEST].stage, 42,
            "changed quest for {label}"
        );
        assert_eq!(
            format!("{:?}", e.vm.export()),
            before_vm,
            "changed VM for {label}"
        );
        assert_eq!(
            Snapshot::capture(&e).scene_lifecycle,
            before_lifecycle,
            "changed scenes for {label}"
        );
        assert_eq!(
            Snapshot::capture(&e).scene_bindings,
            before_bindings,
            "changed bindings for {label}"
        );
        assert_eq!(
            e.scenes.force_greeted, before_markers,
            "changed markers for {label}"
        );
        assert!(e.conversation.is_some(), "cleared conversation for {label}");
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn null_topic_headtracking_scene_remains_loadable() {
    let (_fixture, mut e) = engine("scene-null-topic-validation");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.force_alias_ref(TIMER_SCENE_QUEST, SCENE_ACTION_ALIAS, SCENE_ACTOR);
    attach_scene_test_actor(&mut e);
    assert!(e.start_scene(HEADTRACK_SCENE, false));
    e.update_scenes();
    assert!(e.is_scene_playing(HEADTRACK_SCENE));
    assert!(e.is_scene_action_complete(HEADTRACK_SCENE, 21));

    let saved = Snapshot::capture(&e);
    assert_eq!(saved.scene_lifecycle[0].phase, Some(0));
    saved.apply(&mut e).unwrap();
    e.update_scenes();
    assert!(e.is_scene_playing(HEADTRACK_SCENE));
    assert!(e.is_scene_action_complete(HEADTRACK_SCENE, 21));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic scene records only"]
fn legacy_timer_scene_resave_accepts_past_phase_done_actions() {
    let (_fixture, mut e) = engine("scene-legacy-phase-resave");
    e.scripts
        .quests
        .entry(TIMER_SCENE_QUEST)
        .or_default()
        .running = true;
    e.scripts.real_time = 10.0;
    assert!(e.start_scene(TWO_PHASE_TIMER_SCENE, false));
    e.update_scenes();
    e.scripts.real_time = 12.0;
    e.update_scenes();
    assert_eq!(Snapshot::capture(&e).scene_lifecycle[0].phase, Some(1));

    let mut legacy = Snapshot::capture(&e).write();
    legacy.sections.retain(|(tag, _)| {
        !matches!(
            tag,
            b"SCLF" | b"SCAB" | b"SDLG" | b"SLRP" | b"SRSQ" | b"SFGT"
        )
    });
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    let resaved = Snapshot::capture(&e);
    assert_eq!(resaved.scene_lifecycle[0].phase, Some(1));
    assert_eq!(
        resaved.scene_lifecycle[0].actions[0].state,
        crate::scene::SavedActionState::Done
    );
    let bytes = resaved.write().to_bytes();
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut e)
        .unwrap();
    assert!(e.is_scene_playing(TWO_PHASE_TIMER_SCENE));
    assert!(e.is_scene_action_complete(TWO_PHASE_TIMER_SCENE, 23));
}

#[test]
fn greater_power_cooldown_section_round_trips_and_rejects_bad_entries() {
    let mut s = Snapshot::default();
    s.power_uses.insert(GREATER_POWER_A, 17.25);
    s.power_uses.insert(GREATER_POWER_B, 22.0);
    let valid = s.write();
    let bytes = valid.to_bytes();
    let back = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(back.power_uses, s.power_uses);
    assert_eq!(back.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"PWCD");
    assert!(Snapshot::read(&legacy).unwrap().power_uses.is_empty());

    let mut malformed = Vec::new();
    let mut duplicate = valid.clone();
    let payload = &mut duplicate
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1;
    let first = payload[4..12].to_vec();
    payload[0..4].copy_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&first);
    malformed.push(duplicate);

    let mut truncated = valid.clone();
    truncated
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1
        .pop();
    malformed.push(truncated);
    let mut trailing = valid.clone();
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1
        .push(0);
    malformed.push(trailing);
    let mut unsorted = valid.clone();
    let payload = &mut unsorted
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1;
    let first = payload[4..12].to_vec();
    let second = payload[12..20].to_vec();
    payload[4..12].copy_from_slice(&second);
    payload[12..20].copy_from_slice(&first);
    malformed.push(unsorted);
    for (offset, value) in [(0, 0u32), (8, f32::NAN.to_bits()), (8, (-1.0f32).to_bits())] {
        let mut bad = valid.clone();
        let payload = &mut bad
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"PWCD")
            .unwrap()
            .1;
        if offset == 0 {
            payload[4..8].copy_from_slice(&value.to_le_bytes());
        } else {
            payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        malformed.push(bad);
    }
    for file in malformed {
        assert!(Snapshot::read(&file).is_err());
    }
}

#[test]
fn spent_soul_section_is_deterministic_and_rejects_bad_entries() {
    let mut s = Snapshot::default();
    s.spent_souls.extend([ACTOR, FormId(7)]);
    let valid = s.write();
    let bytes = valid.to_bytes();
    let back = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(back.spent_souls, s.spent_souls);
    assert_eq!(back.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SOUL");
    assert!(Snapshot::read(&legacy).unwrap().spent_souls.is_empty());

    let mut malformed = Vec::new();
    let mut null = valid.clone();
    null.sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1[4..8]
        .copy_from_slice(&0u32.to_le_bytes());
    malformed.push(null);
    let mut duplicate = valid.clone();
    let payload = &mut duplicate
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1;
    let first = payload[4..8].to_vec();
    payload[0..4].copy_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&first);
    malformed.push(duplicate);
    let mut truncated = valid.clone();
    truncated
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1
        .pop();
    malformed.push(truncated);
    let mut trailing = valid;
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1
        .push(0);
    malformed.push(trailing);
    for file in malformed {
        assert!(Snapshot::read(&file).is_err());
    }
}

#[test]
fn malformed_story_and_relationship_sections_are_refused() {
    let mut s = Snapshot::default();
    s.story.last_run.insert(STORY_FIRST, 42.5);
    s.story.ran.insert(FormId(0x880), [STORY_FIRST].into());
    s.story.next_social.insert(ACTOR, 100.25);
    s.story.last_location = Some(Some(CELL));
    s.story.found_bodies.insert((ACTOR, FormId(0x999)));
    s.relationship_ranks.insert((FormId(7), ACTOR), -4);
    for tag in [b"STOR", b"RELA"] {
        let file = s.write();
        let payload = file.section(tag).unwrap();
        for len in 0..payload.len() {
            let mut cut = file.clone();
            cut.sections
                .iter_mut()
                .find(|(t, _)| t == tag)
                .unwrap()
                .1
                .truncate(len);
            assert!(Snapshot::read(&cut).is_err(), "{tag:?} truncated at {len}");
        }
        let mut trailing = file;
        trailing
            .sections
            .iter_mut()
            .find(|(t, _)| t == tag)
            .unwrap()
            .1
            .push(0);
        assert!(Snapshot::read(&trailing).is_err());
    }
    for (a, b, rank) in [
        (ACTOR, FormId(7), 0),
        (FormId(7), ACTOR, -5),
        (FormId(7), ACTOR, 5),
    ] {
        let mut s = Snapshot::default();
        s.relationship_ranks.insert((a, b), rank);
        assert!(Snapshot::read(&s.write()).is_err());
    }
}

#[test]
fn favorite_weapon_usage_section_is_optional_deterministic_and_validated() {
    let mut s = Snapshot::default();
    s.weapon_usage = vec![(FormId(0x901), 2), (FormId(0x902), 1)];
    s.favorite_weapon = Some(FormId(0x901));
    let bytes = s.write().to_bytes();
    let mut reversed = Snapshot::default();
    reversed.weapon_usage = vec![(FormId(0x902), 1), (FormId(0x901), 2)];
    reversed.favorite_weapon = Some(FormId(0x901));
    assert_eq!(reversed.write().to_bytes(), bytes);
    assert_eq!(
        Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
            .unwrap()
            .write()
            .to_bytes(),
        bytes
    );

    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"WUSE");
    let read = Snapshot::read(&old).unwrap();
    assert!(read.weapon_usage.is_empty());
    assert_eq!(read.favorite_weapon, None);

    for (counts, favorite) in [
        (vec![(FormId::NULL, 1)], Some(FormId::NULL)),
        (vec![(FormId(0x901), 0)], Some(FormId(0x901))),
        (
            vec![(FormId(0x901), 1), (FormId(0x901), 1)],
            Some(FormId(0x901)),
        ),
        (vec![(FormId(0x901), 1)], Some(FormId(0x902))),
    ] {
        let mut malformed = Snapshot::default();
        malformed.weapon_usage = counts;
        malformed.favorite_weapon = favorite;
        assert!(Snapshot::read(&malformed.write()).is_err());
    }
}

#[test]
fn weather_section_is_optional_and_validated() {
    let mut s = Snapshot::default();
    s.weather = SavedWeather {
        current: Some(WEATHER),
        outgoing: Some(WEATHER_OTHER),
        pct: 0.25,
        overridden: true,
        before_override: Some(FormId(0x892)),
        next_roll: 3.5,
    };
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.weather, s.weather);
    assert_eq!(read.write().to_bytes(), bytes);

    // Saves from before the section keep no weather: the place picks one.
    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"WTHR");
    assert_eq!(
        Snapshot::read(&old).unwrap().weather,
        SavedWeather::default()
    );

    for weather in [
        SavedWeather {
            pct: 1.5,
            ..s.weather
        },
        SavedWeather {
            pct: f32::NAN,
            ..s.weather
        },
        SavedWeather {
            next_roll: f32::INFINITY,
            ..s.weather
        },
        SavedWeather {
            current: None,
            ..s.weather
        },
    ] {
        let mut malformed = Snapshot::default();
        malformed.weather = weather;
        assert!(Snapshot::read(&malformed.write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_preserves_story_selection_and_ranks_without_dispatching_events() {
    use crate::story::StoryEvent;
    let (_fixture, mut e) = engine("story");
    e.ai_enabled = false;
    e.update_story(); // Establish the no-location baseline.
    e.set_relationship_rank(ACTOR, FormId(7), -2);
    e.stop_quest(RANK_EVENT);
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(e.scripts.quests[&STORY_FIRST].running);
    e.stop_quest(STORY_FIRST);
    assert!(e.send_story_event(StoryEvent::new(b"SCPT")));
    e.stop_quest(COOLDOWN);
    let saved = Snapshot::capture(&e).write();
    // Pollute the destination's rank, rotation, cooldown and dispatch context.
    e.set_relationship_rank(ACTOR, FormId(7), 4);
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    e.story.active = Some(StoryEvent::new(b"TRES"));
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.relationship_rank(FormId(7), ACTOR), -2);
    assert_eq!(e.relationship_rank(ACTOR, PLAYER_REF), -2);
    assert!(!e.scripts.quests[&RANK_EVENT].running);
    assert!(e.story.active.is_none());
    e.update_story();
    assert!(
        !e.scripts
            .quests
            .get(&LOCATION_EVENT)
            .is_some_and(|q| q.running)
    );
    assert!(
        !e.send_story_event(StoryEvent::new(b"SCPT")),
        "cooldown survives load"
    );
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(
        e.scripts.quests[&STORY_SECOND].running,
        "rotation survives load"
    );
    assert!(!e.scripts.quests[&STORY_FIRST].running);
    e.hour += 24.0;
    assert!(
        e.send_story_event(StoryEvent::new(b"SCPT")),
        "cooldown expires normally"
    );
    // Old saves also replace stale state, using the default empty memory.
    let mut old = saved;
    old.sections
        .retain(|(tag, _)| tag != b"STOR" && tag != b"RELA");
    Snapshot::read(&old).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.relationship_rank(ACTOR, FormId(7)), 0);
    assert_eq!(e.story.saved_state(), Default::default());
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(e.scripts.quests[&STORY_FIRST].running);
    assert!(e.send_story_event(StoryEvent::new(b"SCPT")));
}

#[test]
fn deaths_section_keeps_killers_and_times_and_is_optional() {
    use crate::world_state::Death;
    let mut s = Snapshot::default();
    s.deaths.insert(
        FormId(0x8a0),
        Death {
            killer: Some(crate::engine::PLAYER_REF),
            at: 1234.5,
        },
    );
    s.deaths.insert(
        FormId(0x8a1),
        Death {
            killer: None,
            at: 7.25,
        },
    );
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.deaths, s.deaths);
    assert_eq!(read.write().to_bytes(), bytes);

    // Saves from before the section know only the bodies.
    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"DETH");
    assert!(Snapshot::read(&old).unwrap().deaths.is_empty());

    let mut malformed = Snapshot::default();
    malformed.deaths.insert(
        FormId(0x8a0),
        Death {
            killer: None,
            at: f64::NAN,
        },
    );
    assert!(Snapshot::read(&malformed.write()).is_err());
}

fn stealth_points_payload(entries: &[(FormId, f32, f32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (actor, points, regen_wait) in entries {
        w.form(*actor);
        w.f32(*points);
        w.f32(*regen_wait);
    }
    w.0
}

#[test]
fn stealth_point_section_is_sorted_optional_and_rejects_invalid_state() {
    let mut s = Snapshot::default();
    s.stealth_points.insert(FormId(0x8b1), (37.5, 4.0));
    s.stealth_points.insert(PLAYER_REF, (62.25, 0.0));
    let file = s.write();
    let bytes = file.to_bytes();
    assert_eq!(
        s.write().to_bytes(),
        bytes,
        "stealth entries sort deterministically"
    );
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.stealth_points, s.stealth_points);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut old = file.clone();
    old.sections.retain(|(tag, _)| tag != b"STPT");
    assert!(Snapshot::read(&old).unwrap().stealth_points.is_empty());

    let malformed = [
        stealth_points_payload(&[(FormId(0x8b0), f32::NAN, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 100.1, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), -0.1, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, f32::NAN)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, f32::INFINITY)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, -0.1)]),
        stealth_points_payload(&[(FormId::NULL, 50.0, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0), (FormId(0x8b0), 40.0, 1.0)]),
    ];
    for payload in malformed {
        let mut file = Snapshot::default().write();
        file.sections.retain(|(tag, _)| tag != b"STPT");
        file.add(b"STPT", payload);
        assert!(Snapshot::read(&file).is_err());
    }
    for cut in 0..stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0)]).len() {
        let mut file = Snapshot::default().write();
        file.sections.retain(|(tag, _)| tag != b"STPT");
        let mut payload = stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0)]);
        payload.truncate(cut);
        file.add(b"STPT", payload);
        assert!(Snapshot::read(&file).is_err());
    }
    let mut trailing = Snapshot::default().write();
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"STPT")
        .unwrap()
        .1
        .push(0);
    assert!(Snapshot::read(&trailing).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic plugin only"]
fn loading_clears_sleep_wait_and_owned_fade_transients() {
    let (_fixture, mut e) = engine("sleep-transients");
    let saved = Snapshot::capture(&e);
    e.menu = Some(crate::items::Menu::SleepWait { sleeping: true });
    e.start_waiting(4);
    e.update(Default::default(), 0.25, 20.0);
    assert!(e.waiting.is_some());
    assert!(e.imagespace.sleep_fade_amount() > 0.0);

    saved.apply(&mut e).unwrap();
    assert!(e.waiting.is_none());
    assert_ne!(
        e.menu,
        Some(crate::items::Menu::SleepWait { sleeping: true })
    );
    assert_eq!(e.imagespace.sleep_fade_amount(), 0.0);
    assert_eq!(e.scene.post.fade[3], 0.0);
}

#[test]
fn map_travel_section_roundtrip_legacy_and_malformed() {
    let mut s = Snapshot {
        fast_travel_disabled: true,
        ..Default::default()
    };
    s.map_shown.extend([FormId(0x901), FormId(0x900)]);
    s.map_discovered.insert(FormId(0x900));
    s.map_travel.insert(FormId(0x901));
    s.map_denied.insert(FormId(0x900));
    let file = s.write();
    let read = Snapshot::read(&file).unwrap();
    assert!(read.fast_travel_disabled);
    assert_eq!(read.map_discovered, s.map_discovered);
    assert_eq!(read.map_denied, s.map_denied);
    assert_eq!(read.write().section(b"MAPS"), file.section(b"MAPS"));
    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"MAPS");
    let read = Snapshot::read(&legacy).unwrap();
    assert!(!read.fast_travel_disabled);
    assert!(read.map_shown.is_empty());
    let mut duplicate = file.clone();
    duplicate.add(b"MAPS", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());
    for data in [vec![2], vec![0, 255, 255, 255, 255], vec![0; 18]] {
        let mut bad = legacy.clone();
        bad.add(b"MAPS", data);
        assert!(Snapshot::read(&bad).is_err());
    }
    s.map_denied.insert(FormId(0x901));
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
fn teammate_section_rejects_invalid_ids_duplicates_and_trailing_bytes() {
    let mut s = Snapshot::default();
    s.teammates.insert(FormId(0x900), false);
    let file = s.write();
    assert_eq!(Snapshot::read(&file).unwrap().teammates, s.teammates);
    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"TEAM");
    assert!(Snapshot::read(&legacy).unwrap().teammates.is_empty());
    let mut duplicate = file.clone();
    duplicate.add(b"TEAM", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());
    for actor in [FormId::NULL, PLAYER_REF, FormId(7)] {
        let mut w = Writer::default();
        w.len(1);
        w.form(actor);
        w.bool(true);
        let mut bad = legacy.clone();
        bad.add(b"TEAM", w.0);
        assert!(Snapshot::read(&bad).is_err());
    }
    let mut w = Writer::default();
    w.len(2);
    for _ in 0..2 {
        w.form(FormId(0x900));
        w.bool(true);
    }
    let mut bad = legacy.clone();
    bad.add(b"TEAM", w.0);
    assert!(Snapshot::read(&bad).is_err());
    let mut payload = file.section(b"TEAM").unwrap().to_vec();
    payload.push(0);
    let mut bad = legacy;
    bad.add(b"TEAM", payload);
    assert!(Snapshot::read(&bad).is_err());
}

/// Every new optional adjunct rejects duplicate sections, trailing bytes and
/// every truncated prefix, including inflated collection counts.
fn check_continuity_payload(file: &SaveFile, tag: &[u8; 4]) {
    let payload = file.section(tag).unwrap();
    for len in 0..payload.len() {
        let mut broken = file.clone();
        broken.sections.retain(|(t, _)| t != tag);
        broken.add(tag, payload[..len].to_vec());
        assert!(
            Snapshot::read(&broken).is_err(),
            "accepted {tag:?} prefix {len}"
        );
    }
    let mut duplicate = file.clone();
    duplicate.add(tag, payload.to_vec());
    assert!(Snapshot::read(&duplicate).is_err());
    let mut trailing = file.clone();
    trailing.sections.retain(|(t, _)| t != tag);
    let mut bytes = payload.to_vec();
    bytes.push(0);
    trailing.add(tag, bytes);
    assert!(Snapshot::read(&trailing).is_err());
}

#[test]
fn continuity_harvest_payload() {
    let mut s = Snapshot::default();
    s.continuity.harvested.insert(ACTOR, 1.25);
    s.continuity.harvested.insert(CELL, 30.0);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.harvested, s.continuity.harvested);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"HRVT");
    s.continuity.harvested.insert(ACTOR, f64::NAN);
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.harvested.clear();
    s.continuity.harvested.insert(FormId::NULL, 0.0);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_harvest_runtime() {
    let (_fixture, mut e) = engine("continuity-harvest");
    e.hour = 20.0;
    e.harvests.harvested.insert(ACTOR, 10.0);
    let saved = Snapshot::capture(&e).write();
    e.harvests.harvested.insert(CELL, 15.0);
    e.harvests.harvested.remove(&ACTOR);
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert!(e.is_harvested(ACTOR));
    assert!(!e.is_harvested(CELL));
    e.day += 11;
    assert!(!e.is_harvested(ACTOR));
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"HRVT");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.is_harvested(ACTOR));
}

#[test]
fn continuity_cleared_payload() {
    let mut s = Snapshot::default();
    s.continuity.cleared_locations.extend([ACTOR, CELL]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.cleared_locations,
        s.continuity.cleared_locations
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"CLRD");
    s.continuity.cleared_locations.insert(FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_cleared_runtime() {
    let (_fixture, mut e) = engine("continuity-cleared");
    e.cleared_locations.insert(ACTOR);
    let saved = Snapshot::capture(&e).write();
    e.cleared_locations.clear();
    e.cleared_locations.insert(CELL);
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert!(e.is_cleared(ACTOR));
    assert!(!e.is_cleared(CELL));
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"CLRD");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.is_cleared(ACTOR));
}

#[test]
fn continuity_location_keyword_payload() {
    let mut s = Snapshot::default();
    s.continuity
        .location_keyword_data
        .insert((CELL, GLOBAL), -3.25);
    s.continuity
        .location_keyword_data
        .insert((ACTOR, GLOBAL), 0.0);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.location_keyword_data,
        s.continuity.location_keyword_data
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"LKWD");
    s.continuity
        .location_keyword_data
        .insert((CELL, GLOBAL), f32::INFINITY);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_location_keyword_runtime() {
    let (_fixture, mut e) = engine("continuity-location_keyword");
    e.location_keyword_data.insert((CELL, GLOBAL), -3.25);
    let saved = Snapshot::capture(&e).write();
    e.location_keyword_data.insert((ACTOR, GLOBAL), 99.0);
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.location_keyword_data.get(&(CELL, GLOBAL)), Some(&-3.25));
    assert_eq!(e.location_keyword_data.len(), 1);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"LKWD");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.location_keyword_data.is_empty());
}

#[test]
fn continuity_crime_factions_payload() {
    let mut s = Snapshot::default();
    s.continuity.crime_factions.insert(ACTOR, None);
    s.continuity.crime_factions.insert(CELL, Some(GLOBAL));
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.crime_factions,
        s.continuity.crime_factions
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"CRFC");
    s.continuity.crime_factions.insert(FormId::NULL, None);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_crime_factions_runtime() {
    let (_fixture, mut e) = engine("continuity-crime_factions");
    e.set_crime_faction(ACTOR, Some(GLOBAL));
    e.set_crime_faction(CELL, None);
    let saved = Snapshot::capture(&e).write();
    e.set_crime_faction(ACTOR, None);
    e.set_crime_faction(GLOBAL, Some(QUEST));
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.crime_faction(ACTOR), Some(GLOBAL));
    assert_eq!(e.crime.crime_factions.get(&CELL), Some(&None));
    assert_eq!(e.crime.crime_factions.len(), 2);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"CRFC");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.crime_factions.is_empty());
}

#[test]
fn continuity_pickpocket_payload() {
    let caught = |at: f64, away: bool| Some(crate::crime::PickpocketCaught { at, away });
    let mut s = Snapshot::default();
    s.continuity
        .pickpocket_caught
        .extend([(ACTOR, caught(30.0, false)), (CELL, caught(12.5, true))]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.pickpocket_caught,
        s.continuity.pickpocket_caught
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"PKCT");
    check_continuity_payload(&file, b"PKTM");
    // Without PKTM the victims are caught at an unknown time.
    let mut legacy = file.clone();
    legacy.sections.retain(|(tag, _)| tag != b"PKTM");
    let restored = Snapshot::read(&legacy)
        .unwrap()
        .continuity
        .pickpocket_caught;
    assert_eq!(
        restored,
        [(ACTOR, None), (CELL, None)].into_iter().collect()
    );
    // PKTM must cover exactly PKCT's victims.
    let mut fewer = file.clone();
    fewer.sections.retain(|(tag, _)| tag != b"PKCT");
    let mut w = Writer::default();
    w.len(1);
    w.form(ACTOR);
    fewer.add(b"PKCT", w.0);
    assert!(Snapshot::read(&fewer).is_err(), "memory of no victim");
    let mut more = file.clone();
    more.sections.retain(|(tag, _)| tag != b"PKCT");
    let mut w = Writer::default();
    w.len(3);
    for id in [ACTOR, CELL, QUEST] {
        w.form(id);
    }
    more.add(b"PKCT", w.0);
    assert!(Snapshot::read(&more).is_err(), "victim without memory");
    s.continuity
        .pickpocket_caught
        .insert(ACTOR, caught(f64::NAN, false));
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.pickpocket_caught.remove(&ACTOR);
    s.continuity
        .pickpocket_caught
        .insert(FormId::NULL, caught(1.0, false));
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_pickpocket_runtime() {
    let (_fixture, mut e) = engine("continuity-pickpocket");
    let caught = |at: f64, away: bool| crate::crime::PickpocketCaught { at, away };
    e.day = 3;
    e.hour = 0.0;
    e.crime.pickpocket_caught.insert(ACTOR, caught(60.0, true));
    let saved = Snapshot::capture(&e).write();
    e.crime.pickpocket_caught.clear();
    e.crime.pickpocket_caught.insert(CELL, caught(0.0, false));
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(
        e.crime.pickpocket_caught,
        [(ACTOR, caught(60.0, true))].into_iter().collect()
    );
    assert!(e.caught_pickpocketing_by(ACTOR));
    e.day = 4;
    e.hour = 11.5;
    assert!(e.caught_pickpocketing_by(ACTOR), "47.5 hours");
    e.hour = 12.0;
    assert!(!e.caught_pickpocketing_by(ACTOR), "two days and away");
    // A legacy PKCT without PKTM: caught as it loads, and not yet away.
    let mut legacy = saved.clone();
    legacy.sections.retain(|(tag, _)| tag != b"PKTM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(
        e.crime.pickpocket_caught,
        [(ACTOR, caught(72.0, false))].into_iter().collect()
    );
    let mut legacy = saved;
    legacy
        .sections
        .retain(|(tag, _)| tag != b"PKCT" && tag != b"PKTM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.pickpocket_caught.is_empty());
}

#[test]
fn continuity_assault_payload() {
    let mut s = Snapshot::default();
    s.continuity.assaulted.extend([ACTOR, CELL]);
    s.continuity.player_assailants.insert(GLOBAL);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.assaulted, s.continuity.assaulted);
    assert_eq!(
        restored.continuity.player_assailants,
        s.continuity.player_assailants
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"ASLT");
    s.continuity.player_assailants.insert(FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_assault_runtime() {
    let (_fixture, mut e) = engine("continuity-assault");
    e.crime.assaulted.insert(ACTOR);
    e.crime.player_assailants.insert(CELL);
    let saved = Snapshot::capture(&e).write();
    e.crime.assaulted.clear();
    e.crime.player_assailants.insert(GLOBAL);
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert!(e.assaulted_by_player(ACTOR));
    assert_eq!(e.crime.player_assailants.len(), 1);
    assert!(e.crime.player_assailants.contains(&CELL));
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"ASLT");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.assaulted_by_player(ACTOR));
    assert!(e.crime.player_assailants.is_empty());
}

#[test]
fn continuity_witnesses_payload() {
    let mut s = Snapshot::default();
    s.continuity.unreported.push(crate::crime::Unreported {
        faction: GLOBAL,
        gold: 40,
        violent: true,
        witnesses: vec![ACTOR, CELL],
    });
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.unreported[0].witnesses,
        vec![ACTOR, CELL]
    );
    assert_eq!(restored.continuity.unreported[0].gold, 40);
    assert!(restored.continuity.unreported[0].violent);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"UWIT");
    s.continuity.unreported[0].gold = -1;
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_witnesses_runtime() {
    let (_fixture, mut e) = engine("continuity-witnesses");
    e.crime.unreported.push(crate::crime::Unreported {
        faction: GLOBAL,
        gold: 40,
        violent: true,
        witnesses: vec![SCENE_ACTOR],
    });
    e.mod_crime_gold(GLOBAL, 40, true);
    let saved = Snapshot::capture(&e).write();
    e.crime.unreported.clear();
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.crime.unreported.len(), 1);
    attach_scene_test_actor(&mut e);
    e.actor_mut(SCENE_ACTOR).unwrap().dead = true;
    e.update_witnesses();
    assert!(e.crime.unreported.is_empty());
    assert_eq!(e.bounty(GLOBAL).violent, 0);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"UWIT");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.unreported.is_empty());
}

#[test]
fn continuity_last_crime_payload() {
    let mut s = Snapshot::default();
    s.continuity
        .last_crime
        .insert(GLOBAL, crate::crime::CrimeType::Murder);
    s.continuity
        .last_crime
        .insert(CELL, crate::crime::CrimeType::Steal);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.last_crime, s.continuity.last_crime);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"LCRM");
    let mut bad = file.clone();
    let data = &mut bad
        .sections
        .iter_mut()
        .find(|(t, _)| t == b"LCRM")
        .unwrap()
        .1;
    data[8] = 7;
    assert!(Snapshot::read(&bad).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_last_crime_runtime() {
    let (_fixture, mut e) = engine("continuity-last_crime");
    e.crime
        .last_crime
        .insert(GLOBAL, crate::crime::CrimeType::Murder);
    let saved = Snapshot::capture(&e).write();
    e.crime
        .last_crime
        .insert(GLOBAL, crate::crime::CrimeType::Steal);
    e.crime
        .last_crime
        .insert(CELL, crate::crime::CrimeType::Escape);
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(
        e.crime.last_crime.get(&GLOBAL),
        Some(&crate::crime::CrimeType::Murder)
    );
    assert_eq!(e.crime.last_crime.len(), 1);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"LCRM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.last_crime.is_empty());
}

#[test]
fn continuity_controls_payload() {
    let mut s = Snapshot::default();
    s.continuity.controls = 0x1ff;
    s.continuity.cart_mode = true;
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.controls, 0x1ff);
    assert!(restored.continuity.cart_mode);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"CTRL");
    s.continuity.controls = 0x200;
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_controls_runtime() {
    let (_fixture, mut e) = engine("continuity-controls");
    e.disabled_controls.apply(true, [true; 8], 0);
    e.disabled_controls.apply(true, [true; 8], 1);
    e.hud_cart_mode = true;
    let saved = Snapshot::capture(&e).write();
    e.disabled_controls = Default::default();
    e.hud_cart_mode = false;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.disabled_controls.saved_bits(), 0x1ff);
    assert!(e.hud_cart_mode);
    e.disabled_controls.apply(false, [true; 8], 0);
    assert!(e.disabled_controls.camera_switch_disabled());
    e.disabled_controls.apply(false, [true; 8], 1);
    assert!(!e.disabled_controls.any());
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"CTRL");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.disabled_controls.any());
    assert!(!e.hud_cart_mode);
}

#[test]
fn continuity_voice_payload() {
    let mut s = Snapshot::default();
    s.continuity.voice_recovery = (12.5, 30.0);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.voice_recovery, (12.5, 30.0));
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"VREC");
    s.continuity.voice_recovery = (31.0, 30.0);
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.voice_recovery = (f32::NAN, 30.0);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_voice_runtime() {
    let (_fixture, mut e) = engine("continuity-voice");
    e.shouts.recovery = 15.0;
    e.shouts.recovery_total = 60.0;
    let saved = Snapshot::capture(&e).write();
    e.shouts.recovery = 1.0;
    e.shouts.recovery_total = 1.0;
    e.shouts.meter_flashes = 10;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.shout_meter_percent(), 75.0);
    assert_eq!(e.shouts.meter_flashes, 0);
    e.update_shouts(5.0);
    assert_eq!(e.shouts.recovery, 10.0);
    e.update_shouts(10.0);
    assert_eq!(e.shout_meter_percent(), 100.0);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"VREC");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.shouts.recovery, 0.0);
    assert_eq!(e.shout_meter_percent(), 100.0);
}

#[test]
fn continuity_stamina_payload() {
    let mut s = Snapshot::default();
    s.continuity.stamina_wait = 1.25;
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.stamina_wait, 1.25);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"SREG");
    s.continuity.stamina_wait = -0.5;
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.stamina_wait = f32::INFINITY;
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_stamina_runtime() {
    let (_fixture, mut e) = engine("continuity-stamina");
    let stats = crate::ai::combat::CombatStats {
        max_stamina: 100.0,
        stamina_regen: 5.0,
        ..Default::default()
    };
    e.player_stats.set(std::sync::Arc::new(stats)).unwrap();
    e.player_stamina = 10.0;
    e.player_stamina_wait = 1.25;
    let saved = Snapshot::capture(&e).write();
    e.player_stamina_wait = 30.0;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.player_stamina_wait, 1.25);
    e.update_stamina(0.5);
    assert_eq!(e.player_stamina, 10.0);
    e.update_stamina(1.0);
    assert_eq!(e.player_stamina, 10.0);
    e.update_stamina(1.0);
    assert!(e.player_stamina > 10.0);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"SREG");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.player_stamina_wait, 0.0);
}

#[test]
fn continuity_magicka_payload() {
    let mut s = Snapshot::default();
    s.continuity.magicka_wait = 1.25;
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.magicka_wait, 1.25);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"MREG");
    s.continuity.magicka_wait = -0.5;
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.magicka_wait = f32::NAN;
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_magicka_runtime() {
    let (_fixture, mut e) = engine("continuity-magicka");
    e.set_actor_value(PLAYER_REF, esp::actor_value::MAGICKA, 100.0);
    e.set_actor_value(PLAYER_REF, esp::actor_value::MAGICKA_RATE, 3.0);
    e.damage_actor_value(PLAYER_REF, esp::actor_value::MAGICKA, 50.0);
    e.casting.restore_regen_wait(1.25);
    let saved = Snapshot::capture(&e).write();
    e.casting.restore_regen_wait(30.0);
    e.casting.magicka_blinks = 5;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.casting.magicka_blinks, 0);
    let before = e.actor_value(PLAYER_REF, esp::actor_value::MAGICKA);
    e.regen_magicka(0.5);
    assert_eq!(e.actor_value(PLAYER_REF, esp::actor_value::MAGICKA), before);
    e.regen_magicka(1.0);
    e.regen_magicka(1.0);
    assert!(e.actor_value(PLAYER_REF, esp::actor_value::MAGICKA) > before);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"MREG");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.casting.saved_regen_wait(), 0.0);
}

#[test]
fn continuity_journal_payload() {
    let mut s = Snapshot::default();
    s.continuity.journal_memory = Some((true, 15, 25));
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.journal_memory, Some((true, 15, 25)));
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"JMEM");
    s.continuity.journal_memory = Some((false, u64::MAX, 0));
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_journal_runtime() {
    let (_fixture, mut e) = engine("continuity-journal");
    e.scripts.misc_objectives_hidden = true;
    e.scripts.journal_seq = 15;
    e.scripts.fill_seq = 25;
    e.scripts.quests.entry(QUEST).or_default().journal_seq = 10;
    e.scripts
        .quests
        .get_mut(&QUEST)
        .unwrap()
        .filled_at
        .insert(0, 20);
    let saved = Snapshot::capture(&e).write();
    e.scripts.misc_objectives_hidden = false;
    e.scripts.journal_seq = 100;
    e.scripts.fill_seq = 200;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert!(e.scripts.misc_objectives_hidden);
    assert_eq!(e.scripts.journal_seq, 15);
    assert_eq!(e.scripts.fill_seq, 25);
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"JMEM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.scripts.misc_objectives_hidden);
    assert_eq!(e.scripts.journal_seq, 10);
    assert_eq!(e.scripts.fill_seq, 20);
}

#[test]
fn continuity_magic_usage_payload() {
    let mut s = Snapshot::default();
    s.continuity.magic_usage.spells = crate::stats::SavedUsage {
        counts: vec![(ACTOR.0, 3), (CELL.0, 3)],
        favorite: Some(CELL.0),
    };
    s.continuity.magic_usage.schools = crate::stats::SavedUsage {
        counts: vec![(18, 2)],
        favorite: Some(18),
    };
    s.continuity.magic_usage.shouts = crate::stats::SavedUsage {
        counts: vec![(GLOBAL.0, u64::MAX)],
        favorite: Some(GLOBAL.0),
    };
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.magic_usage, s.continuity.magic_usage);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"MUSE");
    s.continuity.magic_usage.schools.counts[0].0 = 17;
    assert!(Snapshot::read(&s.write()).is_err());
    s.continuity.magic_usage.schools.counts[0].0 = 18;
    s.continuity.magic_usage.spells.favorite = Some(GLOBAL.0);
    assert!(Snapshot::read(&s.write()).is_err());
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_magic_usage_runtime() {
    let (_fixture, mut e) = engine("continuity-magic_usage");
    e.scripts.magic_usage = crate::stats::MagicUsage::restore(&crate::stats::SavedMagicUsage {
        spells: crate::stats::SavedUsage {
            counts: vec![(USAGE_SPELL_A.0, 3), (USAGE_SPELL_B.0, 3)],
            favorite: Some(USAGE_SPELL_B.0),
        },
        schools: crate::stats::SavedUsage {
            counts: vec![(18, 3), (20, 3)],
            favorite: Some(20),
        },
        shouts: crate::stats::SavedUsage {
            counts: vec![(USAGE_SHOUT_A.0, 3)],
            favorite: Some(USAGE_SHOUT_A.0),
        },
    })
    .unwrap();
    let saved = Snapshot::capture(&e).write();
    e.scripts.magic_usage = Default::default();
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    let usage = e.scripts.magic_usage.saved();
    assert_eq!(usage.spells.favorite, Some(USAGE_SPELL_B.0));
    assert_eq!(usage.schools.favorite, Some(20));
    assert_eq!(
        e.stat("Favorite Spell"),
        crate::stats::StatValue::Name("Usage Spell B".into())
    );
    e.count_spell_cast(PLAYER_REF, USAGE_SPELL_A);
    assert_eq!(
        e.stat("Favorite Spell"),
        crate::stats::StatValue::Name("Usage Spell A".into())
    );
    e.count_shout_use(USAGE_SHOUT_B);
    e.count_shout_use(USAGE_SHOUT_B);
    e.count_shout_use(USAGE_SHOUT_B);
    assert_eq!(
        e.scripts.magic_usage.saved().shouts.favorite,
        Some(USAGE_SHOUT_A.0)
    );
    e.count_shout_use(USAGE_SHOUT_B);
    assert_eq!(
        e.scripts.magic_usage.saved().shouts.favorite,
        Some(USAGE_SHOUT_B.0)
    );
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"MUSE");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(
        e.scripts.magic_usage.saved(),
        crate::stats::SavedMagicUsage::default()
    );
}

#[test]
fn continuity_camera_payload() {
    let mut s = Snapshot::default();
    s.continuity.camera = Some((true, 350.0));
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.camera, Some((true, 350.0)));
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"CAMR");
    for distance in [f32::NAN, f32::INFINITY, 0.0, 601.0] {
        s.continuity.camera = Some((true, distance));
        assert!(Snapshot::read(&s.write()).is_err());
    }
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_camera_runtime() {
    let (_fixture, mut e) = engine("continuity-camera");
    e.set_third_person(true);
    e.zoom_third_person(150.0);
    let saved = Snapshot::capture(&e).write();
    e.set_third_person(false);
    e.third_person_distance = 200.0;
    e.third_person_combat = 1.0;
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert!(e.third_person);
    assert_eq!(e.third_person_distance, 350.0);
    assert_eq!(e.third_person_combat, 0.0);
    e.zoom_third_person(500.0);
    assert_eq!(
        e.third_person_distance,
        crate::player_body::THIRD_PERSON_MAX
    );
    let mut legacy = saved;
    legacy.sections.retain(|(tag, _)| tag != b"CAMR");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.third_person);
    assert_eq!(
        e.third_person_distance,
        crate::player_body::THIRD_PERSON_DEFAULT
    );
}

#[test]
fn continuity_collection_keys_are_strict_and_counts_are_bounded() {
    let mut s = Snapshot::default();
    s.continuity.harvested.extend([(ACTOR, 1.0), (CELL, 2.0)]);
    s.continuity.cleared_locations.extend([ACTOR, CELL]);
    s.continuity
        .location_keyword_data
        .extend([((ACTOR, GLOBAL), 1.0), ((CELL, GLOBAL), 2.0)]);
    s.continuity
        .crime_factions
        .extend([(ACTOR, None), (CELL, Some(GLOBAL))]);
    let caught = Some(crate::crime::PickpocketCaught {
        at: 1.0,
        away: false,
    });
    s.continuity
        .pickpocket_caught
        .extend([(ACTOR, caught), (CELL, caught)]);
    s.continuity.assaulted.extend([ACTOR, CELL]);
    s.continuity.last_crime.extend([
        (ACTOR, crate::crime::CrimeType::Steal),
        (CELL, crate::crime::CrimeType::Murder),
    ]);
    let file = s.write();
    for (tag, stride) in [
        (b"HRVT", 12),
        (b"CLRD", 4),
        (b"LKWD", 12),
        (b"CRFC", 8),
        (b"PKCT", 4),
        (b"ASLT", 4),
        (b"LCRM", 5),
    ] {
        for mode in 0..3 {
            let mut broken = file.clone();
            let bytes = &mut broken
                .sections
                .iter_mut()
                .find(|(t, _)| t == tag)
                .unwrap()
                .1;
            match mode {
                0 => {
                    // A repeated key must not silently overwrite the first.
                    let key = bytes[4..8].to_vec();
                    bytes[4 + stride..8 + stride].copy_from_slice(&key);
                }
                1 => {
                    // Descending keys cannot bypass canonical ordering.
                    let first = bytes[4..8].to_vec();
                    let second = bytes[4 + stride..8 + stride].to_vec();
                    bytes[4..8].copy_from_slice(&second);
                    bytes[4 + stride..8 + stride].copy_from_slice(&first);
                }
                _ => bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes()),
            }
            assert!(
                Snapshot::read(&broken).is_err(),
                "accepted {tag:?} mode {mode}"
            );
        }
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_invalid_snapshots_leave_the_live_game_intact() {
    let (_fixture, mut e) = engine("continuity-transaction");
    attach_scene_test_actor(&mut e);
    e.scripts.real_time = 10.0;
    let file = Snapshot::capture(&e).write();
    e.scripts.real_time = 20.0;
    e.scripts.globals.insert(GLOBAL, 42.0);
    type Damage = fn(&mut super::continuity::Continuity);
    let cases: &[Damage] = &[
        |c| {
            c.harvested.insert(ACTOR, f64::NAN);
        },
        |c| {
            c.cleared_locations.insert(FormId::NULL);
        },
        |c| {
            c.location_keyword_data.insert((CELL, GLOBAL), f32::NAN);
        },
        |c| {
            c.crime_factions.insert(ACTOR, Some(FormId::NULL));
        },
        |c| {
            c.pickpocket_caught.insert(FormId::NULL, None);
        },
        |c| {
            c.assaulted.insert(FormId::NULL);
        },
        |c| {
            c.player_assailants.insert(FormId::NULL);
        },
        |c| {
            c.unreported.push(crate::crime::Unreported {
                faction: GLOBAL,
                gold: -1,
                violent: true,
                witnesses: vec![ACTOR],
            });
        },
        |c| {
            c.last_crime
                .insert(FormId::NULL, crate::crime::CrimeType::Steal);
        },
        |c| {
            c.controls = 0x200;
        },
        |c| {
            c.voice_recovery = (2.0, 1.0);
        },
        |c| {
            c.stamina_wait = -1.0;
        },
        |c| {
            c.magicka_wait = f32::INFINITY;
        },
        |c| {
            c.journal_memory = Some((false, u64::MAX, 0));
        },
        |c| {
            c.magic_usage.spells.favorite = Some(ACTOR.0);
        },
        |c| {
            c.camera = Some((true, 0.0));
        },
    ];
    for damage in cases {
        let mut snapshot = Snapshot::read(&file).unwrap();
        damage(&mut snapshot.continuity);
        assert!(snapshot.apply(&mut e).is_err());
        assert_eq!(e.scripts.real_time, 20.0);
        assert_eq!(e.scripts.globals.get(&GLOBAL), Some(&42.0));
        assert!(
            e.actor_ref(SCENE_ACTOR).is_some(),
            "invalid save unloaded actors"
        );
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn continuity_load_discards_cast_buttons_from_the_previous_session() {
    let (_fixture, mut e) = engine("continuity-cast-input");
    e.equip_spell(PLAYER_REF, USAGE_SPELL_A, RIGHT);
    let file = Snapshot::capture(&e).write();
    for legacy in [false, true] {
        assert!(e.player_cast_press(RIGHT));
        e.update_casting(0.25);
        let mut saved = file.clone();
        if legacy {
            saved.sections.retain(|(t, _)| t != b"MREG");
        }
        Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
        assert!(!e.player_cast_release(RIGHT));
        assert!(e.scripts.magic_usage.saved().spells.counts.is_empty());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_trespass() {
    let (_fixture, mut e) = engine("durable-trespass");
    e.crime.trespass = Some(crate::crime::Trespass {
        cell: CELL,
        level: 1,
        next: 17.5,
    });
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"TRSP");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.trespass = None;
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.crime.trespass.unwrap().level, 1);
    assert_eq!(e.crime.trespass.unwrap().next, 17.5);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"TRSP");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.trespass.is_none());
}
#[test]
fn durable_trespass_invalid() {
    let mut s = Snapshot::default();
    s.continuity.trespass = Some(crate::crime::Trespass {
        cell: CELL,
        level: 3,
        next: 0.0,
    });
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_private_homes() {
    let (_fixture, mut e) = engine("durable-private_homes");
    e.crime.private_homes.insert(ACTOR, CELL);
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"PHOM");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.private_homes.clear();
    e.crime.private_homes.insert(CELL, ACTOR);
    decoded.apply(&mut e).unwrap();
    assert_eq!(
        e.crime.private_homes,
        std::collections::HashMap::from([(ACTOR, CELL)])
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"PHOM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.private_homes.is_empty());
}
#[test]
fn durable_private_homes_invalid() {
    let mut s = Snapshot::default();
    s.continuity.private_homes.insert(ACTOR, FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_home_warnings() {
    let (_fixture, mut e) = engine("durable-home_warnings");
    e.crime.warned_homes.insert(ACTOR, CELL);
    e.crime.time_to_go_next = 17.0;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"HWAR");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.warned_homes.clear();
    e.crime.time_to_go_next = 99.0;
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.crime.warned_homes.get(&ACTOR), Some(&CELL));
    assert_eq!(e.crime.time_to_go_next, 17.0);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"HWAR");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.warned_homes.is_empty());
    assert_eq!(e.crime.time_to_go_next, 0.0);
}
#[test]
fn durable_home_warnings_invalid() {
    let mut s = Snapshot::default();
    s.continuity.home_warnings.1 = f64::INFINITY;
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_guard_pursuits() {
    let (_fixture, mut e) = engine("durable-guard_pursuits");
    e.crime.arrests.alarmed.insert(
        ACTOR,
        crate::arrest::Pursuit {
            faction: QUEST,
            bark_in: 1.5,
        },
    );
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"GPRS");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.arrests.alarmed.clear();
    e.crime.arrests.arresting = Some((ACTOR, QUEST));
    decoded.apply(&mut e).unwrap();
    assert!(e.is_alarmed(ACTOR));
    assert_eq!(e.crime.arrests.alarmed[&ACTOR].bark_in, 1.5);
    assert!(e.crime.arrests.arresting.is_none());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"GPRS");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.is_alarmed(ACTOR));
}
#[test]
fn durable_guard_pursuits_invalid() {
    let mut s = Snapshot::default();
    s.continuity.pursuits.insert(
        ACTOR,
        crate::arrest::Pursuit {
            faction: QUEST,
            bark_in: f32::NAN,
        },
    );
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_arrest_resistance() {
    let (_fixture, mut e) = engine("durable-arrest_resistance");
    e.crime.arrests.resisting.insert(QUEST);
    e.crime.arrests.yield_attempted = true;
    e.crime.arrests.sheathed_for = 0.75;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"ARRY");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.arrests.resisting.clear();
    e.crime.arrests.yield_attempted = false;
    e.crime.arrests.sheathed_for = 0.0;
    decoded.apply(&mut e).unwrap();
    assert!(e.crime.arrests.resisting.contains(&QUEST));
    assert!(e.crime.arrests.yield_attempted);
    assert_eq!(e.crime.arrests.sheathed_for, 0.75);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"ARRY");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.arrests.resisting.is_empty());
    assert!(!e.crime.arrests.yield_attempted);
    assert_eq!(e.crime.arrests.sheathed_for, 0.0);
}
#[test]
fn durable_arrest_resistance_invalid() {
    let mut s = Snapshot::default();
    s.continuity.resistance.2 = -1.0;
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_jail_sentence() {
    let (_fixture, mut e) = engine("durable-jail_sentence");
    e.crime.arrests.jailed = Some(crate::arrest::Sentence {
        faction: QUEST,
        days: 3,
        inside: ACTOR,
        owed: crate::crime::Bounty {
            violent: 250,
            nonviolent: 50,
            infamy_violent: 300,
            infamy_nonviolent: 100,
        },
    });
    e.crime.arrests.pending = Some(CELL);
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"JSEN");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.arrests.jailed = None;
    e.crime.arrests.pending = None;
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.crime.arrests.jailed.unwrap().days, 3);
    assert_eq!(e.crime.arrests.pending, Some(CELL));
    e.escape_jail();
    assert_eq!(e.bounty(QUEST).violent, 250);
    assert_eq!(e.bounty(QUEST).nonviolent, 150);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"JSEN");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.crime.arrests.jailed.is_none());
    assert!(e.crime.arrests.pending.is_none());
}
#[test]
fn durable_jail_sentence_invalid() {
    let mut s = Snapshot::default();
    s.continuity.sentence.0 = Some(crate::arrest::Sentence {
        faction: QUEST,
        days: 8,
        inside: ACTOR,
        owed: Default::default(),
    });
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_jail_days() {
    let (_fixture, mut e) = engine("durable-jail_days");
    e.crime.arrests.days_in_jail = 12;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"JDYS");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.crime.arrests.days_in_jail = 99;
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.crime.arrests.days_in_jail, 12);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"JDYS");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.crime.arrests.days_in_jail, 0);
}
#[test]
fn durable_jail_days_invalid() {
    let mut s = Snapshot::default();
    s.continuity.jail_days = -1;
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_delayed_activations() {
    let (_fixture, mut e) = engine("durable-delayed_activations");
    e.scripts.real_time = 10.0;
    e.activation.pending = vec![(15.0, ACTOR, CELL), (12.0, CELL, ACTOR)];
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"ACTQ");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.activation.pending = vec![(0.0, QUEST, ACTOR)];
    decoded.apply(&mut e).unwrap();
    assert_eq!(
        e.activation.pending,
        vec![(15.0, ACTOR, CELL), (12.0, CELL, ACTOR)]
    );
    e.update_activations();
    assert_eq!(e.activation.pending.len(), 2);
    e.scripts.real_time = 12.0;
    e.update_activations();
    assert_eq!(e.activation.pending, vec![(15.0, ACTOR, CELL)]);
    e.scripts.real_time = 16.0;
    e.update_activations();
    assert!(e.activation.pending.is_empty());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"ACTQ");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.activation.pending.is_empty());
}
#[test]
fn durable_delayed_activations_invalid() {
    let mut s = Snapshot::default();
    s.continuity.activations.push((f64::NAN, ACTOR, CELL));
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_random_streams() {
    let (_fixture, mut e) = engine("durable-random_streams");
    e.rng = 12345;
    e.cond_rng.set(789);
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"RNGS");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    let expected = (e.rand(), e.peek_rand(), e.rand(), e.peek_rand());
    e.rng = 1;
    e.cond_rng.set(2);
    decoded.apply(&mut e).unwrap();
    assert_eq!((e.rand(), e.peek_rand(), e.rand(), e.peek_rand()), expected);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"RNGS");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.rng, 1);
    assert_eq!(e.cond_rng.get(), 0x9E37_79B9_7F4A_7C15);
}
#[test]
fn durable_random_streams_invalid() {
    let mut s = Snapshot::default();
    s.continuity.random_streams = Some((0, 1));
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_critical_hits() {
    let (_fixture, mut e) = engine("durable-critical_hits");
    e.critically_hit.insert(ACTOR);
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"LHCR");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.critically_hit.clear();
    e.critically_hit.insert(CELL);
    decoded.apply(&mut e).unwrap();
    assert!(e.critically_hit.contains(&ACTOR));
    assert!(!e.critically_hit.contains(&CELL));
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"LHCR");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.critically_hit.is_empty());
}
#[test]
fn durable_critical_hits_invalid() {
    let mut s = Snapshot::default();
    s.continuity.critical_hits.insert(FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_sneak_stance() {
    let (_fixture, mut e) = engine("durable-sneak_stance");
    e.player.sneaking = true;
    e.player.crouch = 1.0;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"SNEK");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.player.sneaking = false;
    e.player.crouch = 0.0;
    e.player.sprinting = true;
    decoded.apply(&mut e).unwrap();
    assert!(e.player.sneaking);
    assert_eq!(e.player.crouch, 1.0);
    assert!(!e.player.sprinting);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"SNEK");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(!e.player.sneaking);
    assert_eq!(e.player.crouch, 0.0);
}
#[test]
fn durable_sneak_stance_invalid() {
    let mut file = Snapshot::default().write();
    file.sections.retain(|(tag, _)| tag != b"SNEK");
    file.add(b"SNEK", vec![2]);
    assert!(Snapshot::read(&file).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_attack_recovery() {
    let (_fixture, mut e) = engine("durable-attack_recovery");
    e.player_attack_recovery = 0.35;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"ATRC");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.player_attack_recovery = 99.0;
    e.player_blocking = true;
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.player_attack_recovery, 0.35);
    assert!(!e.player_blocking);
    assert!(e.player_swing.is_none());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"ATRC");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.player_attack_recovery, 0.0);
}
#[test]
fn durable_attack_recovery_invalid() {
    let mut s = Snapshot::default();
    s.continuity.attack_recovery = f32::INFINITY;
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_map_camera() {
    let (_fixture, mut e) = engine("durable-map_camera");
    e.map_start = Some((glam::Vec2::new(100.0, -200.0), 0.4, 60.0));
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"MCAM");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.map_start = None;
    e.menu = Some(crate::items::Menu::Map);
    decoded.apply(&mut e).unwrap();
    assert_eq!(
        e.map_start,
        Some((glam::Vec2::new(100.0, -200.0), 0.4, 60.0))
    );
    assert!(e.map_view.is_none());
    assert!(e.menu.is_none());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"MCAM");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.map_start.is_none());
    // Console start preferences are clamped by map opening; capture uses the
    // same bounds even if a save is requested before the map opens.
    e.map_start = Some((glam::Vec2::ZERO, -2.0, 100.0));
    let captured = Snapshot::capture(&e);
    assert_eq!(
        captured.continuity.map_camera,
        Some((glam::Vec2::ZERO, 0.0, 89.0))
    );
    assert!(Snapshot::read(&captured.write()).is_ok());
}
#[test]
fn durable_map_camera_invalid() {
    let mut s = Snapshot::default();
    s.continuity.map_camera = Some((glam::Vec2::ZERO, f32::NAN, 60.0));
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_image_modifiers() {
    let (_fixture, mut e) = engine("durable-image_modifiers");
    e.apply_imod_cross_fade(FormId(0x8D4), 2.0);
    e.update_imagespace(0.5);
    e.apply_imod_cross_fade(FormId(0x8D5), 3.0);
    e.update_imagespace(0.5);
    let expected = e.imagespace.active.clone();
    let blur = e.scene.post.blur;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"IMOD");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.imagespace.active.clear();
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.imagespace.active, expected);
    e.update_imagespace(0.0);
    assert!((e.scene.post.blur - blur).abs() < 1e-5);
    e.update_imagespace(3.0);
    assert_eq!(e.imagespace.active.len(), 1);
    assert_eq!(e.imagespace.active[0].imad, FormId(0x8D5));
    assert!((e.scene.post.blur - 20.0).abs() < 1e-5);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"IMOD");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.imagespace.active.is_empty());
    e.update_imagespace(0.0);
    assert_eq!(e.scene.post.blur, 0.0);
}
#[test]
fn durable_image_modifiers_invalid() {
    let mut s = Snapshot::default();
    s.continuity
        .image_modifiers
        .push(crate::imagespace::Active {
            imad: FormId(0x8D4),
            t: f32::NAN,
            strength: 1.0,
            fade_in: 0.0,
            fade_out: None,
            cross_fade: false,
        });
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_script_fade() {
    let (_fixture, mut e) = engine("durable-script_fade");
    e.fade_out_game(true, false, 0.5, 2.0);
    e.update_imagespace(1.0);
    let expected = e.imagespace.fade;
    let expected_post = e.scene.post.fade;
    let file = Snapshot::capture(&e).write();
    check_continuity_payload(&file, b"GFAD");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    e.fade_out_game(false, true, 0.0, 10.0);
    e.shake_camera(None, 1.0, 100.0);
    decoded.apply(&mut e).unwrap();
    assert_eq!(e.imagespace.fade, expected);
    e.update_imagespace(0.0);
    assert_eq!(e.scene.post.fade, expected_post);
    e.update_imagespace(2.0);
    assert_eq!(e.scene.post.fade, [1.0; 4]);
    e.fade_out_game(false, true, 0.0, 1.0);
    e.update_imagespace(1.0);
    assert!(e.imagespace.fade.is_none());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"GFAD");
    Snapshot::read(&legacy).unwrap().apply(&mut e).unwrap();
    assert!(e.imagespace.fade.is_none());
    e.update_imagespace(0.0);
    assert_eq!(e.scene.post.fade[3], 0.0);
}
#[test]
fn durable_script_fade_invalid() {
    let mut s = Snapshot::default();
    s.continuity.script_fade = Some(crate::imagespace::GameFade {
        out: true,
        color: [0.0; 3],
        delay: 0.0,
        duration: -1.0,
        t: 0.0,
    });
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
fn durable_collections_reject_reordered_duplicate_and_inflated_keys() {
    let mut s = Snapshot::default();
    for id in [ACTOR, CELL] {
        s.continuity.private_homes.insert(id, QUEST);
        s.continuity.home_warnings.0.insert(id, QUEST);
        s.continuity.pursuits.insert(
            id,
            crate::arrest::Pursuit {
                faction: QUEST,
                bark_in: 1.0,
            },
        );
        s.continuity.resistance.0.insert(id);
        s.continuity.critical_hits.insert(id);
    }
    let file = s.write();
    for (tag, offset, stride) in [
        (b"PHOM", 4, 8),
        (b"HWAR", 12, 8),
        (b"GPRS", 4, 12),
        (b"ARRY", 9, 4),
        (b"LHCR", 4, 4),
    ] {
        let payload = file.section(tag).unwrap();
        for mode in 0..3 {
            let mut bytes = payload.to_vec();
            if mode == 0 {
                let key = bytes[offset..offset + 4].to_vec();
                bytes[offset + stride..offset + stride + 4].copy_from_slice(&key);
            } else if mode == 1 {
                let first = bytes[offset..offset + stride].to_vec();
                let second = bytes[offset + stride..offset + 2 * stride].to_vec();
                bytes[offset..offset + stride].copy_from_slice(&second);
                bytes[offset + stride..offset + 2 * stride].copy_from_slice(&first);
            } else {
                bytes[offset - 4..offset].copy_from_slice(&u32::MAX.to_le_bytes());
            }
            let mut bad = file.clone();
            bad.sections.retain(|(t, _)| t != tag);
            bad.add(tag, bytes);
            assert!(
                Snapshot::read(&bad).is_err(),
                "accepted {tag:?} mode {mode}"
            );
        }
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_invalid_apply_is_transactional() {
    let (_fixture, mut e) = engine("durable-transactional");
    e.player_health = 123.0;
    e.rng = 12345;
    e.crime.arrests.days_in_jail = 17;
    e.player.sneaking = true;
    for case in 0..14 {
        let mut s = Snapshot::capture(&e);
        match case {
            0 => {
                s.continuity.trespass = Some(crate::crime::Trespass {
                    cell: CELL,
                    level: -1,
                    next: 0.0,
                })
            }
            1 => {
                s.continuity.private_homes.insert(FormId::NULL, CELL);
            }
            2 => s.continuity.home_warnings.1 = f64::NAN,
            3 => {
                s.continuity.pursuits.insert(
                    ACTOR,
                    crate::arrest::Pursuit {
                        faction: QUEST,
                        bark_in: -1.0,
                    },
                );
            }
            4 => s.continuity.resistance.2 = f32::NAN,
            5 => s.continuity.sentence.1 = Some(FormId::NULL),
            6 => s.continuity.jail_days = -1,
            7 => s.continuity.activations.push((0.0, ACTOR, FormId::NULL)),
            8 => s.continuity.random_streams = Some((0, 0)),
            9 => {
                s.continuity.critical_hits.insert(FormId::NULL);
            }
            10 => s.continuity.attack_recovery = -1.0,
            11 => s.continuity.map_camera = Some((glam::Vec2::ZERO, 2.0, 60.0)),
            12 => s
                .continuity
                .image_modifiers
                .push(crate::imagespace::Active {
                    imad: FormId::NULL,
                    t: 0.0,
                    strength: 1.0,
                    fade_in: 0.0,
                    fade_out: None,
                    cross_fade: false,
                }),
            13 => {
                s.continuity.script_fade = Some(crate::imagespace::GameFade {
                    out: true,
                    color: [0.5; 3],
                    delay: 0.0,
                    duration: 1.0,
                    t: 0.0,
                })
            }
            _ => unreachable!(),
        }
        assert!(s.apply(&mut e).is_err(), "accepted invalid case {case}");
        assert_eq!(e.player_health, 123.0);
        assert_eq!(e.rng, 12345);
        assert_eq!(e.crime.arrests.days_in_jail, 17);
        assert!(e.player.sneaking);
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn durable_cell_attachment_preserves_stance_and_random_continuation() {
    let (_fixture, mut e) = engine("durable-cell-attachment");
    e.player.sneaking = true;
    e.rng = 12345;
    e.cond_rng.set(789);
    let mut s = Snapshot::capture(&e);
    s.spot = Some(Spot {
        location: Location::Interior(CELL),
        feet: Vec3::ZERO,
        yaw: 0.0,
    });
    let expected = (e.rand(), e.peek_rand());
    e.player.sneaking = false;
    s.apply(&mut e).unwrap();
    assert_eq!(e.location, Location::Interior(CELL));
    assert!(e.player.sneaking);
    assert_eq!(e.player.crouch, 1.0);
    assert_eq!((e.rand(), e.peek_rand()), expected);
}

fn carry_weight(e: &Engine) -> f32 {
    e.actor_value(PLAYER_REF, esp::actor_value::CARRY_WEIGHT)
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn active_effects_resume_without_reapplying_their_held_values() {
    let (_a, mut a) = engine("active-effects-a");
    let base = carry_weight(&a);
    assert!(a.add_spell(PLAYER_REF, FORTIFY_CARRY_ABILITY));
    assert!(a.apply_item(FORTIFY_CARRY_SPELL, Some(PLAYER_REF), PLAYER_REF));
    a.update_magic(10.0);
    assert_eq!(carry_weight(&a), base + 50.0);
    let expected = a.saved_active_effects();
    assert_eq!(expected.0.len(), 2);
    let file = Snapshot::capture(&a).write();
    check_continuity_payload(&file, b"AEFF");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());

    // Into a fresh engine: the saved modifiers already hold both effects.
    let (_b, mut b) = engine("active-effects-b");
    decoded.apply(&mut b).unwrap();
    assert_eq!(b.saved_active_effects(), expected);
    assert_eq!(carry_weight(&b), base + 50.0);
    b.update_magic(1.0);
    assert_eq!(
        carry_weight(&b),
        base + 50.0,
        "the ability isn't applied again"
    );
    b.update_magic(19.5);
    assert_eq!(
        carry_weight(&b),
        base + 25.0,
        "the spell keeps its elapsed time"
    );
    // New effects never take a restored id.
    assert!(b.apply_item(FORTIFY_CARRY_SPELL, Some(PLAYER_REF), PLAYER_REF));
    let ids: std::collections::HashSet<u64> = b.magic.effects.iter().map(|x| x.id).collect();
    assert_eq!(ids.len(), b.magic.effects.len());
    assert!(
        b.magic
            .effects
            .iter()
            .all(|x| x.id <= b.saved_active_effects().1)
    );

    // Into the engine that already has (other) effects: they go without
    // giving anything back into the loaded values.
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.saved_active_effects(), expected);
    assert_eq!(carry_weight(&b), base + 50.0);
    b.update_magic(25.0);
    assert_eq!(carry_weight(&b), base + 25.0);

    // A legacy save keeps no effects and drops the live ones unreleased.
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"AEFF");
    let (_c, mut c) = engine("active-effects-legacy");
    assert!(c.apply_item(FORTIFY_CARRY_SPELL, Some(PLAYER_REF), PLAYER_REF));
    Snapshot::read(&legacy).unwrap().apply(&mut c).unwrap();
    assert!(c.saved_active_effects().0.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn active_effects_naming_other_records_are_refused_before_loading() {
    let (_a, mut a) = engine("active-effects-records");
    assert!(a.apply_item(FORTIFY_CARRY_SPELL, Some(PLAYER_REF), PLAYER_REF));
    let mut snapshot = Snapshot::capture(&a);
    snapshot.continuity.active_effects.0[0].index = 1;
    let file = snapshot.write();
    let (_b, mut b) = engine("active-effects-records-b");
    b.day = 77;
    assert!(Snapshot::read(&file).unwrap().apply(&mut b).is_err());
    assert_eq!(b.day, 77, "nothing applied");
}

#[test]
fn active_effects_payload_is_validated() {
    let effect = crate::magic::ActiveEffect {
        id: 3,
        target: PLAYER_REF,
        caster: Some(PLAYER_REF),
        item: FORTIFY_CARRY_SPELL,
        index: 0,
        effect: FORTIFY_CARRY,
        magnitude: 25.0,
        duration: f32::INFINITY,
        elapsed: 2.0,
        active: true,
        held: [25.0, 0.0],
        scripted: true,
        done: false,
        quiet: true,
        at: Some(Vec3::new(1.0, 2.0, 3.0)),
        summoned: Some(FormId(0xFF00_0800)),
        dual: true,
        channelled: false,
    };
    let mut s = Snapshot::default();
    s.continuity.active_effects = (vec![effect.clone()], 4);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.active_effects,
        s.continuity.active_effects
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"AEFF");
    let broken: [fn(&mut crate::magic::ActiveEffect); 6] = [
        |x| x.id = 9,
        |x| x.magnitude = f32::NAN,
        |x| x.duration = -1.0,
        |x| x.elapsed = f32::INFINITY,
        |x| x.held[1] = f32::NAN,
        |x| x.target = FormId::NULL,
    ];
    for b in broken {
        let mut x = effect.clone();
        b(&mut x);
        s.continuity.active_effects = (vec![x], 4);
        assert!(Snapshot::read(&s.write()).is_err());
    }
    let mut twin = effect.clone();
    twin.id = 4;
    s.continuity.active_effects = (vec![effect.clone(), twin.clone()], 4);
    assert!(
        Snapshot::read(&s.write()).is_err(),
        "one summon, two effects"
    );
    twin.summoned = None;
    s.continuity.active_effects = (vec![twin, effect], 4);
    assert!(Snapshot::read(&s.write()).is_err(), "unsorted ids");
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn restored_summon_effects_command_their_summons_again() {
    let (_a, mut a) = engine("active-effects-summon");
    assert!(a.apply_item(FORTIFY_CARRY_SPELL, Some(ACTOR), PLAYER_REF));
    let mut snapshot = Snapshot::capture(&a);
    snapshot.continuity.active_effects.0[0].summoned = Some(FormId(0xFF00_0900));
    a.magic.commanded.insert(FormId(0xFF00_0901), PLAYER_REF);
    Snapshot::read(&snapshot.write())
        .unwrap()
        .apply(&mut a)
        .unwrap();
    assert_eq!(
        a.magic.commanded,
        [(FormId(0xFF00_0900), ACTOR)].into_iter().collect()
    );
}

/// Deliver queued script events, as an update does.
fn run_pending_events(e: &mut Engine) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        for (obj, ev, args) in std::mem::take(&mut host.engine.scripts.pending_events) {
            vm.send_event(&mut host, obj, &ev, args);
        }
        let now = host.engine.scripts.real_time;
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn saved_effects_keep_their_scripts_and_finish_them_after_loading() {
    let (_a, mut a) = engine("effect-scripts-a");
    assert!(a.apply_item(SCRIPTED_SPELL, Some(PLAYER_REF), PLAYER_REF));
    let id = a.magic.effects[0].id;
    let obj = papyrus::ObjectId::Effect(id);
    run_pending_events(&mut a);
    assert_eq!(a.vm.get_var(obj, "starts"), Some(papyrus::Value::Int(1)));
    let file = Snapshot::capture(&a).write();
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.vm.effect_objects(), [id].into());

    let (_b, mut b) = engine("effect-scripts-b");
    decoded.apply(&mut b).unwrap();
    assert_eq!(b.vm.get_var(obj, "starts"), Some(papyrus::Value::Int(1)));
    b.update_magic(1.0);
    run_pending_events(&mut b);
    assert_eq!(
        b.vm.get_var(obj, "starts"),
        Some(papyrus::Value::Int(1)),
        "not started again"
    );
    b.update_magic(30.0);
    run_pending_events(&mut b);
    assert_eq!(b.vm.get_var(obj, "finished"), Some(papyrus::Value::Int(1)));

    // Finished effects' lingering scripts aren't kept.
    let file = Snapshot::capture(&b).write();
    assert!(
        Snapshot::read(&file)
            .unwrap()
            .vm
            .effect_objects()
            .is_empty()
    );

    // Scripts on an effect the save doesn't keep are refused.
    let mut orphan = SaveFile::from_bytes(&Snapshot::capture(&a).write().to_bytes()).unwrap();
    orphan.sections.retain(|(tag, _)| tag != b"AEFF");
    let (_c, mut c) = engine("effect-scripts-orphan");
    c.day = 55;
    assert!(Snapshot::read(&orphan).unwrap().apply(&mut c).is_err());
    assert_eq!(c.day, 55);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_caught_pickpocket_victim_forgets_only_after_two_days_away() {
    let (_fixture, mut e) = engine("pickpocket-forgets");
    attach_scene_test_actor(&mut e);
    e.crime.pickpocket_caught.insert(
        SCENE_ACTOR,
        crate::crime::PickpocketCaught {
            at: e.game_hours_total(),
            away: false,
        },
    );
    e.day += 3;
    assert!(
        e.caught_pickpocketing_by(SCENE_ACTOR),
        "waiting in the same room doesn't reset it"
    );
    e.despawn_actor(SCENE_ACTOR);
    assert!(!e.caught_pickpocketing_by(SCENE_ACTOR));
    e.player.sneaking = true;
    // An expired memory goes when the player tries again.
    assert!(!e.start_pickpocket(SCENE_ACTOR, "Victim"));
    assert!(e.crime.pickpocket_caught.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn tapering_value_changes_run_out_after_loading() {
    let (_a, mut a) = engine("tapers-a");
    a.mod_actor_value(PLAYER_REF, esp::actor_value::CARRY_WEIGHT, 500.0);
    let full = carry_weight(&a);
    assert!(a.apply_item(TAPERING_SPELL, Some(PLAYER_REF), PLAYER_REF));
    a.update_magic(0.5);
    assert_eq!(carry_weight(&a), full - 37.5);
    let file = Snapshot::capture(&a).write();
    check_continuity_payload(&file, b"TAPR");
    let decoded = Snapshot::read(&file).unwrap();
    assert_eq!(decoded.write().to_bytes(), file.to_bytes());
    assert_eq!(decoded.continuity.tapers.len(), 1);

    let (_b, mut b) = engine("tapers-b");
    decoded.apply(&mut b).unwrap();
    assert_eq!(carry_weight(&b), full - 37.5);
    b.update_magic(1.5);
    b.update_magic(1.0);
    assert_eq!(
        carry_weight(&b),
        full - 75.0,
        "the rest of the taper, no more"
    );

    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"TAPR");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    b.update_magic(2.0);
    assert_eq!(carry_weight(&b), full - 37.5, "legacy saves keep no tapers");
}

#[test]
fn graph_open_states_payload() {
    use crate::world::animated::OpenState;
    let mut s = Snapshot::default();
    s.continuity
        .graph_open
        .extend([(ACTOR, OpenState::Opening), (CELL, OpenState::Closed)]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.graph_open, s.continuity.graph_open);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"GOPN");
    for code in [0u8, 5] {
        let mut broken = file.clone();
        broken.sections.retain(|(tag, _)| tag != b"GOPN");
        let mut w = Writer::default();
        w.len(1);
        w.form(ACTOR);
        w.u8(code);
        broken.add(b"GOPN", w.0);
        assert!(Snapshot::read(&broken).is_err(), "state code {code}");
    }
    s.continuity
        .graph_open
        .insert(FormId::NULL, OpenState::Open);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn graph_open_states_answer_get_open_state_after_loading() {
    use crate::world::animated::OpenState;
    let (_a, mut a) = engine("graph-open-a");
    a.world_state.graph_open.insert(ACTOR, OpenState::Open);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("graph-open-b");
    b.world_state.graph_open.insert(CELL, OpenState::Closing);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.open_state(ACTOR), OpenState::Open);
    assert_eq!(
        b.world_state.graph_open,
        [(ACTOR, OpenState::Open)].into_iter().collect()
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"GOPN");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.world_state.graph_open.is_empty());
}

#[test]
fn faction_reactions_payload() {
    use crate::factions::Reaction;
    let mut s = Snapshot::default();
    s.continuity.faction_reactions.extend([
        (
            (ACTOR, CELL),
            Reaction {
                modifier: -10,
                combat: 1,
            },
        ),
        (
            (CELL, ACTOR),
            Reaction {
                modifier: 0,
                combat: 3,
            },
        ),
    ]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.faction_reactions,
        s.continuity.faction_reactions
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"FRCT");
    let mut bad = s.continuity.faction_reactions.clone();
    bad.insert(
        (ACTOR, CELL),
        Reaction {
            modifier: 0,
            combat: 4,
        },
    );
    let mut broken = Snapshot::default();
    broken.continuity.faction_reactions = bad;
    assert!(
        Snapshot::read(&broken.write()).is_err(),
        "combat reaction 4"
    );
    let mut broken = Snapshot::default();
    broken
        .continuity
        .faction_reactions
        .insert((FormId::NULL, CELL), Reaction::default());
    assert!(Snapshot::read(&broken.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn faction_reactions_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("faction-reactions-a");
    a.set_factions_enemies(ACTOR, CELL, false, true);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("faction-reactions-b");
    b.set_factions_allies(CELL, GLOBAL, true, true);
    assert_eq!(b.faction_combat_reaction(CELL, GLOBAL), 3);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.faction_combat_reaction(ACTOR, CELL), 1);
    assert_eq!(b.faction_combat_reaction(CELL, ACTOR), 0);
    assert_eq!(b.faction_combat_reaction(CELL, GLOBAL), 0, "replaced");
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"FRCT");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.factions.reactions.is_empty());
}

#[test]
fn faction_player_flags_payload() {
    use crate::factions::{PLAYER_ENEMY, PLAYER_EXPELLED};
    let mut s = Snapshot::default();
    s.continuity.faction_player_flags.extend([
        (ACTOR, PLAYER_ENEMY),
        (CELL, 0),
        (GLOBAL, PLAYER_ENEMY | PLAYER_EXPELLED),
    ]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.faction_player_flags,
        s.continuity.faction_player_flags
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"FPLR");
    for flags in [1u8, 0x40] {
        let mut broken = Snapshot::default();
        broken.continuity.faction_player_flags.insert(ACTOR, flags);
        assert!(Snapshot::read(&broken.write()).is_err(), "flags {flags:#x}");
    }
    let mut broken = Snapshot::default();
    broken
        .continuity
        .faction_player_flags
        .insert(FormId::NULL, 0);
    assert!(Snapshot::read(&broken.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn faction_player_flags_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("faction-flags-a");
    a.factions
        .player_flags
        .insert(ACTOR, crate::factions::PLAYER_ENEMY);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("faction-flags-b");
    b.factions
        .player_flags
        .insert(CELL, crate::factions::PLAYER_EXPELLED);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert!(b.player_enemy_of(ACTOR));
    assert!(!b.player_expelled_from(CELL), "replaced");
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"FPLR");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.factions.player_flags.is_empty());
}

#[test]
fn attacked_on_sight_payload() {
    let mut s = Snapshot::default();
    s.continuity.attacked_on_sight.extend([PLAYER_REF, ACTOR]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.attacked_on_sight,
        s.continuity.attacked_on_sight
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"AOSG");
    s.continuity.attacked_on_sight.insert(FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn attacked_on_sight_replaces_the_sessions_after_loading() {
    let (_a, mut a) = engine("on-sight-a");
    a.set_attacked_on_sight(PLAYER_REF, true);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("on-sight-b");
    b.set_attacked_on_sight(ACTOR, true);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(
        b.factions.attacked_on_sight,
        [PLAYER_REF].into_iter().collect()
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"AOSG");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.factions.attacked_on_sight.is_empty());
}

#[test]
fn base_flags_payload() {
    use crate::protection::acbs;
    let snapshot = |extra: Option<((FormId, u32), bool)>| {
        let mut s = Snapshot::default();
        s.continuity.base_flags.extend([
            ((ACTOR, acbs::ESSENTIAL), true),
            ((ACTOR, acbs::INVULNERABLE), false),
            ((FormId(0x7), acbs::PROTECTED), true),
        ]);
        s.continuity.base_flags.extend(extra);
        s
    };
    let s = snapshot(None);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.base_flags, s.continuity.base_flags);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"BFLG");
    for bad in [
        ((FormId::NULL, acbs::ESSENTIAL), true),
        ((ACTOR, 0x4), true),
        ((ACTOR, acbs::ESSENTIAL | acbs::PROTECTED), true),
    ] {
        assert!(Snapshot::read(&snapshot(Some(bad)).write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn base_flags_replace_the_sessions_after_loading() {
    use crate::protection::acbs;
    let (_a, mut a) = engine("base-flags-a");
    a.set_base_flag(ACTOR, acbs::ESSENTIAL, true);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("base-flags-b");
    b.set_base_flag(ACTOR, acbs::PROTECTED, true);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(
        b.protection.base_flags,
        [((ACTOR, acbs::ESSENTIAL), true)].into_iter().collect()
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"BFLG");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.protection.base_flags.is_empty());
}

#[test]
fn ghosts_payload() {
    let mut s = Snapshot::default();
    s.continuity
        .ghosts
        .extend([(ACTOR, true), (PLAYER_REF, false)]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.ghosts, s.continuity.ghosts);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"GHST");
    s.continuity.ghosts.insert(FormId::NULL, true);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn ghosts_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("ghosts-a");
    a.set_ghost(ACTOR, true);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("ghosts-b");
    b.set_ghost(PLAYER_REF, true);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.protection.ghosts, [(ACTOR, true)].into_iter().collect());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"GHST");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.protection.ghosts.is_empty());
}

#[test]
fn formlist_additions_payload() {
    let snapshot = |extra: Option<(FormId, Vec<FormId>)>| {
        let mut s = Snapshot::default();
        s.continuity
            .formlist_additions
            .extend([(CELL, vec![ACTOR, PLAYER_REF]), (ACTOR, vec![CELL])]);
        s.continuity.formlist_additions.extend(extra);
        s
    };
    let s = snapshot(None);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.formlist_additions,
        s.continuity.formlist_additions
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"FLAD");
    for bad in [
        (FormId::NULL, vec![ACTOR]),
        (PLAYER_REF, vec![]),
        (PLAYER_REF, vec![FormId::NULL]),
        (PLAYER_REF, vec![ACTOR, ACTOR]),
    ] {
        assert!(Snapshot::read(&snapshot(Some(bad)).write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn formlist_additions_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("formlists-a");
    a.add_to_formlist(CELL, ACTOR);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("formlists-b");
    b.add_to_formlist(CELL, PLAYER_REF);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(
        b.formlists.added,
        [(CELL, vec![ACTOR])].into_iter().collect()
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"FLAD");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.formlists.added.is_empty());
}

#[test]
fn leveled_additions_payload() {
    let snapshot = |extra: Option<(FormId, Vec<(u16, FormId, u16)>)>| {
        let mut s = Snapshot::default();
        s.continuity.leveled_additions.extend([
            (
                CELL,
                vec![(1, ACTOR, 3), (10, PLAYER_REF, 1), (1, ACTOR, 3)],
            ),
            (ACTOR, vec![(0, CELL, 1)]),
        ]);
        s.continuity.leveled_additions.extend(extra);
        s
    };
    let s = snapshot(None);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.leveled_additions,
        s.continuity.leveled_additions
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"LVAD");
    for bad in [
        (FormId::NULL, vec![(1, ACTOR, 1)]),
        (PLAYER_REF, vec![]),
        (PLAYER_REF, vec![(1, FormId::NULL, 1)]),
        (PLAYER_REF, vec![(1, ACTOR, 0)]),
        (PLAYER_REF, vec![(1, ACTOR, 1); 256]),
    ] {
        assert!(Snapshot::read(&snapshot(Some(bad)).write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn leveled_additions_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("leveled-a");
    a.scripts
        .globals
        .leveled
        .added
        .insert(CELL, vec![(2, ACTOR, 1)]);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("leveled-b");
    b.scripts
        .globals
        .leveled
        .added
        .insert(CELL, vec![(5, PLAYER_REF, 1)]);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(
        b.scripts.globals.leveled.added,
        [(CELL, vec![(2, ACTOR, 1)])].into_iter().collect()
    );
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"LVAD");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.scripts.globals.leveled.added.is_empty());
}

#[test]
fn ref_scales_and_favor_payloads() {
    let snapshot = |scale: f32, extra: Option<FormId>| {
        let mut s = Snapshot::default();
        s.continuity
            .ref_scales
            .extend([(CELL, 0.5), (ACTOR, scale)]);
        s.continuity.no_favor.extend([CELL, ACTOR]);
        s.continuity
            .ignore_friendly_hits
            .extend([PLAYER_REF, ACTOR]);
        s.continuity.no_favor.extend(extra);
        s
    };
    let s = snapshot(2.0, None);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.ref_scales, s.continuity.ref_scales);
    assert_eq!(restored.continuity.no_favor, s.continuity.no_favor);
    assert_eq!(
        restored.continuity.ignore_friendly_hits,
        s.continuity.ignore_friendly_hits
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"RSCL");
    check_continuity_payload(&file, b"NOFV");
    check_continuity_payload(&file, b"IFHT");
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(Snapshot::read(&snapshot(bad, None).write()).is_err());
    }
    assert!(Snapshot::read(&snapshot(2.0, Some(FormId::NULL)).write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn ref_scales_and_favor_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("refstate-a");
    a.ref_state.scales.insert(CELL, 2.0);
    a.ref_state.no_favor.insert(ACTOR);
    a.ref_state.ignore_friendly_hits.insert(CELL);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("refstate-b");
    b.ref_state.scales.insert(PLAYER_REF, 3.0);
    b.ref_state.no_favor.insert(PLAYER_REF);
    b.ref_state.ignore_friendly_hits.insert(PLAYER_REF);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.ref_state.scales, [(CELL, 2.0)].into_iter().collect());
    assert_eq!(b.ref_state.no_favor, [ACTOR].into_iter().collect());
    assert_eq!(
        b.ref_state.ignore_friendly_hits,
        [CELL].into_iter().collect()
    );
    let mut legacy = file;
    legacy
        .sections
        .retain(|(tag, _)| tag != b"RSCL" && tag != b"NOFV" && tag != b"IFHT");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.ref_state.scales.is_empty() && b.ref_state.no_favor.is_empty());
    assert!(b.ref_state.ignore_friendly_hits.is_empty());
}

#[test]
fn actor_flags_payload() {
    let snapshot = |stage: i32, secs: f32, extra: Option<FormId>| {
        let mut s = Snapshot::default();
        s.continuity
            .critical_stages
            .extend([(CELL, 2), (ACTOR, stage)]);
        s.continuity.not_on_meter.extend([ACTOR, PLAYER_REF]);
        s.continuity.not_on_meter.extend(extra);
        s.continuity
            .actor_voice_recovery
            .extend([(CELL, 4.5), (ACTOR, secs)]);
        s
    };
    let s = snapshot(4, 1.0, None);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(
        restored.continuity.critical_stages,
        s.continuity.critical_stages
    );
    assert_eq!(restored.continuity.not_on_meter, s.continuity.not_on_meter);
    assert_eq!(
        restored.continuity.actor_voice_recovery,
        s.continuity.actor_voice_recovery
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"ACST");
    for bad in [
        snapshot(0, 1.0, None),
        snapshot(5, 1.0, None),
        snapshot(1, 0.0, None),
        snapshot(1, f32::NAN, None),
        snapshot(1, -1.0, None),
        snapshot(1, 1.0, Some(FormId::NULL)),
    ] {
        assert!(Snapshot::read(&bad.write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn actor_flags_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("actorflags-a");
    a.actor_flags.critical.insert(CELL, 3);
    a.actor_flags.not_on_meter.insert(ACTOR);
    a.set_voice_recovery_time(ACTOR, 30.0);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("actorflags-b");
    b.actor_flags.critical.insert(PLAYER_REF, 1);
    b.actor_flags.not_on_meter.insert(PLAYER_REF);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.actor_flags.critical, [(CELL, 3)].into_iter().collect());
    assert_eq!(b.actor_flags.not_on_meter, [ACTOR].into_iter().collect());
    assert!((b.voice_recovery_time(ACTOR) - 30.0).abs() < 0.01);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"ACST");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.actor_flags.critical.is_empty() && b.actor_flags.not_on_meter.is_empty());
    assert_eq!(b.voice_recovery_time(ACTOR), 0.0);
}

#[test]
fn doing_favor_and_base_outfits_payloads() {
    let snapshot = |extra: Option<FormId>, outfit: FormId| {
        let mut s = Snapshot::default();
        s.continuity.doing_favor.extend([CELL, ACTOR]);
        s.continuity.doing_favor.extend(extra);
        s.continuity
            .base_outfits
            .extend([(CELL, ACTOR), (ACTOR, outfit)]);
        s
    };
    let s = snapshot(None, CELL);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.doing_favor, s.continuity.doing_favor);
    assert_eq!(restored.continuity.base_outfits, s.continuity.base_outfits);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"FAVR");
    check_continuity_payload(&file, b"BOFT");
    assert!(Snapshot::read(&snapshot(Some(FormId::NULL), CELL).write()).is_err());
    assert!(Snapshot::read(&snapshot(None, FormId::NULL).write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn doing_favor_and_base_outfits_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("favor-a");
    a.actor_flags.doing_favor.insert(ACTOR);
    a.scripts.globals.base_outfits.insert(CELL, ACTOR);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("favor-b");
    b.actor_flags.doing_favor.insert(PLAYER_REF);
    b.scripts.globals.base_outfits.insert(ACTOR, CELL);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.actor_flags.doing_favor, [ACTOR].into_iter().collect());
    assert_eq!(
        b.scripts.globals.base_outfits,
        [(CELL, ACTOR)].into_iter().collect()
    );
    let mut legacy = file;
    legacy
        .sections
        .retain(|(tag, _)| tag != b"FAVR" && tag != b"BOFT");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.actor_flags.doing_favor.is_empty() && b.scripts.globals.base_outfits.is_empty());
}

#[test]
fn resets_payloads() {
    let snapshot = |cell: FormId, value: f32, repeat: bool| {
        let mut s = Snapshot::default();
        s.continuity.reset_cells.extend([CELL, ACTOR, cell]);
        s.continuity
            .quest_globals
            .extend([((CELL, ACTOR), 1.5), ((CELL, PLAYER_REF), value)]);
        if repeat {
            s.continuity.quest_globals.insert((ACTOR, CELL), 2.0);
        }
        s
    };
    let s = snapshot(PLAYER_REF, 4.0, true);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.reset_cells, s.continuity.reset_cells);
    assert_eq!(
        restored.continuity.quest_globals,
        s.continuity.quest_globals
    );
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"RSTC");
    check_continuity_payload(&file, b"QGLB");
    assert!(Snapshot::read(&snapshot(FormId::NULL, 4.0, false).write()).is_err());
    assert!(Snapshot::read(&snapshot(PLAYER_REF, f32::NAN, false).write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn resets_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("resets-a");
    a.resets.cells.insert(CELL);
    a.resets.quest_globals.insert((ACTOR, CELL), 7.0);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("resets-b");
    b.resets.cells.insert(PLAYER_REF);
    b.resets.quest_globals.insert((CELL, ACTOR), 1.0);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.resets.cells, [CELL].into_iter().collect());
    assert_eq!(
        b.resets.quest_globals,
        [((ACTOR, CELL), 7.0)].into_iter().collect()
    );
    let mut legacy = file;
    legacy
        .sections
        .retain(|(tag, _)| tag != b"RSTC" && tag != b"QGLB");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.resets.cells.is_empty() && b.resets.quest_globals.is_empty());
}

#[test]
fn game_flags_payload() {
    let snapshot = |unreported: bool, chargen: Option<(bool, bool, bool)>| {
        let mut s = Snapshot::default();
        s.continuity.crime_unreported = unreported;
        s.continuity.chargen = chargen;
        s
    };
    for (unreported, chargen) in [
        (false, None),
        (true, None),
        (false, Some((true, false, true))),
        (true, Some((false, true, false))),
    ] {
        let s = snapshot(unreported, chargen);
        let file = s.write();
        let restored = Snapshot::read(&file).unwrap();
        assert_eq!(restored.continuity.crime_unreported, unreported);
        assert_eq!(restored.continuity.chargen, chargen);
        assert_eq!(restored.write().to_bytes(), file.to_bytes());
        check_continuity_payload(&file, b"GAMF");
    }
    // Chargen "set" with nothing turned off can't be written by a game.
    assert!(Snapshot::read(&snapshot(false, Some((false, false, false))).write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn game_flags_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("gameflags-a");
    a.set_player_report_crime(false);
    a.set_in_chargen(true, false, true);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("gameflags-b");
    b.set_in_chargen(false, true, false);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert!(b.game_flags.crime_unreported);
    assert!(b.saving_disabled() && !b.waiting_disabled());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"GAMF");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(!b.game_flags.crime_unreported && b.game_flags.chargen.is_none());
}

#[test]
fn outfits_payload() {
    let mut s = Snapshot::default();
    s.continuity
        .outfits
        .extend([(ACTOR, CELL), (SCENE_ACTOR, ACTOR)]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.outfits, s.continuity.outfits);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"OUTF");
    s.continuity.outfits.insert(PLAYER_REF, FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn outfits_replace_the_sessions_after_loading() {
    let (_a, mut a) = engine("outfits-a");
    a.outfits.set.insert(ACTOR, CELL);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("outfits-b");
    b.outfits.set.insert(SCENE_ACTOR, CELL);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.outfits.set, [(ACTOR, CELL)].into_iter().collect());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"OUTF");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.outfits.set.is_empty());
}

#[test]
fn music_queue_payload() {
    let mut s = Snapshot::default();
    s.continuity.music_queue = vec![CELL, ACTOR];
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.music_queue, s.continuity.music_queue);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"MUSQ");
    for bad in [vec![FormId::NULL], vec![ACTOR, ACTOR]] {
        s.continuity.music_queue = bad;
        assert!(Snapshot::read(&s.write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn music_queue_replaces_the_sessions_after_loading() {
    let (_a, mut a) = engine("music-a");
    a.music.queue.add(ACTOR, true);
    a.music.queue.add(CELL, false);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("music-b");
    b.music.queue.add(SCENE_ACTOR, true);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.music.queue.script_entries(), [ACTOR]);
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"MUSQ");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.music.queue.script_entries().is_empty());
}

#[test]
fn ai_off_payload() {
    let mut s = Snapshot::default();
    s.continuity.ai_off.extend([ACTOR, SCENE_ACTOR]);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.ai_off, s.continuity.ai_off);
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"AIOF");
    s.continuity.ai_off.insert(FormId::NULL);
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn ai_off_replaces_the_sessions_after_loading() {
    let (_a, mut a) = engine("ai-off-a");
    a.ai_off.insert(ACTOR);
    let file = Snapshot::capture(&a).write();
    let (_b, mut b) = engine("ai-off-b");
    b.ai_off.insert(SCENE_ACTOR);
    Snapshot::read(&file).unwrap().apply(&mut b).unwrap();
    assert_eq!(b.ai_off, [ACTOR].into_iter().collect());
    let mut legacy = file;
    legacy.sections.retain(|(tag, _)| tag != b"AIOF");
    Snapshot::read(&legacy).unwrap().apply(&mut b).unwrap();
    assert!(b.ai_off.is_empty());
}

fn full_execution() -> crate::ai::exec::SavedExec {
    use crate::ai::exec::{Outcome, PackageRun, SavedExec, Stage};
    let run = |id, outcome| PackageRun {
        id,
        started_at: 30.5,
        outcome,
        scene: true,
        activations: 2,
    };
    SavedExec {
        npc: Some(SCENE_ACTOR),
        place: Some(Place::Exterior(FormId(0x3C), (-2, 7))),
        pos: Vec3::new(1.0, 2.0, 3.0),
        heading: 1.5,
        stage: Some(Stage::Seated {
            furniture: FormId(0x9B0),
            marker: 3,
        }),
        settled: true,
        prompted: true,
        ends_at: Some(12.5),
        jump_at: Some(40.0),
        on_arrival: Some(crate::ai::schedule::PlacementHeading::Turn(0.5)),
        stepped_at: 10.0,
        stepped_hours: 31.0,
        eval_slot: Some(62),
        next_eval: 2.0,
        current: Some(1),
        stop: (2, Some(30.75)),
        patrol: Some((FormId(0x9B1), 4)),
        run: Some(run(SCENE_PACKAGE, Outcome::Completed)),
        suspended: Some(run(PACKAGE_SCENE, Outcome::Running)),
        done_once: Some(SCENE_PACKAGE),
        health: 20.0,
        stamina: 40.0,
        bleeding: Some(5.0),
    }
}

#[test]
fn execution_payload() {
    let mut s = Snapshot::default();
    s.continuity.executions.insert(ACTOR, full_execution());
    let bare = crate::ai::exec::SavedExec {
        npc: None,
        place: None,
        pos: Vec3::NAN,
        stage: Some(crate::ai::exec::Stage::Walking),
        on_arrival: Some(crate::ai::schedule::PlacementHeading::Set(-1.0)),
        ..crate::ai::exec::ActorExec::default().saved()
    };
    s.continuity.executions.insert(SCENE_ACTOR, bare);
    let file = s.write();
    let restored = Snapshot::read(&file).unwrap();
    assert_eq!(restored.continuity.executions[&ACTOR], full_execution());
    let back = &restored.continuity.executions[&SCENE_ACTOR];
    assert!(back.pos.is_nan() && back.place.is_none() && back.run.is_none());
    assert_eq!(restored.write().to_bytes(), file.to_bytes());
    check_continuity_payload(&file, b"AEXC");
    // The least an entry takes is what `count` allows for.
    let mut w = Writer::default();
    super::continuity::put_exec(
        &mut w,
        ACTOR,
        &crate::ai::exec::ActorExec::default().saved(),
    );
    assert_eq!(w.0.len(), super::continuity::EXEC_BYTES);

    let refused = |change: fn(&mut crate::ai::exec::SavedExec)| {
        let mut s = Snapshot::default();
        let mut x = full_execution();
        change(&mut x);
        s.continuity.executions.insert(ACTOR, x);
        Snapshot::read(&s.write()).is_err()
    };
    assert!(refused(|x| x.heading = f32::NAN));
    assert!(refused(|x| x.pos.x = f32::INFINITY));
    assert!(refused(|x| x.stepped_at = -1.0));
    assert!(refused(|x| x.ends_at = Some(f64::NAN)));
    assert!(refused(|x| x.bleeding = Some(-1.0)));
    assert!(refused(|x| x.run.as_mut().unwrap().id = FormId::NULL));
    assert!(refused(|x| {
        x.stage = Some(crate::ai::exec::Stage::Seated {
            furniture: FormId::NULL,
            marker: 0,
        })
    }));
    let mut s = Snapshot::default();
    s.continuity
        .executions
        .insert(FormId::NULL, full_execution());
    assert!(Snapshot::read(&s.write()).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn journal_metadata_characters_autosave_preferences_and_thumbnails() {
    let (fixture, mut a) = engine("journal");
    a.settings = crate::settings::Settings::default();
    a.sync_settings();
    for (key, trigger) in [
        ("bSaveOnRest", super::autosave::Trigger::Rest),
        ("bSaveOnWait", super::autosave::Trigger::Wait),
        ("bSaveOnTravel", super::autosave::Trigger::Travel),
    ] {
        a.set_setting(key, 0.).unwrap();
        a.request_autosave(trigger);
        assert!(a.save_requests.is_empty());
        a.set_setting(key, 1.).unwrap();
        a.request_autosave(trigger);
        assert_eq!(a.save_requests.len(), 1);
        a.save_requests.clear();
    }
    a.autosave.pause_minutes = Some(5);
    a.last_save
        .set(std::time::Instant::now() - std::time::Duration::from_secs(301));
    a.menu = Some(crate::items::Menu::Journal);
    a.check_pause_autosave();
    assert_eq!(a.save_requests.len(), 1);
    a.save_requests.clear();
    a.check_pause_autosave();
    assert!(a.save_requests.is_empty());
    a.menu = None;
    a.capture_save_thumbnail();
    let path = fixture.0.join("journal.vrs");
    a.save_game_to(&path).unwrap();
    let prefix = super::file::SaveFile::read_prefix(std::fs::File::open(&path).unwrap()).unwrap();
    let metadata = super::metadata::Metadata::decode(prefix.section(b"META").unwrap()).unwrap();
    assert_eq!(metadata.character, a.scripts.character_id);
    assert!(metadata.thumbnail.is_none());
    let preview = super::metadata::read_thumbnail(std::fs::File::open(&path).unwrap()).unwrap();
    assert_eq!(preview, a.save_thumbnail);
    assert_eq!(metadata.file_num, 1);
    let old_number = metadata.file_num;
    a.save_game_to(&path).unwrap();
    let listing = super::save_list_in(&fixture.0, &["Save.esp".into()]);
    assert_eq!(listing[0].file_num(), old_number);
    let (_other, mut b) = engine("journal-restored");
    assert_ne!(a.scripts.character_id, b.scripts.character_id);
    b.load_game_from(&path).unwrap();
    assert_eq!(a.scripts.character_id, b.scripts.character_id);
    b.change_option(
        crate::settings::SPECS
            .iter()
            .position(|s| s.key == "iDifficulty")
            .unwrap(),
        5.,
    )
    .unwrap();
    assert_eq!(b.difficulty_multiplier(PLAYER_REF, false), 3.);
    assert_eq!(b.difficulty_multiplier(ACTOR, false), 0.25);
    assert_eq!(b.difficulty_multiplier(PLAYER_REF, true), 1.);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic manual records only"]
fn help_manual_uses_default_form_list_titles_and_keeps_button_tags() {
    let (fixture, mut e) = engine("help");
    let mut default = Vec::from(*b"HMPC");
    default.extend_from_slice(&0x900u32.to_le_bytes());
    let records = vec![
        Node::top(
            b"DOBJ",
            vec![RecordBuilder::new(b"DOBJ", FormId(0x31)).field(b"DNAM", &default)],
        ),
        Node::top(
            b"FLST",
            vec![
                RecordBuilder::new(b"FLST", FormId(0x900))
                    .form(b"LNAM", FormId(0x901))
                    .form(b"LNAM", FormId(0x902))
                    .form(b"LNAM", FormId(0x901)),
            ],
        ),
        Node::top(
            b"MESG",
            vec![
                RecordBuilder::new(b"MESG", FormId(0x901))
                    .zstring(b"FULL", "Jumping")
                    .zstring(b"DESC", "Press [Jump]. Count <Global=HelpCounter>."),
                RecordBuilder::new(b"MESG", FormId(0x902)).zstring(b"DESC", "No title"),
            ],
        ),
        Node::top(
            b"GLOB",
            vec![
                RecordBuilder::new(b"GLOB", FormId(0x903))
                    .zstring(b"EDID", "HelpCounter")
                    .field(b"FNAM", b"f")
                    .field(b"FLTV", &42f32.to_le_bytes()),
            ],
        ),
    ];
    std::fs::write(
        fixture.0.join("Manual.esp"),
        write_plugin(0, &[], "test", "manual", 0x904, &records),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&fixture.0, &["Manual.esp".into()]).unwrap();
    let topics = e.help_topics();
    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].title, "Jumping");
    assert_eq!(topics[0].body, "Press [Jump]. Count 42.");
}

#[test]
fn crafting_copy_metadata_round_trip_legacy_and_corruption() {
    use crate::item_copies::{CopyState, ItemCopy};
    let state = CopyState {
        name: Some("Spark".into()),
        enchantment: Some(FormId(0xFF000800)),
        capacity: 250.,
        charge: 100.,
        ..CopyState::at_level(3)
    };
    let mut inv = Inventory::default();
    inv.add(SWORD, 2);
    inv.wear(SWORD);
    inv.add_copy_state(ItemCopy {
        item: SWORD,
        owner: None,
        state: state.clone(),
        count: 1,
    });
    inv.equipped_copies.insert(SWORD, state.clone());
    let mut s = Snapshot::default();
    s.inventories.insert(PLAYER_REF, inv);
    s.item_copies.insert(
        FormId(0xFF000801),
        vec![ItemCopy {
            item: SWORD,
            owner: None,
            state: state.clone(),
            count: 1,
        }],
    );
    let file = s.write();
    let loaded = Snapshot::read(&file).unwrap();
    assert_eq!(loaded.inventories[&PLAYER_REF].copies[0].state, state);
    assert_eq!(loaded.inventories[&PLAYER_REF].copy_state(SWORD), state);
    assert_eq!(loaded.item_copies, s.item_copies);
    let mut legacy = file.clone();
    legacy.sections.retain(|(t, _)| t != b"COPY");
    assert!(
        Snapshot::read(&legacy).unwrap().inventories[&PLAYER_REF]
            .copies
            .is_empty()
    );
    let mut duplicate = file.clone();
    duplicate.add(b"COPY", vec![]);
    assert!(Snapshot::read(&duplicate).is_err());
    for quality in [f32::NAN, 0., f32::INFINITY] {
        s.inventories.get_mut(&PLAYER_REF).unwrap().copies[0]
            .state
            .quality = quality;
        assert!(Snapshot::read(&s.write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_created_forms_references_and_copy_state_share_ids_across_load() {
    use crate::item_copies::{CopyState, ItemCopy};
    let (_dir, mut e) = engine("crafting-runtime-a");
    let potion = e
        .intern_base_form(RecordBuilder::new(b"ALCH", FormId::NULL).zstring(b"FULL", "Test Potion"))
        .unwrap();
    let again = e
        .intern_base_form(RecordBuilder::new(b"ALCH", FormId::NULL).zstring(b"FULL", "Other name"))
        .unwrap();
    assert_eq!(potion, again);
    let reference = e.create_ref(potion, PLAYER_REF, true).unwrap();
    let enchantment = e
        .create_base_form(
            RecordBuilder::new(b"ENCH", FormId::NULL).zstring(b"FULL", "Test Enchantment"),
        )
        .unwrap();
    assert!(potion < reference && reference < enchantment);
    e.inventory_mut(PLAYER_REF).add(SWORD, 1);
    let state = CopyState {
        enchantment: Some(enchantment),
        name: Some("Test sword".into()),
        ..CopyState::at_level(2)
    };
    e.inventory_mut(PLAYER_REF).add_copy_state(ItemCopy {
        item: SWORD,
        owner: None,
        state: state.clone(),
        count: 1,
    });
    let snapshot = Snapshot::read(&Snapshot::capture(&e).write()).unwrap();
    let (_other, mut loaded) = engine("crafting-runtime-b");
    snapshot.apply(&mut loaded).unwrap();
    assert_eq!(loaded.lo.tag_of(potion), Some(esp::Tag(*b"ALCH")));
    assert_eq!(loaded.inventories[&PLAYER_REF].copies[0].state, state);
    let next = loaded
        .create_base_form(RecordBuilder::new(b"ALCH", FormId::NULL).zstring(b"FULL", "New Potion"))
        .unwrap();
    assert!(next > enchantment);
}

#[test]
fn crafting_first_use_help_round_trip_and_legacy() {
    let mut s = Snapshot::default();
    s.crafting_help.insert(*b"HBFG");
    let file = s.write();
    assert_eq!(
        Snapshot::read(&file).unwrap().crafting_help,
        s.crafting_help
    );
    let mut legacy = file.clone();
    legacy.sections.retain(|(t, _)| t != b"CHLP");
    assert!(Snapshot::read(&legacy).unwrap().crafting_help.is_empty());
    s.crafting_help.insert(*b"XXXX");
    assert!(Snapshot::read(&s.write()).is_err());
}
