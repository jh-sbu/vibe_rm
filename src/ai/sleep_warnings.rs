use super::*;

impl Engine {
    pub(super) fn sleep_warning_goal(&self, actor: FormId) -> Option<Goal> {
        let &cell = self.crime.warned_homes.get(&actor)?;
        if self.location != crate::engine::Location::Interior(cell)
            || self.player_dead()
            || self
                .crime
                .trespass
                .is_some_and(|t| t.cell == cell && t.level >= 2)
            || self.actor_cells.get(&actor) != Some(&crate::render::CellKey::Interior(cell))
        {
            return None;
        }
        let a = self.actor_ref(actor)?;
        if a.dead || a.exec.pinned || a.exec.combat.is_some() {
            return None;
        }
        let mut goal = Goal::travel(self.ref_position(PLAYER_REF)?);
        goal.behaviour = Behaviour::Follow;
        goal.target = Some(PLAYER_REF);
        goal.follow_radius = (350., 500.);
        Some(goal)
    }

    /// Preserve the sleep run while its owner approaches an unwanted guest.
    /// Also catches warnings initialized by this frame's package change.
    pub(super) fn update_sleep_warnings(&mut self) {
        let decisions: Vec<_> = self
            .crime
            .warned_homes
            .keys()
            .map(|&actor| (actor, self.sleep_warning_goal(actor)))
            .collect();
        for (actor, goal) in decisions {
            let Some(key) = self.actor_cells.get(&actor) else {
                continue;
            };
            let Some(a) = self
                .cells
                .get_mut(key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
            else {
                continue;
            };
            let sleep = a
                .exec
                .current
                .and_then(|i| a.exec.packages.get(i))
                .is_some_and(|p| a.exec.effective(p).behaviour == Behaviour::Sleep);
            if !sleep {
                continue;
            }
            let following = a
                .exec
                .goal
                .is_some_and(|g| g.behaviour == Behaviour::Follow && g.target == Some(PLAYER_REF));
            if let Some(goal) = goal {
                a.exec.goal = Some(goal);
                if !following {
                    a.interrupt(&mut self.furniture);
                }
            } else if following {
                a.exec.next_eval = 0.;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::exec::tests::{ACTOR, CELL, PACKAGE, fixture, loaded_actor};

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn warning_approach_retains_sleep_run_and_resumes_bed_after_leaving_or_escalation() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.location = crate::engine::Location::Interior(CELL);
        e.crime.warned_homes.insert(ACTOR, CELL);
        e.scripts.alias_gen = 7;
        let a = e.actor_mut(ACTOR).unwrap();
        let mut p = Package::bare(PACKAGE, Behaviour::Sleep);
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
        a.exec.packages = vec![p];
        a.exec.current = Some(0);
        a.exec.begin_run(Some(0), 2., false);
        a.exec.next_eval = 0.;
        let started = a.exec.run.as_ref().unwrap().started_at;
        e.evaluate_packages(0.);
        e.update_sleep_warnings();
        let a = e.actor_ref(ACTOR).unwrap();
        assert_eq!(a.exec.goal.unwrap().behaviour, Behaviour::Follow);
        assert_eq!(a.exec.goal.unwrap().target, Some(PLAYER_REF));
        assert!(a.exec.goal.unwrap().furniture.is_none());
        assert_eq!(a.exec.run.as_ref().unwrap().started_at, started);
        e.crime.trespass = Some(crate::crime::Trespass {
            cell: CELL,
            level: 2,
            next: 0.,
        });
        e.update_sleep_warnings();
        e.evaluate_packages(0.);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
            Behaviour::Sleep
        );
        e.crime.trespass = None;
        e.update_sleep_warnings();
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
            Behaviour::Follow
        );
        e.location = crate::engine::Location::Nowhere;
        e.update_sleep_warnings();
        e.evaluate_packages(0.);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
            Behaviour::Sleep
        );
        assert_eq!(
            e.actor_ref(ACTOR)
                .unwrap()
                .exec
                .run
                .as_ref()
                .unwrap()
                .started_at,
            started
        );
    }
}
