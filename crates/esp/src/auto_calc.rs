//! NPCs' auto-calculated stats ("Auto Calc Stats", `ACBS` flag 0x10, which
//! "PC Level Mult" forces on): skills and health / magicka / stamina from the
//! race, the class's weights (`CLAS` `DATA`) and the level, as UESP's CLAS
//! page gives them. Each level past the first adds 8 skill points and 10
//! attribute points shared by the class's weights, and 5 health of its own.
//! Whole sets of the weights go first; the points left over go one at a
//! time in rounds, each value taking at most its weight more: skills by
//! higher weight, then lower actor value index; attributes by higher
//! weight, then stamina, magicka, health. The Creation Kit stores what this
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
/// most 100. Past 100 UESP has the shares worked out otherwise, unknown.
pub fn skills(level: u32, race_bonuses: &[(u8, u8)], weights: &[u8; 18]) -> [u8; 18] {
    let mut order: Vec<usize> = (0..18).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(weights[i]), std::cmp::Reverse(i)));
    let gained = share(SKILL_POINTS * level.saturating_sub(1), weights, &order);
    std::array::from_fn(|i| {
        let bonus: u32 = race_bonuses
            .iter()
            .filter(|(s, _)| *s as usize == i + crate::actor_value::FIRST_SKILL as usize)
            .map(|(_, b)| *b as u32)
            .sum();
        (SKILL_START + bonus + gained[i]).min(100) as u8
    })
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

    #[test]
    fn level_one_is_the_start() {
        assert_eq!(attributes(1, [50.0; 3], [50.0; 3], [2, 1, 0]), [100.0; 3]);
        assert_eq!(skills(1, &[], &[1; 18]), [15; 18]);
    }
}
