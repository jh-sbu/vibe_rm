//! Passing NPC hellos. Authored quests/scenes choose any ensuing conversation.
use crate::engine::Engine;
use esp::FormId;
use glam::Vec3;
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct HelloContacts(HashSet<(FormId, FormId)>);

impl HelloContacts {
    fn update(&mut self, actors: &[(FormId, Vec3, bool)], reach: f32) -> Vec<(FormId, FormId)> {
        let mut contacts = HashSet::new();
        let mut events = Vec::new();
        for &(actor, at, moving) in actors {
            for &(target, there, target_moving) in actors {
                if actor == target {
                    continue;
                }
                let pair = (actor, target);
                let was = self.0.contains(&pair);
                let limit = reach + if was { 32.0 } else { 0.0 };
                if at.distance(there) > limit {
                    continue;
                }
                contacts.insert(pair);
                if !was && moving && !target_moving {
                    events.push(pair);
                }
            }
        }
        self.0 = contacts;
        events
    }
}

fn allows_conversations(pkdt: &[u8]) -> bool {
    pkdt.get(8..12)
        .is_some_and(|d| u32::from_le_bytes(d.try_into().unwrap()) & 2 != 0)
}

impl Engine {
    pub(crate) fn update_npc_hellos(&mut self) {
        if !self.ai_enabled || self.conversation.is_some() {
            return;
        }
        let mut actors: Vec<_> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| {
                !a.dead
                    && a.exec.combat.is_none()
                    && !self.ai_off.contains(&a.ref_id)
                    && self.scene_of_actor(a.ref_id).is_none()
                    && !self.is_barking(a.ref_id)
            })
            .filter(|a| {
                self.running_package(a.ref_id)
                    .and_then(|p| self.lo.get(p))
                    .is_some_and(|r| r.get(b"PKDT").is_some_and(allows_conversations))
            })
            .map(|a| (a.ref_id, a.feet(), a.speed() > 5.0))
            .collect();
        actors.sort_by_key(|a| a.0.0);
        let reach = crate::ai::combat::gmst_f32(&self.lo, "fAIMinGreetingDistance", 150.0).max(0.0);
        for (actor, target) in self.barks.npc_hellos.update(&actors, reach) {
            self.send_actor_hello(actor, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passing_hello_fires_once_for_a_mover_passing_a_stationary_actor() {
        let mut contacts = HelloContacts::default();
        let pair = |distance, moving, target_moving| {
            [
                (FormId(1), Vec3::ZERO, moving),
                (FormId(2), Vec3::Y * distance, target_moving),
            ]
        };
        assert!(contacts.update(&pair(300.0, true, false), 150.0).is_empty());
        assert_eq!(
            contacts.update(&pair(140.0, true, false), 150.0),
            vec![(FormId(1), FormId(2))]
        );
        assert!(contacts.update(&pair(155.0, true, false), 150.0).is_empty());
        assert!(contacts.update(&pair(145.0, true, false), 150.0).is_empty());
        contacts.update(&pair(200.0, true, true), 150.0);
        assert!(contacts.update(&pair(140.0, true, true), 150.0).is_empty());
        contacts.update(&[], 150.0);
        assert_eq!(
            contacts.update(&pair(140.0, false, true), 150.0),
            vec![(FormId(2), FormId(1))]
        );
    }
    #[test]
    fn package_random_conversations_flag_is_independent_of_player_hellos() {
        for (flags, expected) in [(0u32, false), (1, false), (2, true), (0x82, true)] {
            let mut pkdt = vec![0; 12];
            pkdt[8..12].copy_from_slice(&flags.to_le_bytes());
            assert_eq!(allows_conversations(&pkdt), expected);
        }
        assert!(!allows_conversations(&[]));
    }
}
