//! Wards: Accumulate Magnitude effects (archetype 32) on `WardPower`.
//!
//! A ward builds while it is cast, from nothing at its magnitude a second
//! (whole at once when dual cast) up to `fMagicWardPowerMaxBase` times its
//! magnitude, and builds again after a hit even while it is struck. A spell
//! from in front of a warded actor meets the ward: a hostile one no
//! stronger than the ward is absorbed, taking its strength from the ward
//! (the casting hand recoils: `wardAbsorb`); a stronger one breaks it (the
//! ward is gone, its caster staggered: `WardBreak`) and is stopped too. Neither does anything to the warded actor. Wards from gear
//! (Spellbreaker's while blocking: a constant `WardPower`) absorb as cast
//! ones do but aren't worn down, and let through what is too strong for
//! them. The ward's actor hears `OnWardHit` (caster, spell, status: 0 a
//! friendly spell, which goes on through, 1 absorbed, 2 broken). Ward
//! Absorb (`ModWardMagickaAbsorptionPct`) gives back that share of an
//! absorbed spell's strength as magicka.
//!
//! Sources: UESP's Skyrim:Ward (charging for about a second, whole at once
//! dual cast, building again at a constant rate, the front only in about 90
//! degrees, staggered when broken with no damage from the spell that broke
//! it, spells, shouts and breath blocked, arrows not), Skyrim:Restoration
//! (Ward Absorb: 25% of the spell's base magnitude), the records (`MGEF`
//! `WardConcSelf*`, the `wardAbsorb` perk, `fMagicWardPowerMaxBase`),
//! `MG01PlayerScript` (`OnWardHit`) and CommonLibSSE's
//! `AccumulatingValueModifierEffect` / `MagicSystem::WardState`. Choices:
//! `known_ambiguities/wards.md`.

use esp::FormId;
use esp::actor_value as av;
use glam::Vec3;
use papyrus::Value;

use crate::engine::{Engine, PLAYER_REF};
use crate::magic::{MagicItem, archetype, flags, spell_type};

/// Half the arc in front a ward covers (radians): UESP's "around 90
/// degrees" in all; a choice (`known_ambiguities/wards.md`).
const WARD_HALF_ARC: f32 = std::f32::consts::FRAC_PI_4;

/// `OnWardHit`'s status (`MagicSystem::WardState`).
const FRIENDLY: i32 = 0;
const ABSORBED: i32 = 1;
const BROKEN: i32 = 2;

/// Where a ward builds to after `dt` seconds more: at its magnitude a
/// second, up to `most`.
fn ward_step(now: f32, magnitude: f32, most: f32, dt: f32) -> f32 {
    (now + magnitude * dt).min(most).max(now.min(most))
}

/// What a ward of `ward` (cast wards' part `cast`) does with a hostile
/// spell of `strength`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Meets {
    /// Absorbed: `drain` taken from the cast wards.
    Absorbed { drain: f32 },
    /// The cast wards break, stopping it.
    Broken,
    /// Too strong for a gear ward alone: it goes through.
    Through,
}

fn meet(ward: f32, cast: f32, strength: f32) -> Meets {
    if strength <= ward {
        Meets::Absorbed {
            drain: strength.min(cast),
        }
    } else if cast > 0.0 {
        Meets::Broken
    } else {
        Meets::Through
    }
}

/// Whether `from` lies within the ward's arc in front of one facing
/// `forward` at `at` (level, as the arc is round the body).
fn in_front(at: Vec3, forward: Vec3, from: Vec3) -> bool {
    let to = (from - at).truncate();
    let f = forward.truncate();
    if to.length_squared() < 1.0 || f.length_squared() < 1e-6 {
        return true;
    }
    to.angle_to(f).abs() <= WARD_HALF_ARC
}

impl Engine {
    /// The most a ward of `magnitude` builds to.
    pub(crate) fn ward_most(&self, magnitude: f32) -> f32 {
        let base = crate::ai::combat::gmst_f32(&self.lo, "fMagicWardPowerMaxBase", 1.3);
        magnitude * base
    }

    /// Cast wards build up.
    pub(crate) fn update_wards(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for i in 0..self.magic.effects.len() {
            let x = &self.magic.effects[i];
            if !x.active || x.done {
                continue;
            }
            let Some(mgef) = self.magic_effect(x.effect) else {
                continue;
            };
            if mgef.archetype != archetype::ACCUMULATE_MAGNITUDE {
                continue;
            }
            let (now, magnitude) = (x.held[0], x.magnitude);
            let to = ward_step(now, magnitude, self.ward_most(magnitude), dt);
            if to != now {
                self.hold(i, &mgef, [to, 0.0]);
            }
        }
    }

    /// The cast wards on an actor: (effect index, what each holds).
    fn cast_wards(&self, actor: FormId) -> Vec<(usize, f32)> {
        self.magic
            .effects
            .iter()
            .enumerate()
            .filter(|(_, x)| x.target == actor && x.active && !x.done)
            .filter(|(_, x)| {
                self.magic_effect(x.effect)
                    .is_some_and(|m| m.archetype == archetype::ACCUMULATE_MAGNITUDE)
            })
            .map(|(i, x)| (i, x.held[0]))
            .collect()
    }

    /// The way an actor (or the player) faces, and where it stands.
    fn facing(&self, actor: FormId) -> Option<(Vec3, Vec3)> {
        if actor == PLAYER_REF {
            return Some((self.player.position, self.camera.forward()));
        }
        self.actor_ref(actor)
            .map(|a| (a.pos, Vec3::new(a.heading.sin(), a.heading.cos(), 0.0)))
    }

    /// `item` cast by `caster` (`power`: dual casting's) reaches `target`:
    /// whether a ward of its stops it.
    pub(crate) fn ward_stops(
        &mut self,
        target: FormId,
        caster: FormId,
        item: &MagicItem,
        power: f32,
    ) -> bool {
        if !matches!(
            item.spell_type,
            spell_type::SPELL
                | spell_type::VOICE
                | spell_type::SCROLL
                | spell_type::STAFF_ENCHANTMENT
        ) {
            return false;
        }
        let ward = self.actor_value(target, av::WARD_POWER);
        if ward <= 0.0 {
            return false;
        }
        // A caster nowhere in the world (unloaded) is taken to be in front.
        let Some((at, forward)) = self.facing(target) else {
            return false;
        };
        if let Some((from, _)) = self.facing(caster)
            && !in_front(at, forward, from)
        {
            return false;
        }
        let event = |e: &Self, status: i32| {
            vec![
                e.object_value(caster),
                e.object_value(item.id),
                Value::Int(status),
            ]
        };
        if !self.item_hostile(item) {
            let args = event(self, FRIENDLY);
            self.send_script_event(target, "OnWardHit", args);
            return false;
        }
        // Its strength: its hostile effects' magnitudes, a landing's share
        // of a second for concentration spells.
        let share = if item.cast_type == crate::spells::CONCENTRATION {
            crate::casting::CONCENTRATION_TICK
        } else {
            1.0
        };
        let strength: f32 = item
            .effects
            .iter()
            .filter_map(|e| Some((e, self.magic_effect(e.effect)?)))
            .filter(|(_, m)| m.has(flags::HOSTILE))
            .map(|(e, m)| {
                let p = if m.has(flags::POWER_AFFECTS_DURATION) {
                    1.0
                } else {
                    power
                };
                e.magnitude * p * share
            })
            .sum();
        let wards = self.cast_wards(target);
        let cast: f32 = wards.iter().map(|w| w.1).sum();
        let status = match meet(ward, cast, strength) {
            Meets::Through => return false,
            Meets::Absorbed { drain } => {
                // The cast wards give it up in turn.
                let mut left = drain;
                for &(i, held) in &wards {
                    let take = left.min(held);
                    left -= take;
                    let x = &self.magic.effects[i];
                    if let Some(mgef) = self.magic_effect(x.effect) {
                        self.hold(i, &mgef, [held - take, 0.0]);
                    }
                }
                if !wards.is_empty() {
                    let share = self.perk_entry_point(
                        crate::perks::ep::MOD_WARD_MAGICKA_ABSORPTION_PCT,
                        target,
                        &[Some(caster), Some(item.id)],
                        0.0,
                    );
                    if share > 0.0 {
                        self.damage_actor_value(target, av::MAGICKA, -strength * share);
                    }
                }
                if !wards.is_empty() {
                    self.ward_graph_event(target, false);
                }
                log::debug!(
                    "{target}'s ward ({ward:.0}) absorbs {} ({strength:.1})",
                    item.name
                );
                ABSORBED
            }
            Meets::Broken => {
                log::info!(
                    "{target}'s ward ({ward:.0}) is broken by {} ({strength:.1})",
                    item.name
                );
                let spells: Vec<(u64, FormId)> = wards
                    .iter()
                    .map(|&(i, _)| (self.magic.effects[i].id, self.magic.effects[i].item))
                    .collect();
                for (id, spell) in spells {
                    if target == PLAYER_REF {
                        self.player_stop_holding(spell);
                    }
                    self.dispel_effect(id);
                }
                self.ward_graph_event(target, true);
                BROKEN
            }
        };
        let args = event(self, status);
        self.send_script_event(target, "OnWardHit", args);
        true
    }

    /// The ward's caster's graph is told: `wardAbsorb` (the ward loop's
    /// recoil) or `WardBreak` (the master graph's stagger; the `WardAbsorbed`
    /// and `WardBroken` idles' events). Graphs without `WardBreak`
    /// (creatures) stagger as from a blow.
    fn ward_graph_event(&mut self, actor: FormId, broken: bool) {
        let event = if broken { "WardBreak" } else { "wardAbsorb" };
        if actor == PLAYER_REF {
            self.player_graph_event(event);
            return;
        }
        if broken && self.is_ethereal(actor) {
            return;
        }
        if let Some(g) = self.actor_mut(actor).and_then(|a| a.graph.as_mut())
            && !g.send_event(event)
            && broken
        {
            g.set_variable("staggerMagnitude", 0.5);
            g.send_event("staggerStart");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wards_build_at_their_magnitude_a_second_to_their_most() {
        assert_eq!(ward_step(0.0, 40.0, 52.0, 0.5), 20.0);
        assert_eq!(ward_step(50.0, 40.0, 52.0, 0.5), 52.0);
        // A hit wears it down; it builds again from there.
        assert_eq!(ward_step(10.0, 40.0, 52.0, 0.25), 20.0);
    }

    #[test]
    fn spells_are_absorbed_break_cast_wards_or_pass_gear_ones() {
        assert_eq!(meet(40.0, 40.0, 25.0), Meets::Absorbed { drain: 25.0 });
        assert_eq!(meet(40.0, 40.0, 60.0), Meets::Broken);
        // Spellbreaker's 50 with a cast ward of 10: the gear part isn't worn.
        assert_eq!(meet(60.0, 10.0, 30.0), Meets::Absorbed { drain: 10.0 });
        assert_eq!(meet(50.0, 0.0, 60.0), Meets::Through);
    }

    #[test]
    fn wards_cover_the_front_only() {
        let at = Vec3::ZERO;
        let north = Vec3::Y;
        assert!(in_front(at, north, Vec3::new(0.0, 500.0, 0.0)));
        assert!(in_front(at, north, Vec3::new(300.0, 500.0, 80.0)));
        assert!(!in_front(at, north, Vec3::new(500.0, 300.0, 0.0)));
        assert!(!in_front(at, north, Vec3::new(0.0, -500.0, 0.0)));
    }

    /// Synthetic records: a ward (40, as Lesser Ward) and a fire spell of
    /// `strength`.
    fn ward_engine(strength: f32) -> (std::path::PathBuf, Engine) {
        use crate::magic::flags as f;
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vrm-wards-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let effect = |id, flags: u32, arch: u32, value: u32| {
            let mut data = [0; 0x6c];
            data[0..4].copy_from_slice(&flags.to_le_bytes());
            data[0x0c..0x10].copy_from_slice(&u32::MAX.to_le_bytes());
            data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
            data[0x40..0x44].copy_from_slice(&arch.to_le_bytes());
            data[0x44..0x48].copy_from_slice(&value.to_le_bytes());
            RecordBuilder::new(b"MGEF", FormId(id)).field(b"DATA", &data)
        };
        let spell = |id: u32, effect: u32, magnitude: f32, casting: u32, delivery: u32| {
            let mut spit = [0; 36];
            spit[16..20].copy_from_slice(&casting.to_le_bytes());
            spit[20..24].copy_from_slice(&delivery.to_le_bytes());
            let mut efit = [0; 12];
            efit[0..4].copy_from_slice(&magnitude.to_le_bytes());
            RecordBuilder::new(b"SPEL", FormId(id))
                .zstring(b"FULL", "Test")
                .field(b"SPIT", &spit)
                .u32(b"EFID", effect)
                .field(b"EFIT", &efit)
        };
        let records = [
            Node::top(
                b"MGEF",
                vec![
                    effect(
                        0x800,
                        f::RECOVER,
                        archetype::ACCUMULATE_MAGNITUDE,
                        av::WARD_POWER,
                    ),
                    effect(0x801, f::HOSTILE | f::DETRIMENTAL, 0, av::HEALTH),
                ],
            ),
            Node::top(
                b"SPEL",
                vec![
                    spell(0x810, 0x800, 40.0, crate::spells::CONCENTRATION, 0),
                    spell(0x811, 0x801, strength, 1, 2),
                ],
            ),
        ];
        let plugin = write_plugin(0, &[], "test", "wards", 0x900, &records);
        std::fs::write(dir.join("Wards.esp"), plugin).unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Wards.esp");
        e.set_actor_value(PLAYER_REF, av::MAGICKA, 1000.0);
        (dir, e)
    }

    const WARD: FormId = FormId(0x810);
    const FIRE: FormId = FormId(0x811);
    /// Somebody casting from nowhere loaded.
    const SOMEONE: FormId = FormId(0x899);

    fn raise_ward(e: &mut Engine, seconds: f32) {
        e.equip_spell(PLAYER_REF, WARD, crate::spells::RIGHT);
        assert!(e.player_cast_press(crate::spells::RIGHT));
        let mut t = 0.0;
        while t < seconds {
            e.update_casting(0.05);
            e.update_magic(0.05);
            t += 0.05;
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_held_ward_absorbs_spells_and_breaks_under_stronger_ones() {
        let (dir, mut e) = ward_engine(25.0);
        raise_ward(&mut e, 0.5);
        let ward = e.actor_value(PLAYER_REF, av::WARD_POWER);
        assert!((18.0..=22.0).contains(&ward), "{ward}");
        raise_ward(&mut e, 1.0);
        let ward = e.actor_value(PLAYER_REF, av::WARD_POWER);
        assert!((ward - 52.0).abs() < 0.01, "{ward}");
        let health = e.actor_value(PLAYER_REF, av::HEALTH);
        assert!(!e.apply_cast(FIRE, SOMEONE, PLAYER_REF, None, false));
        assert_eq!(e.actor_value(PLAYER_REF, av::HEALTH), health);
        let ward = e.actor_value(PLAYER_REF, av::WARD_POWER);
        assert!((ward - 27.0).abs() < 0.01, "{ward}");
        // Twice more: the second breaks it, and the hand stops.
        assert!(!e.apply_cast(FIRE, SOMEONE, PLAYER_REF, None, false));
        assert!(!e.apply_cast(FIRE, SOMEONE, PLAYER_REF, None, false));
        assert_eq!(e.actor_value(PLAYER_REF, av::HEALTH), health);
        e.update_casting(0.05);
        e.update_magic(0.05);
        assert!(!e.player_holds_spell(WARD));
        assert_eq!(e.actor_value(PLAYER_REF, av::WARD_POWER), 0.0);
        // Unwarded, it lands.
        assert!(e.apply_cast(FIRE, SOMEONE, PLAYER_REF, None, false));
        assert!(e.actor_value(PLAYER_REF, av::HEALTH) < health);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_ward_let_go_is_gone() {
        let (dir, mut e) = ward_engine(25.0);
        raise_ward(&mut e, 1.0);
        assert!(e.actor_value(PLAYER_REF, av::WARD_POWER) > 0.0);
        assert!(e.player_cast_release(crate::spells::RIGHT));
        e.update_magic(0.05);
        assert_eq!(e.actor_value(PLAYER_REF, av::WARD_POWER), 0.0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
