//! Journal distance preferences. Distances are world units; fading uses a
//! screen-door transition over the final fifth of the visibility range.
#[derive(Clone, Copy, Debug)]
pub struct Distances {
    pub actors: f32,
}
impl Default for Distances {
    fn default() -> Self {
        Self {
            actors: f32::INFINITY,
        }
    }
}
pub fn alpha(distance: f32, end: f32) -> f32 {
    if end.is_infinite() {
        return 1.;
    }
    if end <= 0. {
        return 0.;
    }
    ((end - distance) / (end * 0.2).max(0.001)).clamp(0., 1.)
}
impl Distances {
    pub fn actor(&self, distance: f32, radius: f32) -> f32 {
        alpha(distance, self.actors * radius.max(1.))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actor_fade_uses_radius_and_culls_at_limit() {
        let p = Distances { actors: 10. };
        assert_eq!(p.actor(79., 10.), 1.);
        assert!((p.actor(90., 10.) - 0.5).abs() < 0.001);
        assert_eq!(p.actor(100., 10.), 0.);
        assert_eq!(Distances::default().actor(1e6, 10.), 1.);
    }
}
