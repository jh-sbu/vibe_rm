//! Skinned meshes in their skeleton's bind pose, as the renderer places them
//! (bone bind × skin-to-bone × vertex), to see where one mesh meets another:
//! a FaceGen head's neck and the body under it.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Vec3};
use nif::{Block, Nif, Ref};

/// One skinned shape posed at the skeleton's bind.
pub struct BindShape {
    pub name: String,
    pub shader_type: u32,
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Vertices on an edge only one triangle uses (the mesh's open borders).
    pub open: Vec<bool>,
    /// Bones the skeleton doesn't have (those vertices stay unposed).
    pub missing_bones: Vec<String>,
}

/// Model-space bind matrices of a skeleton's nodes, by lower-case name.
pub fn skeleton_binds(nif: &Nif) -> HashMap<String, Mat4> {
    fn add(nif: &Nif, r: Ref, parent: Mat4, out: &mut HashMap<String, Mat4>, depth: u32) {
        let Some(Block::Node(n)) = nif.get(r) else {
            return;
        };
        if depth > 128 {
            return;
        }
        let m = parent * n.av.transform.to_mat4();
        out.entry(n.av.net.name.to_ascii_lowercase()).or_insert(m);
        for &c in &n.children {
            add(nif, c, m, out, depth + 1);
        }
    }
    let mut out = HashMap::new();
    for &root in &nif.roots {
        add(nif, Ref(root as i32), Mat4::IDENTITY, &mut out, 0);
    }
    out
}

/// A model's skinned shapes (BSTriShape) posed at the skeleton's bind.
pub fn bind_shapes(nif: &Nif, skeleton: &HashMap<String, Mat4>) -> Vec<BindShape> {
    let mut out = Vec::new();
    for b in &nif.blocks {
        let Block::TriShape(t) = b else { continue };
        let Some(Block::SkinInstance(si)) = nif.get(t.skin) else {
            continue;
        };
        let Some(Block::SkinData(sd)) = nif.get(si.data) else {
            continue;
        };
        let partition = match nif.get(si.partition) {
            Some(Block::SkinPartition(p)) => Some(p.as_ref()),
            _ => None,
        };
        // SSE keeps the vertices in the partition (BSDynamicTriShape: positions
        // in the shape, the rest there), as the renderer reads them.
        let (g, triangles): (&nif::Geometry, Vec<[u32; 3]>) = match partition {
            Some(p) if !p.geometry.uvs.is_empty() || !p.geometry.positions.is_empty() => (
                &p.geometry,
                p.partitions
                    .iter()
                    .flat_map(|q| q.triangles.iter())
                    .map(|t| t.map(u32::from))
                    .collect(),
            ),
            _ => (
                &t.geometry,
                t.geometry
                    .triangles
                    .iter()
                    .map(|t| t.map(u32::from))
                    .collect(),
            ),
        };
        let positions_src = if g.positions.is_empty() {
            &t.geometry.positions
        } else {
            &g.positions
        };
        let mut missing_bones = Vec::new();
        let bones: Vec<Mat4> = si
            .bones
            .iter()
            .zip(&sd.bones)
            .map(|(r, bd)| {
                let name = nif
                    .get(*r)
                    .and_then(|b| b.av())
                    .map(|a| a.net.name.clone())
                    .unwrap_or_default();
                match skeleton.get(&name.to_ascii_lowercase()) {
                    Some(m) => *m * bd.transform.to_mat4(),
                    None => {
                        missing_bones.push(name);
                        Mat4::IDENTITY
                    }
                }
            })
            .collect();
        let positions = positions_src
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let idx = g.bone_indices.get(i).copied().unwrap_or_default();
                let w = g
                    .bone_weights
                    .get(i)
                    .copied()
                    .unwrap_or([1.0, 0.0, 0.0, 0.0]);
                let sum: f32 = w.iter().sum();
                if sum <= 0.0 {
                    return v;
                }
                (0..4)
                    .filter(|&k| w[k] > 0.0)
                    .map(|k| {
                        let m = bones
                            .get(idx[k] as usize)
                            .copied()
                            .unwrap_or(Mat4::IDENTITY);
                        m.transform_point3(v) * (w[k] / sum)
                    })
                    .sum()
            })
            .collect::<Vec<Vec3>>();
        // Vertices split along UV seams are welded first: those edges aren't open.
        let mut welded: HashMap<[i32; 3], u32> = HashMap::new();
        let weld: Vec<u32> = positions_src
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let key = (*p * 1000.0).round().as_ivec3().to_array();
                *welded.entry(key).or_insert(i as u32)
            })
            .collect();
        let id = |v: u32| weld.get(v as usize).copied().unwrap_or(v);
        let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (id(a), id(b));
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut open_ids = std::collections::HashSet::new();
        for (&(a, b), &n) in &edges {
            if n == 1 {
                open_ids.extend([a, b]);
            }
        }
        let open = (0..positions.len() as u32)
            .map(|v| open_ids.contains(&id(v)))
            .collect();
        let shader_type = match nif.get(t.shader) {
            Some(Block::LightingShader(s)) => s.shader_type,
            _ => 0,
        };
        out.push(BindShape {
            name: t.av.net.name.clone(),
            shader_type,
            positions,
            triangles,
            open,
            missing_bones,
        });
    }
    out
}

fn load(v: &vfs::Vfs, path: &str) -> Result<Nif> {
    Ok(nif::Nif::parse(
        &v.read(path).with_context(|| format!("{path} not found"))?,
    )?)
}

/// `nif-bind <data> <skeleton nif> <nif> [shape]`: each skinned shape's
/// vertices at the skeleton's bind, `*` marking those on an open edge.
pub fn nif_bind(v: &vfs::Vfs, args: &[String]) -> Result<()> {
    let skeleton = skeleton_binds(&load(v, &args[0])?);
    let n = load(v, &args[1])?;
    for s in bind_shapes(&n, &skeleton) {
        if args.get(2).is_some_and(|f| !s.name.eq_ignore_ascii_case(f)) {
            continue;
        }
        println!(
            "{} (shader type {}): {} vertices, {} triangles, {} open{}",
            s.name,
            s.shader_type,
            s.positions.len(),
            s.triangles.len(),
            s.open.iter().filter(|o| **o).count(),
            if s.missing_bones.is_empty() {
                String::new()
            } else {
                format!(", bones not in the skeleton {:?}", s.missing_bones)
            }
        );
        for (i, p) in s.positions.iter().enumerate() {
            let o = if s.open[i] { '*' } else { ' ' };
            println!("  {i:5}{o} {:8.3} {:8.3} {:8.3}", p.x, p.y, p.z);
        }
    }
    Ok(())
}

/// `neck-seam <data> <skeleton nif> <head nif> <body _1 nif> [weight]`: how far
/// the FaceGen head's open neck edge lies from the skin of a body (a pair of
/// `_0` / `_1` models) blended at each weight. The head's open-edge vertices
/// within 3 units of the body's open edges at some weight are the neck's.
pub fn neck_seam(v: &vfs::Vfs, args: &[String]) -> Result<()> {
    let skeleton = skeleton_binds(&load(v, &args[0])?);
    let head = bind_shapes(&load(v, &args[1])?, &skeleton);
    let heavy_path = args[2].to_ascii_lowercase();
    let Some(stem) = heavy_path.strip_suffix("_1.nif") else {
        bail!("the body model should be a _1.nif");
    };
    let thin = bind_shapes(&load(v, &format!("{stem}_0.nif"))?, &skeleton);
    let heavy = bind_shapes(&load(v, &heavy_path)?, &skeleton);
    let npc_weight: Option<f32> = args.get(3).map(|w| w.parse()).transpose()?;

    for s in head.iter().chain(&thin).chain(&heavy) {
        if !s.missing_bones.is_empty() {
            println!(
                "{}: bones not in the skeleton {:?}",
                s.name, s.missing_bones
            );
        }
    }
    let Some(face) = head.iter().find(|s| s.shader_type == 4) else {
        bail!("no FaceGen (shader type 4) shape in the head");
    };
    // The skin shapes of the body, paired _0 / _1 by name.
    let mut pairs = Vec::new();
    for h in heavy.iter().filter(|s| s.shader_type == 5) {
        let Some(t) = thin.iter().find(|t| t.name == h.name) else {
            println!("{}: no _0 shape of that name", h.name);
            continue;
        };
        if t.positions.len() != h.positions.len() {
            println!(
                "{}: _0 has {} vertices, _1 {}",
                h.name,
                t.positions.len(),
                h.positions.len()
            );
            continue;
        }
        pairs.push((t, h));
    }
    if pairs.is_empty() {
        bail!("no skin (shader type 5) shapes paired in the body");
    }
    println!(
        "head {} ({} open-edge vertices); body skin {}",
        face.name,
        face.open.iter().filter(|o| **o).count(),
        pairs
            .iter()
            .map(|(_, h)| h.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let body_open = |w: f32| -> Vec<Vec3> {
        pairs
            .iter()
            .flat_map(|(t, h)| {
                (0..h.positions.len())
                    .filter(|&i| h.open[i])
                    .map(move |i| t.positions[i].lerp(h.positions[i], w))
            })
            .collect()
    };
    let nearest = |p: Vec3, set: &[Vec3]| -> f32 {
        set.iter().map(|q| q.distance(p)).fold(f32::MAX, f32::min)
    };
    let weights: Vec<f32> = (0..=20).map(|i| i as f32 * 5.0).collect();
    let sets: Vec<Vec<Vec3>> = weights.iter().map(|w| body_open(w / 100.0)).collect();
    let neck: Vec<Vec3> = (0..face.positions.len())
        .filter(|&i| face.open[i])
        .map(|i| face.positions[i])
        .filter(|&p| sets.iter().any(|s| nearest(p, s) < 3.0))
        .collect();
    if neck.is_empty() {
        bail!("no open edge of the head lies near the body's");
    }
    println!("{} head vertices on the neck edge", neck.len());
    println!("weight   mean gap    max gap  (head neck vertex to nearest body open-edge vertex)");
    let mut best = (f32::MAX, 0.0);
    for (w, set) in weights.iter().zip(&sets) {
        let d: Vec<f32> = neck.iter().map(|&p| nearest(p, set)).collect();
        let mean = d.iter().sum::<f32>() / d.len() as f32;
        let max = d.iter().copied().fold(0.0, f32::max);
        let mut mark = String::new();
        if npc_weight.is_some_and(|n| (n - w).abs() < 2.5) {
            mark += "  <- NPC weight";
        }
        if *w == 100.0 {
            mark += "  <- drawn now (_1)";
        }
        println!("{w:6.0} {mean:10.4} {max:10.4}{mark}");
        if mean < best.0 {
            best = (mean, *w);
        }
    }
    // Refine the best weight to 0.1.
    let (mut lo, mut hi) = ((best.1 - 5.0).max(0.0), (best.1 + 5.0).min(100.0));
    let mean_at = |w: f32| -> f32 {
        let set = body_open(w / 100.0);
        neck.iter().map(|&p| nearest(p, &set)).sum::<f32>() / neck.len() as f32
    };
    for _ in 0..30 {
        let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if mean_at(a) < mean_at(b) {
            hi = b;
        } else {
            lo = a;
        }
    }
    let w = (lo + hi) * 0.5;
    println!("closest at weight {w:.1}: mean gap {:.4}", mean_at(w));
    Ok(())
}
