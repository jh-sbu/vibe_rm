//! Resource loading: models and textures for a set of placed objects,
//! parsed in parallel and uploaded to the GPU.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::prelude::*;

use crate::render::model::{self, CpuModel};
use crate::render::{GpuModel, Renderer, dds};

#[derive(Default)]
pub struct ModelCache {
    map: HashMap<String, Option<Arc<GpuModel>>>,
    collision: crate::physics::CollisionCache,
}

impl ModelCache {
    pub fn get(&self, path: &str) -> Option<Arc<GpuModel>> {
        self.map.get(path).cloned().flatten()
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn collision(&self, path: &str) -> Option<Arc<crate::physics::shapes::CollisionModel>> {
        self.collision.get(path).cloned().flatten()
    }

    /// Ensure all `paths` are loaded (in parallel where possible).
    pub fn load_all(&mut self, renderer: &mut Renderer, vfs: &vfs::Vfs, paths: &[String]) {
        let missing: Vec<&String> = {
            let mut seen = HashSet::new();
            paths.iter().filter(|p| !self.map.contains_key(*p) && seen.insert(*p)).collect()
        };
        if missing.is_empty() {
            return;
        }
        let t0 = std::time::Instant::now();
        let cpu: Vec<(String, Option<CpuModel>, Option<crate::physics::shapes::CollisionModel>)> = missing
            .par_iter()
            .map(|p| {
                let mut col = None;
                let m = vfs.read(p).and_then(|data| match nif::Nif::parse(&data) {
                    Ok(n) => {
                        col = crate::physics::shapes::from_nif(&n);
                        let m = model::convert(&n);
                        for mesh in &m.meshes {
                            log::trace!("mesh in {p}: {:?}", mesh.material);
                        }
                        Some(m)
                    }
                    Err(e) => {
                        log::warn!("{p}: {e}");
                        None
                    }
                });
                if m.is_none() {
                    log::debug!("model not found or unreadable: {p}");
                }
                ((*p).clone(), m, col)
            })
            .collect();
        let t1 = std::time::Instant::now();

        // Textures referenced by the new models.
        let mut tex: HashSet<String> = HashSet::new();
        for (_, m, _) in &cpu {
            if let Some(m) = m {
                let mats = m.meshes.iter().map(|x| &x.material).chain(m.skinned.iter().map(|x| &x.material));
                for mat in mats {
                    for t in [&mat.diffuse, &mat.normal, &mat.glow].into_iter().flatten() {
                        if !renderer.textures.contains(t) {
                            tex.insert(t.clone());
                        }
                    }
                }
            }
        }
        load_textures(renderer, vfs, tex.into_iter().collect());
        let t2 = std::time::Instant::now();

        for (p, m, col) in cpu {
            self.collision.insert(p.clone(), col.map(Arc::new));
            let g = m.filter(|m| !m.meshes.is_empty() || !m.skinned.is_empty()).map(|m| {
                let mut g = renderer.upload_model(&m);
                g.path = p.clone();
                Arc::new(g)
            });
            self.map.insert(p, g);
        }
        log::info!(
            "loaded {} models: parse {:?}, textures {:?}, upload {:?}",
            missing.len(),
            t1 - t0,
            t2 - t1,
            t2.elapsed()
        );
    }
}

pub fn load_textures(renderer: &mut Renderer, vfs: &vfs::Vfs, paths: Vec<String>) {
    let parsed: Vec<(String, Option<dds::Dds>)> = paths
        .into_par_iter()
        .map(|p| {
            let d = vfs.read(&p).and_then(|data| match dds::parse(&data) {
                Ok(d) => {
                    log::trace!("texture {p}: {:?} {}x{} mips {} layers {}", d.format, d.width, d.height, d.mips, d.layers);
                    Some(d)
                }
                Err(e) => {
                    log::warn!("{p}: {e}");
                    None
                }
            });
            if d.is_none() {
                log::debug!("missing texture {p}");
            }
            (p, d)
        })
        .collect();
    for (p, d) in parsed {
        renderer.add_texture(&p, d.as_ref());
    }
}
