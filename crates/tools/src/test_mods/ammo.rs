//! `test-mod ammo`: which ammunition Skyrim SE equips with a bow or
//! crossbow (`plans/bow-ammunition.md`, step 0). UESP's Archery notes say
//! the weakest arrows equip themselves with a bow and stay equipped when it
//! is taken off; what happens when ammunition is already worn, when the worn
//! stack is used up, with only bolts, and how ties are broken isn't
//! published.
//!
//! A start-game-enabled quest's script (`ammo/ammo.psc`) stores the player's
//! items in a barrel placed beside them (`BarrelEmpty01`), gives and removes
//! the test items (`AddItem` / `RemoveItem`, silent) and equips them with
//! `EquipItem`, reading each test item's count and `IsEquipped` a second
//! after each step. Items: iron (damage 8), steel (10) and elven (16)
//! arrows, the Long Bow and Hunting Bow, three arrows of ours copied from
//! iron's record (tieA, damage 8, named "Vrm Tie Zulu"; tieB, damage 8,
//! "Vrm Tie Alpha"; pricey, damage 4, value 100), and Dawnguard's crossbow,
//! steel and dwarven bolts by `Game.GetFormFromFile` (none without
//! Dawnguard).
//!
//! | report | question | the plan's design predicts |
//! | --- | --- | --- |
//! | `Default` | weakest auto-equips with a scripted `EquipItem` | iron E |
//! | `Chosen` | a chosen stack stays; unequipping the bow keeps arrows | steel E throughout; iron E after the last equip |
//! | `Switch` | bow to bow keeps the worn arrows | steel E |
//! | `RemoveHeld` | worn stack removed with the bow held | none, or iron E? |
//! | `RemoveAway` | worn stack removed, then the bow equipped | iron E after the bow |
//! | `OnlyBolts` | a bow with only bolts | nothing E |
//! | `Pickup` | arrows picked up while the bow is held | ? |
//! | `Incompatible` | bolts worn, then a bow | iron E |
//! | `Tie` | equal damage: add order, form id or name | ? |
//! | `Pricey` | weakest by damage or by value | pricey (damage) |
//! | `Crossbow` | crossbow takes the weakest bolt; back to the bow | steel bolt E, iron E for the bow |
//! | `Menu` | the bow equipped in the inventory menu (manual) | iron E |
//! | `RunOut` | three shots from a worn stack of two (manual) | ? after shot 2 and 3 |
//!
//! Run: load a throwaway save somewhere quiet (no bound bow, nothing in
//! hand matters: the items are stored). Wait for the first box, close it;
//! the automatic tests run for about a minute and save. Then two boxes ask
//! for the manual steps: equip the Long Bow from the inventory menu, then
//! fire three arrows. The items come back and the game saves before the
//! "done" box. Read with `vrm-tool test-results`; also say which arrows
//! the inventory showed equipped and any message on screen after the shots.
//!
//! **Run 2026-11-03** (a fresh level 1 character, Dawnguard loaded): the
//! design's predictions held, and the open ones read:
//!
//! - `Default`, `Menu`: iron (the user saw iron marked in the inventory);
//!   `Pricey`: pricey, so the weakest is by damage, not value.
//! - `Chosen`, `Switch`: a chosen stack stays through the bow coming off and
//!   on and a bow swapped for another; with none worn, iron goes on.
//! - `RemoveHeld`, `RemoveAway`, `Pickup`: removing the worn stack or adding
//!   arrows with the bow held puts nothing on; equipping the bow then does.
//! - `OnlyBolts`: nothing worn. `Incompatible`: worn bolts give way to iron.
//! - `Tie`: the stack added first wins, whatever its form id or name.
//! - `Crossbow`: steel bolts (the weaker); the bow then iron arrows, the
//!   crossbow steel bolts again; dwarven bolts worn give way to iron.
//! - `RunOut`: after the second shot used up the worn iron, steel (the
//!   weakest left, not elven) was worn at once and the third shot took it;
//!   the user saw no message.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};

use super::autotest2::Vanilla;
use super::records;
use super::{TestMod, own, script};

const QUEST: u32 = 0x800;
const TIE_A: u32 = 0x810;
const TIE_B: u32 = 0x811;
const PRICEY: u32 = 0x812;
const NEXT_OBJECT_ID: u32 = 0x820;

const SCRIPT: &str = "VrmAmmoTestScript";
const SOURCE: &str = include_str!("ammo/ammo.psc");
const PLAYER_REF: FormId = FormId(0x14);

/// Script properties: name, class, Skyrim.esm editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("Chest", "Container", "BarrelEmpty01"),
    ("LongBow", "Weapon", "LongBow"),
    ("HuntingBow", "Weapon", "HuntingBow"),
    ("Iron", "Ammo", "IronArrow"),
    ("Steel", "Ammo", "SteelArrow"),
    ("Elven", "Ammo", "ElvenArrow"),
];

/// Our copies of iron arrows: property, object id, editor id, name, damage,
/// value.
const COPIES: &[(&str, u32, &str, &str, f32, u32)] = &[
    ("TieA", TIE_A, "VrmAmmoTieA", "Vrm Tie Zulu", 8.0, 1),
    ("TieB", TIE_B, "VrmAmmoTieB", "Vrm Tie Alpha", 8.0, 1),
    (
        "Pricey",
        PRICEY,
        "VrmAmmoPricey",
        "Vrm Pricey Arrow",
        4.0,
        100,
    ),
];

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    script::compile(&[SOURCE])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let mut properties = vec![("Player".to_owned(), PropertyValue::Object(PLAYER_REF))];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), PropertyValue::Object(v.id(edid)?)));
    }
    let mut ammo = Vec::new();
    for &(name, id, edid, full, damage, value) in COPIES {
        properties.push((name.to_owned(), PropertyValue::Object(own(id))));
        let rec = records::copy_inserting(
            lo,
            "IronArrow",
            id,
            edid,
            None,
            |tag, data| {
                // DATA: projectile, flags, damage, value (and weight in SE).
                if tag == b"DATA" && data.len() >= 16 {
                    data[8..12].copy_from_slice(&damage.to_le_bytes());
                    data[12..16].copy_from_slice(&value.to_le_bytes());
                }
            },
            |tag| {
                if tag == b"OBND" {
                    let mut s = full.as_bytes().to_vec();
                    s.push(0);
                    vec![(*b"FULL", s)]
                } else {
                    Vec::new()
                }
            },
        )
        .with_context(|| format!("copying IronArrow as {edid}"))?;
        ammo.push(rec);
    }
    let quest = records::Quest {
        object_id: QUEST,
        edid: "VrmAmmoTestQuest",
        flags: 0x11,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: SCRIPT.into(),
            properties,
        }),
        stages: &[],
        aliases: Vec::new(),
    }
    .record();
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "plans/bow-ammunition.md step 0: which ammunition equips with a bow.",
        NEXT_OBJECT_ID,
        &[Node::top(b"AMMO", ammo), Node::top(b"QUST", vec![quest])],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        for (name, id, ..) in COPIES {
            properties.push((name.to_string(), form(own(*id).0, "Ammo")));
        }
        // The opening box, the two manual steps, the closing box.
        let shown = run(scripts().unwrap(), &[(own(QUEST).0, SCRIPT, properties)], 4);
        assert_eq!(shown.boxes.len(), 4);
        assert_eq!(shown.saved_at_box, Some(3), "saved before the closing box");
    }
}
