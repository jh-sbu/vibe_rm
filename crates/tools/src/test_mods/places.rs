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

/// A slope into still water: points on the bed, along one straight line
/// out from the shore, at each of the depths asked for below the surface.
#[derive(Debug, Clone)]
pub struct Shore {
    pub cell: FormId,
    pub grid: (i32, i32),
    /// The water's surface height.
    pub surface: f32,
    /// A point on the bed per depth, in order.
    pub points: Vec<[f32; 3]>,
    /// The line's heading, out into the water (radians clockwise from
    /// north, as references' `DATA` takes it).
    pub heading: f32,
}

/// Vertices per side of a `LAND` record's heights, 128 units apart.
const LAND_VERTS: usize = 33;

/// `LAND` heights (`VHGT`): offset, then per row a delta from the row
/// above's first vertex and deltas along the row, in units of 8.
fn land_heights(lo: &LoadOrder, land: FormId) -> Option<Vec<f32>> {
    let rec = lo.get(land)?;
    let d = rec.get(b"VHGT")?;
    if d.len() < 4 + LAND_VERTS * LAND_VERTS {
        return None;
    }
    let mut heights = vec![0.0; LAND_VERTS * LAND_VERTS];
    let mut offset = f32::from_le_bytes(d[0..4].try_into().ok()?);
    for r in 0..LAND_VERTS {
        offset += d[4 + r * LAND_VERTS] as i8 as f32;
        let mut h = offset;
        heights[r * LAND_VERTS] = h * 8.0;
        for c in 1..LAND_VERTS {
            h += d[4 + r * LAND_VERTS + c] as i8 as f32;
            heights[r * LAND_VERTS + c] = h * 8.0;
        }
    }
    Some(heights)
}

/// The height of `heights` at `(x, y)` units into the cell, bilinear.
fn height_in_cell(heights: &[f32], x: f32, y: f32) -> f32 {
    let (fx, fy) = (x / 128.0, y / 128.0);
    let max = (LAND_VERTS - 2) as f32;
    let (c, r) = (fx.floor().clamp(0.0, max), fy.floor().clamp(0.0, max));
    let (tx, ty) = (fx - c, fy - r);
    let (c, r) = (c as usize, r as usize);
    let h = |c: usize, r: usize| heights[r * LAND_VERTS + c];
    let top = h(c, r) * (1.0 - tx) + h(c + 1, r) * tx;
    let bottom = h(c, r + 1) * (1.0 - tx) + h(c + 1, r + 1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// A still lake's shore in `world` nearest `near` (XY), with the bed
/// reaching each of `depths` (increasing) along a straight line from a
/// point at most 24 deep: the bed's depth never falls back by more than
/// 6 along it, consecutive points are at least 48 apart, and the line,
/// at most 1600 long, stays 64 inside one cell. A lake is a cell of
/// ordinary (not damaging) water at least a quarter of whose land lies
/// 200 or more under it, which rules out the narrow rivers and their
/// currents. Of a cell's lines, the one closest to 800 long.
pub fn shore(lo: &LoadOrder, world: FormId, near: [f32; 2], depths: &[f32]) -> Result<Shore> {
    let wrld = lo.get(world).context("not a worldspace")?;
    let default_water = wrld
        .get(b"DNAM")
        .filter(|d| d.len() >= 8)
        .map_or(0.0, |d| f32::from_le_bytes(d[4..8].try_into().unwrap()));
    let default_type = wrld
        .get(b"NAM2")
        .map(|d| wrld.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
    let index = lo.world(world).context("not a worldspace")?;
    let mut cells: Vec<((i32, i32), FormId)> = index.cells.iter().map(|(g, c)| (*g, *c)).collect();
    let distance = |g: (i32, i32)| {
        let centre = [(g.0 as f32 + 0.5) * 4096.0, (g.1 as f32 + 0.5) * 4096.0];
        (centre[0] - near[0]).hypot(centre[1] - near[1])
    };
    cells.sort_by(|a, b| distance(a.0).total_cmp(&distance(b.0)));
    let deepest = *depths.last().context("no depths")?;
    for (grid, cell) in cells {
        let Some(rec) = lo.get(cell) else { continue };
        let surface = rec.get(b"XCLW").map_or(default_water, |d| {
            f32::from_le_bytes(d[0..4].try_into().unwrap())
        });
        if surface >= 1.0e30 {
            continue;
        }
        let water_type = rec
            .get(b"XCWT")
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .or(default_type);
        let damaging = water_type
            .and_then(|w| lo.get(w))
            .and_then(|w| w.get(b"FNAM").and_then(|d| d.first()).copied())
            .is_some_and(|flags| flags & 1 != 0);
        if damaging {
            continue;
        }
        let Some(heights) = lo
            .cell(cell)
            .and_then(|c| c.land)
            .and_then(|l| land_heights(lo, l))
        else {
            continue;
        };
        let deep = heights.iter().filter(|h| surface - **h >= 200.0).count();
        if deep * 4 < heights.len() {
            continue;
        }
        let origin = [grid.0 as f32 * 4096.0, grid.1 as f32 * 4096.0];
        let depth = |x: f32, y: f32| surface - height_in_cell(&heights, x, y);
        let inside = |x: f32, y: f32| {
            (64.0..4096.0 - 64.0).contains(&x) && (64.0..4096.0 - 64.0).contains(&y)
        };
        // (|length - 800|, points, heading)
        let mut best: Option<(f32, Vec<[f32; 3]>, f32)> = None;
        for r in 0..LAND_VERTS {
            for c in 0..LAND_VERTS {
                let (x0, y0) = (c as f32 * 128.0, r as f32 * 128.0);
                if !inside(x0, y0) || !(0.0..=24.0).contains(&depth(x0, y0)) {
                    continue;
                }
                for k in 0..16 {
                    let heading = k as f32 * std::f32::consts::TAU / 16.0;
                    let (dx, dy) = (heading.sin(), heading.cos());
                    let mut points = Vec::new();
                    let mut deepest_so_far = depth(x0, y0);
                    let mut last_at = f32::MIN;
                    let mut s = 0.0;
                    while s <= 1600.0 && points.len() < depths.len() {
                        s += 8.0;
                        let (x, y) = (x0 + dx * s, y0 + dy * s);
                        if !inside(x, y) {
                            break;
                        }
                        let d = depth(x, y);
                        if d < deepest_so_far - 6.0 {
                            break;
                        }
                        deepest_so_far = deepest_so_far.max(d);
                        if d >= depths[points.len()] {
                            if s - last_at < 48.0 {
                                break;
                            }
                            last_at = s;
                            points.push([origin[0] + x, origin[1] + y, surface - d]);
                        }
                    }
                    if points.len() == depths.len() && deepest_so_far >= deepest {
                        let score = (s - 800.0).abs();
                        if best.as_ref().is_none_or(|b| score < b.0) {
                            best = Some((score, points, heading));
                        }
                    }
                }
            }
        }
        if let Some((_, points, heading)) = best {
            return Ok(Shore {
                cell,
                grid,
                surface,
                points,
                heading,
            });
        }
    }
    bail!("no lake shore reaching {depths:?}")
}
