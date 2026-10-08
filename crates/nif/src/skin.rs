//! Skinning blocks.

use glam::{Mat3, Vec3};

use crate::Result;
use crate::blocks::{Geometry, Ref, Transform};
use crate::reader::Reader;

#[derive(Debug, Clone)]
pub struct SkinInstance {
    pub data: Ref,
    pub partition: Ref,
    pub skeleton_root: Ref,
    pub bones: Vec<Ref>,
}

#[derive(Debug, Clone)]
pub struct BoneData {
    /// Transform from skin (mesh) space into this bone's space.
    pub transform: Transform,
    pub bound_center: Vec3,
    pub bound_radius: f32,
    /// (vertex index, weight) — only present on older/non-partitioned data.
    pub weights: Vec<(u16, f32)>,
}

#[derive(Debug, Clone)]
pub struct SkinData {
    pub skin_transform: Transform,
    pub bones: Vec<BoneData>,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub num_vertices: u16,
    pub bones: Vec<u16>,
    pub vertex_map: Vec<u16>,
    pub weights: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u8; 4]>,
    pub triangles: Vec<[u16; 3]>,
    pub weights_per_vertex: u16,
}

#[derive(Debug, Clone)]
pub struct SkinPartition {
    /// SSE: the shape's vertex data lives here.
    pub vertex_desc: u64,
    pub geometry: Geometry,
    pub partitions: Vec<Partition>,
}

fn ni_transform(r: &mut Reader) -> Result<Transform> {
    let rotation: Mat3 = r.mat3()?;
    let translation = r.vec3()?;
    let scale = r.f32()?;
    Ok(Transform {
        translation,
        rotation,
        scale,
    })
}

pub(crate) fn skin_instance(r: &mut Reader, dismember: bool) -> Result<SkinInstance> {
    let data = r.block_ref()?;
    let partition = r.block_ref()?;
    let skeleton_root = r.block_ref()?;
    let bones = r.ref_list()?;
    if dismember {
        let n = r.u32()? as usize;
        r.skip(n * 4)?;
    }
    Ok(SkinInstance {
        data,
        partition,
        skeleton_root,
        bones,
    })
}

pub(crate) fn skin_data(r: &mut Reader) -> Result<SkinData> {
    let skin_transform = ni_transform(r)?;
    let n = r.u32()? as usize;
    let has_weights = r.bool()?;
    let mut bones = Vec::with_capacity(n);
    for _ in 0..n {
        let transform = ni_transform(r)?;
        let bound_center = r.vec3()?;
        let bound_radius = r.f32()?;
        let nv = r.u16()? as usize;
        let mut weights = Vec::new();
        if has_weights {
            weights.reserve(nv);
            for _ in 0..nv {
                weights.push((r.u16()?, r.f32()?));
            }
        }
        bones.push(BoneData {
            transform,
            bound_center,
            bound_radius,
            weights,
        });
    }
    Ok(SkinData {
        skin_transform,
        bones,
    })
}

pub(crate) fn skin_partition(r: &mut Reader) -> Result<SkinPartition> {
    let np = r.u32()? as usize;
    let mut sp = SkinPartition {
        vertex_desc: 0,
        geometry: Geometry::default(),
        partitions: Vec::with_capacity(np),
    };
    if r.bs_version == 100 {
        let data_size = r.u32()? as usize;
        let vertex_size = r.u32()? as usize;
        sp.vertex_desc = r.u64()?;
        if data_size > 0 && vertex_size > 0 {
            crate::blocks::read_vertex_data(
                r,
                sp.vertex_desc,
                data_size / vertex_size,
                &mut sp.geometry,
            )?;
        }
    }
    for _ in 0..np {
        let num_vertices = r.u16()?;
        let num_triangles = r.u16()? as usize;
        let num_bones = r.u16()? as usize;
        let num_strips = r.u16()? as usize;
        let wpv = r.u16()?;
        let mut bones = Vec::with_capacity(num_bones);
        for _ in 0..num_bones {
            bones.push(r.u16()?);
        }
        let mut vertex_map = Vec::new();
        if r.bool()? {
            for _ in 0..num_vertices {
                vertex_map.push(r.u16()?);
            }
        }
        let mut weights = Vec::new();
        if r.bool()? {
            for _ in 0..num_vertices {
                let mut w = [0f32; 4];
                for k in 0..wpv as usize {
                    let v = r.f32()?;
                    if k < 4 {
                        w[k] = v;
                    }
                }
                weights.push(w);
            }
        }
        let mut strip_lengths = Vec::with_capacity(num_strips);
        for _ in 0..num_strips {
            strip_lengths.push(r.u16()? as usize);
        }
        let mut triangles = Vec::new();
        if r.bool()? {
            if num_strips == 0 {
                for _ in 0..num_triangles {
                    triangles.push([r.u16()?, r.u16()?, r.u16()?]);
                }
            } else {
                for len in &strip_lengths {
                    let mut pts = Vec::with_capacity(*len);
                    for _ in 0..*len {
                        pts.push(r.u16()?);
                    }
                    for i in 2..*len {
                        let (a, b, c) = (pts[i - 2], pts[i - 1], pts[i]);
                        if a != b && b != c && a != c {
                            triangles.push(if i % 2 == 0 { [a, b, c] } else { [a, c, b] });
                        }
                    }
                }
            }
        }
        let mut bone_indices = Vec::new();
        if r.bool()? {
            for _ in 0..num_vertices {
                let mut b = [0u8; 4];
                for k in 0..wpv as usize {
                    let v = r.u8()?;
                    if k < 4 {
                        b[k] = v;
                    }
                }
                bone_indices.push(b);
            }
        }
        if r.bs_version == 100 {
            r.u8()?; // LOD level
            r.u8()?; // global VB
            r.u64()?; // vertex desc
            // Triangles in terms of the global vertex buffer.
            let mut tris = Vec::with_capacity(num_triangles);
            for _ in 0..num_triangles {
                tris.push([r.u16()?, r.u16()?, r.u16()?]);
            }
            triangles = tris;
        }
        sp.partitions.push(Partition {
            num_vertices,
            bones,
            vertex_map,
            weights,
            bone_indices,
            triangles,
            weights_per_vertex: wpv,
        });
    }
    Ok(sp)
}
