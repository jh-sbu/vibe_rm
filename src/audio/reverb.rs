//! Original eight-delay feedback network; approximates I3DL2, not XAudio2 DSP.
use super::Ramp;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub decay: f32,
    pub hf_reference: f32,
    pub room: f32,
    pub room_hf: f32,
    pub reflections: f32,
    pub reverb: f32,
    pub hf_ratio: f32,
    pub reflect_delay: f32,
    pub reverb_delay: f32,
    pub diffusion: f32,
    pub density: f32,
}
impl Params {
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < 14 {
            return None;
        }
        Some(Self {
            decay: (u16::from_le_bytes([d[0], d[1]]) as f32 / 1000.0).clamp(0.1, 20.0),
            hf_reference: (u16::from_le_bytes([d[2], d[3]]) as f32).clamp(20.0, 20000.0),
            room: d[4] as i8 as f32,
            room_hf: d[5] as i8 as f32,
            reflections: d[6] as i8 as f32,
            reverb: d[7] as i8 as f32,
            hf_ratio: (d[8] as f32 / 100.0).clamp(0.1, 2.0),
            reflect_delay: d[9] as f32 * 0.3 / 249.0,
            reverb_delay: d[10] as f32 / 1000.0,
            diffusion: (d[11] as f32 / 100.0).clamp(0.0, 1.0),
            density: (d[12] as f32 / 100.0).clamp(0.0, 1.0),
        })
    }
}
struct Delay {
    samples: Vec<f32>,
    at: usize,
}
impl Delay {
    fn new(seconds: f32, rate: u32) -> Self {
        Self {
            samples: vec![0.0; (seconds * rate as f32).round().max(1.0) as usize],
            at: 0,
        }
    }
    fn read(&self) -> f32 {
        self.samples[self.at]
    }
    fn write(&mut self, value: f32) {
        self.samples[self.at] = value;
        self.at = (self.at + 1) % self.samples.len();
    }
    fn tick(&mut self, value: f32) -> f32 {
        let out = self.read();
        self.write(value);
        out
    }
}
pub struct Network {
    params: Params,
    gains: [f32; 4],
    lines: [Delay; 8],
    early: Delay,
    late: Delay,
    low: [f32; 8],
    input_low: f32,
    alpha: f32,
    low_gain: [f32; 8],
    high_gain: [f32; 8],
}
fn db(value: f32) -> f32 {
    10f32.powf(value / 20.0)
}
impl Network {
    pub fn new(params: Params, rate: u32) -> Self {
        let seconds = [
            0.0239, 0.0293, 0.0317, 0.0371, 0.0411, 0.0437, 0.0479, 0.0533,
        ];
        let lengths: [f32; 8] = std::array::from_fn(|i| seconds[i] * (0.5 + 0.5 * params.density));
        Self {
            lines: std::array::from_fn(|i| Delay::new(lengths[i], rate)),
            early: Delay::new(params.reflect_delay, rate),
            late: Delay::new(params.reverb_delay, rate),
            low: [0.0; 8],
            input_low: 0.0,
            alpha: 1.0
                - (-std::f32::consts::TAU * params.hf_reference.min(rate as f32 * 0.45)
                    / rate as f32)
                    .exp(),
            low_gain: std::array::from_fn(|i| 0.001f32.powf(lengths[i] / params.decay)),
            high_gain: std::array::from_fn(|i| {
                0.001f32.powf(lengths[i] / (params.decay * params.hf_ratio))
            }),
            gains: [
                db(params.room.min(0.0)),
                db(params.room_hf.min(0.0)),
                db(params.reflections),
                db(params.reverb),
            ],
            params,
        }
    }
    fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        if self.params.room <= -100.0 {
            return [0.0; 2];
        }
        let x = (input[0] + input[1]) * 0.5;
        self.input_low += self.alpha * (x - self.input_low);
        let filtered = self.input_low + (x - self.input_low) * self.gains[1];
        let early = self.early.tick(filtered);
        let late = self.late.tick(early);
        let mut taps = [0.0; 8];
        for (i, tap) in taps.iter_mut().enumerate() {
            let delayed = self.lines[i].read();
            self.low[i] += self.alpha * (delayed - self.low[i]);
            *tap = self.low[i] * self.low_gain[i] + (delayed - self.low[i]) * self.high_gain[i];
        }
        let sum = taps.iter().sum::<f32>() * 0.25;
        for (i, tap) in taps.into_iter().enumerate() {
            let feedback = tap - self.params.diffusion * sum;
            self.lines[i].write(late * 0.25 + feedback);
        }
        let mut out = [early * self.gains[2]; 2];
        for (i, tap) in taps.into_iter().enumerate() {
            out[i % 2] += tap * 0.25 * self.gains[3];
        }
        out.map(|v| v * self.gains[0])
    }
}
/// Construct networks outside the callback. At most two are active during a change.
pub struct Bus {
    pub params: Option<Params>,
    current: Option<Network>,
    previous: Option<Network>,
    fade: Ramp,
}
impl Default for Bus {
    fn default() -> Self {
        Self {
            params: None,
            current: None,
            previous: None,
            fade: Ramp::new(1.0),
        }
    }
}
impl Bus {
    pub fn change(
        &mut self,
        params: Option<Params>,
        network: Option<Network>,
        seconds: f32,
        rate: u32,
    ) {
        self.previous = self.current.take();
        self.current = network;
        self.params = params;
        self.fade = Ramp::new(0.0);
        self.fade.set(1.0, seconds, rate);
    }
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let a = if self.fade.left > 0 {
            self.previous
                .as_mut()
                .map_or([0.0; 2], |n| n.process(input))
        } else {
            [0.0; 2]
        };
        let b = self.current.as_mut().map_or([0.0; 2], |n| n.process(input));
        let t = self.fade.current;
        self.fade.advance();
        // Do not free a previous delay allocation on the audio thread; it is
        // reclaimed on the next change (and is not processed after this fade).
        if self.fade.left == 0 {
            return b;
        }
        std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * t)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn params() -> Params {
        Params::parse(&[
            0xE8,
            3,
            0x88,
            0x13,
            0,
            (-20i8) as u8,
            (-12i8) as u8,
            0,
            70,
            10,
            10,
            100,
            100,
            0,
        ])
        .unwrap()
    }
    #[test]
    fn reverb_has_delayed_finite_decaying_energy_and_default_off_is_silent() {
        let p = params();
        let mut n = Network::new(p, 8000);
        let mut bins = [0.0f64; 4];
        for i in 0..32000 {
            let out = n.process(if i == 0 { [1.0; 2] } else { [0.0; 2] });
            assert!(out.iter().all(|v| v.is_finite()));
            bins[i / 8000] += out.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
            if i < 10 {
                assert_eq!(out, [0.0; 2]);
            }
        }
        assert!(bins[0] > 0.0 && bins[3] < bins[0] * 0.01, "{bins:?}");
        let mut off = Network::new(Params { room: -100.0, ..p }, 8000);
        assert_eq!(off.process([1.0; 2]), [0.0; 2]);
        let p = Params::parse(&[0, 1, 0, 1, 0, 0, 0, 0, 100, 249, 0, 100, 100, 0]).unwrap();
        assert!((p.reflect_delay - 0.3).abs() < 1e-6);
    }
}
