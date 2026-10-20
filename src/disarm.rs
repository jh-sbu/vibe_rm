//! Disarm (effect archetype 9, the shout): the target's drawn weapon is
//! pulled from its hand. UESP's Skyrim:Disarm: the right hand's weapon is
//! dropped, a dual-wielder's left-hand one goes to the inventory instead,
//! bound weapons aren't affected, bows, crossbows and staves are, and
//! sheathing in time avoids it. Its magnitude is the highest level it
//! reaches, as with calm (`archetype::level_capped`: "Disarms enemies up to
//! level 12"). The player's "Weapons Disarmed" statistic counts the actors
//! they disarm (UESP's Skyrim:Statistics). `known_ambiguities/disarm.md`.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::world::inventory::{LEFT, RIGHT, item_droppable, weapon_bound};

impl Engine {
    /// Pull the weapon out of `target`'s hands for `caster`'s Disarm: whether
    /// they held one the effect takes.
    pub(crate) fn disarm(&mut self, target: FormId, caster: Option<FormId>) -> bool {
        if self.is_dead(target) || !self.weapon_drawn(target) {
            return false;
        }
        let lo = &self.lo;
        let Some(inv) = self.inventories.get_mut(&target) else {
            return false;
        };
        // A weapon in both hands (two-handed, bows) is the right hand's.
        let both = inv.both_hands();
        let takes = |f: Option<FormId>| f.filter(|&w| !weapon_bound(lo, w));
        let right = takes(inv.weapon_in(lo, RIGHT));
        let left = takes(inv.weapon_in(lo, LEFT)).filter(|_| !both);
        if right.is_none() && left.is_none() {
            return false;
        }
        if left.is_some() {
            inv.release(lo, LEFT);
        }
        if let Some(w) = right {
            inv.release(lo, RIGHT);
            // What can't be dropped stays in the inventory.
            if item_droppable(lo, w) {
                self.drop_item(target, w, 1);
            }
        }
        log::info!(
            "{target} is disarmed{}",
            caster.map_or(String::new(), |c| format!(" by {c}"))
        );
        if caster == Some(PLAYER_REF) && target != PLAYER_REF {
            self.increment_stat("Weapons Disarmed", 1);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ActorRuntime;
    use crate::engine::CellRuntime;
    use crate::magic::{archetype, flags, spell_type};
    use crate::world::inventory::Inventory;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use glam::Mat4;
    use std::sync::Arc;

    const SWORD: FormId = FormId(0x800);
    const DAGGER: FormId = FormId(0x801);
    const BOUND: FormId = FormId(0x802);
    const BOW: FormId = FormId(0x803);
    const FIXED: FormId = FormId(0x804);
    const MGEF: FormId = FormId(0x810);
    const REACHES_12: FormId = FormId(0x811);
    const REACHES_30: FormId = FormId(0x812);
    const NPC: FormId = FormId(0x830);
    const ACTOR: FormId = FormId(0x831);
    const CELL: FormId = FormId(0x850);

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
            std::env::temp_dir().join(format!("vrm-disarm-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        // `DNAM`: animation type, flags (0x08 can't drop), flags2 (0x2000 bound).
        let weapon = |id, anim: u8, drop_flags: u16, flags2: u32| {
            let mut dnam = [0; 100];
            dnam[0] = anim;
            dnam[12..14].copy_from_slice(&drop_flags.to_le_bytes());
            dnam[40..44].copy_from_slice(&flags2.to_le_bytes());
            RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam)
        };
        let mut mgef = [0; 0x6c];
        mgef[..4].copy_from_slice(&(flags::HOSTILE | 0x1000).to_le_bytes());
        mgef[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        mgef[0x40..0x44].copy_from_slice(&archetype::DISARM.to_le_bytes());
        mgef[0x44..0x48].copy_from_slice(&u32::MAX.to_le_bytes());
        let spell = |id: FormId, level: f32| {
            let mut spit = [0; 36];
            spit[8..12].copy_from_slice(&spell_type::VOICE.to_le_bytes());
            let mut efit = [0; 12];
            efit[..4].copy_from_slice(&level.to_le_bytes());
            RecordBuilder::new(b"SPEL", id)
                .field(b"SPIT", &spit)
                .u32(b"EFID", MGEF.0)
                .field(b"EFIT", &efit)
        };
        let mut acbs = [0; 24];
        acbs[8..10].copy_from_slice(&20u16.to_le_bytes());
        let records = [
            Node::top(
                b"WEAP",
                vec![
                    weapon(SWORD, 1, 0, 0),
                    weapon(DAGGER, 2, 0, 0),
                    weapon(BOUND, 1, 0x88, 0x2000),
                    weapon(BOW, 7, 0, 0),
                    weapon(FIXED, 1, 0x08, 0),
                ],
            ),
            Node::top(
                b"MGEF",
                vec![RecordBuilder::new(b"MGEF", MGEF).field(b"DATA", &mgef)],
            ),
            Node::top(
                b"SPEL",
                vec![spell(REACHES_12, 12.0), spell(REACHES_30, 30.0)],
            ),
            Node::top(
                b"NPC_",
                vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &acbs)],
            ),
            Node::top(
                b"ACHR",
                vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
            ),
        ];
        std::fs::write(
            f.0.join("Disarm.esp"),
            write_plugin(0, &[], "test", "synthetic disarm", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&f.0, "Disarm.esp");
        e.location = crate::engine::Location::Interior(CELL);
        let mut actor = ActorRuntime::new(
            ACTOR,
            NPC,
            Arc::new(crate::world::skeleton::Skeleton::default()),
            Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        actor.health = 100.0;
        actor.drawn = true;
        let key = crate::render::CellKey::Interior(CELL);
        e.actor_cells.insert(ACTOR, key);
        e.cells
            .entry(key)
            .or_insert_with(CellRuntime::default)
            .actors
            .push(actor);
        (f, e)
    }

    /// The actor holds `right` (and `left`), ready to fight.
    fn arm(e: &mut Engine, right: FormId, left: Option<FormId>, both: bool) {
        let mut inv = Inventory::default();
        inv.add(right, 1);
        let hands: &[usize] = if both { &[RIGHT, LEFT] } else { &[RIGHT] };
        inv.hold(&e.lo, right, hands);
        if let Some(l) = left {
            inv.add(l, 1);
            inv.hold(&e.lo, l, &[LEFT]);
        }
        e.inventories.insert(ACTOR, inv);
        e.actor_mut(ACTOR).unwrap().drawn = true;
    }

    /// The weapons lying in the world, by base.
    fn lying(e: &Engine, base: FormId) -> usize {
        e.created_refs
            .refs
            .values()
            .filter(|c| c.base == base)
            .count()
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn the_right_hand_weapon_drops_and_the_left_goes_to_the_inventory() {
        let (_f, mut e) = fixture();
        arm(&mut e, SWORD, Some(DAGGER), false);
        assert!(e.apply_item(REACHES_30, Some(PLAYER_REF), ACTOR));
        let inv = &e.inventories[&ACTOR];
        assert_eq!(inv.held(RIGHT), None);
        assert_eq!(inv.held(LEFT), None);
        assert_eq!(inv.count(SWORD), 0); // dropped in the world
        assert_eq!(inv.count(DAGGER), 1); // put away
        assert!(inv.equipped().is_empty());
        assert_eq!(e.query_stat("Weapons Disarmed"), 1);
        assert_eq!(lying(&e, SWORD), 1);
        assert_eq!(lying(&e, DAGGER), 0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn level_sheathing_and_bound_weapons_keep_the_weapon() {
        let (_f, mut e) = fixture();
        // The actor is level 20: "up to level 12" doesn't reach it.
        arm(&mut e, SWORD, None, false);
        e.apply_item(REACHES_12, Some(PLAYER_REF), ACTOR);
        assert_eq!(e.inventories[&ACTOR].held(RIGHT), Some(SWORD));
        // A sheathed weapon is safe.
        e.actor_mut(ACTOR).unwrap().drawn = false;
        e.apply_item(REACHES_30, Some(PLAYER_REF), ACTOR);
        assert_eq!(e.inventories[&ACTOR].held(RIGHT), Some(SWORD));
        // A bound weapon stays in the hand.
        arm(&mut e, BOUND, Some(DAGGER), false);
        e.apply_item(REACHES_30, Some(PLAYER_REF), ACTOR);
        let inv = &e.inventories[&ACTOR];
        assert_eq!(inv.held(RIGHT), Some(BOUND));
        assert_eq!(inv.held(LEFT), None); // the other hand's weapon still goes
        assert_eq!(e.query_stat("Weapons Disarmed"), 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn bows_drop_from_both_hands() {
        let (_f, mut e) = fixture();
        arm(&mut e, BOW, None, true);
        e.apply_item(REACHES_30, Some(PLAYER_REF), ACTOR);
        let inv = &e.inventories[&ACTOR];
        assert_eq!((inv.held(RIGHT), inv.held(LEFT)), (None, None));
        assert_eq!(inv.count(BOW), 0);
        assert_eq!(lying(&e, BOW), 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn undroppable_weapons_stay_carried() {
        let (_f, mut e) = fixture();
        arm(&mut e, FIXED, None, false);
        e.apply_item(REACHES_30, Some(PLAYER_REF), ACTOR);
        let inv = &e.inventories[&ACTOR];
        assert_eq!(inv.held(RIGHT), None);
        assert_eq!(inv.count(FIXED), 1);
        assert_eq!(lying(&e, FIXED), 0);
    }
}
