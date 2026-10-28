//! Internal order: L R C LFE RL RR BL BR. Devices supply channel count only.
const HALF_POWER: f32 = std::f32::consts::FRAC_1_SQRT_2;
pub fn pan(position: f32, rear: f32) -> [f32; 2] {
    let p = position.clamp(-1.0, 1.0);
    let gain = 1.0 - 0.25 * rear.clamp(0.0, 1.0);
    [
        ((1.0 - p) * 0.5).sqrt() * gain,
        ((1.0 + p) * 0.5).sqrt() * gain,
    ]
}
pub fn stereo(bus: &[f32; 8]) -> [f32; 2] {
    [
        bus[0] + HALF_POWER * (bus[2] + bus[4] + bus[6]),
        bus[1] + HALF_POWER * (bus[2] + bus[5] + bus[7]),
    ]
}
pub fn write(bus: &[f32; 8], out: &mut [f32]) {
    match out.len() {
        0 => {}
        1 => {
            let s = stereo(bus);
            out[0] = (s[0] + s[1]) * 0.5;
        }
        2 => out.copy_from_slice(&stereo(bus)),
        4 => {
            out.copy_from_slice(&[
                bus[0] + bus[2] * HALF_POWER,
                bus[1] + bus[2] * HALF_POWER,
                bus[4] + bus[6] * HALF_POWER,
                bus[5] + bus[7] * HALF_POWER,
            ]);
        }
        n => {
            out.fill(0.0);
            let count = n.min(8);
            out[..count].copy_from_slice(&bus[..count]);
            for (i, &value) in bus.iter().enumerate().skip(count) {
                if i == 3 {
                    continue;
                }
                let target = if n >= 6 && i >= 6 { 4 + (i % 2) } else { i % 2 };
                out[target] += value * HALF_POWER;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panning_has_constant_power_and_mild_rear_attenuation() {
        for p in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let g = pan(p, 0.0);
            assert!((g[0] * g[0] + g[1] * g[1] - 1.0).abs() < 1e-6);
        }
        assert_eq!(pan(-1.0, 0.0), [1.0, 0.0]);
        assert_eq!(pan(1.0, 0.0), [0.0, 1.0]);
        assert_eq!(pan(1.0, 1.0), [0.0, 0.75]);
    }
    #[test]
    fn speaker_downmix_preserves_fronts_folds_surrounds_and_drops_lfe() {
        let mut out = [0.0; 8];
        let bus = [1.0, 2.0, 3.0, 100.0, 4.0, 5.0, 6.0, 7.0];
        write(&bus, &mut out);
        assert_eq!(out, bus);
        write(&bus, &mut out[..2]);
        assert_eq!(out[0], 1.0 + HALF_POWER * 13.0);
        assert_eq!(out[1], 2.0 + HALF_POWER * 15.0);
        write(&bus, &mut out[..6]);
        assert_eq!(out[4], 4.0 + 6.0 * HALF_POWER);
        assert_eq!(out[3], 100.0);
        write(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0], &mut out[..1]);
        assert_eq!(out[0], 0.0);
    }
}
