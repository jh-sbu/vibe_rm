//! Test references placed in vanilla cells: overrides of interior cells,
//! of worldspaces (the world's persistent cell and exterior cells) and of
//! other records, copied from Skyrim.esm, and spots for actors picked on a
//! cell's navmesh.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use esp::navmesh::{NavMesh, NavTriangle};
use esp::writer::{Node, RecordBuilder};
use esp::{FormId, LoadOrder, group_type, record_flags};

/// An override of Skyrim.esm's record `id` (uncompressed): its subrecords
/// but those `skip` names, `FULL` written out as `name` (our plugin isn't
/// localized; left out without one), and `insert` after the editor id.
pub fn override_of(
    lo: &LoadOrder,
    id: FormId,
    name: Option<&str>,
    skip: &[&[u8; 4]],
    insert: &[(&[u8; 4], Vec<u8>)],
) -> Result<RecordBuilder> {
    let rec = lo.get(id).with_context(|| format!("{id} not found"))?;
    anyhow::ensure!(id.0 >> 24 == 0, "{id} isn't Skyrim.esm's");
    let mut out =
        RecordBuilder::new(&rec.tag().0, id).flags(rec.flags() & !record_flags::COMPRESSED);
    for s in rec.subrecords() {
        let tag = &s.tag.0;
        if skip.contains(&tag) {
            continue;
        }
        if tag == b"FULL" {
            if let Some(name) = name {
                out = out.zstring(b"FULL", name);
            }
            continue;
        }
        out = out.field(tag, s.data);
        if tag == b"EDID" {
            for (t, d) in insert {
                out = out.field(t, d);
            }
        }
    }
    Ok(out)
}

/// A cell's children: its persistent and temporary references.
fn cell_children(
    cell: FormId,
    persistent: Vec<RecordBuilder>,
    temporary: Vec<RecordBuilder>,
) -> Node {
    let label = cell.0.to_le_bytes();
    let mut groups = Vec::new();
    for (kind, refs) in [
        (group_type::CELL_PERSISTENT, persistent),
        (group_type::CELL_TEMPORARY, temporary),
    ] {
        if !refs.is_empty() {
            groups.push(Node::Group {
                label,
                group_type: kind,
                children: refs.into_iter().map(Node::Record).collect(),
            });
        }
    }
    Node::Group {
        label,
        group_type: group_type::CELL_CHILDREN,
        children: groups,
    }
}

/// An interior cell overridden with references of ours.
pub struct Interior {
    pub cell: FormId,
    /// Its name (`FULL`), written out.
    pub name: &'static str,
    pub persistent: Vec<RecordBuilder>,
    pub temporary: Vec<RecordBuilder>,
}

/// The top `CELL` group of interior cells: each in its block and sub-block,
/// by the last two decimal digits of its object id.
pub fn interiors(lo: &LoadOrder, cells: Vec<Interior>) -> Result<Node> {
    let mut blocks: BTreeMap<i32, BTreeMap<i32, Vec<Node>>> = BTreeMap::new();
    for c in cells {
        anyhow::ensure!(
            lo.cell(c.cell).is_some_and(|i| i.world.is_none()),
            "{} isn't an interior cell",
            c.cell
        );
        let record = override_of(lo, c.cell, Some(c.name), &[], &[])?;
        let object_id = (c.cell.0 & 0xFF_FFFF) as i32;
        let sub = blocks
            .entry(object_id % 10)
            .or_default()
            .entry(object_id / 10 % 10)
            .or_default();
        sub.push(Node::Record(record));
        sub.push(cell_children(c.cell, c.persistent, c.temporary));
    }
    Ok(Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: blocks
            .into_iter()
            .map(|(block, subs)| Node::Group {
                label: block.to_le_bytes(),
                group_type: group_type::INTERIOR_BLOCK,
                children: subs
                    .into_iter()
                    .map(|(sub, children)| Node::Group {
                        label: sub.to_le_bytes(),
                        group_type: group_type::INTERIOR_SUB_BLOCK,
                        children,
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// A worldspace overridden with references of ours: persistent ones in
/// its persistent cell, temporary ones in the exterior cell at `grid`.
pub struct World {
    pub world: FormId,
    /// Its name (`FULL`), written out.
    pub name: &'static str,
    pub grid: (i32, i32),
    pub persistent: Vec<RecordBuilder>,
    pub temporary: Vec<RecordBuilder>,
}

/// An exterior block or sub-block label: grid y, then x.
fn grid_label(x: i32, y: i32) -> [u8; 4] {
    let mut l = [0; 4];
    l[..2].copy_from_slice(&(y as i16).to_le_bytes());
    l[2..].copy_from_slice(&(x as i16).to_le_bytes());
    l
}

/// The top `WRLD` group of worldspaces. The world records are copied
/// without their large references and offsets (`RNAM`, `OFST`), as the
/// Creation Kit writes a plugin's override: Skyrim.esm's offsets point
/// into Skyrim.esm.
pub fn worlds(lo: &LoadOrder, worlds: Vec<World>) -> Result<Node> {
    let mut children = Vec::new();
    for w in worlds {
        let index = lo.world(w.world).context("not a worldspace")?;
        let persistent_cell = index
            .persistent_cell
            .context("the worldspace has no persistent cell")?;
        let cell = *index
            .cells
            .get(&w.grid)
            .with_context(|| format!("no cell at {:?}", w.grid))?;
        children.push(Node::Record(override_of(
            lo,
            w.world,
            Some(w.name),
            &[b"RNAM", b"OFST"],
            &[],
        )?));
        let (x, y) = w.grid;
        children.push(Node::Group {
            label: w.world.0.to_le_bytes(),
            group_type: group_type::WORLD_CHILDREN,
            children: vec![
                Node::Record(override_of(lo, persistent_cell, None, &[], &[])?),
                cell_children(persistent_cell, w.persistent, Vec::new()),
                Node::Group {
                    label: grid_label(x.div_euclid(32), y.div_euclid(32)),
                    group_type: group_type::EXTERIOR_BLOCK,
                    children: vec![Node::Group {
                        label: grid_label(x.div_euclid(8), y.div_euclid(8)),
                        group_type: group_type::EXTERIOR_SUB_BLOCK,
                        children: vec![
                            Node::Record(override_of(lo, cell, None, &[], &[])?),
                            cell_children(cell, Vec::new(), w.temporary),
                        ],
                    }],
                },
            ],
        });
    }
    Ok(Node::Group {
        label: *b"WRLD",
        group_type: group_type::TOP,
        children,
    })
}

/// Where a test sends its actors in a cell: the player's entry spot, and
/// pairs of an actor's spot and its marker's.
#[derive(Debug, Clone)]
pub struct Spots {
    pub entry: [f32; 3],
    pub pairs: Vec<([f32; 3], [f32; 3])>,
}

/// A navmesh triangle a spot may be on: its centre.
#[derive(Clone, Copy)]
struct Floor {
    centre: [f32; 3],
}

fn xy_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Where on `cell`'s navmesh (as Skyrim.esm has it) an actor can be put:
/// the centres of flat triangles (not water or doors) in the navmesh's
/// largest connected part: in an interior on its most common floor, in an
/// exterior inside the cell, 256 from its edges.
pub fn floors(lo: &LoadOrder, cell: FormId) -> Result<Vec<[f32; 3]>> {
    let index = lo.cell(cell).context("not a cell")?;
    let mut meshes = Vec::new();
    for &n in &index.navmeshes {
        let rec = lo.get(n).context("navmesh")?;
        if let Some(m) = rec
            .get(b"NVNM")
            .and_then(|d| NavMesh::parse(d, |f| rec.fid(f)))
        {
            meshes.push(m);
        }
    }
    // The largest connected part of each mesh, then the largest of those.
    let mut best: Vec<Floor> = Vec::new();
    for m in &meshes {
        let mut part = vec![usize::MAX; m.triangles.len()];
        let mut sizes = Vec::new();
        for start in 0..m.triangles.len() {
            if part[start] != usize::MAX {
                continue;
            }
            let id = sizes.len();
            let mut stack = vec![start];
            part[start] = id;
            let mut size = 0;
            while let Some(t) = stack.pop() {
                size += 1;
                for e in 0..3 {
                    if let Some(n) = m.triangles[t].neighbour(e)
                        && n < part.len()
                        && part[n] == usize::MAX
                    {
                        part[n] = id;
                        stack.push(n);
                    }
                }
            }
            sizes.push(size);
        }
        let Some(largest) = (0..sizes.len()).max_by_key(|&i| sizes[i]) else {
            continue;
        };
        let floors: Vec<Floor> = m
            .triangles
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                part[*i] == largest && t.flags & (NavTriangle::WATER | NavTriangle::DOOR) == 0
            })
            .filter_map(|(_, t)| {
                let v = t.vertices.map(|i| m.vertices.get(i as usize).copied());
                let [Some(a), Some(b), Some(c)] = v else {
                    return None;
                };
                // Flat: within 8 units, or no steeper than 1 in 4 (terrain).
                let zs = [a[2], b[2], c[2]];
                let rise = zs.iter().cloned().fold(f32::MIN, f32::max)
                    - zs.iter().cloned().fold(f32::MAX, f32::min);
                let run = [xy_distance(a, b), xy_distance(b, c), xy_distance(c, a)]
                    .into_iter()
                    .fold(0.0, f32::max);
                let flat = rise < 8.0 || rise < run / 4.0;
                // Twice the area, in XY.
                let area = ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs();
                (flat && area > 2.0 * 1500.0).then(|| Floor {
                    centre: [
                        (a[0] + b[0] + c[0]) / 3.0,
                        (a[1] + b[1] + c[1]) / 3.0,
                        (a[2] + b[2] + c[2]) / 3.0,
                    ],
                })
            })
            .collect();
        if floors.len() > best.len() {
            best = floors;
        }
    }
    let mut floors = best;
    match (index.world, index.grid) {
        (None, _) => {
            // The most common floor height (16-unit bins).
            let mut bins: BTreeMap<i32, usize> = BTreeMap::new();
            for f in &floors {
                *bins.entry((f.centre[2] / 16.0).round() as i32).or_default() += 1;
            }
            let level = bins
                .iter()
                .max_by_key(|(_, n)| **n)
                .map(|(b, _)| *b as f32 * 16.0);
            if let Some(level) = level {
                floors.retain(|f| (f.centre[2] - level).abs() <= 12.0);
            }
        }
        (Some(_), Some((x, y))) => {
            let (x0, y0) = (x as f32 * 4096.0 + 256.0, y as f32 * 4096.0 + 256.0);
            let (x1, y1) = (x0 + 4096.0 - 512.0, y0 + 4096.0 - 512.0);
            floors.retain(|f| (x0..x1).contains(&f.centre[0]) && (y0..y1).contains(&f.centre[1]));
        }
        (Some(_), None) => bail!("{cell} is a persistent cell"),
    }
    Ok(floors.into_iter().map(|f| f.centre).collect())
}

/// `pairs` actor spots and markers on `cell`'s navmesh ([`floors`]), each
/// marker `reach` (min, max) from its actor in XY, every spot at least
/// `apart` from the others, and the player's entry spot.
pub fn spots(
    lo: &LoadOrder,
    cell: FormId,
    pairs: usize,
    reach: (f32, f32),
    apart: f32,
) -> Result<Spots> {
    let floors: Vec<Floor> = floors(lo, cell)?
        .into_iter()
        .map(|centre| Floor { centre })
        .collect();
    if floors.is_empty() {
        bail!("{cell}: no navmesh floor to put actors on");
    }
    // The entry nearest the middle, then each actor as far from those
    // chosen as it can be, and its marker within reach, farthest from the
    // rest too.
    let n = floors.len() as f32;
    let middle = [0, 1].map(|i| floors.iter().map(|f| f.centre[i]).sum::<f32>() / n);
    let entry = floors
        .iter()
        .min_by(|a, b| {
            let d =
                |f: &Floor| (f.centre[0] - middle[0]).powi(2) + (f.centre[1] - middle[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
        .centre;
    let mut chosen = vec![entry];
    let clearance = |p: [f32; 3], chosen: &[[f32; 3]]| {
        chosen
            .iter()
            .map(|c| xy_distance(p, *c))
            .fold(f32::MAX, f32::min)
    };
    let mut out = Vec::new();
    for _ in 0..pairs {
        // Actors spread out first; markers within reach of theirs.
        let mut found = None;
        let mut spots: Vec<&Floor> = floors.iter().collect();
        spots.sort_by(|a, b| clearance(b.centre, &chosen).total_cmp(&clearance(a.centre, &chosen)));
        for s in spots {
            if clearance(s.centre, &chosen) < apart {
                break;
            }
            let mut with = chosen.clone();
            with.push(s.centre);
            let marker = floors
                .iter()
                .filter(|m| (reach.0..=reach.1).contains(&xy_distance(s.centre, m.centre)))
                .filter(|m| clearance(m.centre, &with) >= apart)
                .max_by(|a, b| clearance(a.centre, &with).total_cmp(&clearance(b.centre, &with)));
            if let Some(m) = marker {
                found = Some((s.centre, m.centre));
                break;
            }
        }
        let (s, m) = found.with_context(|| {
            format!("{cell}: no room for {pairs} actors {apart} apart and markers {reach:?} away")
        })?;
        chosen.push(s);
        chosen.push(m);
        out.push((s, m));
    }
    Ok(Spots { entry, pairs: out })
}
