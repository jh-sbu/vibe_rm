//! Audio output: a small mixer with 3D positioned voices on top of cpal.

pub mod decode;
pub mod director;
pub mod regions;
pub mod reverb;
pub mod settings;
pub mod spaces;
pub mod speakers;

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use glam::Vec3;

use decode::Clip;

/// Frames per output callback (about 21 ms at 48 kHz).
const BUFFER_FRAMES: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoiceId(u64);

/// How a positioned sound fades with the distance from the listener: full
/// (or the curve's first point) within `min`, nothing past `max` (the curve's
/// last point), a piecewise linear curve between: the sound output model's
/// five points, at 0, 25, 50, 75 and 100% of the way from `min` to `max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    pub min: f32,
    pub max: f32,
    /// The gain at each of the five points.
    pub curve: [f32; 5],
}

impl Falloff {
    /// The curve most output models in the base game have (100, 50, 20, 5, 0
    /// percent): close to the square of the remaining distance.
    pub const DEFAULT_CURVE: [f32; 5] = [1.0, 0.5, 0.2, 0.05, 0.0];

    pub fn new(min: f32, max: f32) -> Self {
        Falloff {
            min,
            max,
            curve: Self::DEFAULT_CURVE,
        }
    }

    /// The gain at `distance`.
    pub fn gain(&self, distance: f32) -> f32 {
        let x = ((distance - self.min) / (self.max - self.min).max(1.0)).clamp(0.0, 1.0) * 4.0;
        let i = (x as usize).min(3);
        let t = x - i as f32;
        self.curve[i] + (self.curve[i + 1] - self.curve[i]) * t
    }
}

/// What a script did to a sound category (`SoundCategory.Mute`, `Pause`,
/// `SetVolume`, `SetFrequency`): it reaches every voice in the category or
/// below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CategoryState {
    pub muted: bool,
    pub paused: bool,
    pub volume: f32,
    pub frequency: f32,
}

impl Default for CategoryState {
    fn default() -> Self {
        CategoryState {
            muted: false,
            paused: false,
            volume: 1.0,
            frequency: 1.0,
        }
    }
}

/// Sample-clocked linear-amplitude ramp. Retargeting starts at the current gain.
#[derive(Clone, Copy, Debug)]
struct Ramp {
    current: f32,
    target: f32,
    step: f32,
    left: u64,
}
impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
            left: 0,
        }
    }
    fn set(&mut self, target: f32, seconds: f32, rate: u32) {
        if self.target == target {
            if seconds <= 0.0 {
                self.current = target;
                self.left = 0;
                self.step = 0.0;
            }
            return;
        }
        self.target = target;
        self.left = (seconds.max(0.0) * rate as f32) as u64;
        if self.left == 0 {
            self.current = target;
            self.step = 0.0;
        } else {
            self.step = (target - self.current) / self.left as f32;
        }
    }
    fn advance(&mut self) {
        if self.left > 0 {
            self.left -= 1;
            self.current += self.step;
            if self.left == 0 {
                self.current = self.target;
            }
        }
    }
}
#[derive(Clone, Copy)]
struct CategoryMix {
    state: CategoryState,
    gain: Ramp,
}
impl CategoryMix {
    fn new(state: CategoryState, seconds: f32, rate: u32) -> Self {
        let mut value = Self {
            state,
            gain: Ramp::new(1.0),
        };
        value.set(state, seconds, rate);
        value
    }
    fn set(&mut self, state: CategoryState, seconds: f32, rate: u32) {
        self.state = state;
        self.gain.set(
            if state.muted || state.paused {
                0.0
            } else {
                state.volume
            },
            seconds,
            rate,
        );
    }
}

/// Output routing is carried through decoding, even before speaker mixing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OutputMode {
    #[default]
    Spatial,
    Defined([[f32; 8]; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopRegion {
    pub begin: f32,
    pub end: f32,
    pub count: u32,
}
#[derive(Clone, Debug)]
pub struct VoiceParams {
    pub volume: f32,
    pub fade_in: f32,
    pub rate: f32,
    pub looping: bool,
    pub loop_region: Option<LoopRegion>,
    pub position: Option<Vec3>,
    pub falloff: Falloff,
    pub categories: Vec<u32>,
    pub priority: u8,
    pub reverb_send: f32,
    pub output: OutputMode,
    pub exempt: bool,
}
impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            fade_in: 0.0,
            rate: 1.0,
            looping: false,
            loop_region: None,
            position: None,
            falloff: Falloff::new(200.0, 2000.0),
            categories: Vec::new(),
            priority: 128,
            reverb_send: 0.0,
            output: OutputMode::Spatial,
            exempt: false,
        }
    }
}
#[derive(Clone)]
struct Voice {
    id: VoiceId,
    clip: Arc<Clip>,
    pos: f64,
    params: VoiceParams,
    gain: Ramp,
    stopping: bool,
    repeats: u32,
    filter: [f32; 2],
    virtualized: bool,
}

impl Voice {
    /// Normalize immediately after advancing too: a buffer may end on a loop boundary.
    fn wrap_loops(&mut self) {
        let n = self.clip.frames();
        if n == 0 {
            return;
        }
        if let Some(region) = self.params.loop_region {
            let begin = region.begin.max(0.0) as f64 * self.clip.rate as f64;
            let end = (region.end as f64 * self.clip.rate as f64).min(n as f64);
            if end > begin && self.pos >= end && self.repeats < region.count {
                let wraps = ((self.pos - begin) / (end - begin)).floor().max(1.0) as u32;
                let wraps = wraps.min(region.count - self.repeats);
                self.pos -= (end - begin) * wraps as f64;
                self.repeats += wraps;
            }
        }
        if self.params.looping {
            self.pos %= n as f64;
        }
    }
}

struct Mixer {
    voices: Vec<Voice>,
    listener: Vec3,
    listener_right: Vec3,
    rate: u32,
    master: Ramp,
    headroom: f32,
    voice_limit: usize,
    /// Fixed frame buses: no allocation, even for an unexpected callback size.
    dry: [f32; 8],
    send: [f32; 2],
    reverb: reverb::Bus,
    /// Voices stopped while their clips were still decoding.
    cancelled: Vec<VoiceId>,
    categories: HashMap<u32, CategoryMix>,
}

impl Mixer {
    fn category_effect(categories: &HashMap<u32, CategoryMix>, of: &[u32]) -> (bool, f32, f64) {
        let (mut paused, mut volume, mut frequency) = (false, 1.0, 1.0);
        for s in of.iter().filter_map(|c| categories.get(c)) {
            paused |= s.state.paused && s.gain.left == 0;
            volume *= s.gain.current;
            frequency *= s.state.frequency as f64;
        }
        (paused, volume, frequency)
    }
    fn audibility(&self, v: &Voice) -> f32 {
        let (_, gain, _) = Self::category_effect(&self.categories, &v.params.categories);
        v.gain.current
            * gain
            * v.params
                .position
                .map_or(1.0, |p| v.params.falloff.gain(p.distance(self.listener)))
    }
    fn select_voices(&mut self) {
        for v in &mut self.voices {
            v.virtualized = !v.params.exempt;
        }
        for _ in 0..self.voice_limit {
            let best = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.virtualized
                        && !Self::category_effect(&self.categories, &v.params.categories).0
                })
                .min_by(|(_, a), (_, b)| {
                    a.params
                        .priority
                        .cmp(&b.params.priority)
                        .then_with(|| self.audibility(b).total_cmp(&self.audibility(a)))
                        .then(a.id.0.cmp(&b.id.0))
                })
                .map(|(i, _)| i);
            let Some(i) = best else { break };
            self.voices[i].virtualized = false;
        }
        let categories = &self.categories;
        self.voices.retain(|v| {
            !v.virtualized
                || v.params.looping
                || Self::category_effect(categories, &v.params.categories).0
        });
    }
    fn mix(&mut self, out: &mut [f32], channels: usize) {
        self.select_voices();
        out.fill(0.0);
        for frame in out.chunks_exact_mut(channels) {
            self.dry.fill(0.0);
            self.send.fill(0.0);
            for v in &mut self.voices {
                let (paused, gain, frequency) =
                    Self::category_effect(&self.categories, &v.params.categories);
                if paused {
                    if v.stopping {
                        v.gain.advance();
                    }
                    continue;
                }
                let n = v.clip.frames();
                if n == 0 {
                    v.pos = f64::MAX;
                    continue;
                }
                v.wrap_loops();
                let i = v.pos as usize;
                if i >= n {
                    continue;
                }
                if v.virtualized {
                    v.pos += v.clip.rate as f64 / self.rate as f64
                        * v.params.rate.max(0.01) as f64
                        * frequency;
                    v.wrap_loops();
                    v.gain.advance();
                    continue;
                }
                let volume = v.gain.current * gain;
                let (mut gl, mut gr) = (volume, volume);
                let mut rear = 0.0;
                if let Some(p) = v.params.position {
                    let att = v.params.falloff.gain(p.distance(self.listener));
                    let pan = (p - self.listener)
                        .normalize_or_zero()
                        .dot(self.listener_right)
                        .clamp(-1.0, 1.0);
                    rear = (-(p - self.listener)
                        .normalize_or_zero()
                        .dot(Vec3::Z.cross(self.listener_right)))
                    .max(0.0);
                    let gains = speakers::pan(pan, rear);
                    gl *= att * gains[0];
                    gr *= att * gains[1];
                }
                let j = if i + 1 < n {
                    i + 1
                } else if v.params.looping {
                    0
                } else {
                    i
                };
                let t = (v.pos - i as f64) as f32;
                let mut samples: [f32; 2] = std::array::from_fn(|c| {
                    v.clip.samples[i * 2 + c] * (1.0 - t) + v.clip.samples[j * 2 + c] * t
                });
                for (c, sample) in samples.iter_mut().enumerate() {
                    if rear > 0.0 {
                        let cutoff = (20000.0 - 16000.0 * rear).min(self.rate as f32 * 0.45);
                        let alpha =
                            1.0 - (-std::f32::consts::TAU * cutoff / self.rate as f32).exp();
                        v.filter[c] += alpha * (*sample - v.filter[c]);
                        *sample = *sample * (1.0 - rear) + v.filter[c] * rear;
                    } else {
                        v.filter[c] = *sample;
                    }
                }
                let sample = |c: usize| samples[c];
                let routed = if let (None, OutputMode::Defined(rows)) =
                    (v.params.position, v.params.output)
                {
                    std::array::from_fn(|c| {
                        if v.clip.channels == 1 {
                            sample(0) * volume * rows[0][c]
                        } else {
                            (sample(0) * rows[1][c] + sample(1) * rows[2][c]) * volume
                        }
                    })
                } else {
                    [sample(0) * gl, sample(1) * gr, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
                };
                for (c, value) in routed.into_iter().enumerate() {
                    self.dry[c] += value;
                }
                for (c, value) in speakers::stereo(&routed).into_iter().enumerate() {
                    self.send[c] += value * v.params.reverb_send;
                }
                v.pos += v.clip.rate as f64 / self.rate as f64
                    * v.params.rate.max(0.01) as f64
                    * frequency;
                v.wrap_loops();
                v.gain.advance();
            }
            let wet = self.reverb.process(self.send);
            self.dry[0] += wet[0];
            self.dry[1] += wet[1];
            speakers::write(&self.dry, frame);
            for sample in frame {
                *sample = (*sample * self.master.current * self.headroom).clamp(-1.0, 1.0);
            }
            self.master.advance();
            for c in self.categories.values_mut() {
                c.gain.advance();
            }
        }
        self.voices.retain(|v| {
            !(v.stopping && v.gain.left == 0)
                && v.clip.frames() > 0
                && (v.params.looping || v.pos < v.clip.frames() as f64)
        });
    }
}

enum Pending {
    Play { id: VoiceId, params: VoiceParams },
}

pub struct Audio {
    _stream: Option<cpal::Stream>,
    mixer: Arc<Mutex<Mixer>>,
    pub rate: u32,
    cache: HashMap<String, Option<Arc<Clip>>>,
    waiting: HashMap<String, Vec<Pending>>,
    req_tx: Sender<(String, Vec<u8>)>,
    res_rx: Receiver<(String, Option<Clip>)>,
    next_id: u64,
}

impl Audio {
    /// Open the default output device. Returns a silent instance if none is available.
    pub fn new() -> Audio {
        let mixer = Arc::new(Mutex::new(Mixer {
            voices: Vec::new(),
            listener: Vec3::ZERO,
            listener_right: Vec3::X,
            rate: 44100,
            master: Ramp::new(1.0),
            headroom: 0.8,
            voice_limit: 64,
            dry: [0.0; 8],
            send: [0.0; 2],
            reverb: reverb::Bus::default(),
            cancelled: Vec::new(),
            categories: HashMap::new(),
        }));
        let mut rate = 44100;
        let stream = (|| -> Option<cpal::Stream> {
            // Prefer PulseAudio (also served by PipeWire), then the platform default.
            let hosts = cpal::available_hosts();
            log::debug!("audio hosts: {hosts:?}");
            let host = hosts
                .iter()
                .filter(|h| format!("{h:?}").to_ascii_lowercase().contains("pulse"))
                .find_map(|h| cpal::host_from_id(*h).ok())
                .unwrap_or_else(cpal::default_host);
            let device = host
                .default_output_device()
                .or_else(|| cpal::default_host().default_output_device())?;
            let cfg = device.default_output_config().ok()?;
            rate = cfg.sample_rate();
            let channels = cfg.channels() as usize;
            mixer.lock().unwrap().rate = rate;
            let build = |buffer: cpal::BufferSize| {
                let mut config = cfg.config();
                config.buffer_size = buffer;
                let m = mixer.clone();
                device.build_output_stream(
                    config,
                    move |out: &mut [f32], _| {
                        if let Ok(mut mx) = m.lock() {
                            mx.mix(out, channels);
                        } else {
                            out.fill(0.0);
                        }
                    },
                    |e| log::warn!("audio stream error: {e}"),
                    None,
                )
            };
            // A short buffer: sounds start when asked (PulseAudio's default holds
            // about two seconds), and voices start on time within it.
            let frames = match cfg.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => BUFFER_FRAMES.clamp(*min, *max),
                cpal::SupportedBufferSize::Unknown => BUFFER_FRAMES,
            };
            let stream = build(cpal::BufferSize::Fixed(frames))
                .or_else(|e| {
                    log::warn!("audio: a {frames}-frame buffer failed ({e}); using the default");
                    build(cpal::BufferSize::Default)
                })
                .map_err(|e| log::warn!("audio: {e}"))
                .ok()?;
            stream.play().ok()?;
            Some(stream)
        })();
        if stream.is_none() {
            log::warn!("audio: no output device; running silent");
        } else {
            log::info!("audio: {rate} Hz output");
        }
        let (req_tx, req_rx) = channel::<(String, Vec<u8>)>();
        let (res_tx, res_rx) = channel();
        let r = rate;
        std::thread::Builder::new()
            .name("audio-decode".into())
            .spawn(move || {
                while let Ok((path, bytes)) = req_rx.recv() {
                    let clip = decode::decode(&path, &bytes, r);
                    if clip.is_none() {
                        log::debug!("audio: could not decode {path}");
                    }
                    if res_tx.send((path, clip)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn decoder thread");
        Audio {
            _stream: stream,
            mixer,
            rate,
            cache: HashMap::new(),
            waiting: HashMap::new(),
            req_tx,
            res_rx,
            next_id: 1,
        }
    }

    pub fn set_listener(&self, pos: Vec3, right: Vec3) {
        if let Ok(mut m) = self.mixer.lock() {
            m.listener = pos;
            m.listener_right = right;
        }
    }

    pub fn set_master(&self, v: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            let rate = m.rate;
            m.master.set(v.clamp(0.0, 1.0), 0.005, rate);
        }
    }

    pub fn set_reverb(&self, params: Option<reverb::Params>, seconds: f32) {
        if self.mixer.lock().is_ok_and(|m| m.reverb.params == params) {
            return;
        }
        let network = params
            .filter(|p| p.room > -100.0)
            .map(|p| reverb::Network::new(p, self.rate));
        if let Ok(mut m) = self.mixer.lock() {
            let rate = m.rate;
            m.reverb.change(params, network, seconds, rate);
        }
    }

    /// Start a file from the VFS; decoding happens off the callback thread.
    pub fn play(&mut self, vfs: &vfs::Vfs, path: &str, params: VoiceParams) -> Option<VoiceId> {
        let key = vfs::normalize_path(path);
        let id = VoiceId(self.next_id);
        self.next_id += 1;
        let p = Pending::Play { id, params };
        match self.cache.get(&key) {
            Some(Some(clip)) => {
                let clip = clip.clone();
                self.start(clip, p);
            }
            Some(None) => return None,
            None => {
                if !self.waiting.contains_key(&key) {
                    let bytes = vfs.read(&key)?;
                    let _ = self.req_tx.send((key.clone(), bytes));
                }
                self.waiting.entry(key).or_default().push(p);
            }
        }
        Some(id)
    }

    fn start(&self, clip: Arc<Clip>, p: Pending) {
        let Pending::Play { id, params } = p;
        if let Ok(mut m) = self.mixer.lock() {
            if let Some(i) = m.cancelled.iter().position(|&c| c == id) {
                m.cancelled.swap_remove(i);
                return;
            }
            let mut gain = Ramp::new(if params.fade_in > 0.0 {
                0.0
            } else {
                params.volume
            });
            gain.set(params.volume, params.fade_in, m.rate);
            m.voices.push(Voice {
                id,
                clip,
                pos: 0.0,
                params,
                gain,
                stopping: false,
                repeats: 0,
                filter: [0.0; 2],
                virtualized: false,
            });
        }
    }

    /// Set what is done to a sound category.
    pub fn set_category(&self, category: u32, state: CategoryState, seconds: f32) {
        if let Ok(mut m) = self.mixer.lock() {
            let rate = m.rate;
            m.categories
                .entry(category)
                .and_modify(|c| c.set(state, seconds, rate))
                .or_insert_with(|| CategoryMix::new(state, seconds, rate));
        }
    }

    /// Stop over a short ramp, including voices whose decode is pending.
    pub fn stop(&self, id: VoiceId) {
        self.stop_with_fade(id, 0.005);
    }
    pub fn stop_with_fade(&self, id: VoiceId, seconds: f32) {
        let waiting = self
            .waiting
            .values()
            .flatten()
            .any(|p| matches!(p, Pending::Play { id: i, .. } if *i == id));
        if let Ok(mut m) = self.mixer.lock() {
            let rate = m.rate;
            for v in m.voices.iter_mut().filter(|v| v.id == id) {
                v.stopping = true;
                v.gain.set(0.0, seconds, rate);
            }
            if waiting && !m.cancelled.contains(&id) {
                m.cancelled.push(id);
            }
        }
    }

    /// Change a voice's volume (a waiting one's too).
    pub fn set_volume(&mut self, id: VoiceId, volume: f32) {
        self.set_volume_with_fade(id, volume, 0.005);
    }

    pub fn set_volume_with_fade(&mut self, id: VoiceId, volume: f32, seconds: f32) {
        for p in self.waiting.values_mut().flatten() {
            let Pending::Play { id: i, params } = p;
            if *i == id {
                params.volume = volume;
            }
        }
        if let Ok(mut m) = self.mixer.lock() {
            let rate = m.rate;
            for v in m.voices.iter_mut().filter(|v| v.id == id && !v.stopping) {
                v.gain.set(volume, seconds, rate);
            }
        }
    }

    pub fn stop_all(&mut self) {
        self.waiting.clear();
        if let Ok(mut m) = self.mixer.lock() {
            m.voices.clear();
            m.cancelled.clear();
        }
    }

    pub fn is_playing(&self, id: VoiceId) -> bool {
        self.waiting
            .values()
            .flatten()
            .any(|p| matches!(p, Pending::Play { id: i, .. } if *i == id))
            || self
                .mixer
                .lock()
                .map(|m| m.voices.iter().any(|v| v.id == id))
                .unwrap_or(false)
    }

    pub fn voice_position(&self, id: VoiceId) -> Option<f32> {
        self.mixer
            .lock()
            .ok()?
            .voices
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.pos as f32 / v.clip.rate.max(1) as f32)
    }

    /// Duration of a decoded clip, if known.
    pub fn clip_duration(&self, path: &str) -> Option<f32> {
        self.cache
            .get(&vfs::normalize_path(path))
            .and_then(|c| c.as_ref())
            .map(|c| c.duration())
    }

    /// Collect finished decodes and start waiting voices.
    pub fn update(&mut self) {
        while let Ok((path, clip)) = self.res_rx.try_recv() {
            let clip = clip.map(Arc::new);
            if let Some(waiting) = self.waiting.remove(&path)
                && let Some(c) = &clip
            {
                for p in waiting {
                    self.start(c.clone(), p);
                }
            }
            self.cache.insert(path, clip);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A voice in category 7 of the ramp mixer, whose category was changed.
    fn mixed_in_category(state: CategoryState) -> (Mixer, Vec<f32>) {
        let mut m = mixer(1.0);
        m.voices[0].params.categories = vec![3, 7];
        m.categories.insert(7, CategoryMix::new(state, 0.0, m.rate));
        let mut out = vec![0.0; 20];
        m.mix(&mut out, 1);
        (m, out)
    }

    fn audio_with_mixer(m: Mixer) -> Audio {
        let (req_tx, _) = channel();
        let (_, res_rx) = channel();
        Audio {
            _stream: None,
            rate: m.rate,
            mixer: Arc::new(Mutex::new(m)),
            cache: HashMap::new(),
            waiting: HashMap::new(),
            req_tx,
            res_rx,
            next_id: 2,
        }
    }
    #[test]
    fn duck_volume_updates_cannot_retarget_a_stopping_voice() {
        let mut a = audio_with_mixer(mixer(1.0));
        let id = VoiceId(1);
        a.stop_with_fade(id, 0.01);
        a.mixer.lock().unwrap().mix(&mut [0.0; 5], 1);
        a.set_volume_with_fade(id, 1.0, 6.0);
        let mut m = a.mixer.lock().unwrap();
        assert_eq!(m.voices[0].gain.target, 0.0);
        assert_eq!(m.voices[0].gain.left, 5);
        m.mix(&mut [0.0; 5], 1);
        assert!(m.voices.is_empty());
    }
    #[test]
    fn lnam_repeats_survive_a_boundary_on_the_last_buffer_frame() {
        let mut m = mixer(1.0);
        m.voices[0].params.loop_region = Some(LoopRegion {
            begin: 0.0,
            end: 0.1,
            count: 2,
        });
        for repeat in [1, 2] {
            m.mix(&mut [0.0; 100], 1);
            assert_eq!(m.voices.len(), 1);
            assert_eq!(m.voices[0].repeats, repeat);
            assert_eq!(m.voices[0].pos, 0.0);
        }
        m.mix(&mut [0.0; 100], 1);
        assert!(m.voices.is_empty());
        let mut m = mixer(1.5);
        m.voices[0].params.loop_region = Some(LoopRegion {
            begin: 0.0,
            end: 0.1,
            count: 2,
        });
        m.mix(&mut [0.0; 67], 1);
        assert_eq!(m.voices.len(), 1);
        assert_eq!(m.voices[0].repeats, 1);
        assert_eq!(m.voices[0].pos, 0.5);
    }

    #[test]
    fn an_immediate_target_finishes_an_existing_fade_to_the_same_value() {
        let mut ramp = Ramp::new(1.0);
        ramp.set(0.0, 1.0, 1000);
        ramp.advance();
        ramp.set(0.0, 0.0, 1000);
        assert_eq!(ramp.current, 0.0);
        assert_eq!(ramp.left, 0);
    }
    #[test]
    fn voice_cap_prefers_priority_then_audibility_and_virtualizes_loops() {
        let mut m = mixer(1.0);
        m.voice_limit = 2;
        for id in [2, 3] {
            let mut v = m.voices[0].clone();
            v.id = VoiceId(id);
            m.voices.push(v);
        }
        m.voices[0].params.looping = true;
        m.voices[0].params.priority = 192;
        m.voices[1].params.priority = 64;
        m.voices[2].params.priority = 128;
        m.mix(&mut [0.0; 10], 1);
        assert!(m.voices[0].virtualized);
        assert_eq!(m.voices[0].pos, 10.0);
        m.voices.retain(|v| v.id != VoiceId(3));
        m.mix(&mut [0.0; 10], 1);
        assert!(!m.voices[0].virtualized);
        assert_eq!(m.voices[0].pos, 20.0);
        let mut m = mixer(1.0);
        m.voice_limit = 2;
        m.voices[0].gain = Ramp::new(0.1);
        for (id, gain) in [(2, 0.5), (3, 0.9)] {
            let mut v = m.voices[0].clone();
            v.id = VoiceId(id);
            v.gain = Ramp::new(gain);
            m.voices.push(v);
        }
        let mut exempt = m.voices[0].clone();
        exempt.id = VoiceId(4);
        exempt.params.exempt = true;
        m.voices.push(exempt);
        m.mix(&mut [0.0; 1], 1);
        assert_eq!(
            m.voices.iter().map(|v| v.id.0).collect::<Vec<_>>(),
            [2, 3, 4]
        );
    }
    #[test]
    fn defined_output_uses_mono_and_independent_stereo_rows() {
        let mut m = mixer(1.0);
        let mut rows = [[0.0; 8]; 3];
        rows[0][2] = 1.0;
        rows[1][0] = 0.5;
        rows[2][1] = 0.75;
        m.voices[0].params.output = OutputMode::Defined(rows);
        let mut out = [0.0; 88];
        m.mix(&mut out, 8);
        assert!((out[82] - 0.01).abs() < 1e-6);
        assert_eq!(out[80], 0.0);
        let mut m = mixer(1.0);
        Arc::get_mut(&mut m.voices[0].clip).unwrap().channels = 2;
        m.voices[0].params.output = OutputMode::Defined(rows);
        m.mix(&mut out, 8);
        assert!((out[80] - 0.005).abs() < 1e-6);
        assert!((out[81] - 0.0075).abs() < 1e-6);
    }
    #[test]
    fn finite_loop_regions_repeat_then_play_the_tail() {
        let mut m = mixer(1.0);
        m.voices[0].params.loop_region = Some(LoopRegion {
            begin: 0.01,
            end: 0.02,
            count: 2,
        });
        let mut out = [0.0; 45];
        m.mix(&mut out, 1);
        assert!((out[20] - 0.01).abs() < 1e-6);
        assert!((out[30] - 0.01).abs() < 1e-6);
        assert!((out[40] - 0.02).abs() < 1e-6);
    }
    #[test]
    fn half_rate_interpolates_and_loops_without_a_missing_frame() {
        let mut m = mixer(0.5);
        let mut out = [0.0; 5];
        m.mix(&mut out, 1);
        for (i, s) in out.into_iter().enumerate() {
            assert!((s - i as f32 * 0.0005).abs() < 1e-6);
        }
        m.voices[0].params.looping = true;
        m.voices[0].pos = 99.5;
        m.mix(&mut out[..2], 1);
        assert!((out[0] - 0.0495).abs() < 1e-6);
        assert_eq!(out[1], 0.0);
    }

    #[test]
    fn ramps_retarget_and_fade_before_holding_position() {
        let mut ramp = Ramp::new(1.0);
        ramp.set(0.0, 0.01, 1000);
        for _ in 0..5 {
            ramp.advance();
        }
        assert!((ramp.current - 0.5).abs() < 1e-6);
        ramp.set(1.0, 0.005, 1000);
        for _ in 0..5 {
            ramp.advance();
        }
        assert_eq!(ramp.current, 1.0);
        let mut m = mixer(1.0);
        m.voices[0].params.categories = vec![7];
        m.categories.insert(
            7,
            CategoryMix::new(
                CategoryState {
                    paused: true,
                    ..Default::default()
                },
                0.01,
                1000,
            ),
        );
        m.mix(&mut [0.0; 20], 1);
        assert_eq!(m.voices[0].pos, 10.0);
        m.mix(&mut [0.0; 20], 1);
        assert_eq!(m.voices[0].pos, 10.0);
    }

    #[test]
    fn a_sound_category_mutes_pauses_scales_and_speeds_its_voices() {
        let (_, normal) = mixed_in_category(CategoryState::default());
        assert!(normal[10] > 0.0);
        let (_, muted) = mixed_in_category(CategoryState {
            muted: true,
            ..Default::default()
        });
        assert!(muted.iter().all(|&s| s == 0.0));
        let (m, paused) = mixed_in_category(CategoryState {
            paused: true,
            ..Default::default()
        });
        assert!(paused.iter().all(|&s| s == 0.0));
        assert_eq!(m.voices[0].pos, 0.0, "a paused voice doesn't advance");
        let (_, quiet) = mixed_in_category(CategoryState {
            volume: 0.5,
            ..Default::default()
        });
        assert!((quiet[10] - normal[10] * 0.5).abs() < 1e-6);
        let (_, slow) = mixed_in_category(CategoryState {
            frequency: 0.5,
            ..Default::default()
        });
        assert!((slow[10] - normal[5]).abs() < 1e-6, "half the rate");
        // Another category leaves it alone.
        let mut m = mixer(1.0);
        m.voices[0].params.categories = vec![3];
        m.categories.insert(
            7,
            CategoryMix::new(
                CategoryState {
                    muted: true,
                    ..Default::default()
                },
                0.0,
                1000,
            ),
        );
        let mut out = vec![0.0; 20];
        m.mix(&mut out, 1);
        assert_eq!(out, normal);
    }

    fn mixer(pitch: f64) -> Mixer {
        let clip = Arc::new(Clip {
            // A ramp the mix can be read against: frame i holds i / 1000.
            samples: (0..100).flat_map(|i| [i as f32 / 1000.0; 2]).collect(),
            rate: 1000,
            channels: 1,
        });
        Mixer {
            voices: vec![Voice {
                id: VoiceId(1),
                clip,
                pos: 0.0,
                gain: Ramp::new(1.0),
                stopping: false,
                repeats: 0,
                filter: [0.0; 2],
                virtualized: false,
                params: VoiceParams {
                    rate: pitch as f32,
                    ..Default::default()
                },
            }],
            listener: Vec3::ZERO,
            listener_right: Vec3::X,
            rate: 1000,
            master: Ramp::new(1.0),
            headroom: 1.0,
            voice_limit: 64,
            dry: [0.0; 8],
            send: [0.0; 2],
            reverb: reverb::Bus::default(),
            cancelled: Vec::new(),
            categories: HashMap::new(),
        }
    }

    #[test]
    fn falloff_follows_its_curve_between_min_and_max() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        let f = Falloff {
            min: 100.0,
            max: 500.0,
            curve: [1.0, 0.5, 0.2, 0.05, 0.0],
        };
        // The points are at 0, 25, 50, 75 and 100% of the way.
        for (d, g) in [
            (0.0, 1.0),
            (100.0, 1.0),
            (200.0, 0.5),
            (300.0, 0.2),
            (400.0, 0.05),
        ] {
            assert!(near(f.gain(d), g), "{d}: {}", f.gain(d));
        }
        // Linear between them, and nothing after the last.
        assert!(near(f.gain(150.0), 0.75));
        assert!(near(f.gain(350.0), 0.125));
        assert!(near(f.gain(500.0), 0.0) && near(f.gain(9000.0), 0.0));
        // A curve may rise (the distant sounds that are loudest part way).
        let hump = Falloff {
            curve: [0.0, 0.5, 1.0, 0.5, 0.0],
            ..f
        };
        assert!(near(hump.gain(300.0), 1.0) && near(hump.gain(100.0), 0.0));
        // The default is near the old square of the remaining distance.
        let d = Falloff::new(0.0, 1000.0);
        for x in [0.25f32, 0.5, 0.75] {
            assert!((d.gain(x * 1000.0) - (1.0 - x).powi(2)).abs() < 0.07);
        }
    }

    #[test]
    fn positioned_voices_fade_by_their_falloff() {
        let mut m = mixer(1.0);
        m.voices[0].params.position = Some(Vec3::new(0.0, 300.0, 0.0));
        m.voices[0].params.falloff = Falloff {
            min: 100.0,
            max: 500.0,
            curve: [1.0, 0.5, 0.2, 0.05, 0.0],
        };
        let mut out = [0.0; 80];
        m.mix(&mut out, 2);
        // Frame 10 holds 0.010; ahead of the listener (no pan), 20% of it.
        assert!(
            (out[2 * 10] - 0.002 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "{}",
            out[20]
        );
    }

    #[test]
    fn pitch_plays_a_clip_faster_and_shorter() {
        let mut out = [0.0; 80];
        let mut normal = mixer(1.0);
        normal.mix(&mut out, 2);
        // Left channel of frame f is the clip's frame f.
        assert!((out[2 * 10] - 0.010).abs() < 1e-6);
        assert_eq!(normal.voices.len(), 1);

        let mut fast = mixer(2.0);
        fast.mix(&mut out, 2);
        assert!((out[2 * 10] - 0.020).abs() < 1e-6);
        // 100 frames at twice the speed end after 50 of the 40 asked for.
        assert_eq!(fast.voices.len(), 1);
        fast.mix(&mut out, 2);
        assert!(fast.voices.is_empty());

        let mut slow = mixer(0.5);
        slow.mix(&mut out, 2);
        assert!((out[2 * 10] - 0.005).abs() < 1e-6);
    }
}
