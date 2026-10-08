//! Navigation mesh geometry (`NAVM` / `NVNM`, Skyrim SE version 12).

use crate::FormId;

#[derive(Debug, Clone, Copy)]
pub struct NavTriangle {
    pub vertices: [u16; 3],
    /// Per edge (0-1, 1-2, 2-0): neighbouring triangle in this mesh, or, when
    /// the matching `EDGE_LINK` flag is set, an index into `edge_links`.
    pub edges: [i16; 3],
    pub flags: u16,
    pub cover: u16,
}

impl NavTriangle {
    pub const EDGE_LINK: [u16; 3] = [0x1, 0x2, 0x4];
    pub const PREFERRED: u16 = 0x8;
    pub const WATER: u16 = 0x200;
    pub const DOOR: u16 = 0x400;
    pub const FOUND: u16 = 0x800;

    /// Neighbour across `edge` within the same mesh.
    pub fn neighbour(&self, edge: usize) -> Option<usize> {
        (self.flags & Self::EDGE_LINK[edge] == 0 && self.edges[edge] >= 0)
            .then_some(self.edges[edge] as usize)
    }

    /// Index into the edge link table for `edge`.
    pub fn link(&self, edge: usize) -> Option<usize> {
        (self.flags & Self::EDGE_LINK[edge] != 0 && self.edges[edge] >= 0)
            .then_some(self.edges[edge] as usize)
    }
}

/// Connection from one triangle edge to a triangle of another navmesh.
#[derive(Debug, Clone, Copy)]
pub struct EdgeLink {
    pub kind: u32,
    pub navmesh: FormId,
    pub triangle: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct DoorTriangle {
    pub triangle: u16,
    pub door: FormId,
}

#[derive(Debug, Clone)]
pub struct NavMesh {
    pub version: u32,
    /// Worldspace for exterior meshes, else null.
    pub world: FormId,
    /// Cell for interior meshes, or the grid for exterior ones.
    pub cell: FormId,
    pub grid: (i16, i16),
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<NavTriangle>,
    pub edge_links: Vec<EdgeLink>,
    pub doors: Vec<DoorTriangle>,
}

const MAGIC: u32 = 0xA5E9_A03C;

struct Cursor<'a> {
    b: &'a [u8],
    o: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.o..self.o + n)?;
        self.o += n;
        Some(s)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn i16(&mut self) -> Option<i16> {
        self.u16().map(|v| v as i16)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
    fn count(&mut self, elem: usize) -> Option<usize> {
        let n = self.u32()? as usize;
        (n.checked_mul(elem)? <= self.b.len() - self.o).then_some(n)
    }
}

impl NavMesh {
    /// Parse an `NVNM` subrecord. FormIDs are file-local; map them with `fid`.
    pub fn parse(data: &[u8], fid: impl Fn(FormId) -> FormId) -> Option<NavMesh> {
        let mut c = Cursor { b: data, o: 0 };
        let version = c.u32()?;
        if c.u32()? != MAGIC {
            return None;
        }
        let world = FormId(c.u32()?);
        let (cell, grid) = if world.0 == 0 {
            (fid(FormId(c.u32()?)), (0, 0))
        } else {
            let gy = c.i16()?;
            let gx = c.i16()?;
            (FormId(0), (gx, gy))
        };
        let world = if world.0 == 0 { world } else { fid(world) };
        let n = c.count(12)?;
        let mut vertices = Vec::with_capacity(n);
        for _ in 0..n {
            vertices.push([c.f32()?, c.f32()?, c.f32()?]);
        }
        let n = c.count(16)?;
        let mut triangles = Vec::with_capacity(n);
        for _ in 0..n {
            triangles.push(NavTriangle {
                vertices: [c.u16()?, c.u16()?, c.u16()?],
                edges: [c.i16()?, c.i16()?, c.i16()?],
                flags: c.u16()?,
                cover: c.u16()?,
            });
        }
        let n = c.count(10)?;
        let mut edge_links = Vec::with_capacity(n);
        for _ in 0..n {
            edge_links.push(EdgeLink {
                kind: c.u32()?,
                navmesh: fid(FormId(c.u32()?)),
                triangle: c.u16()?,
            });
        }
        let n = c.count(10)?;
        let mut doors = Vec::with_capacity(n);
        for _ in 0..n {
            let triangle = c.u16()?;
            let _ = c.u32()?;
            doors.push(DoorTriangle {
                triangle,
                door: fid(FormId(c.u32()?)),
            });
        }
        Some(NavMesh {
            version,
            world,
            cell,
            grid,
            vertices,
            triangles,
            edge_links,
            doors,
        })
    }

    /// Check internal indices; returns a description of the first problem.
    pub fn validate(&self) -> Result<(), String> {
        let nv = self.vertices.len();
        let nt = self.triangles.len();
        for (i, t) in self.triangles.iter().enumerate() {
            if t.vertices.iter().any(|&v| v as usize >= nv) {
                return Err(format!(
                    "triangle {i} vertex out of range {:?} (n={nv})",
                    t.vertices
                ));
            }
            for e in 0..3 {
                if let Some(n) = t.neighbour(e)
                    && n >= nt
                {
                    return Err(format!(
                        "triangle {i} edge {e} neighbour {n} out of range (n={nt})"
                    ));
                }
                if let Some(l) = t.link(e)
                    && l >= self.edge_links.len()
                {
                    return Err(format!(
                        "triangle {i} edge {e} link {l} out of range (n={})",
                        self.edge_links.len()
                    ));
                }
            }
        }
        for d in &self.doors {
            if d.triangle as usize >= nt {
                return Err(format!("door triangle {} out of range", d.triangle));
            }
        }
        Ok(())
    }
}
