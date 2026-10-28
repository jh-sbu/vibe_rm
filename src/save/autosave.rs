//! Autosave trigger policy, independent of menus and persistent snapshots.
#[derive(Clone, Debug)]
pub struct Preferences {
    pub rest: bool,
    pub wait: bool,
    pub travel: bool,
    pub pause_minutes: Option<u32>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            rest: true,
            wait: true,
            travel: true,
            pause_minutes: None,
        }
    }
}
#[derive(Clone, Copy)]
pub enum Trigger {
    Rest,
    Wait,
    Travel,
}
impl Preferences {
    pub fn enabled(&self, trigger: Trigger) -> bool {
        match trigger {
            Trigger::Rest => self.rest,
            Trigger::Wait => self.wait,
            Trigger::Travel => self.travel,
        }
    }
}
impl crate::engine::Engine {
    pub fn request_autosave(&mut self, trigger: Trigger) {
        if self.autosave.enabled(trigger) {
            self.request_save(true);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trigger_switches_are_independent() {
        let mut p = Preferences::default();
        assert!(p.enabled(Trigger::Rest));
        assert!(p.enabled(Trigger::Wait));
        assert!(p.enabled(Trigger::Travel));
        p.rest = false;
        assert!(!p.enabled(Trigger::Rest));
        assert!(p.enabled(Trigger::Wait));
        p.wait = false;
        p.travel = false;
        assert!(!p.enabled(Trigger::Wait));
        assert!(!p.enabled(Trigger::Travel));
    }
}
