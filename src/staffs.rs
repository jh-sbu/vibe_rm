//! Staffs: a staff in hand casts its enchantment (`EITM`, a staff
//! enchantment) as the hand would a spell, paid for from the staff's charge
//! rather than magicka. A staff holds its `EAMT` of charge; each cast takes
//! the enchantment's cost (`ENIT`; by the second for concentration ones),
//! less for skill in its school as spells' costs are. With too little
//! charge left nothing is cast ("Your item has insufficient charge").
//! Casting a staff trains no skill. The charge left is the equipped
//! weapon's `RightItemCharge` / `LeftItemCharge`.
//!
//! Sources: UESP's Skyrim:Staves (no magicka, the charge depleted and the
//! staff unusable when empty, the cost lower with the school's skill: about
//! 1.7 times the uses at 100, no skill gained, one-handed and dual-wielded),
//! the records (`WEAP` `EITM` / `EAMT`, `DNAM` animation type 8; `ENCH`
//! `ENIT`), `sMagicCastInsufficientItemCharge`, CommonLibSSE's
//! `ActorValue::kRightItemCharge` / `kLeftItemCharge`. Choices:
//! `known_ambiguities/staffs.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::engine::Engine;
use crate::magic::spell_type;
use crate::spells::{LEFT, RIGHT};
use crate::world::inventory::HandType;

impl Engine {
    /// The staff in an actor's hand and its enchantment.
    pub(crate) fn staff_in(&self, actor: FormId, hand: usize) -> Option<(FormId, FormId)> {
        let inv = self.inventories.get(&actor)?;
        if hand > LEFT || inv.hand(&self.lo, hand == LEFT) != HandType::Staff {
            return None;
        }
        let staff = inv.held(hand)?;
        let ench = self.item_enchantment(staff)?;
        self.magic_item(ench)
            .filter(|m| m.spell_type == spell_type::STAFF_ENCHANTMENT)
            .map(|_| (staff, ench))
    }

    /// The most charge an enchanted weapon holds (`EAMT`).
    pub(crate) fn weapon_capacity(&self, weapon: FormId) -> f32 {
        self.lo
            .get(weapon)
            .and_then(|r| {
                let d = r.get(b"EAMT")?;
                let b = d.get(0..4).or(d.get(0..2))?;
                Some(match b.len() {
                    4 => u32::from_le_bytes(b.try_into().unwrap()) as f32,
                    _ => u16::from_le_bytes(b.try_into().unwrap()) as f32,
                })
            })
            .unwrap_or(0.0)
    }

    /// The charge left on one of an actor's `weapon`s.
    pub(crate) fn weapon_charge(&self, actor: FormId, weapon: FormId) -> f32 {
        let full = self.weapon_capacity(weapon);
        self.inventories
            .get(&actor)
            .and_then(|inv| inv.charge_left(weapon))
            .map_or(full, |c| c.min(full))
    }

    /// Take `amount` of charge from one of an actor's `weapon`s.
    pub(crate) fn spend_weapon_charge(&mut self, actor: FormId, weapon: FormId, amount: f32) {
        let full = self.weapon_capacity(weapon);
        let left = self.weapon_charge(actor, weapon) - amount;
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.set_charge(weapon, left, full);
        }
    }

    /// What a cast of a staff enchantment costs `caster` in charge (a second
    /// of it for concentration ones): its `ENIT` cost, less for skill in
    /// its school as a spell's (`spell_cost`).
    pub(crate) fn staff_cost(&self, caster: FormId, ench: FormId) -> f32 {
        let base = self
            .lo
            .get(ench)
            .and_then(|r| {
                let d = r.get(b"ENIT")?.get(0..4)?;
                Some(u32::from_le_bytes(d.try_into().unwrap()) as f32)
            })
            .unwrap_or(0.0);
        let mut cost = base;
        if let Some(school) = self.spell_school(ench) {
            let skill = self.actor_value(caster, school).clamp(0.0, 100.0);
            cost *= crate::spells::skill_cost_factor(&self.lo, skill);
        }
        cost.max(0.0)
    }

    /// The equipped weapon's charge in a hand: (left, most), for
    /// `RightItemCharge` / `LeftItemCharge`.
    pub(crate) fn hand_item_charge(&self, actor: FormId, index: u32) -> Option<(f32, f32)> {
        let hand = match index {
            av::RIGHT_ITEM_CHARGE => RIGHT,
            av::LEFT_ITEM_CHARGE => LEFT,
            _ => return None,
        };
        let weapon = self.inventories.get(&actor)?.held(hand)?;
        let full = self.weapon_capacity(weapon);
        (full > 0.0).then(|| (self.weapon_charge(actor, weapon), full))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::PLAYER_REF;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    const STAFF: FormId = FormId(0x820);
    const ENCH: FormId = FormId(0x810);

    /// A staff of 50 charge whose enchantment (a self-cast heal) costs 20.
    fn staff_engine() -> (std::path::PathBuf, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vrm-staffs-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let mut data = [0; 0x6c];
        data[0x0c..0x10].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&av::HEALTH.to_le_bytes());
        let mut enit = [0; 36];
        enit[0..4].copy_from_slice(&20u32.to_le_bytes());
        enit[8..12].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        enit[0x14..0x18].copy_from_slice(&spell_type::STAFF_ENCHANTMENT.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&5.0f32.to_le_bytes());
        let mut dnam = [0; 100];
        dnam[0] = 8; // staff
        let records = [
            Node::top(
                b"MGEF",
                vec![RecordBuilder::new(b"MGEF", FormId(0x800)).field(b"DATA", &data)],
            ),
            Node::top(
                b"ENCH",
                vec![
                    RecordBuilder::new(b"ENCH", ENCH)
                        .field(b"ENIT", &enit)
                        .u32(b"EFID", 0x800)
                        .field(b"EFIT", &efit),
                ],
            ),
            Node::top(
                b"WEAP",
                vec![
                    RecordBuilder::new(b"WEAP", STAFF)
                        .zstring(b"FULL", "Test Staff")
                        .u32(b"EITM", ENCH.0)
                        .field(b"EAMT", &50u16.to_le_bytes())
                        .field(b"DATA", &[0; 10])
                        .field(b"DNAM", &dnam),
                ],
            ),
        ];
        let plugin = write_plugin(0, &[], "test", "staffs", 0x900, &records);
        std::fs::write(dir.join("Staffs.esp"), plugin).unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Staffs.esp");
        e.set_actor_value(PLAYER_REF, av::MAGICKA, 100.0);
        (dir, e)
    }

    fn cast(e: &mut Engine) {
        assert!(e.player_cast_press(RIGHT));
        e.update_casting(0.05);
        e.player_cast_release(RIGHT);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_staff_casts_from_its_charge_until_too_little_is_left() {
        let (dir, mut e) = staff_engine();
        e.add_item(PLAYER_REF, STAFF, 1);
        e.equip_item(PLAYER_REF, STAFF, true).unwrap();
        assert_eq!(e.staff_in(PLAYER_REF, RIGHT), Some((STAFF, ENCH)));
        assert_eq!(e.actor_value(PLAYER_REF, av::RIGHT_ITEM_CHARGE), 50.0);
        cast(&mut e);
        cast(&mut e);
        assert_eq!(e.actor_value(PLAYER_REF, av::RIGHT_ITEM_CHARGE), 10.0);
        assert_eq!(e.actor_value(PLAYER_REF, av::MAGICKA), 100.0);
        // Ten left: not enough.
        cast(&mut e);
        assert_eq!(e.actor_value(PLAYER_REF, av::RIGHT_ITEM_CHARGE), 10.0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
