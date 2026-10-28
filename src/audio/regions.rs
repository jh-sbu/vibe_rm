//! Pure region scheduler. Chance is provisional; see acoustic-spaces.md.
use esp::{FormId, LoadOrder};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub id: FormId,
    pub flags: u32,
    pub chance: f32,
    pub looping: bool,
}
pub fn load(lo: &LoadOrder) -> HashMap<FormId, Vec<Sound>> {
    let mut all = HashMap::new();
    for &id in lo.ids_of_type(b"REGN") {
        let Some(r) = lo.get(id) else { continue };
        let mut sounds = Vec::new();
        let mut active = false;
        for sr in r.subrecords() {
            if sr.tag.0 == *b"RDAT" {
                active = sr.data.len() >= 4 && sr.u32(0) == 7;
            }
            if sr.tag.0 != *b"RDSA" || !active {
                continue;
            }
            for d in sr.data.chunks_exact(12) {
                let id = r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
                let looping = lo
                    .get(id)
                    .and_then(|r| r.get(b"LNAM").map(|d| d.len() >= 2 && d[1] & 0x38 != 0))
                    .unwrap_or(false);
                sounds.push(Sound {
                    id,
                    flags: u32::from_le_bytes(d[4..8].try_into().unwrap()),
                    chance: f32::from_le_bytes(d[8..12].try_into().unwrap()),
                    looping,
                });
            }
        }
        all.insert(id, sounds);
    }
    all
}
#[derive(Default)]
pub struct Schedule {
    eligible: Vec<Sound>,
    admitted: HashSet<FormId>,
    next_check: f64,
    pub beds: Vec<FormId>,
    pub shots: Vec<FormId>,
}
impl Schedule {
    pub fn update(&mut self, sounds: &[Sound], class: i32, now: f64, menu: bool, mut seed: u64) {
        let mut random = || {
            seed = seed.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            crate::world::sound::unit(z ^ (z >> 31))
        };
        let mut eligible: Vec<_> = sounds
            .iter()
            .copied()
            .filter(|s| {
                (0..4).contains(&class)
                    && s.flags & (1 << class) != 0
                    && s.chance.is_finite()
                    && s.chance > 0.0
            })
            .collect();
        eligible.sort_by_key(|s| s.id.0);
        eligible.dedup_by_key(|s| s.id);
        if eligible != self.eligible {
            self.admitted
                .retain(|id| eligible.iter().any(|s| s.id == *id && s.looping));
            for s in &eligible {
                if s.looping
                    && !self
                        .eligible
                        .iter()
                        .any(|old| old.id == s.id && old.looping)
                    && random() < s.chance.clamp(0.0, 1.0)
                {
                    self.admitted.insert(s.id);
                }
            }
            self.eligible = eligible;
        }
        self.beds = self
            .eligible
            .iter()
            .filter(|s| s.looping && self.admitted.contains(&s.id))
            .map(|s| s.id)
            .collect();
        self.shots.clear();
        if menu {
            self.next_check = now + 1.0;
            return;
        }
        if now >= self.next_check {
            self.next_check = now + 1.0;
            for s in &self.eligible {
                if !s.looping && random() < s.chance.clamp(0.0, 1.0) {
                    self.shots.push(s.id);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weather_filtered_beds_persist_and_shots_have_a_bounded_check_interval() {
        let sounds = [
            Sound {
                id: FormId(1),
                flags: 1,
                chance: 1.0,
                looping: true,
            },
            Sound {
                id: FormId(2),
                flags: 4,
                chance: 1.0,
                looping: false,
            },
        ];
        let mut s = Schedule::default();
        s.update(&sounds, 0, 0.0, false, 1);
        assert_eq!(s.beds, [FormId(1)]);
        assert!(s.shots.is_empty());
        s.update(&sounds, 2, 1.0, false, 1);
        assert!(s.beds.is_empty());
        assert_eq!(s.shots, [FormId(2)]);
        s.update(&sounds, 2, 1.1, false, 1);
        assert!(s.shots.is_empty());
        s.update(&sounds, 2, 9.0, true, 1);
        assert!(s.shots.is_empty());
        s.update(&sounds, 2, 10.0, false, 1);
        assert_eq!(s.shots, [FormId(2)]);
    }
}
