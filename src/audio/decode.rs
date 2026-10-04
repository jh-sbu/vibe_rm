//! Sound file decoding to interleaved stereo f32.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

pub struct Clip {
    /// Interleaved stereo samples.
    pub samples: Vec<f32>,
    pub rate: u32,
}

impl Clip {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
    pub fn duration(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }
}

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

/// Decode a RIFF WAVE file containing PCM (8/16-bit, mono/stereo).
pub fn wav(d: &[u8]) -> Option<Clip> {
    if d.len() < 12 || &d[0..4] != b"RIFF" || &d[8..12] != b"WAVE" {
        return None;
    }
    let mut p = 12;
    let (mut fmt, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut data: Option<&[u8]> = None;
    while p + 8 <= d.len() {
        let id = &d[p..p + 4];
        let size = u32_at(d, p + 4) as usize;
        let body = &d[p + 8..(p + 8 + size).min(d.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                fmt = u16_at(body, 0);
                channels = u16_at(body, 2);
                rate = u32_at(body, 4);
                bits = u16_at(body, 14);
            }
            b"data" => data = Some(body),
            _ => {}
        }
        p += 8 + size + (size & 1);
    }
    let data = data?;
    if fmt != 1 || channels == 0 {
        return None; // compressed WAV: let the general decoder handle it
    }
    let ch = channels as usize;
    let mut samples = Vec::new();
    match bits {
        16 => {
            let frames = data.len() / (2 * ch);
            samples.reserve(frames * 2);
            for f in 0..frames {
                let s = |c: usize| i16::from_le_bytes([data[(f * ch + c) * 2], data[(f * ch + c) * 2 + 1]]) as f32 / 32768.0;
                let (l, r) = if ch == 1 { (s(0), s(0)) } else { (s(0), s(1)) };
                samples.push(l);
                samples.push(r);
            }
        }
        8 => {
            let frames = data.len() / ch;
            for f in 0..frames {
                let s = |c: usize| (data[f * ch + c] as f32 - 128.0) / 128.0;
                let (l, r) = if ch == 1 { (s(0), s(0)) } else { (s(0), s(1)) };
                samples.push(l);
                samples.push(r);
            }
        }
        _ => return None,
    }
    Some(Clip { samples, rate })
}

/// Strip the FUZ (lip sync + xWMA) container, returning the audio payload.
pub fn fuz_audio(d: &[u8]) -> Option<&[u8]> {
    if d.len() < 12 || &d[0..4] != b"FUZE" {
        return None;
    }
    let lip = u32_at(d, 8) as usize;
    d.get(12 + lip..)
}

/// Decode anything FFmpeg understands (xWMA, ADPCM WAV, ...) via the `ffmpeg` executable.
pub fn ffmpeg(d: &[u8], rate: u32) -> Option<Clip> {
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i", "pipe:0", "-f", "f32le", "-ac", "2", "-ar"])
        .arg(rate.to_string())
        .arg("pipe:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = d.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let mut out = Vec::new();
    child.stdout.take()?.read_to_end(&mut out).ok()?;
    let _ = writer.join();
    let _ = child.wait();
    if out.is_empty() {
        return None;
    }
    let samples = out.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    Some(Clip { samples, rate })
}

/// Decode a sound file by path/extension.
pub fn decode(path: &str, d: &[u8], rate: u32) -> Option<Clip> {
    if path.ends_with(".fuz") {
        return ffmpeg(fuz_audio(d)?, rate);
    }
    if path.ends_with(".wav")
        && let Some(c) = wav(d)
    {
        return Some(c);
    }
    ffmpeg(d, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_vfs() -> Option<vfs::Vfs> {
        let dir = vfs::locate_data_dir()?;
        let names = esp::LoadOrder::default_plugin_list(&dir, None);
        Some(vfs::Vfs::new(dir, &names))
    }

    #[test]
    fn decode_game_sounds() {
        let Some(v) = game_vfs() else {
            eprintln!("no game data; skipping");
            return;
        };
        let wav_path = v.list("sound/fx/").into_iter().find(|p| p.ends_with(".wav")).expect("a wav");
        let w = decode(&wav_path, &v.read(&wav_path).unwrap(), 44100).expect("wav decodes");
        assert!(w.frames() > 0);
        let xwm_path = v.list("music/").into_iter().find(|p| p.ends_with(".xwm")).expect("an xwm");
        let x = decode(&xwm_path, &v.read(&xwm_path).unwrap(), 44100).expect("xwm decodes via ffmpeg");
        assert!(x.duration() > 1.0, "{xwm_path}: {}s", x.duration());
        let fuz_path = v.list("sound/voice/skyrim.esm/").into_iter().find(|p| p.ends_with(".fuz")).expect("a fuz");
        let f = decode(&fuz_path, &v.read(&fuz_path).unwrap(), 44100).expect("fuz decodes");
        assert!(f.duration() > 0.1);
        eprintln!("{wav_path}: {:.2}s, {xwm_path}: {:.2}s, {fuz_path}: {:.2}s", w.duration(), x.duration(), f.duration());
    }
}
