//! `ShaderParticleGeometry.Apply(afFadeInTime)` / `Remove(afFadeOutTime)`:
//! a script's rain, snow or dust (an `SPGD`) falling round the camera, over
//! the weather's and indoors too, fading in and out
//! (`research/pex-natives/render-batch18-shader-particles.md`).

use esp::FormId;

use crate::engine::Engine;
use crate::render::precip::SCRIPT_SLOT;

/// The script's particle geometry and its fade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptPrecip {
    pub spgd: FormId,
    /// How much falls now (0..1), and how fast it changes (a second).
    pub amount: f32,
    pub rate: f32,
}

impl Engine {
    /// `Apply`: this geometry falls, fading in over `fade` seconds (in
    /// place of another a script applied).
    pub fn apply_shader_particles(&mut self, spgd: FormId, fade: f32) {
        let Some(p) = crate::world::weather::load_precipitation(&self.lo, spgd) else {
            log::debug!("ShaderParticleGeometry.Apply({spgd}): not an SPGD");
            return;
        };
        let amount = match self.script_precip {
            Some(s) if s.spgd == spgd => s.amount,
            _ => 0.0,
        };
        crate::world::loader::load_textures(&mut self.renderer, &self.vfs, vec![p.texture.clone()]);
        let Some(tex) = self.renderer.textures.get(&p.texture).flatten() else {
            log::debug!(
                "ShaderParticleGeometry.Apply({spgd}): no texture {}",
                p.texture
            );
            return;
        };
        let r = &mut self.renderer;
        r.precip
            .set_layer(&r.device, &r.sampler, SCRIPT_SLOT, Some((p, tex)));
        self.script_precip = Some(ScriptPrecip {
            spgd,
            amount,
            rate: rate(fade),
        });
        self.renderer.precip.set_script_intensity(amount);
    }

    /// `Remove`: the geometry fades out over `fade` seconds.
    pub fn remove_shader_particles(&mut self, spgd: FormId, fade: f32) {
        if let Some(s) = self.script_precip.as_mut().filter(|s| s.spgd == spgd) {
            s.rate = -rate(fade);
        }
    }

    /// Fade the script's geometry (called every frame).
    pub(crate) fn update_script_precip(&mut self, dt: f32) {
        let Some(mut s) = self.script_precip else {
            return;
        };
        s.amount = (s.amount + s.rate * dt).clamp(0.0, 1.0);
        if s.amount <= 0.0 && s.rate < 0.0 {
            self.script_precip = None;
            let r = &mut self.renderer;
            r.precip.set_layer(&r.device, &r.sampler, SCRIPT_SLOT, None);
            return;
        }
        self.renderer.precip.set_script_intensity(s.amount);
        self.script_precip = Some(s);
    }
}

/// The change a second of a fade over `secs` (at once for none).
fn rate(secs: f32) -> f32 {
    if secs > 0.0 {
        1.0 / secs
    } else {
        f32::INFINITY
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_fade_takes_its_seconds_and_none_is_at_once() {
        assert_eq!(super::rate(4.0), 0.25);
        assert!((0.0 + super::rate(0.0) * 0.016_f32).clamp(0.0, 1.0) == 1.0);
    }
}
