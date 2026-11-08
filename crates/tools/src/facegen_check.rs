//! `facegen-check <Data> [npc]`: rebuild vanilla NPCs' heads from their
//! records (`facegen`) and compare each part with the pre-built FaceGen head's
//! shape of the same name, to settle the rules of `research/facegen.md` on
//! every race. Prints the worst and mean vertex errors per part type and
//! the worst NPCs.
use anyhow::{Context, Result};
use glam::Vec3;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Default)]
struct Stats {
    shapes: usize,
    worst: f32,
    sum: f64,
    verts: usize,
    over: usize,
}

const TOLERANCE: f32 = 0.05;

fn shapes(n: &nif::Nif) -> Vec<(String, Vec<Vec3>)> {
    n.blocks
        .iter()
        .filter_map(|b| match b {
            nif::Block::TriShape(t) => Some((t.av.net.name.clone(), t.geometry.positions.clone())),
            _ => None,
        })
        .collect()
}

pub fn run(data: &str, only: Option<&str>) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let lo = esp::LoadOrder::load(data, &names)?;
    let v = vfs::Vfs::new(data, &names);
    let npcs: Vec<esp::FormId> = match only {
        Some(s) => vec![match u32::from_str_radix(s, 16) {
            Ok(id) if s.len() == 8 => esp::FormId(id),
            _ => lo.find_editor_id(s).context("editor id not found")?,
        }],
        None => lo.ids_of_type(b"NPC_").to_vec(),
    };
    let mut tris: HashMap<String, Option<Arc<nif::tri::Tri>>> = HashMap::new();
    let mut models: HashMap<String, Option<Vec<(String, Vec<Vec3>)>>> = HashMap::new();
    let mut by_type: BTreeMap<u32, Stats> = BTreeMap::new();
    let mut worst_npcs: Vec<(f32, String, String)> = Vec::new();
    let (mut checked, mut missing_shape) = (0usize, 0usize);
    let mut missing_names: BTreeMap<String, usize> = BTreeMap::new();
    for npc in npcs {
        let Some((plugin, local)) = lo.origin(npc) else {
            continue;
        };
        let path = format!(
            "meshes/actors/character/facegendata/facegeom/{}/{:08x}.nif",
            plugin.to_ascii_lowercase(),
            local
        );
        let Some(prebuilt) = v.read(&path).and_then(|d| nif::Nif::parse(&d).ok()) else {
            continue;
        };
        let Some(face) = facegen::Face::of_npc(&lo, npc) else {
            continue;
        };
        checked += 1;
        let prebuilt = shapes(&prebuilt);
        let edid = lo.get(npc).and_then(|r| r.editor_id()).unwrap_or_default();
        for part in facegen::head_parts(&lo, &face) {
            let Some(part_edid) = lo.get(part).and_then(|r| r.editor_id()) else {
                continue;
            };
            let Some((_, target)) = prebuilt
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&part_edid))
            else {
                missing_shape += 1;
                *missing_names.entry(part_edid.clone()).or_default() += 1;
                continue;
            };
            let Some(model) = facegen::part_model(&lo, part) else {
                continue;
            };
            let base = models
                .entry(model.clone())
                .or_insert_with(|| {
                    v.read(&model)
                        .and_then(|d| nif::Nif::parse(&d).ok())
                        .map(|n| shapes(&n))
                })
                .as_ref()
                .and_then(|s| s.iter().find(|(_, p)| p.len() == target.len()))
                .map(|(_, p)| p.clone());
            let Some(mut positions) = base else { continue };
            // BASE=tri: start from the chargen (else race) file's vertices.
            if std::env::var("BASE").is_ok_and(|b| b == "tri")
                && let Some(t) = facegen::part_tri(&lo, part, 2)
                    .or_else(|| facegen::part_tri(&lo, part, 0))
                    .and_then(|f| v.read(&f))
                    .and_then(|d| nif::tri::Tri::parse(&d).ok())
                    .filter(|t| t.positions.len() == positions.len())
            {
                positions = t.positions;
            }
            let uses = facegen::part_morphs(&lo, part, &face);
            facegen::apply(&mut positions, &uses, |f| {
                tris.entry(f.to_owned())
                    .or_insert_with(|| {
                        v.read(f)
                            .and_then(|d| nif::tri::Tri::parse(&d).ok())
                            .map(Arc::new)
                    })
                    .clone()
            });
            let errs: Vec<f32> = positions
                .iter()
                .zip(target)
                .map(|(a, b)| (*a - *b).abs().max_element())
                .collect();
            let worst = errs.iter().copied().fold(0.0, f32::max);
            let s = by_type
                .entry(facegen::part_type(&lo, part).unwrap_or(99))
                .or_default();
            s.shapes += 1;
            s.worst = s.worst.max(worst);
            s.sum += errs.iter().map(|&e| e as f64).sum::<f64>();
            s.verts += errs.len();
            if worst > TOLERANCE {
                s.over += 1;
                worst_npcs.push((worst, edid.clone(), part_edid.clone()));
            }
            if only.is_some() {
                println!(
                    "  {model} tri files {:?}",
                    [0, 1, 2].map(|k| facegen::part_tri(&lo, part, k))
                );
                println!(
                    "{part_edid}: worst {worst:.4} mean {:.4} ({} morphs: {:?})",
                    errs.iter().sum::<f32>() / errs.len().max(1) as f32,
                    uses.len(),
                    uses.iter()
                        .map(|u| format!("{}×{:.2}", u.morph, u.weight))
                        .collect::<Vec<_>>()
                );
                if worst > TOLERANCE {
                    // What's left, projected on each morph the part's files
                    // have: the weight that morph would need alone.
                    let resid: Vec<Vec3> = target
                        .iter()
                        .zip(&positions)
                        .map(|(t, p)| *t - *p)
                        .collect();
                    let mut best: Vec<(f32, f32, String)> = Vec::new();
                    for f in [0, 1, 2]
                        .into_iter()
                        .filter_map(|k| facegen::part_tri(&lo, part, k))
                    {
                        let Some(tri) = v.read(&f).and_then(|d| nif::tri::Tri::parse(&d).ok())
                        else {
                            continue;
                        };
                        for m in tri.morphs.iter().filter(|m| m.deltas.len() == resid.len()) {
                            let dd: f32 = m.deltas.iter().map(|d| d.length_squared()).sum();
                            if dd == 0.0 {
                                continue;
                            }
                            let w = m
                                .deltas
                                .iter()
                                .zip(&resid)
                                .map(|(d, r)| d.dot(*r))
                                .sum::<f32>()
                                / dd;
                            let left: f32 = m
                                .deltas
                                .iter()
                                .zip(&resid)
                                .map(|(d, r)| (*r - *d * w).abs().max_element())
                                .fold(0.0, f32::max);
                            best.push((left, w, m.name.clone()));
                        }
                    }
                    best.sort_by(|a, b| a.0.total_cmp(&b.0));
                    for (left, w, name) in best.iter().take(4) {
                        println!("    with {name}×{w:.3} more: worst {left:.4}");
                    }
                }
            }
        }
    }
    println!(
        "{checked} NPCs with pre-built heads; {missing_shape} parts without a shape of their name"
    );
    let mut missing: Vec<_> = missing_names.into_iter().collect();
    missing.sort_by(|a, b| b.1.cmp(&a.1));
    println!("most missed: {:?}", &missing[..missing.len().min(12)]);
    for (t, s) in &by_type {
        println!(
            "part type {t}: {} shapes, worst {:.4}, mean {:.5}, {} over {TOLERANCE}",
            s.shapes,
            s.worst,
            s.sum / s.verts.max(1) as f64,
            s.over
        );
    }
    worst_npcs.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (w, npc, part) in worst_npcs
        .iter()
        .filter(|w| std::env::var("SKIP").map_or(true, |s| !w.2.contains(s.as_str())))
        .take(25)
    {
        println!("  {w:8.3} {npc} {part}");
    }
    Ok(())
}

/// `facegen-tints <Data> <npc>`: an NPC's tint layers with the race's
/// masks they name: index, mask type, mask file, colour and value.
pub fn tints(data: &str, npc: &str) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let lo = esp::LoadOrder::load(data, &names)?;
    let npc = match u32::from_str_radix(npc, 16) {
        Ok(id) if npc.len() == 8 => esp::FormId(id),
        _ => lo.find_editor_id(npc).context("editor id not found")?,
    };
    let face = facegen::Face::of_npc(&lo, npc).context("not an NPC")?;
    let masks = facegen::race_tint_masks(&lo, face.race, face.female);
    if let Some(q) = lo.get(npc).and_then(|r| r.get(b"QNAM").map(<[u8]>::to_vec)) {
        let f: Vec<f32> = q
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        println!("QNAM {f:?}");
    }
    for l in facegen::npc_tint_layers(&lo, npc) {
        let m = masks.iter().find(|m| m.index == l.index);
        println!(
            "TINI {:3} kind {:2} value {:.2} color {:?} preset {} mask {}",
            l.index,
            m.map_or(0, |m| m.kind),
            l.value,
            l.color,
            l.preset,
            m.map_or("?", |m| m.file.as_str())
        );
    }
    Ok(())
}

/// `facegen-tint-check <Data> [npc]`: composite NPCs' face tints from their
/// records (`facegen::tint`) and compare them with the pre-built `facetint`
/// textures: mean and 99th-percentile channel error (0..255).
pub fn tint_check(data: &str, only: Option<&str>) -> Result<()> {
    let data = std::path::Path::new(data);
    let names = esp::LoadOrder::default_plugin_list(data, None);
    let lo = esp::LoadOrder::load(data, &names)?;
    let v = vfs::Vfs::new(data, &names);
    let npcs: Vec<esp::FormId> = match only {
        Some(s) => vec![match u32::from_str_radix(s, 16) {
            Ok(id) if s.len() == 8 => esp::FormId(id),
            _ => lo.find_editor_id(s).context("editor id not found")?,
        }],
        None => lo.ids_of_type(b"NPC_").to_vec(),
    };
    let mut masks: HashMap<String, Option<Arc<facegen::tint::Image>>> = HashMap::new();
    let mut results: Vec<(f32, f32, String)> = Vec::new();
    for npc in npcs {
        let Some((plugin, local)) = lo.origin(npc) else {
            continue;
        };
        let path = format!(
            "textures/actors/character/facegendata/facetint/{}/{:08x}.dds",
            plugin.to_ascii_lowercase(),
            local
        );
        let Some(prebuilt) = v.read(&path).and_then(|d| facegen::tint::decode_dds(&d)) else {
            continue;
        };
        let Some(face) = facegen::Face::of_npc(&lo, npc) else {
            continue;
        };
        let skin = facegen::npc_skin_tone(&lo, npc).unwrap_or([1.0; 3]);
        let race_masks = facegen::race_tint_masks(&lo, face.race, face.female);
        let layers = facegen::npc_tint_layers(&lo, npc);
        let ours = facegen::tint::composite(prebuilt.width, skin, &layers, &race_masks, |f| {
            masks
                .entry(f.to_owned())
                .or_insert_with(|| {
                    v.read(f)
                        .and_then(|d| facegen::tint::decode_dds(&d))
                        .map(Arc::new)
                })
                .clone()
        });
        if ours.height != prebuilt.height {
            continue;
        }
        let mut errs: Vec<f32> = ours
            .rgb
            .iter()
            .zip(&prebuilt.rgb)
            .flat_map(|(a, b)| (0..3).map(move |k| (a[k] - b[k]).abs() * 255.0))
            .collect();
        let mean = errs.iter().sum::<f32>() / errs.len() as f32;
        let k = errs.len() * 99 / 100;
        let p99 = *errs.select_nth_unstable_by(k, f32::total_cmp).1;
        let edid = lo.get(npc).and_then(|r| r.editor_id()).unwrap_or_default();
        results.push((mean, p99, edid));
    }
    results.sort_by(|a, b| b.0.total_cmp(&a.0));
    let n = results.len().max(1) as f32;
    println!(
        "{} NPCs with pre-built tints; mean error {:.2}, mean p99 {:.2}; over 4 mean: {}",
        results.len(),
        results.iter().map(|r| r.0).sum::<f32>() / n,
        results.iter().map(|r| r.1).sum::<f32>() / n,
        results.iter().filter(|r| r.0 > 4.0).count()
    );
    for (mean, p99, npc) in results.iter().take(if only.is_some() { 1 } else { 25 }) {
        println!("  mean {mean:6.2} p99 {p99:6.2} {npc}");
    }
    Ok(())
}
