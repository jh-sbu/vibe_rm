//! Paired actor/furniture graph notifications (the sawmill load animation).
use crate::engine::Engine;
use esp::FormId;

impl Engine {
    /// A raised actor notification can drive the furniture currently being
    /// used. Notify its scripts only if its own graph accepted that event.
    /// This is distinct from echoing arbitrary PlayAnimation input events.
    pub(crate) fn relay_furniture_events(&mut self, furniture: FormId, events: &[String]) {
        for event in events {
            if self.play_object_animation(furniture, event) {
                self.raise_anim_events(furniture, std::slice::from_ref(event));
            }
        }
    }
}
