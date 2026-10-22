//! Fleeing by threat ratio. Each fighter's combat strength is how much damage it
//! deals a second times the health its armor makes it worth; its side's strength
//! over the other side's is the threat ratio (above 1 it should outlive them).
//! Every few seconds a fighter that has been hurt compares that ratio with its
//! confidence (plus a modifier rolled for the fight) and flees below it, or, fleeing,
//! turns back once the ratio has recovered. This follows the model the GECK
//! documents for Fallout 3 / New Vegas (Confidence, GetThreatRatio, Combat Flee)
//! with Skyrim's own settings; what can't be confirmed for Skyrim is listed in
//! `known_ambiguities/combat-fleeing.md`.

use std::collections::HashMap;

use esp::{FormId, LoadOrder};

use super::combat::{Combat, CombatStats, gmst_f32};
use crate::engine::{Engine, Location, PLAYER_REF};

/// Seconds between a fighter's strength updates (`fCombatStrengthUpdateTime`) and
/// its flee checks (`fCombatThreatRatioUpdateTime`).
const STRENGTH_UPDATE: (&str, f32) = ("fCombatStrengthUpdateTime", 1.0);
const GROUP_STRENGTH_UPDATE: (&str, f32) = ("fCombatGroupCombatStrengthUpdateTime", 2.0);
const THREAT_RATIO_UPDATE: (&str, f32) = ("fCombatThreatRatioUpdateTime", 5.0);
/// FLEE dialogue attempt intervals. The settings and per-actor timer are public;
/// exact Skyrim scheduling remains open (see combat-fleeing.md).
const FLEE_DIALOGUE_MIN: (&str, f32) = ("fCombatDialogueFleeMinElapsedTime", 3.0);
const FLEE_DIALOGUE_MAX: (&str, f32) = ("fCombatDialogueFleeMaxElapsedTime", 5.0);

/// CommonLibSSE's CombatMember has a groupStrengthUpdateTimer and threatValue,
/// separate from CombatState's personal strength timer. These contributions
/// sample the strength estimate directly, independently of the personal cache.
/// Membership and timer phases are our policy; see combat-fleeing.md.
#[derive(Default)]
pub(crate) struct GroupStrengths {
    members: HashMap<FormId, GroupStrength>,
}

struct GroupStrength {
    value: f32,
    remaining: f32,
}

impl GroupStrengths {
    /// Drop departed members immediately; new members are sampled immediately.
    /// Subsequent samples require elapsed simulation time, even at interval 0.
    fn due(&mut self, members: &[FormId], dt: f32, interval: f32) -> Vec<FormId> {
        self.members.retain(|id, _| members.contains(id));
        let interval = if interval.is_finite() {
            interval.max(0.0)
        } else {
            GROUP_STRENGTH_UPDATE.1
        };
        let dt = dt.max(0.0);
        let mut due = Vec::new();
        for &id in members {
            match self.members.entry(id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(GroupStrength {
                        value: 0.0,
                        remaining: interval,
                    });
                    due.push(id);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let sample = entry.get_mut();
                    sample.remaining = (sample.remaining - dt).max(0.0);
                    if dt > 0.0 && sample.remaining <= 0.0 {
                        sample.remaining = interval;
                        due.push(id);
                    }
                }
            }
        }
        due
    }

    fn store(&mut self, id: FormId, value: f32) {
        if let Some(sample) = self.members.get_mut(&id) {
            sample.value = value;
        }
    }

    fn strength(&self, id: FormId) -> f32 {
        self.members.get(&id).map_or(0.0, |sample| sample.value)
    }

    pub(crate) fn remove(&mut self, id: FormId) {
        self.members.remove(&id);
    }
}

fn flee_dialogue_interval(min: f32, max: f32, roll: u64) -> f32 {
    let min = if min.is_finite() {
        min.max(0.0)
    } else {
        FLEE_DIALOGUE_MIN.1
    };
    let max = if max.is_finite() {
        max.max(min)
    } else {
        FLEE_DIALOGUE_MAX.1.max(min)
    };
    min + (roll % 10_000) as f32 / 10_000.0 * (max - min)
}

impl Combat {
    /// A blocked dialogue channel leaves the timer due. Inactive flees cancel
    /// their wait, so another flee starts with an immediate attempt.
    fn flee_dialogue_due(&mut self, dt: f32) -> bool {
        if !self.fleeing {
            self.flee_dialogue = 0.0;
            return false;
        }
        self.flee_dialogue = (self.flee_dialogue - dt.max(0.0)).max(0.0);
        self.flee_dialogue <= 0.0
    }
}

/// A fleeing actor this near its threat thinks again (the GECK's
/// `fCombatFleeBoostConfidenceTargetRadius`; Skyrim has no such setting).
pub(crate) const FLEE_THREAT_NEAR: f32 = 512.0;

/// Seconds between a fighter's blows, for its damage a second: our fighters'
/// own pace (a swing plus the cooldown after it, on average). Skyrim's own
/// estimate isn't documented.
const ATTACK_INTERVAL: f32 = 2.0;
const BOW_INTERVAL: f32 = 2.6;

/// The threat ratio below which an actor of a confidence level flees.
pub(crate) fn confidence_value(lo: &LoadOrder, confidence: u8) -> f32 {
    match confidence {
        0 => gmst_f32(lo, "fConfidenceCowardly", 1000.0),
        1 => gmst_f32(lo, "fConfidenceCautious", 0.375),
        2 => gmst_f32(lo, "fConfidenceAverage", 0.15),
        3 => gmst_f32(lo, "fConfidenceBrave", 0.0375),
        _ => gmst_f32(lo, "fConfidenceFoolhardy", 0.0),
    }
}

/// A new fight: its confidence modifier, rolled between
/// `fCombatConfidenceModifierMin` and `Max` (none for the foolhardy), and its
/// first flee check at a random point within the update time.
pub(crate) fn new_combat(
    lo: &LoadOrder,
    target: FormId,
    stats: &CombatStats,
    rolls: (u64, u64),
) -> Combat {
    let unit = |r: u64| (r % 10_000) as f32 / 10_000.0;
    let mut c = Combat::new(target);
    if stats.confidence < 4 {
        let (lo_m, hi_m) = (
            gmst_f32(lo, "fCombatConfidenceModifierMin", -0.25),
            gmst_f32(lo, "fCombatConfidenceModifierMax", 1.0),
        );
        c.confidence_mod = lo_m + unit(rolls.0) * (hi_m - lo_m);
    }
    c.threat_check = unit(rolls.1) * gmst_f32(lo, THREAT_RATIO_UPDATE.0, THREAT_RATIO_UPDATE.1);
    c
}

/// A fighter in the threat sums: whom it fights and how strong it is (nothing
/// while fleeing: it neither strikes nor stands its ground).
struct Fighter {
    id: FormId,
    target: Option<FormId>,
    strength: f32,
}

/// The two sides of `actor`'s fight: its allies (itself included) and its
/// enemies. Everyone fighting its target (or whoever fights its side) is on its
/// side, everyone fighting its side against it on the other. `None` when it
/// isn't fighting.
fn combat_sides(actor: FormId, fighters: &[Fighter]) -> Option<(Vec<FormId>, Vec<FormId>)> {
    let me = fighters.iter().find(|f| f.id == actor)?;
    let mut allies = vec![actor];
    let mut enemies: Vec<FormId> = me.target.into_iter().collect();
    // Who fights an enemy, or is fought by one, is an ally, and the other way
    // round; until nobody new joins either side.
    loop {
        let before = allies.len() + enemies.len();
        for f in fighters {
            let Some(t) = f.target else { continue };
            let (side, other) = if enemies.contains(&f.id) {
                (&mut enemies, &mut allies)
            } else if allies.contains(&f.id) {
                (&mut allies, &mut enemies)
            } else if enemies.contains(&t) {
                allies.push(f.id);
                continue;
            } else if allies.contains(&t) {
                enemies.push(f.id);
                continue;
            } else {
                continue;
            };
            if !side.contains(&t) && !other.contains(&t) {
                other.push(t);
            }
        }
        if allies.len() + enemies.len() == before {
            break;
        }
    }
    Some((allies, enemies))
}

/// The threat ratio of `actor`'s side against its enemies (see `combat_sides`).
/// `own`: its own strength, counted even while it flees (what it would bring if
/// it turned back).
fn group_threat_ratio(actor: FormId, own: f32, fighters: &[Fighter]) -> f32 {
    let Some((allies, enemies)) = combat_sides(actor, fighters) else {
        return f32::INFINITY;
    };
    let strength = |ids: &[FormId]| -> f32 {
        ids.iter()
            .filter_map(|id| fighters.iter().find(|f| f.id == *id))
            .map(|f| if f.id == actor { own } else { f.strength })
            .sum()
    };
    let theirs = strength(&enemies);
    if theirs <= 0.0 {
        f32::INFINITY
    } else {
        strength(&allies) / theirs
    }
}

impl Engine {
    /// Everyone fighting, and the player while anyone fights them.
    fn fighters(&self) -> Vec<Fighter> {
        let mut fighters: Vec<Fighter> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.bleeding.is_none())
            .filter_map(|a| {
                let c = a.combat.as_ref()?;
                Some(Fighter {
                    id: a.ref_id,
                    target: Some(c.target),
                    strength: if c.fleeing {
                        0.0
                    } else {
                        self.combat_group_strengths.strength(a.ref_id)
                    },
                })
            })
            .collect();
        if fighters.iter().any(|f| f.target == Some(PLAYER_REF)) && !self.player_dead() {
            fighters.push(Fighter {
                id: PLAYER_REF,
                target: None,
                strength: self.combat_group_strengths.strength(PLAYER_REF),
            });
        }
        fighters
    }

    /// The members of the combat group `actor` fights in (itself included) and
    /// the group's targets, none when it isn't fighting.
    pub fn combat_group(&self, actor: FormId) -> Option<(Vec<FormId>, Vec<FormId>)> {
        combat_sides(actor, &self.fighters())
    }

    /// Combat strength: damage a second times health, the health counted for what
    /// armor takes off each blow (`health / (1 - reduction)`).
    fn combat_strength(&self, actor: FormId) -> f32 {
        let reduction = self.protection(actor).reduction.min(0.99);
        let (health, dps) = if actor == PLAYER_REF {
            let damage = self
                .player_weapon()
                .map_or(self.player_stats().unarmed_damage, |w| {
                    super::combat::weapon_damage(&self.lo, w)
                });
            (self.player_health, damage / ATTACK_INTERVAL)
        } else {
            let Some(a) = self.actor_ref(actor) else {
                return 0.0;
            };
            let interval = if a.bow { BOW_INTERVAL } else { ATTACK_INTERVAL };
            (
                a.health,
                self.attack_damage(a, None, crate::world::inventory::RIGHT) / interval,
            )
        };
        dps * health.max(0.0) / (1.0 - reduction)
    }

    /// GetThreatRatio: `actor`'s combat strength over `other`'s.
    pub fn threat_ratio(&self, actor: FormId, other: FormId) -> f32 {
        let theirs = self.combat_strength(other);
        if theirs <= 0.0 {
            f32::INFINITY
        } else {
            self.combat_strength(actor) / theirs
        }
    }

    /// IsFleeing.
    pub fn is_fleeing(&self, actor: FormId) -> bool {
        self.actor_ref(actor)
            .and_then(|a| a.combat.as_ref())
            .is_some_and(|c| c.fleeing)
    }

    /// Strength updates and flee checks for everyone fighting; those who start to
    /// flee say so. Fleeing actors safe long enough (`fFleeIsSafeTimer`) leave the
    /// fight.
    pub(crate) fn update_threat(&mut self, dt: f32) {
        let strength_every = gmst_f32(&self.lo, STRENGTH_UPDATE.0, STRENGTH_UPDATE.1);
        let check_every = gmst_f32(&self.lo, THREAT_RATIO_UPDATE.0, THREAT_RATIO_UPDATE.1);
        let safe_after = gmst_f32(&self.lo, "fFleeIsSafeTimer", 30.0);
        let flee_distance = if matches!(self.location, Location::Interior(_)) {
            gmst_f32(&self.lo, "fCombatFleeDistanceInterior", 2048.0)
        } else {
            gmst_f32(&self.lo, "fCombatFleeDistanceExterior", 4096.0)
        };
        // Strengths, each refreshed on its own timer.
        let mut due = Vec::new();
        for a in self
            .cells
            .values_mut()
            .flat_map(|rt| rt.actors.iter_mut())
            .filter(|a| a.combat.is_some() && !a.dead)
        {
            a.strength_in -= dt;
            if a.strength_in <= 0.0 {
                a.strength_in = strength_every;
                due.push(a.ref_id);
            }
        }
        let fresh: Vec<(FormId, f32)> = due
            .into_iter()
            .map(|id| (id, self.combat_strength(id)))
            .collect();
        let mut checks = Vec::new();
        let mut safe = Vec::new();
        for a in self.cells.values_mut().flat_map(|rt| rt.actors.iter_mut()) {
            if let Some(&(_, s)) = fresh.iter().find(|f| f.0 == a.ref_id) {
                a.strength = s;
            }
            let Some(c) = a
                .combat
                .as_mut()
                .filter(|_| !a.dead && a.bleeding.is_none())
            else {
                continue;
            };
            if c.fleeing && c.safe >= safe_after {
                safe.push(a.ref_id);
                continue;
            }
            c.threat_check -= dt;
            // Cowards (the frightened among them) think of fleeing at once.
            let coward = a.stats.confidence == 0 && !c.fleeing;
            if c.threat_check <= 0.0 || std::mem::take(&mut c.recheck) || coward {
                c.threat_check = check_every;
                checks.push(a.ref_id);
            }
        }
        for r in safe {
            log::info!("{r} is safe");
            self.end_combat(r);
        }
        // Keep this clock running even when no threat-ratio check is due.
        // Death, bleedout, unloading and combat exit remove a contribution
        // immediately; fleeing only masks it in the sums, leaving its clock live.
        let mut members: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.combat.is_some() && !a.dead && a.bleeding.is_none())
            .map(|a| a.ref_id)
            .collect();
        if !self.player_dead()
            && members.iter().any(|&r| {
                self.actor_ref(r)
                    .and_then(|a| a.combat.as_ref())
                    .is_some_and(|c| c.target == PLAYER_REF)
            })
        {
            members.push(PLAYER_REF);
        }
        let group_every = gmst_f32(&self.lo, GROUP_STRENGTH_UPDATE.0, GROUP_STRENGTH_UPDATE.1);
        let due = self.combat_group_strengths.due(&members, dt, group_every);
        for id in due {
            let strength = self.combat_strength(id);
            self.combat_group_strengths.store(id, strength);
        }
        if checks.is_empty() {
            self.update_flee_dialogue(dt);
            return;
        }
        let fighters = self.fighters();
        for r in checks {
            let own = self.actor_ref(r).map_or(0.0, |a| a.strength);
            let ratio = group_threat_ratio(r, own, &fighters);
            let Some(a) = self.actor_mut(r) else { continue };
            let Some(c) = a.combat.as_mut() else { continue };
            let threshold = a.stats.confidence_value + c.confidence_mod;
            // Only the hurt think of fleeing (cowards always do).
            let hurt = a.health < a.stats.max_health || a.stats.confidence == 0;
            log::debug!(
                "{r}: threat ratio {ratio:.3} against {threshold:.3}{}{}",
                if hurt { "" } else { " (unhurt)" },
                if c.fleeing { ", fleeing" } else { "" }
            );
            if !c.fleeing && hurt && ratio < threshold {
                c.fleeing = true;
                c.flee_distance = flee_distance;
                c.safe = 0.0;
                if !c.swinging() {
                    c.attack = None;
                    c.draw = Default::default();
                }
                a.set_guard(0.0);
                log::info!(
                    "{r} flees (threat ratio {ratio:.3} < {threshold:.3}; health {:.0} / {:.0})",
                    a.health,
                    a.stats.max_health
                );
            } else if c.fleeing && ratio >= threshold {
                c.fleeing = false;
                c.away_to = None;
                log::info!("{r} stops fleeing (threat ratio {ratio:.3} >= {threshold:.3})");
            }
        }
        self.update_flee_dialogue(dt);
    }

    /// Repeat FLEE lines on their own timer, independent of strength and threat
    /// ratio checks. Busy channels defer an attempt; an attempted selection
    /// (including no eligible INFO) waits a full new interval before retrying.
    fn update_flee_dialogue(&mut self, dt: f32) {
        let due: Vec<FormId> = self
            .cells
            .values_mut()
            .flat_map(|rt| rt.actors.iter_mut())
            .filter(|a| !a.dead && a.bleeding.is_none())
            .filter_map(|a| {
                let c = a.combat.as_mut()?;
                c.flee_dialogue_due(dt).then_some(a.ref_id)
            })
            .collect();
        if due.is_empty() {
            return;
        }
        let min = gmst_f32(&self.lo, FLEE_DIALOGUE_MIN.0, FLEE_DIALOGUE_MIN.1);
        let max = gmst_f32(&self.lo, FLEE_DIALOGUE_MAX.0, FLEE_DIALOGUE_MAX.1);
        for r in due {
            if !self.bark_channel_available() {
                break;
            }
            if self.scene_of_actor(r).is_some() {
                continue;
            }
            let wait = flee_dialogue_interval(min, max, self.rand());
            if let Some(c) = self.actor_mut(r).and_then(|a| a.combat.as_mut()) {
                c.flee_dialogue = wait;
            }
            self.bark(r, b"FLEE");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Combat, Fighter, GroupStrengths, flee_dialogue_interval, group_threat_ratio};
    use crate::engine::PLAYER_REF;
    use esp::FormId;

    #[test]
    fn group_contributions_refresh_independently_of_personal_strength() {
        let (a, b) = (FormId(1), FormId(2));
        let mut groups = GroupStrengths::default();
        assert_eq!(groups.due(&[a, b], 0.0, 2.0), [a, b]);
        groups.store(a, 100.0);
        groups.store(b, 200.0);
        // Personal strengths may already have refreshed at 1 s. Others still
        // contribute their last group sample, but the actor counts its own
        // personal strength, including when it is fleeing itself.
        let own = 80.0;
        assert!(groups.due(&[a, b], 1.0, 2.0).is_empty());
        let fighters = |groups: &GroupStrengths| {
            [
                Fighter {
                    id: a,
                    target: Some(b),
                    strength: 0.0, // Fleeing masks a's contribution to others.
                },
                Fighter {
                    id: b,
                    target: Some(a),
                    strength: groups.strength(b),
                },
            ]
        };
        assert_eq!(group_threat_ratio(a, own, &fighters(&groups)), 80.0 / 200.0);
        assert_eq!(groups.due(&[a, b], 1.0, 2.0), [a, b]);
        // Sampling the current estimate (40) must not reuse a stale personal
        // cache (say, 70) when the group's timer expires first.
        groups.store(a, 60.0);
        groups.store(b, 40.0);
        assert_eq!(group_threat_ratio(a, own, &fighters(&groups)), 2.0);
        assert_eq!(groups.strength(a), 60.0); // Fleeing did not stop its clock.
        assert_eq!(
            group_threat_ratio(b, 70.0, &fighters(&groups)),
            f32::INFINITY
        );
    }

    #[test]
    fn group_membership_drops_departed_members_and_resamples_reentry() {
        let a = FormId(1);
        let mut groups = GroupStrengths::default();
        assert_eq!(groups.due(&[a, PLAYER_REF], 0.0, 2.0), [a, PLAYER_REF]);
        groups.store(a, 100.0);
        groups.store(PLAYER_REF, 300.0);
        assert!(groups.due(&[a, PLAYER_REF], 1.0, 2.0).is_empty());
        assert_eq!(groups.strength(PLAYER_REF), 300.0);
        // The engine excludes dead, bleeding, unloaded and departed fighters.
        // A player no longer targeted by anyone is removed the same way.
        assert!(groups.due(&[], 0.0, 2.0).is_empty());
        assert_eq!(groups.strength(a), 0.0);
        assert_eq!(groups.strength(PLAYER_REF), 0.0);
        assert_eq!(groups.due(&[a, PLAYER_REF], 0.0, 2.0), [a, PLAYER_REF]);
        groups.store(a, 50.0);
        groups.store(PLAYER_REF, 90.0);
        assert_eq!(groups.strength(PLAYER_REF), 90.0);
        // Starting or switching fights invalidates the actor's old sample,
        // even if no update observed it outside combat.
        groups.remove(a);
        assert_eq!(groups.due(&[a, PLAYER_REF], 0.0, 2.0), [a]);
        assert_eq!(groups.strength(a), 0.0);
        assert_eq!(groups.strength(PLAYER_REF), 90.0);
    }

    #[test]
    fn group_timer_pauses_and_uses_the_setting_at_each_refresh() {
        let a = FormId(1);
        let mut groups = GroupStrengths::default();
        assert_eq!(groups.due(&[a], 0.0, 2.0), [a]);
        groups.store(a, 100.0);
        assert!(groups.due(&[a], 0.0, 2.0).is_empty());
        assert!(groups.due(&[a], -1.0, 2.0).is_empty());
        assert!(groups.due(&[a], f32::NAN, 2.0).is_empty());
        // A changed interval applies after the current wait, not retroactively.
        assert!(groups.due(&[a], 1.0, 0.5).is_empty());
        assert_eq!(groups.due(&[a], 1.0, 0.5), [a]);
        assert!(groups.due(&[a], 0.25, 0.5).is_empty());
        assert_eq!(groups.due(&[a], 0.25, 0.5), [a]);
        // A large frame samples once and starts a new full interval.
        assert_eq!(groups.due(&[a], 30.0, 0.5), [a]);
        assert!(groups.due(&[a], 0.0, 0.5).is_empty());
        assert_eq!(groups.due(&[a], 0.5, -1.0), [a]);
        assert!(groups.due(&[a], 0.0, 0.0).is_empty());
        assert_eq!(groups.due(&[a], 0.1, 0.0), [a]);
        assert_eq!(groups.due(&[a], 0.1, f32::INFINITY), [a]);
        assert!(groups.due(&[a], 1.0, f32::NAN).is_empty());
        assert_eq!(groups.due(&[a], 1.0, f32::NAN), [a]);
        assert_eq!(groups.strength(a), 100.0); // Due alone cannot change a sample.
    }

    #[test]
    fn flee_dialogue_repeats_independently_of_threat_checks() {
        let mut c = Combat::new(FormId(1));
        c.threat_check = 100.0;
        assert!(!c.flee_dialogue_due(1.0));
        c.fleeing = true;
        assert!(c.flee_dialogue_due(0.0));
        c.flee_dialogue = flee_dialogue_interval(3.0, 5.0, 5_000);
        assert_eq!(c.flee_dialogue, 4.0);
        assert!(!c.flee_dialogue_due(3.0));
        assert!(!c.flee_dialogue_due(0.0)); // A paused frame does not consume time.
        assert!(c.flee_dialogue_due(1.0));
        assert_eq!(c.threat_check, 100.0);
        // A blocked channel does not restart the timer; one large update does
        // not accumulate several attempts for the next frame.
        assert!(c.flee_dialogue_due(30.0));
        assert_eq!(c.flee_dialogue, 0.0);
        c.flee_dialogue = 5.0;
        c.fleeing = false;
        assert!(!c.flee_dialogue_due(0.0));
        c.fleeing = true;
        assert!(c.flee_dialogue_due(0.0));
        assert_eq!(Combat::new(FormId(2)).flee_dialogue, 0.0);
    }

    #[test]
    fn flee_dialogue_settings_bound_the_interval() {
        assert_eq!(flee_dialogue_interval(7.0, 7.0, 9_999), 7.0);
        assert_eq!(flee_dialogue_interval(3.0, 5.0, 0), 3.0);
        assert!((3.0..5.0).contains(&flee_dialogue_interval(3.0, 5.0, 9_999)));
        assert_eq!(flee_dialogue_interval(8.0, 2.0, 5_000), 8.0);
        assert_eq!(flee_dialogue_interval(-1.0, -2.0, 5_000), 0.0);
        assert_eq!(flee_dialogue_interval(f32::NAN, f32::INFINITY, 5_000), 4.0);
    }

    #[test]
    fn sides_sum_their_strength() {
        let (a, b, c, p) = (FormId(1), FormId(2), FormId(3), FormId(0x14));
        // a and b fight the player; c fights a (so c is on the player's side).
        let fighters = [
            Fighter {
                id: a,
                target: Some(p),
                strength: 100.0,
            },
            Fighter {
                id: b,
                target: Some(p),
                strength: 300.0,
            },
            Fighter {
                id: c,
                target: Some(a),
                strength: 200.0,
            },
            Fighter {
                id: p,
                target: None,
                strength: 700.0,
            },
        ];
        assert_eq!(group_threat_ratio(a, 100.0, &fighters), 400.0 / 900.0);
        assert_eq!(group_threat_ratio(c, 200.0, &fighters), 900.0 / 400.0);
        // A fleeing fighter adds nothing to its side, but counts itself.
        let mut fleeing = fighters;
        fleeing[0].strength = 0.0;
        assert_eq!(group_threat_ratio(b, 300.0, &fleeing), 300.0 / 900.0);
        assert_eq!(group_threat_ratio(a, 100.0, &fleeing), 400.0 / 900.0);
    }
}
