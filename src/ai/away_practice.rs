//! Logical practice away: counts and clocks, without animation, resource use or hits.
use super::package::{UseMagic, UseWeapon};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Progress {
    pub seed: u64,
    pub magic: bool,
    pub elapsed: f64,
    pub credit: f64,
    pub wait: f64,
    pub magic_left: Option<u32>,
    pub attacks_left: u32,
    pub barrages: u32,
    pub target: Option<u32>,
    /// The wait ends an action; otherwise it ends recovery/cooldown.
    pub action: bool,
    pub finished: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum Settings {
    Magic {
        inputs: UseMagic,
        charge: f32,
        concentration: bool,
    },
    Weapon {
        inputs: UseWeapon,
        bow: bool,
    },
}

impl Progress {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            seed: seed | 1,
            ..Default::default()
        }
    }
    fn random(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    fn number(&mut self, range: (u32, u32)) -> u32 {
        let lo = range.0.min(range.1);
        let hi = range.0.max(range.1);
        lo + (self.random() % (u64::from(hi) - u64::from(lo) + 1)) as u32
    }
    fn seconds(&mut self, range: (f32, f32)) -> f64 {
        let lo = if range.0.is_finite() {
            range.0.max(0.0) as f64
        } else {
            0.0
        };
        let hi = if range.1.is_finite() {
            range.1.max(0.0) as f64
        } else {
            0.0
        };
        lo.min(hi) + (self.random() & 0xFFFF_FFFF) as f64 / u32::MAX as f64 * (hi - lo).abs()
    }
    pub(crate) fn advance(&mut self, dt: f64, settings: Settings) {
        if self.finished || !dt.is_finite() || dt < 0.0 {
            return;
        }
        self.magic = matches!(settings, Settings::Magic { .. });
        self.elapsed += dt;
        self.credit += dt;
        if let Settings::Weapon { inputs, .. } = settings {
            if inputs.max_time > 0.0 && self.elapsed >= inputs.max_time as f64 {
                self.finished = true;
                return;
            }
            if inputs.aim_only {
                self.credit = 0.0;
                return;
            }
        }
        // Large menu advances may span many casts. Retain unspent time if a
        // deliberately extreme package exhausts the per-step work budget.
        for _ in 0..4096 {
            if self.wait > self.credit {
                self.wait -= self.credit;
                self.credit = 0.0;
                return;
            }
            self.credit -= self.wait;
            self.wait = 0.0;
            match settings {
                Settings::Magic {
                    inputs,
                    charge,
                    concentration,
                } => {
                    let left = match self.magic_left {
                        Some(n) => n,
                        None => {
                            let n = self.number(inputs.casts).max(1);
                            self.magic_left = Some(n);
                            n
                        }
                    };
                    if self.action {
                        self.magic_left = Some(left.saturating_sub(1));
                        self.action = false;
                        if left <= 1 {
                            self.finished = true;
                            return;
                        }
                        self.wait = self.seconds(inputs.cooldown);
                    } else {
                        self.action = true;
                        self.wait = if charge.is_finite() {
                            charge.max(0.0) as f64
                        } else {
                            0.0
                        };
                        if concentration {
                            self.wait += self
                                .seconds(inputs.cast_time)
                                .max(super::casters::CHANNEL_TICK as f64);
                        }
                    }
                }
                Settings::Weapon { inputs, bow } => {
                    if self.action {
                        self.attacks_left = self.attacks_left.saturating_sub(1);
                        self.action = false;
                        if self.attacks_left == 0 {
                            self.barrages = self.barrages.saturating_add(1);
                            if !inputs.never_end
                                && inputs.barrages > 0
                                && self.barrages >= inputs.barrages
                            {
                                self.finished = true;
                                return;
                            }
                        }
                        self.wait = self.seconds(if bow { (0.6, 1.6) } else { (1.2, 2.2) });
                        if self.attacks_left == 0 {
                            self.wait += self.seconds(inputs.pause);
                        }
                    } else {
                        if self.attacks_left == 0 {
                            self.attacks_left = self.number(inputs.attacks).max(1);
                        }
                        self.action = true;
                        self.wait = if bow {
                            super::archery::DRAW_TIME as f64 + self.seconds((0.4, 1.2)) + 0.8
                        } else {
                            0.0
                        };
                    }
                }
            }
        }
    }
}

impl crate::engine::Engine {
    pub(crate) fn away_practice_settings(&self, actor: esp::FormId) -> Option<Settings> {
        use super::package::{Behaviour, WeaponKind};
        let Some(x) = self.away.get(&actor).filter(|x| {
            x.settled && x.combat.is_none() && x.bleeding.is_none() && !x.run_completed()
        }) else {
            return None;
        };
        let Some(base) = x.current.and_then(|i| x.packages.get(i)) else {
            return None;
        };
        let p = x.effective(base);
        let settings = match p.behaviour {
            Behaviour::UseMagic => {
                let Some(inputs) = p.use_magic else {
                    return None;
                };
                let Some(spell) = inputs.spell.and_then(|id| self.practice_spell(id)) else {
                    return None;
                };
                Settings::Magic {
                    inputs,
                    charge: spell.charge,
                    concentration: spell.concentration,
                }
            }
            Behaviour::UseWeapon => {
                let Some(inputs) = p.use_weapon else {
                    return None;
                };
                let eligible =
                    inputs
                        .targets
                        .iter()
                        .zip(inputs.triggers)
                        .any(|(&target, trigger)| {
                            let Some(target) = self
                                .package_target_of(actor, &p, target)
                                .and_then(|r| self.ref_position(r))
                            else {
                                return false;
                            };
                            trigger.is_none_or(|t| {
                                self.package_target_of(actor, &p, Some(t.reference))
                                    .and_then(|r| self.ref_position(r))
                                    .is_some_and(|origin| origin.distance(target) <= t.radius)
                            })
                        });
                if !eligible {
                    return None;
                }
                let carried = self
                    .inventories
                    .get(&actor)
                    .and_then(|i| i.weapon(&self.lo));
                let bow_form = |id| {
                    self.lo
                        .get(id)
                        .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
                        .is_some_and(|kind| matches!(kind, 7 | 9))
                };
                let bow = match inputs.weapon {
                    WeaponKind::Ranged => true,
                    WeaponKind::Specific(id) => bow_form(id),
                    WeaponKind::Any => carried.is_some_and(bow_form),
                    WeaponKind::Melee => false,
                };
                Settings::Weapon { inputs, bow }
            }
            _ => return None,
        };
        Some(settings)
    }

    pub(crate) fn advance_away_practice(&mut self, actor: esp::FormId, dt: f32) {
        let Some(settings) = self.away_practice_settings(actor) else {
            return;
        };
        let Some(x) = self.away.get_mut(&actor) else {
            return;
        };
        let Some(run) = x.run.as_mut() else {
            return;
        };
        let started = run.practice.is_none();
        let progress = run
            .practice
            .get_or_insert_with(|| Progress::new(actor.0 as u64 ^ run.started_at.to_bits()));
        progress.advance(if started { 0.0 } else { dt as f64 }, settings);
        if progress.finished {
            x.complete_procedure(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::package::{Target, WeaponKind};
    use super::*;
    fn magic() -> Settings {
        Settings::Magic {
            inputs: UseMagic {
                spell: None,
                target: None,
                hold_when_blocked: false,
                cast_time: (0., 0.),
                cooldown: (1., 1.),
                casts: (3, 3),
                dual: false,
            },
            charge: 2.,
            concentration: false,
        }
    }
    fn weapon() -> UseWeapon {
        UseWeapon {
            weapon: WeaponKind::Melee,
            targets: [Some(Target::SelfRef), None, None],
            triggers: [None; 3],
            aim_only: false,
            always_power_attack: false,
            pause: (4., 4.),
            attacks: (2, 2),
            never_end: false,
            barrages: 3,
            max_time: 0.,
        }
    }
    #[test]
    fn finite_magic_counts_charge_and_cooldown_without_a_final_cooldown() {
        let mut p = Progress::new(1);
        p.advance(0., magic());
        p.advance(7., magic());
        assert_eq!(p.magic_left, Some(1));
        assert!(!p.finished);
        p.advance(1., magic());
        assert!(p.finished);
    }
    #[test]
    fn large_and_small_advances_reach_the_same_weapon_progress() {
        let s = Settings::Weapon {
            inputs: weapon(),
            bow: true,
        };
        let mut whole = Progress::new(42);
        let mut slices = whole;
        whole.advance(15., s);
        for _ in 0..60 {
            slices.advance(0.25, s);
        }
        assert_eq!(
            (whole.barrages, whole.attacks_left, whole.seed, whole.action),
            (
                slices.barrages,
                slices.attacks_left,
                slices.seed,
                slices.action
            )
        );
        assert!((whole.wait - slices.wait).abs() < 1e-9);
        whole.advance(100., s);
        assert!(whole.finished);
        assert_eq!(whole.barrages, 3);
    }
    #[test]
    fn aim_only_and_never_end_keep_their_loaded_completion_rules() {
        let mut inputs = weapon();
        inputs.aim_only = true;
        let mut p = Progress::new(1);
        p.advance(100., Settings::Weapon { inputs, bow: false });
        assert!(!p.finished);
        assert_eq!(p.barrages, 0);
        inputs.max_time = 10.;
        p.advance(0., Settings::Weapon { inputs, bow: false });
        assert!(p.finished);
        inputs.aim_only = false;
        inputs.max_time = 0.;
        inputs.never_end = true;
        let mut p = Progress::new(1);
        p.advance(100., Settings::Weapon { inputs, bow: false });
        assert!(!p.finished);
        assert!(p.barrages > 3);
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_away_finite_weapon_package_ends_without_a_projectile_or_resource_use() {
        use super::super::{
            exec::{
                ActorExec,
                tests::{ACTOR, CELL, MARKER, PACKAGE, fixture},
            },
            package::{Behaviour, Location, LocationKind, Package},
            schedule::Place,
        };
        let (_f, mut e) = fixture();
        let mut p = Package::bare(PACKAGE, Behaviour::UseWeapon);
        let mut inputs = weapon();
        inputs.targets = [Some(Target::Ref(MARKER)), None, None];
        p.use_weapon = Some(inputs);
        p.location = Some(Location {
            kind: LocationKind::NearReference(MARKER),
            radius: 0.0,
        });
        let mut x = ActorExec::new(vec![p], glam::Vec3::new(3000., 0., 0.), 0.0, 0.0);
        x.current = Some(0);
        x.place = Some(Place::Interior(CELL));
        x.settled = true;
        x.begin_run(Some(0), 0.0, false);
        e.away.insert(ACTOR, x);
        let magicka = e.actor_value(ACTOR, esp::actor_value::MAGICKA);
        let health = e.away[&ACTOR].health;
        let projectiles = e.projectiles.len();
        e.advance_away_practice(ACTOR, 0.0);
        e.advance_away_practice(ACTOR, 100.0);
        assert!(e.away[&ACTOR].run_completed());
        assert_eq!(
            e.away[&ACTOR]
                .run
                .as_ref()
                .unwrap()
                .practice
                .unwrap()
                .barrages,
            3
        );
        assert_eq!(e.away[&ACTOR].health, health);
        assert_eq!(e.actor_value(ACTOR, esp::actor_value::MAGICKA), magicka);
        assert_eq!(e.projectiles.len(), projectiles);
    }

    #[test]
    #[ignore = "needs installed Skyrim SE masters and a headless GPU adapter"]
    fn the_vanilla_shimmermist_single_cast_completes_away_without_firing() {
        use super::super::{exec::ActorExec, package, procedure::Cursor, schedule::Place};
        let data = vfs::locate_data_dir().expect("installed Skyrim SE Data");
        let names = esp::BASE_MASTERS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
        let p =
            e.lo.ids_of_type(b"PACK")
                .iter()
                .filter_map(|&id| {
                    let r = e.lo.get(id)?;
                    r.editor_id()?
                        .to_ascii_lowercase()
                        .contains("shimmermist")
                        .then(|| package::parse(&e.lo, id))
                        .flatten()
                })
                .find(|p| p.use_magic.is_some_and(|u| u.casts == (1, 1)))
                .expect("Shimmermist boss single-cast package");
        let actor = esp::FormId(0xFF00_0900);
        let mut cursor = Cursor {
            tree_key: p.editor_id.clone(),
            ..Default::default()
        };
        cursor.step(p.tree.as_ref().unwrap(), 0.0, true, |_| true, || 1);
        let mut x = ActorExec::new(vec![p], glam::Vec3::ZERO, 0.0, 0.0);
        x.current = Some(0);
        x.place = Some(Place::Interior(esp::FormId(0x133C9)));
        x.settled = true;
        x.begin_run(Some(0), 0.0, false);
        x.run.as_mut().unwrap().cursor = Some(cursor);
        e.away.insert(actor, x);
        let magicka = e.actor_value(actor, esp::actor_value::MAGICKA);
        let projectiles = e.projectiles.len();
        e.advance_away_practice(actor, 0.0);
        e.advance_away_practice(actor, 1000.0);
        assert!(e.away[&actor].run_completed());
        assert_eq!(e.actor_value(actor, esp::actor_value::MAGICKA), magicka);
        assert_eq!(e.projectiles.len(), projectiles);
    }
}
