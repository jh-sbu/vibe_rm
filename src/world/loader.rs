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
    /// Addon nodes' models (`meshes/...`) by their index (ADDN `DATA`).
    addons: Arc<HashMap<i32, String>>,
}

/// The addon nodes' models by index (ADDN `DATA`, `MODL`).
pub fn addon_models(lo: &esp::LoadOrder) -> HashMap<i32, String> {
    let mut out = HashMap::new();
    for &id in lo.ids_of_type(b"ADDN") {
        let Some(rec) = lo.get(id) else { continue };
        let Some(index) = rec.get(b"DATA").and_then(|d| d.get(..4)) else {
            continue;
        };
        let index = i32::from_le_bytes(index.try_into().unwrap());
        if let Some(path) = super::records::model_path(&rec) {
            out.insert(index, path);
        }
    }
    out
}

/// Place the models of a model's addon nodes in it (and in its animated
/// parts'), each converted at its node. Addons of addons aren't followed.
fn attach_addons(
    m: &mut CpuModel,
    addons: &HashMap<i32, String>,
    vfs: &vfs::Vfs,
    nifs: &mut HashMap<i32, Option<nif::Nif>>,
) {
    for part in &mut m.animated {
        attach_addons(&mut part.model, addons, vfs, nifs);
    }
    for (index, at) in std::mem::take(&mut m.addons) {
        let nif = nifs.entry(index).or_insert_with(|| {
            let path = addons.get(&index)?;
            let n = vfs.read(path).and_then(|d| nif::Nif::parse(&d).ok());
            if n.is_none() {
                log::debug!("addon node {index}: {path} not found or unreadable");
            }
            n
        });
        if let Some(n) = nif {
            m.merge(model::convert_at(n, at));
        }
    }
}

/// Keyframe-animated parts of a model and the sequences that move them.
pub struct ModelAnim {
    pub parts: Vec<AnimPart>,
    pub sequences: Vec<model::Sequence>,
    /// The behaviour graph project moving the parts (`meshes/...hkx`), and the
    /// model's nodes as the skeleton its clips pose.
    pub graph: Option<(String, Arc<crate::world::skeleton::Skeleton>)>,
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
    /// Set the addon nodes' models ([`addon_models`]).
    pub fn set_addons(&mut self, addons: HashMap<i32, String>) {
        self.addons = Arc::new(addons);
    }

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
        let addons = self.addons.clone();
        #[allow(clippy::type_complexity)]
        let cpu: Vec<(
            String,
            Option<CpuModel>,
            Option<crate::physics::shapes::CollisionModel>,
            Option<Arc<[nif::FurnitureMarker]>>,
            Option<(String, Arc<crate::world::skeleton::Skeleton>)>,
        )> = missing
            .par_iter()
            .map(|p| {
                let mut col = None;
                let mut furn = None;
                let mut graph = None;
                // `path#blade` / `path#scb`: a weapon without / only its scabbard;
                // `path#rigid`: skinned shapes posed at rest (sky models);
                // `path#grass`: drawn as grass ([`crate::world::grass`]).
                let (file, variant) = p
                    .split_once('#')
                    .map_or((p.as_str(), None), |(f, v)| (f, Some(v)));
                let m = vfs
                    .read(file)
                    .and_then(|data| match nif::Nif::parse(&data) {
                        Ok(n) => {
                            col = crate::physics::shapes::from_nif(&n);
                            furn = furniture_markers(&n);
                            graph = n.behavior_graph().map(|f| {
                                (
                                    format!("meshes/{}", f.replace('\\', "/").to_ascii_lowercase()),
                                    Arc::new(crate::world::skeleton::Skeleton::from_nif(&n)),
                                )
                            });
                            let m = match variant {
                                Some("rigid") => model::convert_rigid(&n),
                                // Grass: its vertex alpha is how far it sways.
                                Some("grass") => {
                                    let mut m = model::convert(&n);
                                    for mesh in &mut m.meshes {
                                        mesh.material.grass = true;
                                        mesh.material.flags1 &= !nif::sf1::VERTEX_ALPHA;
                                    }
                                    m
                                }
                                Some("scb") => model::convert_filtered(&n, &|name| {
                                    name.to_ascii_lowercase().starts_with("scb")
                                }),
                                Some(_) => model::convert_filtered(&n, &|name| {
                                    !name.to_ascii_lowercase().starts_with("scb")
                                }),
                                None => {
                                    // Nodes of bodies of their own, and those a
                                    // behaviour graph moves, are drawn apart.
                                    let mut split =
                                        col.as_ref().map(|c| c.body_nodes()).unwrap_or_default();
                                    if graph.is_some() {
                                        split.extend(n.animated_nodes());
                                    }
                                    model::convert_split(&n, &|_| true, &split)
                                }
                            };
                            let mut m = m;
                            if !m.addons.is_empty() {
                                attach_addons(&mut m, &addons, vfs, &mut HashMap::new());
                            }
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
                ((*p).clone(), m, col, furn, graph)
            })
            .collect();
        let t1 = std::time::Instant::now();

        // Textures referenced by the new models.
        let mut tex: HashSet<String> = HashSet::new();
        for (_, m, _, _, _) in &cpu {
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
                    .chain(m.particles.iter().map(|x| &x.material))
                    .chain(parts);
                for mat in mats {
                    for t in [
                        &mat.diffuse,
                        &mat.normal,
                        &mat.glow,
                        &mat.env,
                        &mat.env_mask,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if !renderer.textures.contains(t) {
                            tex.insert(t.clone());
                        }
                    }
                }
            }
        }
        load_textures(renderer, vfs, tex.into_iter().collect());
        let t2 = std::time::Instant::now();

        for (p, mut m, col, furn, graph) in cpu {
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
                self.anims.insert(
                    p.clone(),
                    Arc::new(ModelAnim {
                        parts,
                        sequences,
                        graph,
                    }),
                );
            }
            let g = m
                .filter(|m| {
                    !m.meshes.is_empty() || !m.skinned.is_empty() || !m.particles.is_empty()
                })
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
