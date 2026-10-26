//! Rigid art attached to the target's own skeleton.
use super::effect::EffectInstance;
use glam::Mat4;
use std::collections::HashMap;

pub struct ActorFrame {
    pub transform: Mat4,
    pub bones: HashMap<String, Mat4>,
}
impl ActorFrame {
    pub fn node(&self, name: &str) -> Option<Mat4> {
        self.bones.get(name).map(|p| self.transform * *p)
    }
    pub fn fallback(&self, name: &str) -> Mat4 {
        self.node(name).unwrap_or(self.transform)
    }
}
pub struct ArtEffect {
    pub effect: EffectInstance,
    pub default_node: String,
    /// Active effect ids retaining this persistent art (refreshes do not stack).
    pub owners: Vec<u64>,
    pub persistent: bool,
    pub flags: u32,
    pub facing: Option<esp::FormId>,
    pub scale: f32,
    pub stopping_age: Option<f32>,
}
impl ArtEffect {
    pub fn attach(&mut self, frame: &ActorFrame) {
        self.effect.transform =
            frame.fallback(&self.default_node) * Mat4::from_scale(glam::Vec3::splat(self.scale));
        self.effect.anchors.clear();
        for (&block, tags) in &self.effect.tags {
            if let Some(node) = tags.named_nodes.first() {
                self.effect
                    .anchors
                    .insert(block, frame.node(node).unwrap_or(self.effect.transform));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    #[test]
    fn attachment_uses_actor_bones_then_default_then_root() {
        let frame = ActorFrame {
            transform: Mat4::from_translation(Vec3::X * 10.0),
            bones: HashMap::from([("hand".into(), Mat4::from_translation(Vec3::Z * 4.0))]),
        };
        assert_eq!(
            frame.fallback("hand").w_axis.truncate(),
            Vec3::new(10.0, 0.0, 4.0)
        );
        assert_eq!(frame.fallback("missing"), frame.transform);
    }
}
