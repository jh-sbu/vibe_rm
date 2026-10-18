//! Soul trap: an actor dying under a soul trap effect gives its soul to the
//! caster's soul gems (the game's effect is scripted: `magicSoulTrapFXScript`
//! calls `Actor.TrapSoul` as it finishes; archetype 23 effects do it here) (UESP's Skyrim:Soul Trap and Soul
//! Gem). A creature's soul is petty, lesser, common, greater or grand by its
//! level (`i<Size>SoulActorLevel`: 4, 16, 28, 38); people's (`ActorTypeNPC`)
//! are black, held only by black soul gems (`SLGM` record flag 0x20000);
//! `NoSoulTrap` actors and races give none. The smallest empty gem that can
//! hold it takes it ("Soul captured!", Souls Trapped); filled to its size,
//! a gem becomes its linked filled form (`NAM0`), else it keeps the soul as
//! the inventory's side list (`Inventory::souls`). Each body gives one soul.
//! `known_ambiguities/soul-trap.md`.

use esp::FormId;

use crate::ai::combat::gmst_i32;
use crate::engine::{Engine, PLAYER_REF};

/// `SLGM` record flag: can hold an NPC's (black) soul.
const BLACK_GEM: u32 = 0x0002_0000;

/// A soul gem as its record has it.
pub(crate) struct Gem {
    pub form: FormId,
    /// The soul its record holds (0 empty).
    pub soul: u8,
    pub capacity: u8,
    pub black: bool,
    pub filled: Option<FormId>,
}

impl Engine {
    pub(crate) fn soul_gem(&self, form: FormId) -> Option<Gem> {
        let rec = self.lo.get(form).filter(|r| r.tag().0 == *b"SLGM")?;
        let byte = |tag: &[u8; 4]| rec.get(tag).and_then(|d| d.first().copied()).unwrap_or(0);
        Some(Gem {
            form,
            soul: byte(b"SOUL"),
            capacity: byte(b"SLCP"),
            black: rec.flags() & BLACK_GEM != 0,
            filled: rec
                .get(b"NAM0")
                .filter(|d| d.len() >= 4)
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
                .filter(|f| !f.is_null()),
        })
    }

    /// The soul an actor gives: its size (1 petty .. 5 grand) and whether
    /// it is black; none for `NoSoulTrap` ones.
    fn soul_of(&self, actor: FormId) -> Option<(u8, bool)> {
        let kw = |name: &str| self.lo.find_editor_id(name);
        if kw("NoSoulTrap").is_some_and(|k| self.has_keyword(actor, k)) {
            return None;
        }
        if kw("ActorTypeNPC").is_some_and(|k| self.has_keyword(actor, k)) {
            return Some((5, true));
        }
        let level = self.actor_level(actor);
        let at = |name: &str, d: i32| gmst_i32(&self.lo, name, d);
        let size = [
            at("iLesserSoulActorLevel", 4),
            at("iCommonSoulActorLevel", 16),
            at("iGreaterSoulActorLevel", 28),
            at("iGrandSoulActorLevel", 38),
        ]
        .iter()
        .filter(|&&min| level >= min)
        .count() as u8
            + 1;
        Some((size, false))
    }

    /// `victim` died under `caster`'s soul trap: its soul goes into the
    /// smallest of the caster's empty soul gems that can hold it. Whether it
    /// did (`TrapSoul`).
    pub(crate) fn trap_soul(&mut self, caster: FormId, victim: FormId) -> bool {
        if !self.is_dead(victim) || self.magic.souls_taken.contains(&victim) {
            return false;
        }
        let Some((size, black)) = self.soul_of(victim) else {
            log::info!("{victim} has no soul to trap");
            return false;
        };
        let inv = self.inventory_mut(caster).clone();
        let mut empty: Vec<Gem> = inv
            .items
            .iter()
            .filter(|&&(f, _)| inv.without_soul(f) > 0)
            .filter_map(|&(f, _)| self.soul_gem(f))
            .filter(|g| g.soul == 0)
            .collect();
        let had_empty = !empty.is_empty();
        empty.retain(|g| g.capacity >= size && (!black || g.black));
        // White souls go into black gems only when nothing else holds them.
        empty.sort_by_key(|g| (g.black && !black, g.capacity));
        let Some(gem) = empty.into_iter().next() else {
            log::info!("{victim}'s soul ({size}, black {black}): no gem for it");
            if caster == PLAYER_REF && had_empty {
                let msg = self
                    .gmst_string("sSoulGemTooSmall")
                    .unwrap_or_else(|| "No soul gem large enough.".into());
                self.scripts.notify(msg);
            }
            return false;
        };
        self.magic.souls_taken.insert(victim);
        match gem.filled.filter(|_| gem.capacity == size) {
            Some(filled) => {
                let inv = self.inventory_mut(caster);
                let parts = inv.remove_split(gem.form, 1, None);
                for (owner, n) in parts {
                    inv.add_owned(filled, owner, n);
                }
            }
            None => self.inventory_mut(caster).put_soul(gem.form, size),
        }
        log::info!(
            "{victim}'s soul ({size}, black {black}) into {} ({})",
            gem.form,
            self.form_name(gem.form)
        );
        if caster == PLAYER_REF {
            let msg = self
                .gmst_string("sSoulCaptured")
                .unwrap_or_else(|| "Soul captured!".into());
            self.scripts.notify(msg);
            self.increment_stat("Souls Trapped", 1);
        }
        true
    }
}
