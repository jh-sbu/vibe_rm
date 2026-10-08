//! Animation events for scripts: `RegisterForAnimationEvent` has the events a
//! reference's behaviour graph raises (an actor's or an object's) sent to the
//! registering script as `OnAnimationEvent`, and `PlayAnimationAndWait` waits
//! for its event from the graph.

use std::hash::{Hash, Hasher};

use esp::FormId;
use papyrus::{ObjectId, Value};

use crate::engine::Engine;

/// How long `PlayAnimationAndWait` waits for an event that doesn't come (real
/// seconds; no source: the game's wait has no limit, but graphs here may not
/// raise every event the game's do).
pub(crate) const WAIT_LIMIT: f32 = 10.0;

/// One `RegisterForAnimationEvent`.
struct Registration {
    /// The registering script: its object and class.
    receiver: ObjectId,
    script: String,
    sender: FormId,
    /// Lower case.
    event: String,
}

#[derive(Default)]
pub(crate) struct AnimEvents {
    registrations: Vec<Registration>,
    /// `OnAnimationEvent`s to deliver: the receiving script and its arguments.
    pub(crate) pending: Vec<(ObjectId, String, Vec<Value>)>,
}

/// The key `PlayAnimationAndWait`'s thread waits on for `event` from `sender`.
pub(crate) fn wait_key(sender: FormId, event: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    "anim".hash(&mut h);
    sender.0.hash(&mut h);
    event.to_ascii_lowercase().hash(&mut h);
    h.finish()
}

impl Engine {
    /// Whether `r` runs a behaviour graph now (an actor's or an object's).
    pub(crate) fn has_behavior_graph(&self, r: FormId) -> bool {
        self.actor_ref(r).is_some_and(|a| a.graph.is_some()) || self.object_has_graph(r)
    }

    /// Papyrus `RegisterForAnimationEvent`: false, registering nothing, when the
    /// sender has no graph running (not loaded, or without one).
    pub(crate) fn register_anim_event(
        &mut self,
        receiver: ObjectId,
        script: &str,
        sender: FormId,
        event: &str,
    ) -> bool {
        if !self.has_behavior_graph(sender) {
            log::debug!("{receiver:?}: no graph on {sender} to register for {event}");
            return false;
        }
        let event = event.to_ascii_lowercase();
        let r = &mut self.anim_events.registrations;
        if !r.iter().any(|x| {
            x.receiver == receiver && x.script == script && x.sender == sender && x.event == event
        }) {
            log::debug!("{script} on {receiver:?} registers for {event} from {sender}");
            r.push(Registration {
                receiver,
                script: script.to_owned(),
                sender,
                event,
            });
        }
        true
    }

    /// Papyrus `UnregisterForAnimationEvent`.
    pub(crate) fn unregister_anim_event(
        &mut self,
        receiver: ObjectId,
        script: &str,
        sender: FormId,
        event: &str,
    ) {
        self.anim_events.registrations.retain(|x| {
            !(x.receiver == receiver
                && x.script == script
                && x.sender == sender
                && x.event.eq_ignore_ascii_case(event))
        });
    }

    /// Events `sender`'s graph raised: they end `PlayAnimationAndWait`s waiting
    /// for them and go to the scripts registered for them.
    pub(crate) fn raise_anim_events(&mut self, sender: FormId, events: &[String]) {
        for ev in events {
            if self.vm.signal(wait_key(sender, ev)) > 0 {
                log::debug!("{sender}: {ev} ends a PlayAnimationAndWait");
            }
            let to: Vec<(ObjectId, String)> = self
                .anim_events
                .registrations
                .iter()
                .filter(|x| x.sender == sender && x.event.eq_ignore_ascii_case(ev))
                .map(|x| (x.receiver, x.script.clone()))
                .collect();
            if to.is_empty() {
                continue;
            }
            let source = self.object_value(sender);
            for (receiver, script) in to {
                log::debug!("OnAnimationEvent {ev} from {sender} -> {script}");
                self.anim_events.pending.push((
                    receiver,
                    script,
                    vec![source.clone(), Value::str(ev)],
                ));
            }
        }
    }
}
