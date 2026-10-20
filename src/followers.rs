//! The player's teammates (`Actor.SetPlayerTeammate`): the flag the follower
//! scripts set on whoever travels with the player (`DialogueFollowerScript`,
//! the Companions' and Dark Brotherhood's housekeeping, the Dawnguard's
//! thralls). Scripts and conditions read it (`IsPlayerTeammate`,
//! `GetPlayerTeammate`, `GetPlayerTeammateCount`); in a fight, teammates take the
//! player's side, and what they do to others the player answers for. See
//! `known_ambiguities/followers.md`.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};

impl Engine {
    /// `SetPlayerTeammate(abTeammate, abCanDoFavor)`: the flag, with whether the
    /// player can ask it for favors (kept for the dialogue that asks).
    pub fn set_player_teammate(&mut self, actor: FormId, teammate: bool, can_do_favor: bool) {
        if actor == PLAYER_REF || actor == FormId(0x7) {
            return;
        }
        if teammate {
            if self.scripts.teammates.insert(actor, can_do_favor).is_none() {
                log::info!("{actor} is the player's teammate");
            }
        } else if self.scripts.teammates.remove(&actor).is_some() {
            log::info!("{actor} is no longer the player's teammate");
        }
    }

    /// `IsPlayerTeammate`, `GetPlayerTeammate`.
    pub fn is_player_teammate(&self, actor: FormId) -> bool {
        self.scripts.teammates.contains_key(&actor)
    }

    /// `GetPlayerTeammateCount`: the teammates still alive.
    pub fn player_teammate_count(&self) -> usize {
        self.scripts
            .teammates
            .keys()
            .filter(|&&r| !self.is_dead(r))
            .count()
    }

    /// Who answers for what `actor` does to others: the player, for their own
    /// blows and those of their teammates and summons (UESP *Skyrim:Crime*:
    /// "Attacks by followers, summoned, commanded, or reanimated allies also
    /// count as assault"), else the actor itself.
    pub(crate) fn responsible_for(&self, actor: FormId) -> FormId {
        let mut who = actor;
        // A commander's own commander (a summoner's summon's summon) answers too.
        for _ in 0..4 {
            who = match self.commander_of(who) {
                Some(c) => c,
                None => break,
            };
        }
        if self.is_player_teammate(who) {
            PLAYER_REF
        } else {
            who
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn teammates_are_flagged_counted_and_answered_for() {
        let dir = std::env::temp_dir().join(format!("vrm-teammates-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Empty.esp"),
            esp::writer::write_plugin(0, &[], "test", "no records", 0x900, &[]),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Empty.esp");
        let (follower, stranger, summon) = (FormId(0x901), FormId(0x902), FormId(0x903));
        assert!(!e.is_player_teammate(follower));
        assert_eq!(e.player_teammate_count(), 0);
        e.set_player_teammate(follower, true, false);
        e.set_player_teammate(PLAYER_REF, true, true);
        assert!(e.is_player_teammate(follower));
        assert_eq!(e.player_teammate_count(), 1);
        // The player answers for their teammates', their summons' and the
        // teammates' summons' blows, not a stranger's.
        assert_eq!(e.responsible_for(follower), PLAYER_REF);
        assert_eq!(e.responsible_for(PLAYER_REF), PLAYER_REF);
        assert_eq!(e.responsible_for(stranger), stranger);
        e.magic.commanded.insert(summon, follower);
        assert_eq!(e.responsible_for(summon), PLAYER_REF);
        e.magic.commanded.insert(stranger, summon);
        assert_eq!(e.responsible_for(stranger), PLAYER_REF);
        // Dismissed.
        e.set_player_teammate(follower, false, true);
        assert!(!e.is_player_teammate(follower));
        assert_eq!(e.responsible_for(follower), follower);
        assert_eq!(e.player_teammate_count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
