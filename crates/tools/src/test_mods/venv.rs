//! `test-mod venv`: a test plugin for the vendor factions' third `VENV` flag
//! byte. One interior cell, `VenvTestCell`,
//! with two fences that differ only in that byte, iron daggers to steal and
//! daggers free to take.
//!
//! Both factions buy stolen goods (byte 8) and use an inverted list (byte 9,
//! `VendorItemsMisc`, as the fence Tonilia), so daggers are bought either
//! way. What each fence buys tells the readings apart:
//!
//! | reading of byte 10    | flag off (0) buys | flag on (1) buys |
//! | --------------------- | ----------------- | ---------------- |
//! | only buys stolen      | both              | stolen only      |
//! | buys non-stolen       | stolen only       | both             |
//! | unused                | both              | both             |
//!
//! Result in Skyrim SE (2026-10-12): both fences bought every dagger, stolen
//! or not, so the byte is unused.

use anyhow::{Context, Result, ensure};
use esp::writer::{Node, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, group_type, record_flags};

use super::own;

const CELL: u32 = 0x800;
const FACTION_OFF: u32 = 0x801;
const FACTION_ON: u32 = 0x802;
const OWNER_FACTION: u32 = 0x803;
const CHEST: u32 = 0x804;
const VENDOR_OFF: u32 = 0x805;
const VENDOR_ON: u32 = 0x806;
const ACHR_OFF: u32 = 0x807;
const ACHR_ON: u32 = 0x808;
const CHEST_REF_OFF: u32 = 0x809;
const CHEST_REF_ON: u32 = 0x80A;
const COC_MARKER: u32 = 0x80B;
const FLOOR: u32 = 0x80C;
/// Iron daggers, unowned then owned; the next free object id after them.
const DAGGERS: u32 = 0x80D;
const DAGGER_COUNT: u32 = 4;
const NEXT_OBJECT_ID: u32 = DAGGERS + 2 * DAGGER_COUNT;

/// The vanilla forms used, by editor id (all from Skyrim.esm).
struct Vanilla {
    job_merchant: FormId,
    job_fence: FormId,
    vendor_items_misc: FormId,
    male_template: FormId,
    female_template: FormId,
    race: FormId,
    class: FormId,
    combat_style: FormId,
    outfit: FormId,
    gold: FormId,
    iron_dagger: FormId,
    coc_marker: FormId,
    floor: FormId,
}

impl Vanilla {
    fn resolve(lo: &LoadOrder) -> Result<Self> {
        let id = |edid: &str| -> Result<FormId> {
            let id = lo
                .find_editor_id(edid)
                .with_context(|| format!("{edid} not found"))?;
            ensure!(id.0 >> 24 == 0, "{edid} ({id}) isn't from Skyrim.esm");
            Ok(id)
        };
        Ok(Vanilla {
            job_merchant: id("JobMerchantFaction")?,
            job_fence: id("JobFenceFaction")?,
            vendor_items_misc: id("VendorItemsMisc")?,
            // World encounter merchants: their FaceGen ships with the game,
            // and a "Use Traits" template takes the face from them.
            male_template: id("WEMerchantMiscMaleCommoner")?,
            female_template: id("WEMerchantMiscFemaleCommoner")?,
            race: id("ImperialRace")?,
            class: id("Citizen")?,
            combat_style: id("csHumanMeleeLvl1")?,
            outfit: id("MerchantClothesOutfit01")?,
            gold: id("Gold001")?,
            iron_dagger: id("IronDagger")?,
            coc_marker: id("COCMarkerHeading")?,
            floor: id("WRIntFloorSTMid01Large")?,
        })
    }
}

fn bytes<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let v: Vec<u8> = parts.concat();
    v.try_into().expect("field size")
}

fn bounds(b: [i16; 6]) -> Vec<u8> {
    b.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A placed reference's DATA: position, then rotation in radians.
pub(super) fn placement(pos: [f32; 3], rot_z: f32) -> Vec<u8> {
    [pos[0], pos[1], pos[2], 0.0, 0.0, rot_z]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

fn vendor_faction(object_id: u32, edid: &str, name: &str, third: u8, v: &Vanilla) -> RecordBuilder {
    let chest = if object_id == FACTION_OFF {
        CHEST_REF_OFF
    } else {
        CHEST_REF_ON
    };
    // VENV: hours 0 to 24, radius 0, buys stolen, inverted list, the byte
    // under test, unused.
    let venv: [u8; 12] = bytes(&[
        &0u16.to_le_bytes(),
        &24u16.to_le_bytes(),
        &0u32.to_le_bytes(),
        &[1, 1, third, 0],
    ]);
    // PLVD: in cell (type 1), the test cell.
    let plvd: [u8; 12] = bytes(&[
        &1u32.to_le_bytes(),
        &own(CELL).0.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]);
    RecordBuilder::new(b"FACT", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", name)
        .u32(b"DATA", 0x4000) // vendor
        // CRVA: arrest, attack on sight, then no crime gold values.
        .field(
            b"CRVA",
            &[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        )
        .form(b"VEND", v.vendor_items_misc)
        .form(b"VENC", own(chest))
        .field(b"VENV", &venv)
        .field(b"PLVD", &plvd)
}

fn vendor_npc(
    object_id: u32,
    edid: &str,
    name: &str,
    faction: u32,
    female: bool,
    v: &Vanilla,
) -> RecordBuilder {
    // ACBS: flags (female), magicka and stamina offsets, level 1, calc
    // min / max, speed 100, disposition 35, template flags (Use Traits, Use
    // Stats), health offset, bleedout override.
    let acbs: [u8; 24] = bytes(&[
        &u32::from(female).to_le_bytes(),
        &0i16.to_le_bytes(),
        &0i16.to_le_bytes(),
        &1u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &100u16.to_le_bytes(),
        &35i16.to_le_bytes(),
        &0x0003u16.to_le_bytes(),
        &0i16.to_le_bytes(),
        &0u16.to_le_bytes(),
    ]);
    let rank = |f: FormId| -> [u8; 8] { bytes(&[&f.0.to_le_bytes(), &[0, 0, 0, 0]]) };
    // AIDT: unaggressive, average confidence, energy 50, no crime, neutral,
    // helps nobody; no aggro radius.
    let aidt = [
        0u8, 2, 50, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    // DNAM: 18 skills at 5 and no offsets, health 30, magicka 25, stamina 70,
    // far away model distance 0, geared up weapons 0.
    let dnam: [u8; 52] = bytes(&[
        &[5; 18],
        &[0; 18],
        &30u16.to_le_bytes(),
        &25u16.to_le_bytes(),
        &70u16.to_le_bytes(),
        &[0; 10],
    ]);
    RecordBuilder::new(b"NPC_", own(object_id))
        .zstring(b"EDID", edid)
        .field(b"OBND", &bounds([-41, -25, 0, 41, 72, 86]))
        .field(b"ACBS", &acbs)
        .field(b"SNAM", &rank(v.job_merchant))
        .field(b"SNAM", &rank(v.job_fence))
        .field(b"SNAM", &rank(own(faction)))
        .form(
            b"TPLT",
            if female {
                v.female_template
            } else {
                v.male_template
            },
        )
        .form(b"RNAM", v.race)
        .field(b"AIDT", &aidt)
        .form(b"CNAM", v.class)
        .zstring(b"FULL", name)
        .field(b"DATA", &[])
        .field(b"DNAM", &dnam)
        .form(b"ZNAM", v.combat_style)
        .field(b"NAM5", &[0xFF, 0])
        .f32(b"NAM6", 1.0)
        .f32(b"NAM7", 50.0)
        .u32(b"NAM8", 1) // sound level: normal
        .form(b"DOFT", v.outfit)
}

/// Interior lighting: bright and even, no fog, nothing inherited from a
/// lighting template.
pub(super) fn xcll() -> Vec<u8> {
    let rgb = |c: u8| [c, c, c, 0];
    let mut b = Vec::new();
    b.extend_from_slice(&rgb(110)); // ambient
    b.extend_from_slice(&rgb(200)); // directional
    b.extend_from_slice(&rgb(0)); // fog near colour
    b.extend_from_slice(&0f32.to_le_bytes()); // fog near
    b.extend_from_slice(&8000f32.to_le_bytes()); // fog far
    b.extend_from_slice(&0i32.to_le_bytes()); // directional rotation xy
    b.extend_from_slice(&45i32.to_le_bytes()); // directional rotation z
    b.extend_from_slice(&1f32.to_le_bytes()); // directional fade
    b.extend_from_slice(&0f32.to_le_bytes()); // fog clip distance
    b.extend_from_slice(&1f32.to_le_bytes()); // fog power
    for _ in 0..6 {
        b.extend_from_slice(&rgb(90)); // directional ambient, +-x, +-y, +-z
    }
    b.extend_from_slice(&rgb(0)); // specular
    b.extend_from_slice(&1f32.to_le_bytes()); // fresnel power
    b.extend_from_slice(&rgb(0)); // fog far colour
    b.extend_from_slice(&1f32.to_le_bytes()); // fog max
    b.extend_from_slice(&0f32.to_le_bytes()); // light fade begin
    b.extend_from_slice(&0f32.to_le_bytes()); // light fade end
    b.extend_from_slice(&0u32.to_le_bytes()); // inherits nothing
    assert_eq!(b.len(), 92);
    b
}

pub fn build(lo: &LoadOrder) -> Result<Vec<u8>> {
    let v = Vanilla::resolve(lo)?;
    let persistent = record_flags::PERSISTENT;
    let pi = std::f32::consts::PI;

    let factions = vec![
        vendor_faction(
            FACTION_OFF,
            "VenvTestVendorFactionFlagOff",
            "VENV Test Fence (flag off)",
            0,
            &v,
        ),
        vendor_faction(
            FACTION_ON,
            "VenvTestVendorFactionFlagOn",
            "VENV Test Fence (flag on)",
            1,
            &v,
        ),
        // Owns the daggers to steal. No crime faction, so a theft is
        // nobody's crime: the daggers are stolen without a bounty.
        RecordBuilder::new(b"FACT", own(OWNER_FACTION))
            .zstring(b"EDID", "VenvTestOwnerFaction")
            .zstring(b"FULL", "VENV Test Owner")
            .u32(b"DATA", 0x8000) // can be owner
            .field(b"CRVA", &[0; 20]),
    ];
    let chests = vec![
        RecordBuilder::new(b"CONT", own(CHEST))
            .zstring(b"EDID", "VenvTestMerchantChest")
            .field(b"OBND", &bounds([-48, -23, 0, 48, 26, 49]))
            .zstring(b"FULL", "Merchant Chest")
            .zstring(b"MODL", r"Clutter\Upperclass\UpperChest01.nif")
            .u32(b"COCT", 1)
            .field(
                b"CNTO",
                &bytes::<8>(&[&v.gold.0.to_le_bytes(), &2000u32.to_le_bytes()]),
            )
            .field(b"DATA", &[0, 0, 0, 0, 0]), // flags, weight
    ];
    let npcs = vec![
        vendor_npc(
            VENDOR_OFF,
            "VenvTestVendorFlagOff",
            "Fence Flag Off",
            FACTION_OFF,
            false,
            &v,
        ),
        vendor_npc(
            VENDOR_ON,
            "VenvTestVendorFlagOn",
            "Fence Flag On",
            FACTION_ON,
            true,
            &v,
        ),
    ];

    // The cell: one 1024 x 1024 floor piece around the origin. The player
    // arrives at the south edge facing north; the fences stand at the north
    // edge facing them, flag off to the west, flag on to the east; daggers in
    // between, the free ones west, the owned ones east. The merchant chests
    // are under the floor.
    let cell = RecordBuilder::new(b"CELL", own(CELL))
        .zstring(b"EDID", "VenvTestCell")
        .zstring(b"FULL", "VENV Test")
        .field(b"DATA", &1u16.to_le_bytes()) // interior
        .field(b"XCLL", &xcll())
        .u32(b"LTMP", 0);
    let actor = |id: u32, base: u32, x: f32| {
        RecordBuilder::new(b"ACHR", own(id))
            .flags(persistent)
            .form(b"NAME", own(base))
            .field(b"DATA", &placement([x, 250.0, 2.0], pi))
    };
    let chest = |id: u32, faction: u32, x: f32| {
        RecordBuilder::new(b"REFR", own(id))
            .flags(persistent)
            .form(b"NAME", own(CHEST))
            .form(b"XOWN", own(faction))
            .field(b"DATA", &placement([x, 250.0, -400.0], pi))
    };
    let persistent_refs = vec![
        actor(ACHR_OFF, VENDOR_OFF, -150.0),
        actor(ACHR_ON, VENDOR_ON, 150.0),
        chest(CHEST_REF_OFF, FACTION_OFF, -150.0),
        chest(CHEST_REF_ON, FACTION_ON, 150.0),
    ];
    let mut temporary_refs = vec![
        RecordBuilder::new(b"REFR", own(COC_MARKER))
            .form(b"NAME", v.coc_marker)
            .field(b"DATA", &placement([0.0, -350.0, 0.0], 0.0)),
        RecordBuilder::new(b"REFR", own(FLOOR))
            .form(b"NAME", v.floor)
            .field(b"DATA", &placement([0.0, 0.0, 0.0], 0.0)),
    ];
    for i in 0..DAGGER_COUNT {
        let y = -150.0 + 40.0 * i as f32;
        temporary_refs.push(
            RecordBuilder::new(b"REFR", own(DAGGERS + i))
                .form(b"NAME", v.iron_dagger)
                .field(b"DATA", &placement([-120.0, y, 5.0], 0.0)),
        );
        temporary_refs.push(
            RecordBuilder::new(b"REFR", own(DAGGERS + DAGGER_COUNT + i))
                .form(b"NAME", v.iron_dagger)
                .form(b"XOWN", own(OWNER_FACTION))
                .field(b"DATA", &placement([120.0, y, 5.0], 0.0)),
        );
    }

    let cell_label = own(CELL).0.to_le_bytes();
    let children = Node::Group {
        label: cell_label,
        group_type: group_type::CELL_CHILDREN,
        children: vec![
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_PERSISTENT,
                children: persistent_refs.into_iter().map(Node::Record).collect(),
            },
            Node::Group {
                label: cell_label,
                group_type: group_type::CELL_TEMPORARY,
                children: temporary_refs.into_iter().map(Node::Record).collect(),
            },
        ],
    };
    // Interior cells are grouped by the last two decimal digits of their
    // object id: block, then sub-block.
    let block = (CELL % 10) as i32;
    let sub_block = (CELL / 10 % 10) as i32;
    let cells = Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: vec![Node::Group {
            label: block.to_le_bytes(),
            group_type: group_type::INTERIOR_BLOCK,
            children: vec![Node::Group {
                label: sub_block.to_le_bytes(),
                group_type: group_type::INTERIOR_SUB_BLOCK,
                children: vec![Node::Record(cell), children],
            }],
        }],
    };

    // Top groups in the order the Creation Kit writes them.
    Ok(write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Two fences differing only in VENV's third flag byte. coc VenvTestCell.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"FACT", factions),
            Node::top(b"CONT", chests),
            Node::top(b"NPC_", npcs),
            cells,
        ],
    ))
}
