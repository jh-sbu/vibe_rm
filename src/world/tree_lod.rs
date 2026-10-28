//! Vanilla tree LOD: LST atlas rectangles and BTT placed crossed cards.
//! Independent parser based on public format fields, checked against installed
//! assets. See research/ui/system-page.md; no xEdit implementation copied.
use crate::render::{
    GpuModel, Instance, Renderer,
    distance::Class,
    model::{CpuMesh, CpuModel, MaterialDesc, Vertex},
};
use crate::save::codec::{Corrupt, Reader, Result};
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Clone, Debug)]
pub struct Kind {
    pub id: u32,
    pub width: f32,
    pub height: f32,
    pub uv: [f32; 4],
}
#[derive(Clone, Debug)]
pub struct Tree {
    pub kind: u32,
    pub pos: Vec3,
    pub rotation: f32,
    pub scale: f32,
    pub reference: u32,
}
pub fn read_kinds(bytes: &[u8]) -> Result<Vec<Kind>> {
    let mut r = Reader::new(bytes);
    let mut kinds = Vec::new();
    let mut seen = HashSet::new();
    for _ in 0..r.count(32)? {
        let id = r.u32()?;
        let width = r.f32()?;
        let height = r.f32()?;
        let uv = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        r.u32()?;
        if !seen.insert(id)
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || uv.iter().any(|v| !v.is_finite())
            || uv[0] >= uv[2]
            || uv[1] >= uv[3]
        {
            return Err(Corrupt("invalid tree atlas entry".into()));
        }
        kinds.push(Kind {
            id,
            width,
            height,
            uv,
        });
    }
    r.finish()?;
    Ok(kinds)
}
pub fn read_trees(bytes: &[u8]) -> Result<Vec<Tree>> {
    let mut r = Reader::new(bytes);
    let mut trees = Vec::new();
    for _ in 0..r.count(8)? {
        let kind = r.u32()?;
        for _ in 0..r.count(32)? {
            let pos = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let rotation = r.f32()?;
            let scale = r.f32()?;
            let reference = r.u32()?;
            r.u32()?;
            r.u32()?;
            if !pos.is_finite() || !rotation.is_finite() || !scale.is_finite() || scale <= 0. {
                return Err(Corrupt("invalid tree LOD reference".into()));
            }
            trees.push(Tree {
                kind,
                pos,
                rotation,
                scale,
                reference,
            });
        }
    }
    r.finish()?;
    Ok(trees)
}
fn model(kind: &Kind, atlas: &str) -> CpuModel {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let [u0, v0, u1, v1] = kind.uv;
    for quarter in 0..2 {
        let rot = Mat4::from_rotation_z(quarter as f32 * std::f32::consts::FRAC_PI_2);
        let base = vertices.len() as u32;
        for (x, z, uv) in [
            (-0.5, 0., [u0, v1]),
            (0.5, 0., [u1, v1]),
            (0.5, 1., [u1, v0]),
            (-0.5, 1., [u0, v0]),
        ] {
            vertices.push(Vertex {
                position: rot
                    .transform_point3(Vec3::new(x * kind.width, 0., z * kind.height))
                    .to_array(),
                normal: rot.transform_vector3(-Vec3::Y).to_array(),
                tangent: rot.transform_vector3(Vec3::X).to_array(),
                bitangent: Vec3::Z.to_array(),
                uv,
                color: [1.; 4],
            });
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let center = Vec3::Z * (kind.height * 0.5);
    let radius = Vec2::new(kind.width * 0.5, kind.height * 0.5).length();
    CpuModel {
        meshes: vec![CpuMesh {
            vertices,
            indices,
            bound_center: center,
            bound_radius: radius,
            material: MaterialDesc {
                diffuse: Some(atlas.into()),
                alpha_test: Some(0.5),
                double_sided: true,
                flags1: nif::sf1::ZBUFFER_TEST,
                flags2: nif::sf2::ZBUFFER_WRITE,
                ..Default::default()
            },
        }],
        skinned: Vec::new(),
        bound_center: center,
        bound_radius: radius,
        animated: Vec::new(),
        sequences: Vec::new(),
        particles: Vec::new(),
        addons: Vec::new(),
    }
}
pub struct Trees {
    world: String,
    available: HashMap<(i32, i32), String>,
    loaded: HashMap<(i32, i32), Vec<Instance>>,
    models: HashMap<u32, Arc<GpuModel>>,
}
impl Trees {
    pub fn new(vfs: &vfs::Vfs, renderer: &mut Renderer, world: &str) -> Option<Self> {
        let world = world.to_ascii_lowercase();
        let prefix = format!("meshes/terrain/{world}/trees/");
        let bytes = vfs.read(&format!("{prefix}{world}.lst"))?;
        let kinds = match read_kinds(&bytes) {
            Ok(k) => k,
            Err(e) => {
                log::warn!("tree LOD {world}: {e}");
                return None;
            }
        };
        let atlas = format!("textures/terrain/{world}/trees/{world}treelod.dds");
        let bytes = vfs.read(&atlas)?;
        let dds = crate::render::dds::parse(&bytes).ok()?;
        renderer.add_texture(&atlas, Some(&dds));
        let models = kinds
            .iter()
            .map(|kind| {
                (
                    kind.id,
                    Arc::new(renderer.upload_model(&model(kind, &atlas))),
                )
            })
            .collect();
        let mut available = HashMap::new();
        for path in vfs.list(&prefix) {
            let Some(stem) = path.rsplit('/').next().and_then(|s| s.strip_suffix(".btt")) else {
                continue;
            };
            let parts: Vec<_> = stem.split('.').collect();
            if parts.len() == 4
                && parts[1] == "4"
                && let (Ok(x), Ok(y)) = (parts[2].parse(), parts[3].parse())
            {
                available.insert((x, y), path);
            }
        }
        log::info!(
            "tree LOD {world}: {} kinds, {} blocks",
            kinds.len(),
            available.len()
        );
        Some(Self {
            world,
            available,
            loaded: HashMap::new(),
            models,
        })
    }
    pub fn stream(&mut self, vfs: &vfs::Vfs, camera: Vec3, radius: i32) -> bool {
        let reach = (radius.max(0) as f32 + 8.) * 4096.;
        let mut want: Vec<_> = self
            .available
            .keys()
            .copied()
            .filter(|&(x, y)| {
                let lo = Vec2::new(x as f32, y as f32) * 4096.;
                let hi = lo + Vec2::splat(16384.);
                let nearest = camera.truncate().clamp(lo, hi);
                nearest.distance(camera.truncate()) <= reach
            })
            .collect();
        want.sort_by(|&(ax, ay), &(bx, by)| {
            let distance = |x: i32, y: i32| {
                Vec2::new((x + 2) as f32, (y + 2) as f32)
                    .distance_squared(camera.truncate() / 4096.)
            };
            distance(ax, ay)
                .total_cmp(&distance(bx, by))
                .then((ax, ay).cmp(&(bx, by)))
        });
        let keep: HashSet<_> = want.iter().copied().collect();
        let old = self.loaded.len();
        self.loaded.retain(|k, _| keep.contains(k));
        let mut changed = self.loaded.len() != old;
        let missing: Vec<_> = want
            .into_iter()
            .filter(|k| !self.loaded.contains_key(k))
            .take(4)
            .collect();
        for key in missing {
            let rows = vfs
                .read(&self.available[&key])
                .and_then(|b| match read_trees(&b) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        log::warn!("tree LOD {} {:?}: {e}", self.world, key);
                        None
                    }
                })
                .unwrap_or_default();
            let instances = rows
                .into_iter()
                .filter_map(|tree| {
                    let model = self.models.get(&tree.kind)?.clone();
                    let transform = Mat4::from_translation(tree.pos)
                        * Mat4::from_rotation_z(-tree.rotation)
                        * Mat4::from_scale(Vec3::splat(tree.scale));
                    let mut instance = Instance::new(model, transform);
                    instance.ref_id = tree.reference;
                    instance.fade_class = Class::TreeLod;
                    instance.tint = Vec4::ONE;
                    Some(instance)
                })
                .collect();
            self.loaded.insert(key, instances);
            changed = true;
        }
        changed
    }
    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.loaded.values().flatten()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::codec::Writer;
    #[test]
    fn bounded_lod_parsers_and_crossed_cards() {
        let mut w = Writer::default();
        w.u32(1);
        w.u32(7);
        for f in [100., 200., -0.0005, 0., 0.5, 1.001] {
            w.f32(f);
        }
        w.u32(0);
        let kinds = read_kinds(&w.0).unwrap();
        let mesh = model(&kinds[0], "synthetic.dds");
        assert_eq!(mesh.meshes[0].vertices.len(), 8);
        assert_eq!(mesh.meshes[0].indices.len(), 12);
        assert!(read_kinds(&w.0[..w.0.len() - 1]).is_err());
        let mut w = Writer::default();
        w.u32(1);
        w.u32(7);
        w.u32(1);
        for f in [10., 20., 30., 0., 1.] {
            w.f32(f);
        }
        w.u32(0x800);
        w.u32(0);
        w.u32(0);
        let trees = read_trees(&w.0).unwrap();
        assert_eq!(trees[0].reference, 0x800);
        assert_eq!(trees[0].pos, Vec3::new(10., 20., 30.));
        let mut bad = w.0;
        bad.extend([0]);
        assert!(read_trees(&bad).is_err());
        assert!(read_trees(&u32::MAX.to_le_bytes()).is_err());
    }
}
