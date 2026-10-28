//! Journal distance preferences. Distances are world units; fading uses a
//! screen-door transition over the final fifth of the visibility range.
#[derive(Clone, Copy, Debug)]
pub struct Distances {
    pub actors: f32,
    pub items: f32,
    pub objects: f32,
    pub grass: f32,
    pub shadows: f32,
}
impl Default for Distances {
    fn default() -> Self {
        Self {
            actors: f32::INFINITY,
            items: f32::INFINITY,
            objects: f32::INFINITY,
            grass: crate::world::grass::FADE_START,
            shadows: 12000.,
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
        let p = Distances {
            actors: 10.,
            ..Default::default()
        };
        assert_eq!(p.actor(79., 10.), 1.);
        assert!((p.actor(90., 10.) - 0.5).abs() < 0.001);
        assert_eq!(p.actor(100., 10.), 0.);
        assert_eq!(Distances::default().actor(1e6, 10.), 1.);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Class {
    #[default]
    Object,
    Item,
    Tree,
}
impl Distances {
    pub fn instance(&self, class: Class, distance: f32, radius: f32) -> f32 {
        match class {
            Class::Item => alpha(distance, self.items * radius.max(1.)),
            Class::Object => alpha(distance, self.objects * radius.max(1.)),
            _ => 1.,
        }
    }
}
pub fn item_tag(tag: &[u8; 4]) -> bool {
    matches!(
        tag,
        b"WEAP"
            | b"ARMO"
            | b"AMMO"
            | b"ALCH"
            | b"INGR"
            | b"BOOK"
            | b"MISC"
            | b"KEYM"
            | b"SLGM"
            | b"SCRL"
    )
}
