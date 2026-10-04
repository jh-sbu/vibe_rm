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
}

impl ModelCache {
    pub fn get(&self, path: &str) -> Option<Arc<GpuModel>> {
        self.map.get(path).cloned().flatten()
    }
    pub fn len(&self) -> usize {
        self.map.len()
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
        let cpu: Vec<(String, Option<CpuModel>)> = missing
            .par_iter()
            .map(|p| {
                let m = vfs.read(p).and_then(|data| match nif::Nif::parse(&data) {
                    Ok(n) => Some(model::convert(&n)),
                    Err(e) => {
                        log::warn!("{p}: {e}");
                        None
                    }
                });
                if m.is_none() {
                    log::debug!("model not found or unreadable: {p}");
                }
                ((*p).clone(), m)
            })
            .collect();
        let t1 = std::time::Instant::now();

        // Textures referenced by the new models.
        let mut tex: HashSet<String> = HashSet::new();
        for (_, m) in &cpu {
            if let Some(m) = m {
                for mesh in &m.meshes {
                    for t in [&mesh.material.diffuse, &mesh.material.normal, &mesh.material.glow].into_iter().flatten() {
                        if !renderer.textures.contains(t) {
                            tex.insert(t.clone());
                        }
                    }
                }
            }
        }
        load_textures(renderer, vfs, tex.into_iter().collect());
        let t2 = std::time::Instant::now();

        for (p, m) in cpu {
            let g = m.filter(|m| !m.meshes.is_empty()).map(|m| Arc::new(renderer.upload_model(&m)));
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
                Ok(d) => Some(d),
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
