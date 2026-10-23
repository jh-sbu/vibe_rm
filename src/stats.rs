//! The player's statistics (`Game.IncrementStat` / `QueryStat`, the
//! journal's Stats page) and the calendar the journal dates its bottom bar
//! with.
//!
//! Which page category each statistic is listed under, and the calendar's
//! weekday on the first day, aren't published: see `known_ambiguities/journal.md`.

use std::collections::HashMap;
use std::hash::Hash;

use esp::FormId;

use crate::engine::Engine;

/// The Stats page's categories (`StatsPage`'s list order).
pub const GENERAL: u8 = 0;
pub const QUEST: u8 = 1;
pub const COMBAT: u8 = 2;
pub const MAGIC: u8 = 3;
pub const CRAFTING: u8 = 4;
pub const CRIME: u8 = 5;

/// Every statistic the Stats page lists, in order, with its category. The
/// names are the game's (`$Name` in the interface translations).
pub const STATS: &[(&str, u8)] = &[
    ("Locations Discovered", GENERAL),
    ("Dungeons Cleared", GENERAL),
    ("Days Passed", GENERAL),
    ("Hours Slept", GENERAL),
    ("Hours Waiting", GENERAL),
    ("Standing Stones Found", GENERAL),
    ("Gold Found", GENERAL),
    ("Most Gold Carried", GENERAL),
    ("Chests Looted", GENERAL),
    ("Skill Increases", GENERAL),
    ("Skill Books Read", GENERAL),
    ("Food Eaten", GENERAL),
    ("Training Sessions", GENERAL),
    ("Books Read", GENERAL),
    ("Horses Owned", GENERAL),
    ("Houses Owned", GENERAL),
    ("Stores Invested In", GENERAL),
    ("Barters", GENERAL),
    ("Persuasions", GENERAL),
    ("Bribes", GENERAL),
    ("Intimidations", GENERAL),
    ("Diseases Contracted", GENERAL),
    ("Days as a Vampire", GENERAL),
    ("Days as a Werewolf", GENERAL),
    ("Necks Bitten", GENERAL),
    ("Vampirism Cures", GENERAL),
    ("Werewolf Transformations", GENERAL),
    ("Mauls", GENERAL),
    ("Quests Completed", QUEST),
    ("Misc Objectives Completed", QUEST),
    ("Main Quests Completed", QUEST),
    ("Side Quests Completed", QUEST),
    ("The Companions Quests Completed", QUEST),
    ("College of Winterhold Quests Completed", QUEST),
    ("Thieves' Guild Quests Completed", QUEST),
    ("The Dark Brotherhood Quests Completed", QUEST),
    ("Civil War Quests Completed", QUEST),
    ("Daedric Quests Completed", QUEST),
    ("Dawnguard Quests Completed", QUEST),
    ("Dragonborn Quests Completed", QUEST),
    ("Questlines Completed", QUEST),
    ("People Killed", COMBAT),
    ("Animals Killed", COMBAT),
    ("Creatures Killed", COMBAT),
    ("Undead Killed", COMBAT),
    ("Daedra Killed", COMBAT),
    ("Automatons Killed", COMBAT),
    ("Dragons Killed", COMBAT),
    ("Favorite Weapon", COMBAT),
    ("Critical Strikes", COMBAT),
    ("Sneak Attacks", COMBAT),
    ("Backstabs", COMBAT),
    ("Weapons Disarmed", COMBAT),
    ("Brawls Won", COMBAT),
    ("Bunnies Slaughtered", COMBAT),
    ("Spells Learned", MAGIC),
    ("Favorite Spell", MAGIC),
    ("Favorite School", MAGIC),
    ("Dragon Souls Collected", MAGIC),
    ("Words Of Power Learned", MAGIC),
    ("Words Of Power Unlocked", MAGIC),
    ("Shouts Learned", MAGIC),
    ("Shouts Unlocked", MAGIC),
    ("Shouts Mastered", MAGIC),
    ("Times Shouted", MAGIC),
    ("Favorite Shout", MAGIC),
    ("Soul Gems Used", CRAFTING),
    ("Souls Trapped", CRAFTING),
    ("Magic Items Made", CRAFTING),
    ("Weapons Improved", CRAFTING),
    ("Weapons Made", CRAFTING),
    ("Armor Improved", CRAFTING),
    ("Armor Made", CRAFTING),
    ("Potions Mixed", CRAFTING),
    ("Potions Used", CRAFTING),
    ("Poisons Mixed", CRAFTING),
    ("Poisons Used", CRAFTING),
    ("Ingredients Harvested", CRAFTING),
    ("Ingredients Eaten", CRAFTING),
    ("Nirnroots Found", CRAFTING),
    ("Wings Plucked", CRAFTING),
    ("Total Lifetime Bounty", CRIME),
    ("Largest Bounty", CRIME),
    ("Locks Picked", CRIME),
    ("Pockets Picked", CRIME),
    ("Items Pickpocketed", CRIME),
    ("Times Jailed", CRIME),
    ("Days Jailed", CRIME),
    ("Fines Paid", CRIME),
    ("Jail Escapes", CRIME),
    ("Items Stolen", CRIME),
    ("Assaults", CRIME),
    ("Murders", CRIME),
    ("Horses Stolen", CRIME),
    ("Trespasses", CRIME),
    ("Eastmarch Bounty", CRIME),
    ("Falkreath Bounty", CRIME),
    ("Haafingar Bounty", CRIME),
    ("Hjaalmarch Bounty", CRIME),
    ("The Pale Bounty", CRIME),
    ("The Reach Bounty", CRIME),
    ("The Rift Bounty", CRIME),
    ("Tribal Orcs Bounty", CRIME),
    ("Whiterun Bounty", CRIME),
    ("Winterhold Bounty", CRIME),
];

/// The favorites the page shows by name.
const NAMED: &[&str] = &[
    "Favorite Weapon",
    "Favorite Spell",
    "Favorite School",
    "Favorite Shout",
];

/// The holds' bounty statistics and their crime factions.
const HOLD_BOUNTIES: &[(&str, &str)] = &[
    ("Eastmarch Bounty", "CrimeFactionEastmarch"),
    ("Falkreath Bounty", "CrimeFactionFalkreath"),
    ("Haafingar Bounty", "CrimeFactionHaafingar"),
    ("Hjaalmarch Bounty", "CrimeFactionHjaalmarch"),
    ("The Pale Bounty", "CrimeFactionPale"),
    ("The Reach Bounty", "CrimeFactionReach"),
    ("The Rift Bounty", "CrimeFactionRift"),
    ("Tribal Orcs Bounty", "CrimeFactionOrcs"),
    ("Whiterun Bounty", "CrimeFactionWhiterun"),
    ("Winterhold Bounty", "CrimeFactionWinterhold"),
];

/// The months (`GameMonth` 0 to 11) and their days.
const MONTHS: [(&str, u32); 12] = [
    ("Morning Star", 31),
    ("Sun's Dawn", 28),
    ("First Seed", 31),
    ("Rain's Hand", 30),
    ("Second Seed", 31),
    ("Midyear", 30),
    ("Sun's Height", 31),
    ("Last Seed", 31),
    ("Hearthfire", 30),
    ("Frostfall", 31),
    ("Sun's Dusk", 30),
    ("Evening Star", 31),
];

const WEEKDAYS: [&str; 7] = [
    "Sundas", "Morndas", "Tirdas", "Middas", "Turdas", "Fredas", "Loredas",
];

/// A statistic's value as the page shows it: a count, or a name.
#[derive(Debug, Clone, PartialEq)]
pub enum StatValue {
    Count(i32),
    Name(String),
}

/// Usage favorites are independent of the player's manually favorited items.
/// Ties keep the incumbent; native weighting and ties are unconfirmed (journal.md).
#[derive(Default)]
pub struct MagicUsage {
    spells: Usage<FormId>,
    schools: Usage<u32>,
    shouts: Usage<FormId>,
}

/// Stable native-save counts and incumbent. Keys are raw FormIDs for spell/
/// shout histories and actor-value indices for school history.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SavedUsage {
    pub counts: Vec<(u32, u64)>,
    pub favorite: Option<u32>,
}
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SavedMagicUsage {
    pub spells: SavedUsage,
    pub schools: SavedUsage,
    pub shouts: SavedUsage,
}
impl MagicUsage {
    pub(crate) fn saved(&self) -> SavedMagicUsage {
        SavedMagicUsage {
            spells: self.spells.saved_with(|id| id.0),
            schools: self.schools.saved_with(|id| id),
            shouts: self.shouts.saved_with(|id| id.0),
        }
    }
    pub(crate) fn restore(saved: &SavedMagicUsage) -> anyhow::Result<Self> {
        Ok(Self {
            spells: Usage::restore_with(&saved.spells, |id| id != 0, FormId)?,
            schools: Usage::restore_with(
                &saved.schools,
                |id| crate::spells::SCHOOLS.contains(&id),
                |id| id,
            )?,
            shouts: Usage::restore_with(&saved.shouts, |id| id != 0, FormId)?,
        })
    }
}
impl<T: Copy + Eq + Hash> Usage<T> {
    fn saved_with(&self, raw: impl Fn(T) -> u32) -> SavedUsage {
        let mut counts: Vec<_> = self.counts.iter().map(|(&id, &n)| (raw(id), n)).collect();
        counts.sort_by_key(|(id, _)| *id);
        SavedUsage {
            counts,
            favorite: self.favorite.map(raw),
        }
    }
    fn restore_with(
        saved: &SavedUsage,
        valid: impl Fn(u32) -> bool,
        key: impl Fn(u32) -> T,
    ) -> anyhow::Result<Self> {
        let mut counts = HashMap::new();
        let mut previous = None;
        for &(id, count) in &saved.counts {
            anyhow::ensure!(valid(id) && count > 0, "invalid magic usage key/count");
            anyhow::ensure!(
                previous.is_none_or(|p| p < id),
                "duplicate or unsorted magic usage key"
            );
            previous = Some(id);
            counts.insert(key(id), count);
        }
        let favorite = saved.favorite.map(&key);
        if counts.is_empty() {
            anyhow::ensure!(favorite.is_none(), "magic favorite without counts");
        } else {
            let top = counts.values().copied().max().unwrap();
            anyhow::ensure!(
                favorite.is_some_and(|id| counts.get(&id) == Some(&top)),
                "magic favorite is not a maximum"
            );
        }
        Ok(Self { counts, favorite })
    }
}

/// Counts of player melee attacks and bow releases by weapon base form.
#[derive(Default)]
pub struct WeaponUsage {
    usage: Usage<FormId>,
}

impl WeaponUsage {
    /// Each eligible held weapon participates once in an accepted attack.
    pub(crate) fn record(&mut self, weapon: FormId) {
        self.usage.record(weapon);
    }

    /// Stable native-save representation.
    pub(crate) fn saved(&self) -> (Vec<(FormId, u64)>, Option<FormId>) {
        let mut counts: Vec<_> = self.usage.counts.iter().map(|(&id, &n)| (id, n)).collect();
        counts.sort_by_key(|(id, _)| *id);
        (counts, self.usage.favorite)
    }

    /// Restore counts after structural validation, retaining the persisted
    /// incumbent on a tied maximum.
    pub(crate) fn restore(
        counts: Vec<(FormId, u64)>,
        favorite: Option<FormId>,
    ) -> anyhow::Result<Self> {
        let mut usage = Usage::default();
        let mut previous = None;
        for (id, count) in counts {
            anyhow::ensure!(id != FormId::NULL, "null favorite-weapon form");
            anyhow::ensure!(count > 0, "zero favorite-weapon usage count");
            anyhow::ensure!(
                previous.is_none_or(|p| p < id),
                "duplicate or unsorted favorite-weapon form"
            );
            previous = Some(id);
            usage.counts.insert(id, count);
        }
        if usage.counts.is_empty() {
            anyhow::ensure!(favorite.is_none(), "favorite weapon without usage counts");
        } else {
            let top = usage.counts.values().copied().max().unwrap();
            anyhow::ensure!(
                favorite.is_some_and(|id| usage.counts.get(&id) == Some(&top)),
                "favorite weapon does not have the highest usage count"
            );
            usage.favorite = favorite;
        }
        Ok(Self { usage })
    }

    pub(crate) fn valid_form(lo: &esp::LoadOrder, id: FormId) -> bool {
        let Some(rec) = lo.get(id).filter(|r| r.tag().0 == *b"WEAP") else {
            return false;
        };
        let Some(kind) = rec.get(b"DNAM").and_then(|d| d.first()).copied() else {
            return false;
        };
        (1..=6).contains(&kind) || crate::ai::archery::is_bow(lo, id)
    }
}

#[derive(Default)]
struct Usage<T> {
    counts: HashMap<T, u64>,
    favorite: Option<T>,
}

impl<T: Copy + Eq + Hash> Usage<T> {
    fn record(&mut self, id: T) {
        let count = self.counts.entry(id).or_default();
        *count = count.saturating_add(1);
        if self
            .favorite
            .is_none_or(|f| self.counts[&id] > self.counts[&f])
        {
            self.favorite = Some(id);
        }
    }
}

/// 1st, 2nd, 3rd, 4th ... 11th, 12th, 13th, 21st.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

impl Engine {
    /// One paid hand cast (a concentration channel counts at its first delivery),
    /// or one explicit Spell.Cast/RemoteCast. Landing effects never count here.
    pub(crate) fn count_spell_cast(&mut self, caster: FormId, spell: FormId) {
        if caster != crate::engine::PLAYER_REF
            || !self.lo.tag_of(spell).is_some_and(|t| t.0 == *b"SPEL")
            || self
                .magic_item(spell)
                .is_none_or(|m| m.spell_type != crate::magic::spell_type::SPELL)
        {
            return;
        }
        let school = self.spell_school(spell);
        self.scripts.magic_usage.spells.record(spell);
        if let Some(school) = school {
            self.scripts.magic_usage.schools.record(school);
        }
    }

    pub(crate) fn count_shout_use(&mut self, shout: FormId) {
        self.scripts.magic_usage.shouts.record(shout);
    }

    /// Record only actual Skyrim melee weapons and bows. This also keeps
    /// staves, fists and malformed equipment out of the journal statistic.
    pub(crate) fn count_weapon_use(&mut self, weapon: FormId) {
        if WeaponUsage::valid_form(&self.lo, weapon) {
            self.scripts.weapon_usage.record(weapon);
        }
    }

    /// `Game.IncrementStat`.
    pub fn increment_stat(&mut self, name: &str, by: i32) {
        *self.scripts.stats.entry(name.to_string()).or_default() += by;
    }

    /// An actor the player killed, counted by what it is: the first of its
    /// actor type keywords in this order (a vampire is a person, a hare an
    /// animal and a bunny).
    pub(crate) fn count_kill(&mut self, actor: FormId) {
        const KINDS: [(&str, &str); 7] = [
            ("ActorTypeDragon", "Dragons Killed"),
            ("ActorTypeNPC", "People Killed"),
            ("ActorTypeDaedra", "Daedra Killed"),
            ("ActorTypeDwarven", "Automatons Killed"),
            ("ActorTypeUndead", "Undead Killed"),
            ("ActorTypeAnimal", "Animals Killed"),
            ("ActorTypeCreature", "Creatures Killed"),
        ];
        let kind = KINDS.iter().find(|(kw, _)| {
            self.lo
                .find_editor_id(kw)
                .is_some_and(|k| self.has_keyword(actor, k))
        });
        if let Some((_, stat)) = kind {
            self.increment_stat(stat, 1);
        }
        let hare = self.lo.find_editor_id("HareRace");
        if hare.is_some() && self.race_of(actor) == hare {
            self.increment_stat("Bunnies Slaughtered", 1);
        }
    }

    /// A statistic only goes up to `n` (the most gold carried, the largest
    /// bounty).
    pub(crate) fn raise_stat_to(&mut self, name: &str, n: i32) {
        let v = self.scripts.stats.entry(name.to_string()).or_default();
        *v = (*v).max(n);
    }

    /// `Game.QueryStat`: a counted statistic (names give 0).
    pub fn query_stat(&self, name: &str) -> i32 {
        match self.stat(name) {
            StatValue::Count(n) => n,
            StatValue::Name(_) => 0,
        }
    }

    /// A statistic: those the engine keeps itself (the days passed), else
    /// the count.
    pub fn stat(&self, name: &str) -> StatValue {
        if name.eq_ignore_ascii_case("Days Passed") {
            return StatValue::Count(self.day as i32);
        }
        // Every bounty ever added is the infamy, with every faction.
        if name.eq_ignore_ascii_case("Total Lifetime Bounty") {
            let infamy = self.crime.bounties.values();
            return StatValue::Count(infamy.map(|b| b.infamy_violent + b.infamy_nonviolent).sum());
        }
        // The holds' bounties now.
        if let Some(&(_, faction)) = HOLD_BOUNTIES
            .iter()
            .find(|(stat, _)| stat.eq_ignore_ascii_case(name))
        {
            let gold = self
                .lo
                .find_editor_id(faction)
                .map_or(0, |f| self.bounty(f).total());
            return StatValue::Count(gold);
        }
        if NAMED.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            let usage = &self.scripts.magic_usage;
            let value = if name.eq_ignore_ascii_case("Favorite Spell") {
                usage
                    .spells
                    .favorite
                    .and_then(|id| self.magic_item(id).map(|m| m.name.clone()))
            } else if name.eq_ignore_ascii_case("Favorite School") {
                usage.schools.favorite.map(|skill| self.skill_name(skill))
            } else if name.eq_ignore_ascii_case("Favorite Shout") {
                usage
                    .shouts
                    .favorite
                    .and_then(|id| self.shout(id).map(|s| s.name.clone()))
            } else if name.eq_ignore_ascii_case("Favorite Weapon") {
                self.scripts
                    .weapon_usage
                    .usage
                    .favorite
                    .map(|id| self.form_name(id))
            } else {
                None
            };
            return StatValue::Name(value.unwrap_or_default());
        }
        let count = self
            .scripts
            .stats
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map_or(0, |(_, &v)| v);
        StatValue::Count(count)
    }

    /// The date the game started on: the `GameDay`, `GameMonth` and
    /// `GameYear` globals' own values (17 Last Seed 201).
    fn start_date(&self) -> (u32, u32, u32) {
        let global = |edid: &str, default: f32| {
            self.lo
                .find_editor_id(edid)
                .and_then(|g| self.lo.get(g))
                .and_then(|r| {
                    r.get(b"FLTV")
                        .map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap()))
                })
                .unwrap_or(default)
        };
        (
            global("GameDay", 17.0).max(1.0) as u32,
            global("GameMonth", 7.0).clamp(0.0, 11.0) as u32,
            global("GameYear", 201.0).max(0.0) as u32,
        )
    }

    /// Today: (weekday, day of the month, month, year), the start date moved
    /// on by the days passed.
    pub fn calendar(&self) -> (usize, u32, usize, u32) {
        let (mut day, mut month, mut year) = self.start_date();
        for _ in 0..self.day {
            day += 1;
            if day > MONTHS[month as usize].1 {
                day = 1;
                month += 1;
                if month == 12 {
                    month = 0;
                    year += 1;
                }
            }
        }
        // The first day is a Sundas.
        let weekday = self.day as usize % 7;
        (weekday, day, month as usize, year)
    }

    /// The date and time as the journal's bottom bar shows them: "Sundas,
    /// 8:15 PM, 17th of Last Seed, 4E 201".
    pub fn date_text(&self) -> String {
        let (weekday, day, month, year) = self.calendar();
        let minutes = (self.hour.rem_euclid(24.0) * 60.0) as u32;
        let (h, m) = (minutes / 60, minutes % 60);
        let (h12, half) = match h {
            0 => (12, "AM"),
            1..=11 => (h, "AM"),
            12 => (12, "PM"),
            _ => (h - 12, "PM"),
        };
        format!(
            "{}, {h12}:{m:02} {half}, {} of {}, 4E {year}",
            WEEKDAYS[weekday],
            ordinal(day),
            MONTHS[month].0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Usage, WeaponUsage, ordinal};
    use esp::FormId;

    #[test]
    fn usage_favorites_keep_the_incumbent_on_ties() {
        let mut usage = Usage::<u32>::default();
        assert_eq!(usage.favorite, None);
        usage.record(2);
        usage.record(1);
        assert_eq!(usage.favorite, Some(2));
        usage.record(1);
        assert_eq!(usage.favorite, Some(1));
        usage.record(2);
        assert_eq!(usage.favorite, Some(1));
        usage.record(2);
        assert_eq!(usage.favorite, Some(2));
    }

    #[test]
    fn weapon_usage_round_trips_stably_and_rejects_inconsistent_winners() {
        let mut usage = WeaponUsage::default();
        usage.record(FormId(2));
        usage.record(FormId(1));
        usage.record(FormId(1));
        assert_eq!(
            usage.saved(),
            (vec![(FormId(1), 2), (FormId(2), 1)], Some(FormId(1)))
        );
        let restored =
            WeaponUsage::restore(vec![(FormId(1), 2), (FormId(2), 2)], Some(FormId(1))).unwrap();
        assert_eq!(restored.saved().1, Some(FormId(1)));
        assert!(WeaponUsage::restore(vec![(FormId(1), 1)], Some(FormId(2))).is_err());
        assert!(WeaponUsage::restore(vec![(FormId(1), 0)], Some(FormId(1))).is_err());
        assert!(WeaponUsage::restore(vec![(FormId::NULL, 1)], Some(FormId::NULL)).is_err());
        assert!(
            WeaponUsage::restore(vec![(FormId(2), 1), (FormId(1), 1)], Some(FormId(2))).is_err()
        );
    }

    #[test]
    fn ordinals() {
        let got: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 31]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            got,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "31st"
            ]
        );
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
pub(crate) mod action_tests;
