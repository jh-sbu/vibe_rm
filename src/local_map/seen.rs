//! Sparse per-space local-map exploration in 256-unit squares.
use std::collections::HashMap;

use esp::FormId;
use glam::Vec2;

pub const SQUARE_SIZE: f32 = 256.0;
const SIDE: i32 = 16;

/// Interiors have their own grid; exterior grids are shared by worldspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Space {
    Interior(FormId),
    Exterior(FormId),
}

/// One 16 by 16 page. Exterior pages coincide with exterior cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Page {
    pub space: Space,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    pub pages: HashMap<Page, [u64; 4]>,
}

fn square_at(position: Vec2) -> Option<(i32, i32)> {
    let p = (position / SQUARE_SIZE).floor();
    (p.is_finite()
        && p.x >= i32::MIN as f32
        && p.x < i32::MAX as f32
        && p.y >= i32::MIN as f32
        && p.y < i32::MAX as f32)
        .then_some((p.x as i32, p.y as i32))
}

fn index(space: Space, x: i32, y: i32) -> (Page, usize) {
    let page = Page {
        space,
        x: x.div_euclid(SIDE),
        y: y.div_euclid(SIDE),
    };
    let bit = (y.rem_euclid(SIDE) * SIDE + x.rem_euclid(SIDE)) as usize;
    (page, bit)
}

impl Seen {
    pub fn set_square(&mut self, space: Space, x: i32, y: i32) {
        let (page, bit) = index(space, x, y);
        self.pages.entry(page).or_default()[bit / 64] |= 1u64 << (bit % 64);
    }

    pub fn contains(&self, space: Space, position: Vec2) -> bool {
        let Some((x, y)) = square_at(position) else {
            return false;
        };
        let (page, bit) = index(space, x, y);
        self.pages
            .get(&page)
            .is_some_and(|bits| bits[bit / 64] & (1u64 << (bit % 64)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_positions_and_pages_do_not_alias_other_cells_or_spaces() {
        let mut seen = Seen::default();
        let a = Space::Interior(FormId(1));
        let b = Space::Exterior(FormId(1));
        seen.set_square(a, -1, -17);
        assert!(seen.contains(a, Vec2::new(-1.0, -4097.0)));
        assert!(!seen.contains(a, Vec2::new(0.0, -4097.0)));
        assert!(!seen.contains(b, Vec2::new(-1.0, -4097.0)));
        assert_eq!(seen.pages.keys().next().map(|p| (p.x, p.y)), Some((-1, -2)));
        assert!(!seen.contains(a, Vec2::splat(f32::NAN)));
    }
}
