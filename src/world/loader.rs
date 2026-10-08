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
    furniture: HashMap<String, Option<Arc<[nif::FurnitureMarker]>>>,
    anims: HashMap<String, Arc<ModelAnim>>,
}

/// Keyframe-animated parts of a model and the sequences that move them.
pub struct ModelAnim {
    pub parts: Vec<AnimPart>,
    pub sequences: Vec<model::Sequence>,
}

pub struct AnimPart {
    pub node: String,
    pub parent: glam::Mat4,
    pub rest: glam::Mat4,
    pub model: Arc<GpuModel>,
}

fn furniture_markers(n: &nif::Nif) -> Option<Arc<[nif::FurnitureMarker]>> {
    n.blocks.iter().find_map(|b| match b {
        nif::Block::ExtraData(nif::ExtraData::Furniture(m)) => Some(m.as_slice().into()),
        _ => None,
    })
}

impl ModelCache {
    pub fn get(&self, path: &str) -> Option<Arc<GpuModel>> {
        self.map.get(path).cloned().flatten()
    }
    /// Animated parts and sequences of a loaded model, if it has any.
    pub fn anim(&self, path: &str) -> Option<Arc<ModelAnim>> {
        self.anims.get(path).cloned()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn collision(&self, path: &str) -> Option<Arc<crate::physics::shapes::CollisionModel>> {
        self.collision.get(path).cloned().flatten()
    }

    /// Furniture markers of a model (parsed on demand if it isn't loaded).
    pub fn furniture(&mut self, vfs: &vfs::Vfs, path: &str) -> Option<Arc<[nif::FurnitureMarker]>> {
        if let Some(m) = self.furniture.get(path) {
            return m.clone();
        }
        let m = vfs
            .read(path)
            .and_then(|d| nif::Nif::parse(&d).ok())
            .and_then(|n| furniture_markers(&n));
        self.furniture.insert(path.to_owned(), m.clone());
        m
    }

    /// Ensure all `paths` are loaded (in parallel where possible).
    pub fn load_all(&mut self, renderer: &mut Renderer, vfs: &vfs::Vfs, paths: &[String]) {
        let missing: Vec<&String> = {
            let mut seen = HashSet::new();
            paths
                .iter()
                .filter(|p| !self.map.contains_key(*p) && seen.insert(*p))
                .collect()
        };
        if missing.is_empty() {
            return;
        }
        let t0 = std::time::Instant::now();
        #[allow(clippy::type_complexity)]
        let cpu: Vec<(
            String,
            Option<CpuModel>,
            Option<crate::physics::shapes::CollisionModel>,
            Option<Arc<[nif::FurnitureMarker]>>,
        )> = missing
            .par_iter()
            .map(|p| {
                let mut col = None;
                let mut furn = None;
                // `path#blade` / `path#scb`: a weapon without / only its scabbard.
                let (file, variant) = p
                    .split_once('#')
                    .map_or((p.as_str(), None), |(f, v)| (f, Some(v)));
                let m = vfs
                    .read(file)
                    .and_then(|data| match nif::Nif::parse(&data) {
                        Ok(n) => {
                            col = crate::physics::shapes::from_nif(&n);
                            furn = furniture_markers(&n);
                            let m = match variant {
                                Some("scb") => model::convert_filtered(&n, &|name| {
                                    name.to_ascii_lowercase().starts_with("scb")
                                }),
                                Some(_) => model::convert_filtered(&n, &|name| {
                                    !name.to_ascii_lowercase().starts_with("scb")
                                }),
                                None => model::convert_split(
                                    &n,
                                    &|_| true,
                                    &col.as_ref().map(|c| c.body_nodes()).unwrap_or_default(),
                                ),
                            };
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
                ((*p).clone(), m, col, furn)
            })
            .collect();
        let t1 = std::time::Instant::now();

        // Textures referenced by the new models.
        let mut tex: HashSet<String> = HashSet::new();
        for (_, m, _, _) in &cpu {
            if let Some(m) = m {
                let parts = m
                    .animated
                    .iter()
                    .flat_map(|a| a.model.meshes.iter().map(|x| &x.material));
                let mats = m
                    .meshes
                    .iter()
                    .map(|x| &x.material)
                    .chain(m.skinned.iter().map(|x| &x.material))
                    .chain(parts);
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

        for (p, mut m, col, furn) in cpu {
            self.collision.insert(p.clone(), col.map(Arc::new));
            self.furniture.insert(p.clone(), furn);
            if let Some(cpu) = m.as_mut().filter(|m| !m.animated.is_empty()) {
                let parts = std::mem::take(&mut cpu.animated)
                    .into_iter()
                    .filter(|a| !a.model.meshes.is_empty())
                    .map(|a| {
                        let mut g = renderer.upload_model(&a.model);
                        g.path = format!("{p}#{}", a.node);
                        AnimPart {
                            node: a.node,
                            parent: a.parent,
                            rest: a.rest,
                            model: Arc::new(g),
                        }
                    })
                    .collect();
                let sequences = std::mem::take(&mut cpu.sequences);
                self.anims
                    .insert(p.clone(), Arc::new(ModelAnim { parts, sequences }));
            }
            let g = m
                .filter(|m| !m.meshes.is_empty() || !m.skinned.is_empty())
                .map(|m| {
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
                    log::trace!(
                        "texture {p}: {:?} {}x{} mips {} layers {}",
                        d.format,
                        d.width,
                        d.height,
                        d.mips,
                        d.layers
                    );
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
