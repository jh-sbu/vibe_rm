//! The player's statistics (`Game.IncrementStat` / `QueryStat`, the
//! journal's Stats page) and the calendar the journal dates its bottom bar
//! with.
//!
//! Which page category each statistic is listed under, and the calendar's
//! weekday on the first day, aren't published: see `known_gaps/journal.md`.

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

/// The favorites the page shows by name; none counted yet.
const NAMED: &[&str] = &[
    "Favorite Weapon",
    "Favorite Spell",
    "Favorite School",
    "Favorite Shout",
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
    /// `Game.IncrementStat`.
    pub fn increment_stat(&mut self, name: &str, by: i32) {
        *self.scripts.stats.entry(name.to_string()).or_default() += by;
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
        if NAMED.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return StatValue::Name(String::new());
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
    use super::ordinal;

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
