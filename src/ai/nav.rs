//! Navigation over the navmeshes of loaded cells: point location, A* across
//! edge links between meshes, and funnel string pulling.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use esp::FormId;
use esp::navmesh::{NavMesh, NavTriangle};
use glam::{Vec2, Vec3};

/// Spatial bucket size for triangle lookup.
const BUCKET: f32 = 256.0;
/// How far above / below a triangle a point may be and still be "on" it.
const Z_TOLERANCE: f32 = 160.0;
const MAX_EXPANSIONS: usize = 30_000;

struct Mesh {
    verts: Vec<Vec3>,
    tris: Vec<NavTriangle>,
    links: Vec<esp::navmesh::EdgeLink>,
    min: Vec2,
    max: Vec2,
    cols: usize,
    rows: usize,
    buckets: Vec<Vec<u16>>,
}

impl Mesh {
    fn new(m: NavMesh) -> Option<Mesh> {
        if m.triangles.is_empty() {
            return None;
        }
        let verts: Vec<Vec3> = m.vertices.iter().map(|v| Vec3::from(*v)).collect();
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for v in &verts {
            min = min.min(v.truncate());
            max = max.max(v.truncate());
        }
        let cols = (((max.x - min.x) / BUCKET).floor() as usize + 1).min(512);
        let rows = (((max.y - min.y) / BUCKET).floor() as usize + 1).min(512);
        let mut mesh = Mesh { verts, tris: m.triangles, links: m.edge_links, min, max, cols, rows, buckets: vec![Vec::new(); cols * rows] };
        for (i, t) in mesh.tris.iter().enumerate() {
            let p = t.vertices.map(|v| mesh.verts[v as usize].truncate());
            let lo = p[0].min(p[1]).min(p[2]);
            let hi = p[0].max(p[1]).max(p[2]);
            let (c0, r0) = mesh.bucket_of(lo);
            let (c1, r1) = mesh.bucket_of(hi);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    mesh.buckets[r * cols + c].push(i as u16);
                }
            }
        }
        Some(mesh)
    }

    fn bucket_of(&self, p: Vec2) -> (usize, usize) {
        let c = ((p.x - self.min.x) / BUCKET).floor().clamp(0.0, (self.cols - 1) as f32) as usize;
        let r = ((p.y - self.min.y) / BUCKET).floor().clamp(0.0, (self.rows - 1) as f32) as usize;
        (c, r)
    }

    fn corners(&self, t: usize) -> [Vec3; 3] {
        self.tris[t].vertices.map(|v| self.verts[v as usize])
    }

    fn centroid(&self, t: usize) -> Vec3 {
        let [a, b, c] = self.corners(t);
        (a + b + c) / 3.0
    }

    /// Height of triangle `t` at `p` if `p` lies inside it (in XY).
    fn height_in(&self, t: usize, p: Vec2) -> Option<f32> {
        let [a, b, c] = self.corners(t);
        let v0 = b.truncate() - a.truncate();
        let v1 = c.truncate() - a.truncate();
        let v2 = p - a.truncate();
        let d = v0.perp_dot(v1);
        if d.abs() < 1e-6 {
            return None;
        }
        let u = v2.perp_dot(v1) / d;
        let v = v0.perp_dot(v2) / d;
        const E: f32 = -1e-3;
        (u >= E && v >= E && u + v <= 1.0 - E).then(|| a.z + u * (b.z - a.z) + v * (c.z - a.z))
    }
}

/// A triangle in a particular navmesh.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node {
    pub mesh: FormId,
    pub tri: u16,
}

#[derive(Default)]
pub struct NavWorld {
    meshes: HashMap<FormId, Mesh>,
}

#[derive(PartialEq)]
struct Open {
    f: f32,
    node: Node,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f)
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl NavWorld {
    pub fn clear(&mut self) {
        self.meshes.clear();
    }

    /// Load the navmeshes of a cell. Returns the ids that were added.
    pub fn load_cell(&mut self, lo: &esp::LoadOrder, cell: FormId) -> Vec<FormId> {
        let mut added = Vec::new();
        let Some(idx) = lo.cell(cell) else { return added };
        for &id in &idx.navmeshes {
            if self.meshes.contains_key(&id) {
                continue;
            }
            let Some(r) = lo.get(id) else { continue };
            if r.header.is_deleted() {
                continue;
            }
            if let Some(m) = r.get(b"NVNM").and_then(|d| NavMesh::parse(d, |f| r.fid(f))).and_then(Mesh::new) {
                self.meshes.insert(id, m);
                added.push(id);
            }
        }
        added
    }

    pub fn unload(&mut self, ids: &[FormId]) {
        for id in ids {
            self.meshes.remove(id);
        }
    }

    /// The triangle under (or nearest to) `p`.
    pub fn locate(&self, p: Vec3) -> Option<(Node, f32)> {
        let q = p.truncate();
        let mut best: Option<(Node, f32, f32)> = None;
        for (&id, m) in &self.meshes {
            if q.x < m.min.x - 1.0 || q.y < m.min.y - 1.0 || q.x > m.max.x + 1.0 || q.y > m.max.y + 1.0 {
                continue;
            }
            let (c, r) = m.bucket_of(q);
            for &t in &m.buckets[r * m.cols + c] {
                if let Some(z) = m.height_in(t as usize, q) {
                    let dz = (z - p.z).abs();
                    if dz < Z_TOLERANCE && best.is_none_or(|b| dz < b.2) {
                        best = Some((Node { mesh: id, tri: t }, z, dz));
                    }
                }
            }
        }
        if let Some((n, z, _)) = best {
            return Some((n, z));
        }
        // Off the mesh: snap to the nearest triangle centroid close by.
        let mut near: Option<(Node, f32, f32)> = None;
        for (&id, m) in &self.meshes {
            if q.x < m.min.x - BUCKET || q.y < m.min.y - BUCKET || q.x > m.max.x + BUCKET || q.y > m.max.y + BUCKET {
                continue;
            }
            let (c, r) = m.bucket_of(q);
            for &t in &m.buckets[r * m.cols + c] {
                let cen = m.centroid(t as usize);
                let d = cen.distance_squared(p);
                if d < 200.0 * 200.0 && near.is_none_or(|b| d < b.2) {
                    near = Some((Node { mesh: id, tri: t }, cen.z, d));
                }
            }
        }
        near.map(|(n, z, _)| (n, z))
    }

    /// Ground height of the navmesh at `p`.
    pub fn height_at(&self, p: Vec3) -> Option<f32> {
        self.locate(p).map(|(_, z)| z)
    }

    fn mesh(&self, n: Node) -> &Mesh {
        &self.meshes[&n.mesh]
    }

    /// Neighbours of `n` with the shared edge (as two endpoints of `n`'s triangle).
    fn neighbours(&self, n: Node, out: &mut Vec<(Node, Vec3, Vec3)>) {
        out.clear();
        let m = self.mesh(n);
        let t = &m.tris[n.tri as usize];
        for e in 0..3 {
            let a = m.verts[t.vertices[e] as usize];
            let b = m.verts[t.vertices[(e + 1) % 3] as usize];
            if let Some(nb) = t.neighbour(e) {
                out.push((Node { mesh: n.mesh, tri: nb as u16 }, a, b));
            } else if let Some(l) = t.link(e) {
                let link = m.links[l];
                if let Some(other) = self.meshes.get(&link.navmesh)
                    && (link.triangle as usize) < other.tris.len()
                {
                    out.push((Node { mesh: link.navmesh, tri: link.triangle }, a, b));
                }
            }
        }
    }

    /// Find a path from `from` to `to`, returning waypoints (excluding the start).
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (start, _) = self.locate(from)?;
        let (goal, gz) = self.locate(to)?;
        let to = Vec3::new(to.x, to.y, gz);
        if start == goal {
            return Some(vec![to]);
        }
        let centre = |n: Node| self.mesh(n).centroid(n.tri as usize);
        let mut open = BinaryHeap::new();
        let mut came: HashMap<Node, (Node, Vec3, Vec3)> = HashMap::new();
        let mut cost: HashMap<Node, f32> = HashMap::new();
        // Positions are approximated by the centre of the edge used to enter a triangle.
        let mut entry: HashMap<Node, Vec3> = HashMap::new();
        cost.insert(start, 0.0);
        entry.insert(start, from);
        open.push(Open { f: from.distance(to), node: start });
        let mut buf = Vec::new();
        let mut expansions = 0;
        let mut found = false;
        while let Some(Open { node, .. }) = open.pop() {
            if node == goal {
                found = true;
                break;
            }
            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                return None;
            }
            let g = cost[&node];
            let here = entry[&node];
            self.neighbours(node, &mut buf);
            for &(nb, a, b) in &buf {
                let mid = (a + b) * 0.5;
                let ng = g + here.distance(mid);
                if cost.get(&nb).is_none_or(|&c| ng < c) {
                    cost.insert(nb, ng);
                    entry.insert(nb, mid);
                    came.insert(nb, (node, a, b));
                    let h = if nb == goal { mid.distance(to) } else { mid.distance(to).min(centre(nb).distance(to) + 1.0) };
                    open.push(Open { f: ng + h, node: nb });
                }
            }
        }
        if !found {
            return None;
        }
        // Portals from start to goal, oriented (left, right) relative to travel.
        let mut portals = Vec::new();
        let mut n = goal;
        while let Some(&(prev, a, b)) = came.get(&n) {
            let c = centre(prev);
            let d = ((a + b) * 0.5 - c).truncate();
            let (l, r) = if d.perp_dot((a - c).truncate()) > 0.0 { (a, b) } else { (b, a) };
            portals.push((l, r));
            n = prev;
        }
        portals.reverse();
        Some(string_pull(from, to, &portals))
    }

    /// A random point on the navmesh within `radius` of `centre` (in XY).
    pub fn random_point(&self, centre: Vec3, radius: f32, mut rand: impl FnMut() -> u64) -> Option<Vec3> {
        let c = centre.truncate();
        let mut candidates: Vec<(FormId, u16)> = Vec::new();
        for (&id, m) in &self.meshes {
            if c.x + radius < m.min.x || c.y + radius < m.min.y || c.x - radius > m.max.x || c.y - radius > m.max.y {
                continue;
            }
            for (i, t) in m.tris.iter().enumerate() {
                if t.flags & (NavTriangle::WATER | NavTriangle::DOOR) != 0 {
                    continue;
                }
                let cen = m.centroid(i);
                if cen.truncate().distance(c) <= radius && (cen.z - centre.z).abs() < radius.max(300.0) {
                    candidates.push((id, i as u16));
                }
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let (id, t) = candidates[(rand() % candidates.len() as u64) as usize];
        let [a, b, cc] = self.meshes[&id].corners(t as usize);
        let mut u = (rand() % 1000) as f32 / 1000.0;
        let mut v = (rand() % 1000) as f32 / 1000.0;
        if u + v > 1.0 {
            u = 1.0 - u;
            v = 1.0 - v;
        }
        Some(a + (b - a) * u + (cc - a) * v)
    }
}

/// Simple stupid funnel algorithm over `(left, right)` portals.
fn string_pull(from: Vec3, to: Vec3, portals: &[(Vec3, Vec3)]) -> Vec<Vec3> {
    let mut pts: Vec<(Vec3, Vec3)> = Vec::with_capacity(portals.len() + 2);
    pts.push((from, from));
    // Pull portal endpoints inwards a little so actors don't hug walls.
    for &(l, r) in portals {
        let w = l.distance(r);
        let inset = (w * 0.5 - 1.0).clamp(0.0, 24.0);
        let dir = (r - l).normalize_or_zero();
        pts.push((l + dir * inset, r - dir * inset));
    }
    pts.push((to, to));
    // Signed area in XY: positive when c is to the left of a->b.
    let area = |a: Vec3, b: Vec3, c: Vec3| (b - a).truncate().perp_dot((c - a).truncate());
    let mut out = Vec::new();
    let (mut apex, mut left, mut right) = (from, pts[0].0, pts[0].1);
    let (mut left_i, mut right_i) = (0usize, 0usize);
    let mut i = 1;
    let mut guard = 0;
    while i < pts.len() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        let (pl, pr) = pts[i];
        // Tighten the right side.
        if area(apex, right, pr) >= 0.0 {
            if apex == right || area(apex, left, pr) < 0.0 {
                right = pr;
                right_i = i;
            } else {
                apex = left;
                out.push(apex);
                right = apex;
                right_i = left_i;
                i = left_i + 1;
                continue;
            }
        }
        // Tighten the left side.
        if area(apex, left, pl) <= 0.0 {
            if apex == left || area(apex, right, pl) > 0.0 {
                left = pl;
                left_i = i;
            } else {
                apex = right;
                out.push(apex);
                left = apex;
                left_i = right_i;
                i = right_i + 1;
                continue;
            }
        }
        i += 1;
    }
    out.push(to);
    out.dedup_by(|a, b| a.distance_squared(*b) < 1.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funnel_straight_corridor() {
        // Travelling +Y, the left side is -X.
        let portals: Vec<(Vec3, Vec3)> =
            (1..5).map(|i| (Vec3::new(-50.0, i as f32 * 100.0, 0.0), Vec3::new(50.0, i as f32 * 100.0, 0.0))).collect();
        let p = string_pull(Vec3::ZERO, Vec3::new(0.0, 600.0, 0.0), &portals);
        assert_eq!(p, vec![Vec3::new(0.0, 600.0, 0.0)]);
    }

    #[test]
    fn funnel_turns_corner() {
        // Corridor going +Y then turning to +X: the path bends around the inner
        // (right-hand) corner at (50, 200), offset by the wall inset.
        let v = |x: f32, y: f32| Vec3::new(x, y, 0.0);
        let portals = vec![
            (v(-50.0, 100.0), v(50.0, 100.0)),
            (v(-50.0, 200.0), v(50.0, 200.0)),
            (v(50.0, 300.0), v(50.0, 200.0)),
            (v(150.0, 300.0), v(150.0, 200.0)),
        ];
        let goal = v(300.0, 250.0);
        let p = string_pull(Vec3::ZERO, goal, &portals);
        assert_eq!(p, vec![v(26.0, 200.0), v(50.0, 224.0), goal]);
    }
}
