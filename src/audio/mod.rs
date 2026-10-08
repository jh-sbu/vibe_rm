//! Audio output: a small mixer with 3D positioned voices on top of cpal.

pub mod decode;

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

struct Voice {
    id: VoiceId,
    clip: Arc<Clip>,
    pos: f64,
    volume: f32,
    looping: bool,
    /// World position for 3D sounds.
    at: Option<Vec3>,
    min_dist: f32,
    max_dist: f32,
}

struct Mixer {
    voices: Vec<Voice>,
    listener: Vec3,
    listener_right: Vec3,
    rate: u32,
    master: f32,
}

impl Mixer {
    fn mix(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let frames = out.len() / channels;
        let listener = self.listener;
        let right = self.listener_right;
        for v in &mut self.voices {
            let (mut gl, mut gr) = (v.volume, v.volume);
            if let Some(p) = v.at {
                let d = p.distance(listener);
                let att = if d <= v.min_dist {
                    1.0
                } else {
                    (1.0 - (d - v.min_dist) / (v.max_dist - v.min_dist).max(1.0))
                        .clamp(0.0, 1.0)
                        .powi(2)
                };
                let pan = ((p - listener).normalize_or_zero().dot(right)).clamp(-1.0, 1.0);
                gl *= att * (1.0 - pan.max(0.0) * 0.6);
                gr *= att * (1.0 + pan.min(0.0) * 0.6);
            }
            let step = v.clip.rate as f64 / self.rate as f64;
            let n = v.clip.frames();
            if n == 0 {
                v.pos = f64::MAX;
                continue;
            }
            for f in 0..frames {
                let i = v.pos as usize;
                if i >= n {
                    if v.looping {
                        v.pos -= n as f64;
                        continue;
                    }
                    break;
                }
                let (l, r) = (v.clip.samples[i * 2], v.clip.samples[i * 2 + 1]);
                out[f * channels] += l * gl;
                if channels > 1 {
                    out[f * channels + 1] += r * gr;
                }
                v.pos += step;
            }
        }
        self.voices
            .retain(|v| v.looping || (v.pos as usize) < v.clip.frames());
        for s in out.iter_mut() {
            *s = (*s * self.master).clamp(-1.0, 1.0);
        }
    }
}

enum Pending {
    Play {
        id: VoiceId,
        volume: f32,
        looping: bool,
        at: Option<Vec3>,
        min_dist: f32,
        max_dist: f32,
    },
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
            master: 0.8,
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
            m.master = v;
        }
    }

    /// Start playing a file from the VFS. Decoding happens off-thread; playback starts once ready.
    #[allow(clippy::too_many_arguments)]
    pub fn play(
        &mut self,
        vfs: &vfs::Vfs,
        path: &str,
        volume: f32,
        looping: bool,
        at: Option<Vec3>,
        min_dist: f32,
        max_dist: f32,
    ) -> Option<VoiceId> {
        let key = vfs::normalize_path(path);
        let id = VoiceId(self.next_id);
        self.next_id += 1;
        let p = Pending::Play {
            id,
            volume,
            looping,
            at,
            min_dist,
            max_dist,
        };
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
        let Pending::Play {
            id,
            volume,
            looping,
            at,
            min_dist,
            max_dist,
        } = p;
        if let Ok(mut m) = self.mixer.lock() {
            m.voices.push(Voice {
                id,
                clip,
                pos: 0.0,
                volume,
                looping,
                at,
                min_dist,
                max_dist,
            });
        }
    }

    pub fn stop(&self, id: VoiceId) {
        if let Ok(mut m) = self.mixer.lock() {
            m.voices.retain(|v| v.id != id);
        }
    }

    pub fn stop_all(&mut self) {
        self.waiting.clear();
        if let Ok(mut m) = self.mixer.lock() {
            m.voices.clear();
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
