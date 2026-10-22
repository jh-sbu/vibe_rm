//! A worldspace's map data: `MNAM` (the cells the map shows and the world
//! map camera's limits) and `ONAM` (the offset and scale of the map's
//! image). The map view and the markers on it are placed with these.

/// Cell size in world units.
const CELL: f32 = 4096.0;

/// The world map camera's limits (`MNAM`'s last twelve bytes, Skyrim 1.8 on).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraLimits {
    /// The lowest the camera comes (the most zoomed in), world units.
    pub min_height: f32,
    /// The highest the camera goes (the most zoomed out).
    pub max_height: f32,
    /// The camera's pitch when the map opens, degrees from the horizon.
    pub initial_pitch: f32,
}

impl Default for CameraLimits {
    /// The defaults the wiki gives for records without them.
    fn default() -> Self {
        CameraLimits {
            min_height: 50000.0,
            max_height: 80000.0,
            initial_pitch: 50.0,
        }
    }
}

/// `MNAM`: the part of the worldspace the map shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldMapData {
    /// The usable width and height (0 in Skyrim.esm's Tamriel).
    pub usable: (u32, u32),
    /// The north-west and south-east cells.
    pub nw: (i16, i16),
    pub se: (i16, i16),
    pub camera: CameraLimits,
}

impl WorldMapData {
    /// A 16 or 28 byte `MNAM`; the camera limits are the defaults for the
    /// shorter.
    pub fn parse(d: &[u8]) -> Option<WorldMapData> {
        if d.len() < 16 {
            return None;
        }
        let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let i16_at = |o: usize| i16::from_le_bytes(d[o..o + 2].try_into().unwrap());
        let f32_at = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let camera = if d.len() >= 28 {
            CameraLimits {
                min_height: f32_at(16),
                max_height: f32_at(20),
                initial_pitch: f32_at(24),
            }
        } else {
            CameraLimits::default()
        };
        Some(WorldMapData {
            usable: (u32_at(0), u32_at(4)),
            nw: (i16_at(8), i16_at(10)),
            se: (i16_at(12), i16_at(14)),
            camera,
        })
    }

    /// The map's extent in world units: (min x, min y, max x, max y). The
    /// corner cells are inside it.
    pub fn bounds(&self) -> [f32; 4] {
        [
            self.nw.0 as f32 * CELL,
            self.se.1 as f32 * CELL,
            (self.se.0 as f32 + 1.0) * CELL,
            (self.nw.1 as f32 + 1.0) * CELL,
        ]
    }

    /// Where a world position falls on the map as fractions of its width
    /// (from the west) and height (from the north): outside 0 to 1 when it
    /// is off the map.
    pub fn project(&self, x: f32, y: f32) -> (f32, f32) {
        let [min_x, min_y, max_x, max_y] = self.bounds();
        ((x - min_x) / (max_x - min_x), (max_y - y) / (max_y - min_y))
    }

    /// The reverse of `project`.
    pub fn unproject(&self, fx: f32, fy: f32) -> (f32, f32) {
        let [min_x, min_y, max_x, max_y] = self.bounds();
        (min_x + fx * (max_x - min_x), max_y - fy * (max_y - min_y))
    }
}

/// `ONAM`: how the map's image sits (the scale and the cell offsets), 16
/// bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldMapOffset {
    pub scale: f32,
    pub offset: [f32; 3],
}

impl WorldMapOffset {
    pub fn parse(d: &[u8]) -> Option<WorldMapOffset> {
        if d.len() < 16 {
            return None;
        }
        let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        Some(WorldMapOffset {
            scale: f(0),
            offset: [f(4), f(8), f(12)],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skyrim.esm's Tamriel.
    const TAMRIEL: [u8; 28] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0xe2, 0xff, 0x0f, 0x00, 0x28, 0x00, 0xd8, 0xff, 0x00, 0x50, 0x43,
        0x47, 0x00, 0x40, 0x9c, 0x47, 0x00, 0x00, 0x48, 0x42,
    ];

    #[test]
    fn tamriel_map_data() {
        let m = WorldMapData::parse(&TAMRIEL).unwrap();
        assert_eq!(m.nw, (-30, 15));
        assert_eq!(m.se, (40, -40));
        assert_eq!(m.camera.min_height, 50000.0);
        assert_eq!(m.camera.max_height, 80000.0);
        assert_eq!(m.camera.initial_pitch, 50.0);
        assert_eq!(m.bounds(), [-122880.0, -163840.0, 167936.0, 65536.0]);
    }

    #[test]
    fn short_records_use_default_camera() {
        let m = WorldMapData::parse(&TAMRIEL[..16]).unwrap();
        assert_eq!(m.camera, CameraLimits::default());
        assert!(WorldMapData::parse(&TAMRIEL[..15]).is_none());
    }

    #[test]
    fn projection_covers_the_corner_cells() {
        let m = WorldMapData::parse(&TAMRIEL).unwrap();
        // The north-west cell's north-west corner is the map's.
        assert_eq!(m.project(-30.0 * CELL, 16.0 * CELL), (0.0, 0.0));
        // The south-east cell's south-east corner.
        assert_eq!(m.project(41.0 * CELL, -40.0 * CELL), (1.0, 1.0));
        let (fx, fy) = m.project(1000.0, -3000.0);
        let (x, y) = m.unproject(fx, fy);
        assert!((x - 1000.0).abs() < 0.5 && (y + 3000.0).abs() < 0.5);
    }

    #[test]
    fn offset_data() {
        let mut d = Vec::new();
        for v in [1.0f32, 2.0, 3.0, 4.0] {
            d.extend(v.to_le_bytes());
        }
        let o = WorldMapOffset::parse(&d).unwrap();
        assert_eq!(o.scale, 1.0);
        assert_eq!(o.offset, [2.0, 3.0, 4.0]);
        assert!(WorldMapOffset::parse(&d[..12]).is_none());
    }
}
