//! Landscape (LAND) decoding.

use esp::{FormId, LoadOrder};
use glam::{Vec2, Vec3};

pub const CELL_SIZE: f32 = 4096.0;
pub const VERTS: usize = 33;
pub const QUAD_VERTS: usize = 17;
pub const MAX_LAYERS: usize = 8;
pub const DEFAULT_TEXTURE: &str = "textures/landscape/dirt02.dds";

#[derive(Debug, Clone)]
pub struct Layer {
    pub diffuse: String,
    pub normal: Option<String>,
    /// Opacity per vertex of the quadrant (17x17).
    pub opacity: Vec<f32>,
    /// The landscape texture (LTEX), if not the default.
    pub ltex: Option<FormId>,
}

#[derive(Debug, Clone)]
pub struct Quadrant {
    /// layers[0] is the base layer (fully opaque).
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone)]
pub struct Land {
    pub x: i32,
    pub y: i32,
    pub heights: Vec<f32>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<Vec3>,
    pub quadrants: [Quadrant; 4],
}

impl Land {
    pub fn height_at_vertex(&self, col: usize, row: usize) -> f32 {
        self.heights[row * VERTS + col]
    }
    pub fn origin(&self) -> Vec2 {
        Vec2::new(self.x as f32 * CELL_SIZE, self.y as f32 * CELL_SIZE)
    }
    /// The landscape texture (LTEX) showing most at a point of the cell: the
    /// nearest vertex's layer that the ones above it cover least.
    pub fn texture_at(&self, p: Vec2) -> Option<FormId> {
        let local = ((p - self.origin()) / (CELL_SIZE / 32.0)).round().clamp(Vec2::ZERO, Vec2::splat(32.0));
        let (gc, gr) = (local.x as usize, local.y as usize);
        let (qx, qy) = ((gc / 16).min(1), (gr / 16).min(1));
        let quad = &self.quadrants[qy * 2 + qx];
        let i = (gr - qy * 16) * QUAD_VERTS + (gc - qx * 16);
        let mut best = (0.0, None);
        let mut above = 1.0;
        for layer in quad.layers.iter().rev() {
            let op = layer.opacity[i];
            let shown = op * above;
            if shown > best.0 {
                best = (shown, layer.ltex);
            }
            above *= 1.0 - op;
        }
        best.1
    }

    /// Bilinear height lookup in world coordinates (assumes the point lies in this cell).
    pub fn height_at(&self, p: Vec2) -> f32 {
        let local = (p - self.origin()) / (CELL_SIZE / 32.0);
        let cx = local.x.clamp(0.0, 31.999);
        let cy = local.y.clamp(0.0, 31.999);
        let (ix, iy) = (cx as usize, cy as usize);
        let (fx, fy) = (cx - ix as f32, cy - iy as f32);
        let h00 = self.height_at_vertex(ix, iy);
        let h10 = self.height_at_vertex(ix + 1, iy);
        let h01 = self.height_at_vertex(ix, iy + 1);
        let h11 = self.height_at_vertex(ix + 1, iy + 1);
        // Match the triangle split used by the renderer (diagonal from (0,0) to (1,1)).
        if fx > fy {
            h00 + (h10 - h00) * fx + (h11 - h10) * fy
        } else {
            h00 + (h01 - h00) * fy + (h11 - h01) * fx
        }
    }
}

/// Texture paths for a landscape texture (LTEX -> TXST).
pub fn land_texture(lo: &LoadOrder, ltex: FormId) -> Option<(String, Option<String>)> {
    let rec = lo.get(ltex)?;
    let tnam = rec.get(b"TNAM")?;
    let txst = rec.fid(FormId(u32::from_le_bytes(tnam.get(..4)?.try_into().ok()?)));
    let t = lo.get(txst)?;
    let diffuse = t.get(b"TX00").map(esp::decode_zstring)?;
    let normal = t.get(b"TX01").map(esp::decode_zstring).filter(|s| !s.is_empty());
    Some((
        crate::world::records::texture_path(&diffuse),
        normal.map(|n| crate::world::records::texture_path(&n)),
    ))
}

pub fn load_land(lo: &LoadOrder, land: FormId, x: i32, y: i32, default_height: f32) -> Option<Land> {
    let rec = lo.get(land)?;
    let mut heights = vec![default_height; VERTS * VERTS];
    let mut normals = vec![Vec3::Z; VERTS * VERTS];
    let mut colors = vec![Vec3::ONE; VERTS * VERTS];
    let empty = || Quadrant { layers: Vec::new() };
    let mut quadrants = [empty(), empty(), empty(), empty()];
    let mut base: [Option<FormId>; 4] = [None; 4];
    // Additional layers: (quadrant, layer index, ltex, opacity)
    let mut extra: Vec<(usize, u16, FormId, Vec<f32>)> = Vec::new();
    let mut cur_extra: Option<usize> = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"VHGT" if sr.data.len() >= 4 + VERTS * VERTS => {
                let mut offset = sr.f32(0);
                for r in 0..VERTS {
                    offset += sr.data[4 + r * VERTS] as i8 as f32;
                    let mut row = offset;
                    heights[r * VERTS] = row * 8.0;
                    for c in 1..VERTS {
                        row += sr.data[4 + r * VERTS + c] as i8 as f32;
                        heights[r * VERTS + c] = row * 8.0;
                    }
                }
            }
            b"VNML" if sr.data.len() >= VERTS * VERTS * 3 => {
                for i in 0..VERTS * VERTS {
                    let n = Vec3::new(
                        sr.data[i * 3] as i8 as f32,
                        sr.data[i * 3 + 1] as i8 as f32,
                        sr.data[i * 3 + 2] as i8 as f32,
                    );
                    normals[i] = n.normalize_or(Vec3::Z);
                }
            }
            b"VCLR" if sr.data.len() >= VERTS * VERTS * 3 => {
                for i in 0..VERTS * VERTS {
                    colors[i] = Vec3::new(sr.data[i * 3] as f32, sr.data[i * 3 + 1] as f32, sr.data[i * 3 + 2] as f32) / 255.0;
                }
            }
            b"BTXT" => {
                let q = sr.u8(4) as usize;
                if q < 4 {
                    base[q] = Some(rec.fid(sr.form_id(0)));
                }
            }
            b"ATXT" => {
                let q = sr.u8(4) as usize;
                let layer = sr.u16(6);
                if q < 4 {
                    extra.push((q, layer, rec.fid(sr.form_id(0)), vec![0.0; QUAD_VERTS * QUAD_VERTS]));
                    cur_extra = Some(extra.len() - 1);
                } else {
                    cur_extra = None;
                }
            }
            b"VTXT" => {
                if let Some(i) = cur_extra {
                    for e in sr.data.chunks_exact(8) {
                        let pos = u16::from_le_bytes([e[0], e[1]]) as usize;
                        let op = f32::from_le_bytes(e[4..8].try_into().unwrap());
                        if pos < QUAD_VERTS * QUAD_VERTS {
                            extra[i].3[pos] = op;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let resolve = |id: Option<FormId>| -> (String, Option<String>) {
        id.and_then(|id| land_texture(lo, id)).unwrap_or_else(|| (DEFAULT_TEXTURE.to_owned(), None))
    };
    for (q, quad) in quadrants.iter_mut().enumerate() {
        let (d, n) = resolve(base[q]);
        quad.layers.push(Layer { diffuse: d, normal: n, opacity: vec![1.0; QUAD_VERTS * QUAD_VERTS], ltex: base[q] });
        let mut layers: Vec<&(usize, u16, FormId, Vec<f32>)> = extra.iter().filter(|e| e.0 == q).collect();
        layers.sort_by_key(|e| e.1);
        for e in layers.into_iter().take(MAX_LAYERS - 1) {
            let (d, n) = resolve(Some(e.2));
            quad.layers.push(Layer { diffuse: d, normal: n, opacity: e.3.clone(), ltex: Some(e.2) });
        }
    }
    Some(Land { x, y, heights, normals, colors, quadrants })
}
