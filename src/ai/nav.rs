//! Navigation over the navmeshes of loaded cells: point location, A* across
//! edge links between meshes, and funnel string pulling.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use esp::FormId;
use esp::navmesh::{NavMesh, NavTriangle};
use glam::{Vec2, Vec3};

/// Spatial bucket size for triangle lookup.
const BUCKET: f32 = 256.0;
/// How far above / below a triangle a point may be and still be "on" it.
const Z_TOLERANCE: f32 = 160.0;
const MAX_EXPANSIONS: usize = 30_000;
/// Searching for the nearest point to a target it can't reach explores all it
/// can: this far, then the nearest found (later repaths carry on from there).
const MAX_NEAREST_EXPANSIONS: usize = 5_000;

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
        let mut mesh = Mesh {
            verts,
            tris: m.triangles,
            links: m.edge_links,
            min,
            max,
            cols,
            rows,
            buckets: vec![Vec::new(); cols * rows],
        };
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
        let c = ((p.x - self.min.x) / BUCKET)
            .floor()
            .clamp(0.0, (self.cols - 1) as f32) as usize;
        let r = ((p.y - self.min.y) / BUCKET)
            .floor()
            .clamp(0.0, (self.rows - 1) as f32) as usize;
        (c, r)
    }

    fn corners(&self, t: usize) -> [Vec3; 3] {
        self.tris[t].vertices.map(|v| self.verts[v as usize])
    }

    fn centroid(&self, t: usize) -> Vec3 {
        let [a, b, c] = self.corners(t);
        (a + b + c) / 3.0
    }

    /// The point of triangle `t` nearest `p` in XY, at the triangle's height there.
    fn closest_in(&self, t: usize, p: Vec2) -> Vec3 {
        if let Some(z) = self.height_in(t, p) {
            return p.extend(z);
        }
        let c = self.corners(t);
        (0..3)
            .map(|e| {
                let (a, b) = (c[e], c[(e + 1) % 3]);
                let ab = (b - a).truncate();
                let k =
                    ((p - a.truncate()).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                a + (b - a) * k
            })
            .min_by(|x, y| {
                x.truncate()
                    .distance_squared(p)
                    .total_cmp(&y.truncate().distance_squared(p))
            })
            .unwrap_or(c[0])
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
    /// Bumped whenever meshes load or unload (obstacles are found again).
    generation: u64,
    /// The triangles each obstacle (a reference whose base is flagged
    /// `Obstacle`: a closed portcullis) stands on, and all of them: paths
    /// don't cross those.
    obstacles: HashMap<FormId, Vec<Node>>,
    blocked: HashSet<Node>,
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
        self.obstacles.clear();
        self.blocked.clear();
        self.generation += 1;
    }

    /// Changes whenever navmeshes load or unload.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The triangles `owner` blocks now (none: it no longer does).
    pub fn set_obstacle(&mut self, owner: FormId, nodes: Vec<Node>) {
        if nodes.is_empty() {
            if self.obstacles.remove(&owner).is_none() {
                return;
            }
            log::debug!("obstacle {owner} blocks no navmesh triangles");
        } else if self.obstacles.get(&owner) == Some(&nodes) {
            return;
        } else {
            log::debug!("obstacle {owner} blocks {} navmesh triangles", nodes.len());
            self.obstacles.insert(owner, nodes);
        }
        self.blocked = self.obstacles.values().flatten().copied().collect();
    }

    /// Whether an obstacle stands on the triangle.
    pub fn is_blocked(&self, n: Node) -> bool {
        self.blocked.contains(&n)
    }

    /// The triangles (and their corners) whose bounds overlap the box `min`..`max`.
    pub fn triangles_in(&self, min: Vec3, max: Vec3) -> Vec<(Node, [Vec3; 3])> {
        let mut out = Vec::new();
        for (&id, m) in &self.meshes {
            if max.x < m.min.x || max.y < m.min.y || min.x > m.max.x || min.y > m.max.y {
                continue;
            }
            let (c0, r0) = m.bucket_of(min.truncate());
            let (c1, r1) = m.bucket_of(max.truncate());
            let mut seen = HashSet::new();
            for r in r0..=r1 {
                for c in c0..=c1 {
                    for &t in &m.buckets[r * m.cols + c] {
                        if !seen.insert(t) {
                            continue;
                        }
                        let p = m.corners(t as usize);
                        let lo = p[0].min(p[1]).min(p[2]);
                        let hi = p[0].max(p[1]).max(p[2]);
                        if lo.cmple(max).all() && hi.cmpge(min).all() {
                            out.push((Node { mesh: id, tri: t }, p));
                        }
                    }
                }
            }
        }
        out
    }

    /// Load the navmeshes of a cell. Returns the ids that were added.
    pub fn load_cell(&mut self, lo: &esp::LoadOrder, cell: FormId) -> Vec<FormId> {
        let mut added = Vec::new();
        let Some(idx) = lo.cell(cell) else {
            return added;
        };
        for &id in &idx.navmeshes {
            if self.meshes.contains_key(&id) {
                continue;
            }
            let Some(r) = lo.get(id) else { continue };
            if r.header.is_deleted() {
                continue;
            }
            if let Some(m) = r
                .get(b"NVNM")
                .and_then(|d| NavMesh::parse(d, |f| r.fid(f)))
                .and_then(Mesh::new)
            {
                self.meshes.insert(id, m);
                added.push(id);
            }
        }
        self.generation += 1;
        added
    }

    pub fn unload(&mut self, ids: &[FormId]) {
        for id in ids {
            self.meshes.remove(id);
        }
        self.generation += 1;
    }

    /// The triangle under (or nearest to) `p`.
    pub fn locate(&self, p: Vec3) -> Option<(Node, f32)> {
        let q = p.truncate();
        let mut best: Option<(Node, f32, f32)> = None;
        for (&id, m) in &self.meshes {
            if q.x < m.min.x - 1.0
                || q.y < m.min.y - 1.0
                || q.x > m.max.x + 1.0
                || q.y > m.max.y + 1.0
            {
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
            if q.x < m.min.x - BUCKET
                || q.y < m.min.y - BUCKET
                || q.x > m.max.x + BUCKET
                || q.y > m.max.y + BUCKET
            {
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

    /// Where an actor put at `p` comes to stand: the highest navmesh at or
    /// below it (a step above counts), as one placed in the air falls onto the
    /// floor beneath, not the one above. Off the navmesh, its nearest height;
    /// failing that, `p`.
    pub fn floor_below(&self, p: Vec3) -> Vec3 {
        const STEP: f32 = 32.0;
        let q = p.truncate();
        let mut best: Option<f32> = None;
        for m in self.meshes.values() {
            if q.x < m.min.x || q.y < m.min.y || q.x > m.max.x || q.y > m.max.y {
                continue;
            }
            let (c, r) = m.bucket_of(q);
            for &t in &m.buckets[r * m.cols + c] {
                if let Some(z) = m.height_in(t as usize, q)
                    && z <= p.z + STEP
                    && best.is_none_or(|b| z > b)
                {
                    best = Some(z);
                }
            }
        }
        match best.or_else(|| self.height_at(p)) {
            Some(z) => q.extend(z),
            None => p,
        }
    }

    fn mesh(&self, n: Node) -> &Mesh {
        &self.meshes[&n.mesh]
    }

    /// Neighbours of `n` with the shared edge (as two endpoints of `n`'s
    /// triangle), but those an obstacle blocks.
    fn neighbours(&self, n: Node, out: &mut Vec<(Node, Vec3, Vec3)>) {
        out.clear();
        let m = self.mesh(n);
        let t = &m.tris[n.tri as usize];
        for e in 0..3 {
            let a = m.verts[t.vertices[e] as usize];
            let b = m.verts[t.vertices[(e + 1) % 3] as usize];
            if let Some(nb) = t.neighbour(e) {
                out.push((
                    Node {
                        mesh: n.mesh,
                        tri: nb as u16,
                    },
                    a,
                    b,
                ));
            } else if let Some(l) = t.link(e) {
                let link = m.links[l];
                if let Some(other) = self.meshes.get(&link.navmesh)
                    && (link.triangle as usize) < other.tris.len()
                {
                    out.push((
                        Node {
                            mesh: link.navmesh,
                            tri: link.triangle,
                        },
                        a,
                        b,
                    ));
                }
            }
        }
        out.retain(|(nb, ..)| !self.blocked.contains(nb));
    }

    /// Find a path from `from` to `to`, returning waypoints (excluding the start).
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        self.route(from, to, true).map(|(path, _)| path)
    }

    /// A path to `to`, or where it can't be reached (off the navmesh, on a part
    /// the actor can't get to), to the reachable point nearest it; and whether
    /// it reaches `to`. None when `from` is off the navmesh.
    pub fn path_towards(&self, from: Vec3, to: Vec3) -> Option<(Vec<Vec3>, bool)> {
        self.route(from, to, false)
    }

    fn route(&self, from: Vec3, to: Vec3, exact: bool) -> Option<(Vec<Vec3>, bool)> {
        let (start, _) = self.locate(from)?;
        let goal = self.locate(to);
        if exact && goal.is_none() {
            return None;
        }
        let target = to;
        let to = goal.map_or(to, |(_, gz)| Vec3::new(to.x, to.y, gz));
        let goal = goal.map(|(n, _)| n);
        if goal == Some(start) {
            return Some((vec![to], true));
        }
        // The explored triangle nearest the target, and its point nearest it.
        let nearest = |n: Node| {
            let p = self.mesh(n).closest_in(n.tri as usize, target.truncate());
            (p, p.distance_squared(target))
        };
        let mut best = (start, nearest(start));
        let centre = |n: Node| self.mesh(n).centroid(n.tri as usize);
        let mut open = BinaryHeap::new();
        let mut came: HashMap<Node, (Node, Vec3, Vec3)> = HashMap::new();
        let mut cost: HashMap<Node, f32> = HashMap::new();
        // Positions are approximated by the centre of the edge used to enter a triangle.
        let mut entry: HashMap<Node, Vec3> = HashMap::new();
        cost.insert(start, 0.0);
        entry.insert(start, from);
        open.push(Open {
            f: from.distance(to),
            node: start,
        });
        let mut buf = Vec::new();
        let mut expansions = 0;
        let mut found = false;
        while let Some(Open { node, .. }) = open.pop() {
            if Some(node) == goal {
                found = true;
                break;
            }
            if !exact {
                let near = nearest(node);
                if near.1 < best.1.1 {
                    best = (node, near);
                }
            }
            expansions += 1;
            if expansions
                > if exact {
                    MAX_EXPANSIONS
                } else {
                    MAX_NEAREST_EXPANSIONS
                }
            {
                break;
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
                    let h = if Some(nb) == goal {
                        mid.distance(to)
                    } else {
                        mid.distance(to).min(centre(nb).distance(to) + 1.0)
                    };
                    open.push(Open {
                        f: ng + h,
                        node: nb,
                    });
                }
            }
        }
        let (goal, to, reached) = match goal {
            Some(g) if found => (g, to, true),
            _ if exact => return None,
            _ => {
                let (node, (end, _)) = best;
                log::trace!("can't reach {target:?}: as near as {end:?}");
                if node == start {
                    return Some((vec![end], false));
                }
                (node, end, false)
            }
        };
        // Portals from start to goal, oriented (left, right) relative to travel.
        let mut portals = Vec::new();
        let mut n = goal;
        while let Some(&(prev, a, b)) = came.get(&n) {
            let c = centre(prev);
            let d = ((a + b) * 0.5 - c).truncate();
            let (l, r) = if d.perp_dot((a - c).truncate()) > 0.0 {
                (a, b)
            } else {
                (b, a)
            };
            portals.push((l, r));
            n = prev;
        }
        portals.reverse();
        Some((string_pull(from, to, &portals), reached))
    }

    /// A random point anywhere on the given meshes.
    pub fn random_point_in(&self, ids: &[FormId], mut rand: impl FnMut() -> u64) -> Option<Vec3> {
        let meshes: Vec<&Mesh> = ids.iter().filter_map(|id| self.meshes.get(id)).collect();
        let total: usize = meshes.iter().map(|m| m.tris.len()).sum();
        if total == 0 {
            return None;
        }
        let mut k = (rand() % total as u64) as usize;
        for m in meshes {
            if k < m.tris.len() {
                return Some(m.centroid(k));
            }
            k -= m.tris.len();
        }
        None
    }

    /// A random point on the navmesh within `radius` of `centre` (in XY).
    pub fn random_point(
        &self,
        centre: Vec3,
        radius: f32,
        mut rand: impl FnMut() -> u64,
    ) -> Option<Vec3> {
        let c = centre.truncate();
        let mut candidates: Vec<(FormId, u16)> = Vec::new();
        for (&id, m) in &self.meshes {
            if c.x + radius < m.min.x
                || c.y + radius < m.min.y
                || c.x - radius > m.max.x
                || c.y - radius > m.max.y
            {
                continue;
            }
            for (i, t) in m.tris.iter().enumerate() {
                if t.flags & (NavTriangle::WATER | NavTriangle::DOOR) != 0
                    || self.blocked.contains(&Node {
                        mesh: id,
                        tri: i as u16,
                    })
                {
                    continue;
                }
                let cen = m.centroid(i);
                if cen.truncate().distance(c) <= radius
                    && (cen.z - centre.z).abs() < radius.max(300.0)
                {
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

    /// A corridor 100 wide along +X, `n` squares long, two triangles each,
    /// as one navmesh.
    fn corridor(n: usize) -> NavWorld {
        let mut w = NavWorld::default();
        w.meshes.insert(FormId(2), corridor_mesh(n, 0.0));
        w
    }

    /// [`corridor`]'s mesh at height `z`.
    fn corridor_mesh(n: usize, z: f32) -> Mesh {
        let mut vertices = Vec::new();
        for i in 0..=n {
            vertices.push([i as f32 * 100.0, 0.0, z]);
            vertices.push([i as f32 * 100.0, 100.0, z]);
        }
        let mut corners: Vec<[u16; 3]> = Vec::new();
        for i in 0..n as u16 {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
            corners.push([a, c, b]);
            corners.push([b, c, d]);
        }
        let triangles = corners
            .iter()
            .enumerate()
            .map(|(t, v)| {
                let edges = std::array::from_fn(|e| {
                    let (p, q) = (v[e], v[(e + 1) % 3]);
                    corners
                        .iter()
                        .enumerate()
                        .find(|&(o, w)| o != t && w.contains(&p) && w.contains(&q))
                        .map_or(-1, |(o, _)| o as i16)
                });
                NavTriangle {
                    vertices: *v,
                    edges,
                    flags: 0,
                    cover: 0,
                }
            })
            .collect();
        let mesh = NavMesh {
            version: 12,
            world: FormId::NULL,
            cell: FormId(1),
            grid: (0, 0),
            vertices,
            triangles,
            edge_links: Vec::new(),
            doors: Vec::new(),
        };
        Mesh::new(mesh).unwrap()
    }

    #[test]
    fn placed_actors_fall_onto_the_floor_beneath() {
        // A ground floor and an upper floor 240 above it.
        let mut w = corridor(4);
        w.meshes.insert(FormId(3), corridor_mesh(4, 240.0));
        let at = |z: f32| w.floor_below(Vec3::new(150.0, 50.0, z)).z;
        assert_eq!(at(25.0), 0.0, "in the air below the upper floor");
        assert_eq!(at(-10.0), 0.0, "a little under the ground");
        assert_eq!(at(260.0), 240.0, "over the upper floor");
        assert_eq!(at(250.0), 240.0);
        assert_eq!(at(215.0), 240.0, "within a step below it");
        // Off the navmesh: as it is.
        let off = Vec3::new(150.0, 500.0, 30.0);
        assert_eq!(w.floor_below(off), off);
    }

    #[test]
    fn obstacles_block_paths_until_they_move() {
        let mut w = corridor(6);
        let (from, to) = (Vec3::new(50.0, 50.0, 0.0), Vec3::new(550.0, 50.0, 0.0));
        assert!(w.find_path(from, to).is_some());
        // A gate across the corridor at x = 300.
        let gate: Vec<Node> = w
            .triangles_in(Vec3::new(295.0, 0.0, -10.0), Vec3::new(305.0, 100.0, 10.0))
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert!(!gate.is_empty());
        w.set_obstacle(FormId(3), gate);
        assert!(w.find_path(from, to).is_none());
        let (path, reached) = w.path_towards(from, to).unwrap();
        assert!(!reached);
        assert!(path.last().unwrap().x <= 300.0, "{path:?}");
        // Raised: the way is open again.
        w.set_obstacle(FormId(3), Vec::new());
        assert!(w.find_path(from, to).is_some());
    }

    #[test]
    fn funnel_straight_corridor() {
        // Travelling +Y, the left side is -X.
        let portals: Vec<(Vec3, Vec3)> = (1..5)
            .map(|i| {
                (
                    Vec3::new(-50.0, i as f32 * 100.0, 0.0),
                    Vec3::new(50.0, i as f32 * 100.0, 0.0),
                )
            })
            .collect();
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
