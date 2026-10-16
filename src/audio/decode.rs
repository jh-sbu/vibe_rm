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
                let s = |c: usize| {
                    i16::from_le_bytes([data[(f * ch + c) * 2], data[(f * ch + c) * 2 + 1]]) as f32
                        / 32768.0
                };
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
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            "pipe:0",
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
        ])
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
    let samples = out
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
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

    fn pcm_wav(channels: u16, bits: u16, samples: &[u8]) -> Vec<u8> {
        let rate = 44100u32;
        let align = channels * bits / 8;
        let mut bytes = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * u32::from(align)).to_le_bytes());
        bytes.extend(align.to_le_bytes());
        bytes.extend(bits.to_le_bytes());
        // An odd-sized unknown chunk tests RIFF padding as well as sample decoding.
        bytes.extend(b"JUNK\x01\0\0\0\x42\0data");
        bytes.extend((samples.len() as u32).to_le_bytes());
        bytes.extend(samples);
        if samples.len() % 2 != 0 {
            bytes.push(0);
        }
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn decode_synthetic_pcm_wavs() {
        for (channels, bits, data, expected) in [
            (
                1,
                8,
                vec![0, 128, 192],
                vec![-1.0, -1.0, 0.0, 0.0, 0.5, 0.5],
            ),
            (2, 8, vec![0, 192, 128, 64], vec![-1.0, 0.5, 0.0, -0.5]),
            (
                1,
                16,
                vec![0, 128, 0, 0, 0, 64],
                vec![-1.0, -1.0, 0.0, 0.0, 0.5, 0.5],
            ),
            (
                2,
                16,
                vec![0, 128, 0, 64, 0, 0, 0, 192],
                vec![-1.0, 0.5, 0.0, -0.5],
            ),
        ] {
            let clip = decode("synthetic.wav", &pcm_wav(channels, bits, &data), 22050)
                .expect("synthetic PCM decodes without FFmpeg");
            assert_eq!(clip.samples, expected);
            assert_eq!(clip.rate, 44100);
            assert_eq!(clip.frames(), expected.len() / 2);
            assert_eq!(clip.duration(), clip.frames() as f32 / 44100.0);
        }
    }

    fn synthetic_xwma() -> Vec<u8> {
        let pcm: Vec<u8> = (0..11025)
            .flat_map(|i| {
                let angle = i as f32 * std::f32::consts::TAU * 440.0 / 44100.0;
                ((angle.sin() * 10000.0) as i16).to_le_bytes()
            })
            .collect();
        let mut child = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "s16le",
                "-ar",
                "44100",
                "-ac",
                "1",
                "-i",
                "pipe:0",
                "-c:a",
                "wmav2",
                "-b:a",
                "48k",
                "-f",
                "wav",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("FFmpeg is required for the synthetic compressed-audio test");
        let mut stdin = child.stdin.take().unwrap();
        let writer = std::thread::spawn(move || stdin.write_all(&pcm));
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut bytes = output.stdout;
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        // FFmpeg has a WMA WAV muxer, but no xWMA muxer. xWMA uses the same
        // fmt/data chunks under RIFF/XWMA. Keep the generated codec extradata;
        // this tests our decoding paths, not vanilla encoder compatibility.
        // Format reference: https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/xwma.c
        bytes[8..12].copy_from_slice(b"XWMA");
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        let mut p = 12;
        loop {
            assert!(p + 8 <= bytes.len(), "encoded WAV needs a data chunk");
            let size = u32_at(&bytes, p + 4) as usize;
            if &bytes[p..p + 4] == b"data" {
                // Pipe output uses an unknown chunk length; our in-memory
                // fixture has a known end and can supply the actual size.
                let size = (bytes.len() - p - 8) as u32;
                bytes[p + 4..p + 8].copy_from_slice(&size.to_le_bytes());
                break;
            }
            p += 8 + size + (size & 1);
        }
        bytes
    }

    #[test]
    fn decode_synthetic_xwma_and_fuz() {
        let audio = synthetic_xwma();
        let clip = decode("synthetic.xwm", &audio, 22050).expect("xWMA decodes via FFmpeg");
        assert_eq!(clip.rate, 22050);
        assert!(clip.duration() > 0.1 && clip.duration() < 0.5);
        assert!(clip.samples.iter().all(|s| s.is_finite()));
        assert!(clip.samples.iter().any(|s| s.abs() > 0.01));
        assert!(clip.samples.chunks_exact(2).all(|s| s[0] == s[1]));

        let lip = b"synthetic lip data";
        let mut fuz = b"FUZE".to_vec();
        fuz.extend(1u32.to_le_bytes());
        fuz.extend((lip.len() as u32).to_le_bytes());
        fuz.extend(lip);
        fuz.extend(&audio);
        assert_eq!(fuz_audio(&fuz), Some(audio.as_slice()));
        let wrapped = decode("synthetic.fuz", &fuz, 22050).expect("FUZ payload decodes");
        assert_eq!(wrapped.rate, clip.rate);
        assert_eq!(wrapped.samples, clip.samples);
        assert!(fuz_audio(b"FUZE").is_none());
        assert!(fuz_audio(b"NOPE\x01\0\0\0\0\0\0\0").is_none());
        assert!(fuz_audio(b"FUZE\x01\0\0\0\xff\xff\xff\xff").is_none());
    }
}
