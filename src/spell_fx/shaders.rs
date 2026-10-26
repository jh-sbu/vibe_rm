//! Lifetimes for hit, enchantment and script-played effect shaders.
use super::records::EffectShader;
use std::sync::Arc;

pub struct ShaderEffect {
    pub shader: Arc<EffectShader>,
    pub age: f32,
    pub stopped: Option<f32>,
    pub duration: f32,
    pub persistent: bool,
    pub weapon: Option<esp::FormId>,
    pub particles: super::particles::ShaderParticles,
    pub particle_material: Option<Arc<crate::render::GpuMaterial>>,
}
impl ShaderEffect {
    pub fn new(shader: EffectShader, duration: f32, persistent: bool) -> Self {
        Self {
            shader: Arc::new(shader),
            age: 0.0,
            stopped: None,
            duration,
            persistent,
            weapon: None,
            particles: Default::default(),
            particle_material: None,
        }
    }
    pub fn stop(&mut self) {
        self.stopped.get_or_insert(self.age);
    }
    pub fn advance(&mut self, dt: f32, active: bool) -> bool {
        self.age += dt;
        if (self.persistent && !active) || self.age >= self.duration {
            self.stop();
        }
        let fade = self
            .shader
            .float(0x1C)
            .max(self.shader.float(0x44))
            .max(self.shader.float(0x78) + self.shader.float(0x84));
        self.stopped
            .is_none_or(|stop| self.age - stop <= fade.max(0.05))
    }
}
