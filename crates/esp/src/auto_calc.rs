//! NPCs' auto-calculated stats ("Auto Calc Stats", `ACBS` flag 0x10, which
//! "PC Level Mult" forces on): skills and health / magicka / stamina from the
//! race, the class's weights (`CLAS` `DATA`) and the level, as UESP's CLAS
//! page gives them. Each level past the first adds 8 skill points and 10
//! attribute points shared by the class's weights, and 5 health of its own.
//! Whole sets of the weights go first; the points left over go one at a
//! time in rounds, each value taking at most its weight more: skills by
//! higher weight, then lower actor value index; attributes by higher
//! weight, then stamina, magicka, health. Skills past 100 follow a rule
//! fitted to the stored values (`skills`). The Creation Kit stores what this
//! gives at the record's level in `DNAM` (`vrm-tool npc-stats` compares).

/// Skill points shared out each level.
const SKILL_POINTS: u32 = 8;
/// Attribute points shared out each level.
const ATTRIBUTE_POINTS: u32 = 10;
/// Health each level adds besides its share.
const HEALTH_PER_LEVEL: u32 = 5;
/// Every skill's start before the race's bonus.
const SKILL_START: u32 = 15;

/// `total` points shared out by `weights`: whole sets, then what is left
/// one at a time in rounds, in `order`.
fn share(total: u32, weights: &[u8], order: &[usize]) -> Vec<u32> {
    let sum: u32 = weights.iter().map(|&w| w as u32).sum();
    if sum == 0 {
        return vec![0; weights.len()];
    }
    let sets = total / sum;
    let mut out: Vec<u32> = weights.iter().map(|&w| sets * w as u32).collect();
    let mut left = total - sets * sum;
    let mut round = 0;
    while left > 0 {
        for &i in order {
            if left > 0 && (weights[i] as u32) > round {
                out[i] += 1;
                left -= 1;
            }
        }
        round += 1;
    }
    out
}

/// The 18 skills at `level`: 15, the race's bonuses (`RACE` `DATA`: seven
/// pairs of skill actor value index and bonus) and the class's share, at
/// most 100.
///
/// Once whole sets alone take a skill to 100, UESP's CLAS page has the
/// shares "not yet worked out"; this is the rule fitted to the vanilla
/// `DNAM`s (1582 of the 1583 auto-calc NPCs at a fixed level; the one left,
/// `DLC2EbonyWarrior`, and the fit are in `known_ambiguities/actor-values.md`).
/// The capped skills' excess past 100 at the whole sets (`x`, counting the
/// shortfall of skills capped later as negative) and the points left over
/// after the sets give the others at most one more set by weight, in the
/// usual rounds; then each of them gets the same `(x - z) / n`, where `z`
/// is how many skills have no weight and `n` how many are still under 100.
/// A skill pushed past 100 by that joins the capped ones and the shares are
/// worked out again.
pub fn skills(level: u32, race_bonuses: &[(u8, u8)], weights: &[u8; 18]) -> [u8; 18] {
    let base: [i32; 18] = std::array::from_fn(|i| {
        let bonus: u32 = race_bonuses
            .iter()
            .filter(|(s, _)| *s as usize == i + crate::actor_value::FIRST_SKILL as usize)
            .map(|(_, b)| *b as u32)
            .sum();
        (SKILL_START + bonus) as i32
    });
    let mut order: Vec<usize> = (0..18).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(weights[i]), std::cmp::Reverse(i)));
    let total = SKILL_POINTS * level.saturating_sub(1);
    let sum: u32 = weights.iter().map(|&w| w as u32).sum();
    let sets = if sum == 0 { 0 } else { total / sum };
    let raw: [i32; 18] = std::array::from_fn(|i| base[i] + (sets * weights[i] as u32) as i32);
    let mut capped: [bool; 18] = std::array::from_fn(|i| weights[i] > 0 && raw[i] >= 100);
    if !capped.contains(&true) {
        let gained = share(total, weights, &order);
        return std::array::from_fn(|i| (base[i] + gained[i] as i32).min(100) as u8);
    }
    let left = (total - sets * sum) as i32;
    let unweighted = weights.iter().filter(|&&w| w == 0).count() as i32;
    loop {
        let excess: i32 = (0..18).filter(|&i| capped[i]).map(|i| raw[i] - 100).sum();
        let active: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| weights[i] > 0 && !capped[i])
            .collect();
        let mut value: [i32; 18] = std::array::from_fn(|i| if capped[i] { 100 } else { raw[i] });
        if !active.is_empty() {
            let mut pool = excess + left;
            for round in 0..weights[active[0]] {
                for &i in &active {
                    if pool > 0 && weights[i] > round {
                        value[i] += 1;
                        pool -= 1;
                    }
                }
            }
            let even = (excess - unweighted).div_euclid(active.len() as i32).max(0);
            for &i in &active {
                value[i] += even;
            }
        }
        let over: Vec<usize> = active.into_iter().filter(|&i| value[i] > 100).collect();
        if over.is_empty() {
            return value.map(|v| v.min(100) as u8);
        }
        for i in over {
            capped[i] = true;
        }
    }
}

/// Health, magicka and stamina at `level`: the race's starting values, the
/// NPC's offsets (`ACBS`) and the class's share by its attribute weights.
pub fn attributes(level: u32, start: [f32; 3], offsets: [f32; 3], weights: [u8; 3]) -> [f32; 3] {
    let gained = level.saturating_sub(1);
    let mut order = [0, 1, 2];
    order.sort_by_key(|&i| (std::cmp::Reverse(weights[i]), std::cmp::Reverse(i)));
    let share = share(ATTRIBUTE_POINTS * gained, &weights, &order);
    std::array::from_fn(|i| {
        let own = if i == 0 { HEALTH_PER_LEVEL * gained } else { 0 };
        start[i] + offsets[i] + (share[i] + own) as f32
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dervenin: a level 6 Wood Elf beggar (`DNAM` 91, 67, 67; UESP's page
    /// the same).
    #[test]
    fn dervenin() {
        let a = attributes(6, [50.0; 3], [0.0; 3], [1, 1, 1]);
        assert_eq!(a, [91.0, 67.0, 67.0]);
        let weights = [1, 0, 1, 1, 1, 0, 0, 3, 1, 2, 1, 4, 1, 0, 0, 0, 0, 1];
        let race = [
            (8, 10),
            (15, 5),
            (14, 5),
            (13, 5),
            (12, 5),
            (16, 5),
            (255, 0),
        ];
        assert_eq!(
            skills(6, &race, &weights),
            [
                17, 15, 27, 17, 17, 15, 20, 27, 22, 25, 23, 24, 18, 15, 15, 15, 15, 18
            ]
        );
    }

    /// `EncDremoraMelee06` at level 46 (`DNAM`; Skyrim SE read the same for
    /// a "PC level mult" copy, autotest21): OneHanded and TwoHanded capped,
    /// Block and HeavyArmor 84, Sneak 51.
    #[test]
    fn dremora_past_100() {
        let mut weights = [0; 18];
        for (i, w) in [(0, 3), (1, 3), (3, 2), (5, 2), (9, 1)] {
            weights[i] = w;
        }
        let s = skills(46, &[], &weights);
        assert_eq!([s[0], s[1], s[3], s[5], s[9]], [100, 100, 84, 84, 51]);
        assert!(
            s.iter()
                .enumerate()
                .all(|(i, &v)| weights[i] > 0 || v == 15)
        );
    }

    /// `EncWarlock07TemplateBossFire` at level 50: Alteration and
    /// Conjuration reach 100 only from the others' excess, Conjuration
    /// exactly, which doesn't start another round (`DNAM` Sneak 69).
    #[test]
    fn capped_later() {
        let mut weights = [0; 18];
        for (i, w) in [(9, 1), (12, 2), (13, 2), (14, 3), (16, 3)] {
            weights[i] = w;
        }
        let race = [
            (20, 10),
            (18, 5),
            (21, 5),
            (15, 5),
            (12, 5),
            (16, 5),
            (255, 0),
        ];
        let s = skills(50, &race, &weights);
        assert_eq!([s[9], s[12], s[13], s[14], s[16]], [69, 100, 100, 100, 100]);
    }

    #[test]
    fn level_one_is_the_start() {
        assert_eq!(attributes(1, [50.0; 3], [50.0; 3], [2, 1, 0]), [100.0; 3]);
        assert_eq!(skills(1, &[], &[1; 18]), [15; 18]);
    }
}
