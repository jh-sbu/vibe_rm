//! Skeletons: node hierarchies used to pose skinned meshes.

use std::collections::HashMap;

use glam::Mat4;
use nif::{Block, Nif, Ref, Transform};

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub bind: Transform,
}

#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    pub by_name: HashMap<String, usize>,
}

impl Skeleton {
    pub fn from_nif(nif: &Nif) -> Skeleton {
        let mut s = Skeleton::default();
        for &root in &nif.roots {
            s.add(nif, Ref(root as i32), None, 0);
        }
        s
    }

    fn add(&mut self, nif: &Nif, r: Ref, parent: Option<usize>, depth: u32) {
        if depth > 128 {
            return;
        }
        let Some(Block::Node(n)) = nif.get(r) else { return };
        let idx = self.bones.len();
        self.bones.push(Bone { name: n.av.net.name.clone(), parent, bind: n.av.transform });
        self.by_name.entry(n.av.net.name.to_ascii_lowercase()).or_insert(idx);
        for &c in &n.children {
            self.add(nif, c, Some(idx), depth + 1);
        }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }

    /// Model-space matrices for a set of local transforms (one per bone; parents precede children).
    pub fn model_space(&self, locals: &[Transform]) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let local = locals.get(i).copied().unwrap_or(b.bind).to_mat4();
            let m = match b.parent {
                Some(p) => out[p] * local,
                None => local,
            };
            out.push(m);
        }
        out
    }

    pub fn bind_locals(&self) -> Vec<Transform> {
        self.bones.iter().map(|b| b.bind).collect()
    }
}
