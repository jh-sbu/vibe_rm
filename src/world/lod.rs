//! Distant land: pre-generated terrain (BTR) and object (BTO) LOD blocks.
//!
//! Blocks of `level` x `level` cells are stored scaled down by `level`; the
//! object blocks carry their world origin in the shape transform, terrain
//! blocks are relative to the block's south-west cell corner.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};

use crate::render::{GpuModel, Instance};
use crate::world::terrain::CELL_SIZE;

pub const LEVELS: [i32; 4] = [32, 16, 8, 4];

#[derive(Default)]
pub struct Lod {
    pub world: String,
    available: HashSet<(i32, i32, i32, bool)>,
    loaded: HashMap<(i32, i32, i32), Vec<Instance>>,
    pub dirty: bool,
}

impl Lod {
    pub fn new(vfs: &vfs::Vfs, world_edid: &str) -> Lod {
        let world = world_edid.to_ascii_lowercase();
        let mut available = HashSet::new();
        for (prefix, objects) in [
            (format!("meshes/terrain/{world}/"), false),
            (format!("meshes/terrain/{world}/objects/"), true),
        ] {
            for p in vfs.list(&prefix) {
                let name = p.rsplit('/').next().unwrap_or("");
                let ext = if objects { ".bto" } else { ".btr" };
                let Some(stem) = name.strip_suffix(ext) else {
                    continue;
                };
                let parts: Vec<&str> = stem.split('.').collect();
                if parts.len() == 4
                    && let (Ok(l), Ok(x), Ok(y)) =
                        (parts[1].parse(), parts[2].parse(), parts[3].parse())
                {
                    available.insert((l, x, y, objects));
                }
            }
        }
        log::info!("lod: {} blocks for {world}", available.len());
        Lod {
            world,
            available,
            loaded: HashMap::new(),
            dirty: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }

    /// Blocks to show for a camera position: a quadtree refined near the viewer.
    pub fn desired(&self, cam: Vec2) -> Vec<(i32, i32, i32)> {
        let mut out = Vec::new();
        let roots: HashSet<(i32, i32)> = self
            .available
            .iter()
            .filter(|b| b.0 == 32 && !b.3)
            .map(|b| (b.1, b.2))
            .collect();
        for (x, y) in roots {
            self.refine(32, x, y, cam, &mut out);
        }
        out
    }

    fn refine(&self, level: i32, x: i32, y: i32, cam: Vec2, out: &mut Vec<(i32, i32, i32)>) {
        let size = level as f32 * CELL_SIZE;
        let min = Vec2::new(x as f32, y as f32) * CELL_SIZE;
        let closest = cam.clamp(min, min + Vec2::splat(size));
        let dist = closest.distance(cam);
        let half = level / 2;
        let children = [(x, y), (x + half, y), (x, y + half), (x + half, y + half)];
        let can_split = level > 4
            && children
                .iter()
                .all(|c| self.available.contains(&(half, c.0, c.1, false)));
        if can_split && dist < size * 1.2 {
            for c in children {
                self.refine(half, c.0, c.1, cam, out);
            }
        } else {
            out.push((level, x, y));
        }
    }

    pub fn has(&self, level: i32, x: i32, y: i32, objects: bool) -> bool {
        self.available.contains(&(level, x, y, objects))
    }

    pub fn paths(&self, level: i32, x: i32, y: i32) -> (String, String) {
        let w = &self.world;
        (
            format!("meshes/terrain/{w}/{w}.{level}.{x}.{y}.btr"),
            format!("meshes/terrain/{w}/objects/{w}.{level}.{x}.{y}.bto"),
        )
    }

    pub fn loaded_keys(&self) -> Vec<(i32, i32, i32)> {
        self.loaded.keys().copied().collect()
    }

    pub fn remove(&mut self, key: (i32, i32, i32)) {
        self.loaded.remove(&key);
        self.dirty = true;
    }

    pub fn insert(&mut self, key: (i32, i32, i32), instances: Vec<Instance>) {
        self.loaded.insert(key, instances);
        self.dirty = true;
    }

    pub fn is_loaded(&self, key: (i32, i32, i32)) -> bool {
        self.loaded.contains_key(&key)
    }

    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.loaded.values().flatten()
    }
}

/// Load a LOD block NIF, applying the block's scale (and origin for terrain).
pub fn load_block(
    vfs: &vfs::Vfs,
    path: &str,
    level: i32,
    x: i32,
    y: i32,
    terrain: bool,
) -> Option<crate::render::model::CpuModel> {
    let mut n = nif::Nif::parse(&vfs.read(path)?).ok()?;
    let s = level as f32;
    for b in &mut n.blocks {
        let av = match b {
            nif::Block::TriShape(t) => &mut t.av,
            nif::Block::NiTriShape(g) | nif::Block::NiTriStrips(g) => &mut g.av,
            nif::Block::Node(node) if node.av.net.name.eq_ignore_ascii_case("WATER") => {
                // LOD water is drawn by the water renderer instead.
                node.av.flags |= 1;
                continue;
            }
            _ => continue,
        };
        av.transform.scale *= s;
        if terrain {
            av.transform.translation = Vec3::new(x as f32 * CELL_SIZE, y as f32 * CELL_SIZE, 0.0);
        }
    }
    let mut m = crate::render::model::convert(&n);
    m.meshes.retain(|mesh| mesh.material.diffuse.is_some());
    for mesh in &mut m.meshes {
        mesh.material.lod = true;
        // Distant geometry is never double sided and casts no detail lighting.
        mesh.material.specular = Vec3::ZERO;
    }
    Some(m)
}

pub fn instance(model: Arc<GpuModel>) -> Instance {
    Instance::new(model, Mat4::IDENTITY)
}
