//! Force greet packages (the ForceGreet / ForceGreetFromSitting templates): the
//! actor waits at its wait location (or sandboxes there, or sits in its seat)
//! until the player comes within the trigger location, seen if the package
//! wants that, then walks up to within the force greet distance and opens a
//! conversation with the package's topic. Seated greeters speak from the seat.

use esp::FormId;
use glam::Vec3;

use super::package::{ForceGreet, GreetTopic, Location, LocationKind};
use crate::dialogue::{Info, Topic};
use crate::engine::{Engine, PLAYER_REF};

/// Seconds between package re-evaluations for force greeters (others: 3).
pub(super) const EVAL_INTERVAL: f32 = 0.5;
/// Seconds between checks of whether the player is there to be greeted.
const CHECK_INTERVAL: f32 = 0.5;
/// Seconds after a forced conversation ends before the greeter may come again.
const COOLDOWN: f32 = 10.0;
/// Within this much beyond the force greet distance counts as there.
const SLACK: f32 = 48.0;
/// The templates' own trigger location: within 500 units of the greeter.
const DEFAULT_TRIGGER: Location = Location { kind: LocationKind::NearCurrent, radius: 500.0 };

impl Engine {
    /// Decide which force greeters go up to the player, and greet those there
    /// (called every frame).
    pub(crate) fn update_force_greets(&mut self, dt: f32) {
        let talking = self.conversation.as_ref().map(|c| c.npc_ref);
        let player = self.player_feet();
        let mut checks: Vec<(FormId, ForceGreet)> = Vec::new();
        let mut ready: Vec<(FormId, ForceGreet)> = Vec::new();
        for a in self.cells.values_mut().flat_map(|rt| rt.actors.iter_mut()) {
            let Some(g) = a.current.and_then(|i| a.packages.get(i)).and_then(|p| p.greet) else {
                a.greeting = false;
                continue;
            };
            // The wait starts once the conversation is over.
            if talking == Some(a.ref_id) {
                continue;
            }
            a.greet_wait = (a.greet_wait - dt).max(0.0);
            a.greet_check -= dt;
            let close = g.seated || a.pos.truncate().distance(player.truncate()) <= g.distance + SLACK;
            if a.greeting && close {
                ready.push((a.ref_id, g));
            } else if a.greet_check <= 0.0 {
                a.greet_check = CHECK_INTERVAL;
                checks.push((a.ref_id, g));
            }
        }
        for (r, g) in checks {
            let due = self.force_greet_due(r, &g).is_some();
            if let Some(a) = self.actor_mut(r)
                && a.greeting != due
            {
                log::debug!("{r} {} the player", if due { "sets off to greet" } else { "stops going to greet" });
                a.greeting = due;
                // Choose the goal for it now.
                a.next_eval = 0.0;
            }
        }
        for (r, g) in ready {
            if self.conversation.is_some() {
                break;
            }
            let Some(greeting) = self.force_greet_due(r, &g) else {
                if let Some(a) = self.actor_mut(r) {
                    a.greeting = false;
                    a.next_eval = 0.0;
                }
                continue;
            };
            log::info!("{r} force greets the player ({} {})", greeting.0.editor_id, greeting.1.id);
            if let Some(a) = self.actor_mut(r) {
                a.greeting = false;
                a.greet_wait = COOLDOWN;
                a.next_eval = 0.0;
            }
            if self.scenes.packages.contains_key(&r) {
                self.scenes.force_greeted.insert(r);
            }
            self.open_conversation(r, Some(greeting));
        }
    }

    /// Whether `r` should greet the player now: the line it would open with.
    fn force_greet_due(&mut self, r: FormId, g: &ForceGreet) -> Option<(Topic, Info)> {
        if self.conversation.is_some() || self.menu.is_some() || self.player_dead() || !self.ai_enabled {
            return None;
        }
        let a = self.actor_ref(r)?;
        if a.dead || a.bleeding.is_some() || a.combat.is_some() || a.search.is_some() || a.exiting.is_some() || a.greet_wait > 0.0 {
            return None;
        }
        let quest = a.current.and_then(|i| a.packages.get(i)).and_then(|p| p.quest);
        let (centre, radius) = self.location_target(a, g.trigger.unwrap_or(DEFAULT_TRIGGER), quest);
        let player = self.player_feet();
        if player.distance(centre) > radius {
            return None;
        }
        if g.must_detect && !self.detects(r, PLAYER_REF) {
            return None;
        }
        self.greet_line(r, g.topic)
    }

    /// The topic and line a force greet opens with: the package's topic, or the
    /// first topic of its subtype with a line for `r`.
    fn greet_line(&mut self, r: FormId, topic: GreetTopic) -> Option<(Topic, Info)> {
        let topics = match topic {
            GreetTopic::Topic(id) => crate::dialogue::topic(&self.lo, id).into_iter().collect(),
            GreetTopic::Subtype(sub) => self.bark_topics(&sub),
        };
        topics.into_iter().find_map(|t| {
            let i = self.select_info(&t, r)?;
            Some((t, i))
        })
    }

    /// The player's feet.
    fn player_feet(&self) -> Vec3 {
        self.player.position - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius)
    }
}
