//! `test-mod regen`: how the player's health, magicka and stamina come back.
//!
//! UESP's Skyrim:Health ("Passive Regeneration",
//! <https://en.uesp.net/w/index.php?oldid=3636643>) gives health a second as
//! max x `HealRate`% x `HealRateMult`% x (in combat: `CombatHealthRegenMult`),
//! 0.7% out of combat and 0.49% in it. Skyrim.esm: the races' `HealRate`,
//! `MagickaRate`, `StaminaRate` are 0.7, 3 and 5; the player's
//! `CombatHealthRegenMult` 0.7 comes from the ability `PCHealRateCombat` on
//! the Player record (the value's base is 0). The settings are
//! `fDamagedHealthRegenDelay`, `fDamagedMagickaRegenDelay`,
//! `fDamagedStaminaRegenDelay` (0.5), `fDamagedAVRegenDelay` (1) and
//! `fCombatHealthRegenRateMult` (0), `fCombatMagickaRegenRateMult` (0.33),
//! `fCombatStaminaRegenRateMult` (0.35). Unpublished: what the delays wait
//! for, whether `HealRateMult` still counts in combat (a 2011 Skyrim
//! talk:Health comment says `CombatHealthRegenMult` replaces it), and how the
//! rate multipliers and combat settings apply to magicka and stamina.
//!
//! A start-game-enabled quest's script (`regen/regen.psc`) samples the
//! player's values with `GetActorValue` about every frame (timed by
//! `GetCurrentRealTime`), lowering them with `DamageActorValue` and filling
//! them with `RestoreActorValue`. Rates are percent of the most a second,
//! measured for 3 s (health 5 s) from 1.5 s after a drop. For combat a
//! mudcrab (`EncMudcrabLarge`) is placed 600 units off with its AI off and
//! `StartCombat(player)` (autotest21: such an actor stays in combat state 1
//! and can't move or attack).
//!
//! | report | question | prediction (UESP and the settings' names) |
//! | --- | --- | --- |
//! | `Setup` | the values and settings as the game reads them | `CombatHealthRegenMult` 0.7 (base 0); settings as above |
//! | `Delay` | first rise after one drop to half | about 0.5 s for each |
//! | `Repeat` | hits 0.3 s apart: does each restart the wait? | no rise between hits; 0.5 s after the last |
//! | `Cross` | one value hit: do the others pause? | no |
//! | `Rates` | out of combat; with the rate mult +100; `CombatHealthRegenMult` -0.35 | health 0.7, 1.4, 0.7; magicka 3, 6; stamina 5, 10 |
//! | `CombatRates` | the same in combat | health 0.49, 0.98, 0.245; magicka 0.99, 1.98; stamina 1.75, 3.5 |
//! | `Hit` | a real hit (Firebolt from the mudcrab): health's wait; magicka and stamina go on? | about 0.5 s; yes |
//!
//! Alternatives the rates tell apart: `HealRateMult` ignored in combat
//! (0.49 at +100), `fCombatHealthRegenRateMult` applying to the player
//! (0 in combat), the combat settings not applying to the player's
//! magicka or stamina (3 and 5 in combat).
//!
//! Run: a throwaway save of a character without regeneration gear,
//! potions or blessings acting (a fresh level 1 character is best;
//! Survival Mode off), standing somewhere quiet and open with no one
//! around. Wait for the first box, close it, and touch nothing for about
//! 3 minutes until the done box; the game saves before it. Read with
//! `vrm-tool test-results`.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};

use super::autotest2::Vanilla;
use super::records;
use super::{TestMod, script};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x810;

const SCRIPT: &str = "VrmRegenTestScript";
const SOURCE: &str = include_str!("regen/regen.psc");
const PLAYER_REF: FormId = FormId(0x14);

/// Script properties: name, class, Skyrim.esm editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("EnemyBase", "ActorBase", "EncMudcrabLarge"),
    ("Bolt", "Spell", "Firebolt"),
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
    let quest = records::Quest {
        object_id: QUEST,
        edid: "VrmRegenTestQuest",
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
        "How the player's health, magicka and stamina regenerate.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", vec![quest])],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::own;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        // The opening box and the closing one.
        let shown = run(scripts().unwrap(), &[(own(QUEST).0, SCRIPT, properties)], 2);
        assert_eq!(shown.boxes.len(), 2);
        assert_eq!(shown.saved_at_box, Some(1), "saved before the closing box");
    }
}
