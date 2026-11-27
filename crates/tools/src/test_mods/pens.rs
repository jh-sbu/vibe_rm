//! A battery of test pens: an interior cell of ours holding nine pens in a
//! 3 × 3 grid, [`SPACING`] apart, and a closed booth for the player above
//! the middle one. Each pen is a floor piece walled on four sides, with a
//! navmesh of its own, so actors placed in it can move and fight there, out
//! of sight of the other pens; a batch can run nine setups at once. The
//! booth (floor, walls and ceiling, no navmesh) keeps the player out of
//! reach and out of sight, every pen within about 2300 units.
//!
//! The cell's references and navmeshes are written as the game's own are:
//! - The pens and booth are `WRIntFloorSTMid01Large` (a flat 1024 square at
//!   z 0, `OBND` ±512), walls being the piece stood on edge.
//! - Each navmesh (`NAVM` / `NVNM`, version 12) is a flat square grid of
//!   triangles. The layout is the one xEdit's `wbDefinitionsTES5.pas`
//!   reads, and UESP's *Mod File Format/NVNM Field*
//!   (https://en.uesp.net/w/index.php?oldid=3089096). The triangles are
//!   counter-clockwise from above, edge *e* runs from vertex *e* to *e* + 1,
//!   and every triangle has the 0x800 flag, as in Skyrim.esm's interior
//!   navmeshes and Survival Mode's new cell's, which carry no cover and a
//!   grid of divisor 1 for a mesh this small.
//! - Each navmesh gets an entry (`NVMI`) in an override of the navmesh info
//!   map (`NAVI` 0x12FB4): Survival Mode's plugin adds a cell and its
//!   navmesh the same way, its `NAVI` carrying only its own 11 entries
//!   (the game merges the plugins' entries). Every master's `NAVI` also
//!   carries a full preferred-pathing table (`NVPP`, 25696 bytes in each);
//!   ours is Skyrim.esm's, copied as the plugin is built.

use anyhow::{Context, Result, ensure};
use esp::writer::{Node, RecordBuilder};
use esp::{FormId, LoadOrder, group_type, record_flags};

use super::own;
use super::venv::xcll;

/// Distance between neighbouring pens' centres.
pub const SPACING: f32 = 1400.0;
/// Pens a battery holds.
pub const PENS: usize = 9;
/// Height of the booth's floor above the pens'.
pub const BOOTH_Z: f32 = 1100.0;
/// Half the floor piece's side.
const HALF: f32 = 512.0;
/// Half the side of a pen's navmesh: inside the walls by 72.
const NAV_HALF: f32 = 440.0;
/// Quads along each side of a pen's navmesh (two triangles each).
const NAV_QUADS: usize = 4;
/// Skyrim.esm's navmesh info map.
const NAVI: FormId = FormId(0x0001_2FB4);
/// The `NVNM` / `NVMI` "pathing cell" hash (`PathingCell` in xEdit).
const PATHING_CELL: u32 = 0xA5E9_A03C;
/// The triangle flag every vanilla interior triangle here has.
const TRIANGLE_FOUND: u16 = 0x800;

/// Object ids of a battery: the cell, a heading marker at each pen's
/// centre (`markers`, nine in a row), the booth's marker, the statics
/// (`statics`, [`STATICS`] in a row) and the navmeshes (`navmeshes`, nine
/// in a row).
pub struct Ids {
    pub cell: u32,
    pub markers: u32,
    pub booth: u32,
    pub statics: u32,
    pub navmeshes: u32,
}

/// Statics a battery places: five per pen, six for the booth.
pub const STATICS: u32 = PENS as u32 * 5 + 6;

/// A built battery: the cell record, its children group and the `NAVI`
/// override (its own top group, written before `CELL`).
pub struct Battery {
    pub cell: RecordBuilder,
    pub children: Node,
    pub navi: RecordBuilder,
}

/// Pen `i`'s centre: row-major from the south-west, the fifth in the middle.
pub fn centre(i: usize) -> [f32; 3] {
    let col = (i % 3) as f32 - 1.0;
    let row = (i / 3) as f32 - 1.0;
    [col * SPACING, row * SPACING, 0.0]
}

/// The booth's centre: above the middle pen.
pub fn booth() -> [f32; 3] {
    [0.0, 0.0, BOOTH_Z]
}

/// A reference's `DATA`: position, then rotation (radians) about x, y, z.
fn data(pos: [f32; 3], rot: [f32; 3]) -> Vec<u8> {
    pos.iter()
        .chain(rot.iter())
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

/// The floor piece, walls and ceiling of a box `height` tall over `at`:
/// the floor if `floor`, four walls, the ceiling if `ceiling`.
fn box_pieces(at: [f32; 3], floor: bool, ceiling: bool) -> Vec<([f32; 3], [f32; 3])> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let [x, y, z] = at;
    let mut out = Vec::new();
    if floor {
        out.push(([x, y, z], [0.0; 3]));
    }
    // The piece is drawn from above only, so each wall faces in. Stood on
    // edge about x it lies in the xz plane, its face turned to +y by +90°
    // (our engine's render of the first build showed it); about y in the
    // yz plane, turned to -x by +90°.
    out.push(([x, y + HALF, z + HALF], [-FRAC_PI_2, 0.0, 0.0]));
    out.push(([x, y - HALF, z + HALF], [FRAC_PI_2, 0.0, 0.0]));
    out.push(([x + HALF, y, z + HALF], [0.0, FRAC_PI_2, 0.0]));
    out.push(([x - HALF, y, z + HALF], [0.0, -FRAC_PI_2, 0.0]));
    if ceiling {
        // Upside down, facing the floor.
        out.push(([x, y, z + 2.0 * HALF], [PI, 0.0, 0.0]));
    }
    out
}

/// A flat square navmesh `2 * half` across at `at` in interior `cell`:
/// `quads` × `quads` squares, two triangles each.
pub fn nvnm(cell: FormId, at: [f32; 3], half: f32, quads: usize) -> Vec<u8> {
    let n = quads;
    let step = 2.0 * half / n as f32;
    let mut b = Vec::new();
    let put = |b: &mut Vec<u8>, bytes: &[u8]| b.extend_from_slice(bytes);
    put(&mut b, &12u32.to_le_bytes());
    put(&mut b, &PATHING_CELL.to_le_bytes());
    put(&mut b, &0u32.to_le_bytes()); // no worldspace: an interior
    put(&mut b, &cell.0.to_le_bytes());
    // Vertices, row by row from the south-west.
    put(&mut b, &(((n + 1) * (n + 1)) as u32).to_le_bytes());
    for j in 0..=n {
        for i in 0..=n {
            for v in [
                at[0] - half + i as f32 * step,
                at[1] - half + j as f32 * step,
                at[2],
            ] {
                put(&mut b, &v.to_le_bytes());
            }
        }
    }
    // Triangles: in quad (i, j), A = (v00, v10, v11), B = (v00, v11, v01).
    let vertex = |i: usize, j: usize| (j * (n + 1) + i) as u16;
    let a = |i: usize, j: usize| (2 * (j * n + i)) as i16;
    let b_of = |i: usize, j: usize| a(i, j) + 1;
    put(&mut b, &((2 * n * n) as u32).to_le_bytes());
    for j in 0..n {
        for i in 0..n {
            let tris = [
                (
                    [vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1)],
                    [
                        if j > 0 { b_of(i, j - 1) } else { -1 },
                        if i + 1 < n { b_of(i + 1, j) } else { -1 },
                        b_of(i, j),
                    ],
                ),
                (
                    [vertex(i, j), vertex(i + 1, j + 1), vertex(i, j + 1)],
                    [
                        a(i, j),
                        if j + 1 < n { a(i, j + 1) } else { -1 },
                        if i > 0 { a(i - 1, j) } else { -1 },
                    ],
                ),
            ];
            for (vs, es) in tris {
                for v in vs {
                    put(&mut b, &v.to_le_bytes());
                }
                for e in es {
                    put(&mut b, &e.to_le_bytes());
                }
                put(&mut b, &TRIANGLE_FOUND.to_le_bytes());
                put(&mut b, &0u16.to_le_bytes()); // no cover
            }
        }
    }
    put(&mut b, &0u32.to_le_bytes()); // edge links
    put(&mut b, &0u32.to_le_bytes()); // door links
    put(&mut b, &0u32.to_le_bytes()); // cover triangles
    // The grid: one cell holding every triangle.
    let (min, max) = (
        [at[0] - half, at[1] - half, at[2]],
        [at[0] + half, at[1] + half, at[2]],
    );
    put(&mut b, &1u32.to_le_bytes());
    put(&mut b, &(max[0] - min[0]).to_le_bytes());
    put(&mut b, &(max[1] - min[1]).to_le_bytes());
    for v in min.iter().chain(max.iter()) {
        put(&mut b, &v.to_le_bytes());
    }
    put(&mut b, &((2 * n * n) as u32).to_le_bytes());
    for t in 0..2 * n * n {
        put(&mut b, &(t as u16).to_le_bytes());
    }
    b
}

/// A navmesh's entry in the info map: not an island, no links or doors,
/// centred on `at`, in interior `cell`.
fn nvmi(navmesh: FormId, cell: FormId, at: [f32; 3]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&navmesh.0.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // flags
    for v in at {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&0f32.to_le_bytes()); // preferred %
    b.extend_from_slice(&0u32.to_le_bytes()); // edge links
    b.extend_from_slice(&0u32.to_le_bytes()); // preferred edge links
    b.extend_from_slice(&0u32.to_le_bytes()); // door links
    b.push(0); // not an island
    b.extend_from_slice(&PATHING_CELL.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // no worldspace
    b.extend_from_slice(&cell.0.to_le_bytes());
    b
}

/// The battery's cell `edid` (shown as `name`), its references and
/// navmeshes, and the `NAVI` override listing them.
pub fn build(lo: &LoadOrder, ids: &Ids, edid: &str, name: &str) -> Result<Battery> {
    let find = |e: &str| {
        lo.find_editor_id(e)
            .with_context(|| format!("Skyrim.esm {e}"))
    };
    let floor = find("WRIntFloorSTMid01Large")?;
    let heading = find("XMarkerHeading")?;
    let cell_id = own(ids.cell);
    let cell = RecordBuilder::new(b"CELL", cell_id)
        .zstring(b"EDID", edid)
        .zstring(b"FULL", name)
        .field(b"DATA", &1u16.to_le_bytes())
        .field(b"XCLL", &xcll())
        .u32(b"LTMP", 0);

    let marker = |object_id: u32, pos: [f32; 3]| {
        RecordBuilder::new(b"REFR", own(object_id))
            .flags(record_flags::PERSISTENT)
            .form(b"NAME", heading)
            .field(b"DATA", &data(pos, [0.0; 3]))
    };
    let mut persistent: Vec<RecordBuilder> = (0..PENS)
        .map(|i| marker(ids.markers + i as u32, centre(i)))
        .collect();
    persistent.push(marker(ids.booth, booth()));

    let mut pieces = Vec::new();
    for i in 0..PENS {
        pieces.extend(box_pieces(centre(i), true, false));
    }
    pieces.extend(box_pieces(booth(), true, true));
    ensure!(pieces.len() as u32 == STATICS);
    let mut temporary: Vec<RecordBuilder> = pieces
        .into_iter()
        .enumerate()
        .map(|(k, (pos, rot))| {
            RecordBuilder::new(b"REFR", own(ids.statics + k as u32))
                .form(b"NAME", floor)
                .field(b"DATA", &data(pos, rot))
        })
        .collect();

    let vanilla = lo.get(NAVI).context("Skyrim.esm's navmesh info map")?;
    ensure!(
        lo.origin(NAVI)
            .is_some_and(|(p, _)| p.eq_ignore_ascii_case("Skyrim.esm")),
        "the navmesh info map isn't Skyrim.esm's"
    );
    let mut navi = RecordBuilder::new(b"NAVI", NAVI).u32(b"NVER", 12);
    for i in 0..PENS {
        let id = own(ids.navmeshes + i as u32);
        temporary.push(
            RecordBuilder::new(b"NAVM", id)
                .field(b"NVNM", &nvnm(cell_id, centre(i), NAV_HALF, NAV_QUADS)),
        );
        navi = navi.field(b"NVMI", &nvmi(id, cell_id, centre(i)));
    }
    let nvpp = vanilla.get(b"NVPP").context("Skyrim.esm's NVPP")?;
    let navi = navi.field(b"NVPP", nvpp);

    let label = cell_id.0.to_le_bytes();
    let children = Node::Group {
        label,
        group_type: group_type::CELL_CHILDREN,
        children: vec![
            Node::Group {
                label,
                group_type: group_type::CELL_PERSISTENT,
                children: persistent.into_iter().map(Node::Record).collect(),
            },
            Node::Group {
                label,
                group_type: group_type::CELL_TEMPORARY,
                children: temporary.into_iter().map(Node::Record).collect(),
            },
        ],
    };
    Ok(Battery {
        cell,
        children,
        navi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::navmesh::NavMesh;

    #[test]
    fn a_pen_navmesh_parses_and_its_triangles_join_as_vanilla_ones_do() {
        let cell = FormId(0x0100_0A31);
        let at = centre(2);
        let data = nvnm(cell, at, NAV_HALF, NAV_QUADS);
        let mesh = NavMesh::parse(&data, |f| f).expect("parses");
        mesh.validate().unwrap();
        assert_eq!(mesh.version, 12);
        assert_eq!(mesh.cell, cell);
        assert_eq!(mesh.vertices.len(), 25);
        assert_eq!(mesh.triangles.len(), 32);
        let mut borders = 0;
        for (k, t) in mesh.triangles.iter().enumerate() {
            let [a, b, c] = t.vertices.map(|v| mesh.vertices[v as usize]);
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            assert!(area > 0.0, "triangle {k} is counter-clockwise from above");
            for e in 0..3 {
                let Some(n) = t.neighbour(e) else {
                    borders += 1;
                    continue;
                };
                let shared = [t.vertices[e], t.vertices[(e + 1) % 3]];
                let other = mesh.triangles[n];
                assert!(
                    shared.iter().all(|v| other.vertices.contains(v)),
                    "triangle {k} edge {e} meets {n} on its edge"
                );
                assert!(
                    (0..3).any(|f| other.neighbour(f) == Some(k)),
                    "{n} names {k} back"
                );
            }
        }
        assert_eq!(borders, 4 * NAV_QUADS, "the outline");
        // After the triangles: no links, doors or cover, then the grid.
        let tail = 16 + 4 + 25 * 12 + 4 + 32 * 16;
        let word = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
        assert_eq!([word(tail), word(tail + 4), word(tail + 8)], [0, 0, 0]);
        assert_eq!(word(tail + 12), 1, "divisor");
        let cells = tail + 12 + 4 + 8 + 24;
        assert_eq!(word(cells), 32);
        assert_eq!(data.len(), cells + 4 + 32 * 2, "nothing after the grid");
    }

    #[test]
    fn an_info_map_entry_has_survival_modes_shape() {
        // Survival Mode's entry for its new cell's navmesh is 49 bytes:
        // navmesh, flags, centre, preferred %, three empty lists, no
        // island, then the pathing cell.
        let e = nvmi(FormId(0x0100_0B80), FormId(0x0100_0A31), [1.0, 2.0, 3.0]);
        assert_eq!(e.len(), 49);
        assert_eq!(e[36], 0, "not an island");
        assert_eq!(&e[37..41], &PATHING_CELL.to_le_bytes());
        assert_eq!(&e[45..49], &0x0100_0A31u32.to_le_bytes());
    }

    #[test]
    fn pens_are_far_apart_and_the_booth_near_them_all() {
        for i in 0..PENS {
            for j in 0..i {
                let (a, b) = (centre(i), centre(j));
                let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                assert!(d >= SPACING - 0.1);
            }
            let (a, b) = (centre(i), booth());
            let d = (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();
            assert!(d < 2300.0, "pen {i} is {d} from the booth");
        }
        assert_eq!(centre(4), [0.0, 0.0, 0.0]);
    }
}
