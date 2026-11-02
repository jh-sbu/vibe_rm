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

/// Approximately twenty standing body lengths (128 units each).
pub const REVEAL_RADIUS: f32 = 20.0 * 128.0;

impl Seen {
    /// Reveal squares whose centres are within the radius. `loaded` prevents
    /// exterior pages that are not currently loaded from becoming explored.
    pub fn reveal(
        &mut self,
        space: Space,
        position: Vec2,
        radius: f32,
        loaded: impl Fn(Page) -> bool,
    ) {
        if !position.is_finite() || !radius.is_finite() || radius < 0.0 {
            return;
        }
        let Some((min_x, min_y)) = square_at(position - Vec2::splat(radius)) else {
            return;
        };
        let Some((max_x, max_y)) = square_at(position + Vec2::splat(radius)) else {
            return;
        };
        // Avoid pathological work from corrupt or unrealistic positions/radii.
        if i64::from(max_x) - i64::from(min_x) > 1024 || i64::from(max_y) - i64::from(min_y) > 1024
        {
            return;
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let center = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * SQUARE_SIZE;
                if center.distance_squared(position) <= radius * radius
                    && loaded(index(space, x, y).0)
                {
                    self.set_square(space, x, y);
                }
            }
        }
    }
}

impl crate::engine::Engine {
    pub(crate) fn reveal_local_map(&mut self) {
        let space = match self.location {
            crate::engine::Location::Interior(id) => Space::Interior(id),
            crate::engine::Location::Exterior { world, .. } => Space::Exterior(world),
            crate::engine::Location::Nowhere => return,
        };
        let cells = &self.scene.cells;
        self.local_seen.reveal(
            space,
            self.player.position.truncate(),
            REVEAL_RADIUS,
            |page| {
                matches!(page.space, Space::Interior(_))
                    || cells.contains_key(&crate::render::CellKey::Exterior(page.x, page.y))
            },
        );
    }

    pub(crate) fn update_local_seen(&mut self, dt: f32) {
        self.local_seen_next -= dt.max(0.0);
        if self.local_seen_next <= 0.0 {
            self.local_seen_next = 0.5;
            self.reveal_local_map();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reveal_is_radial_and_limited_to_loaded_pages() {
        let mut seen = Seen::default();
        let space = Space::Exterior(FormId(1));
        seen.reveal(space, Vec2::new(4090.0, 128.0), 512.0, |page| {
            page.x == 0 && page.y == 0
        });
        assert!(seen.contains(space, Vec2::new(4000.0, 128.0)));
        assert!(!seen.contains(space, Vec2::new(4200.0, 128.0)));
        assert!(!seen.contains(space, Vec2::new(3000.0, 128.0)));
    }

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
