//! Header-only clip duration. No codec invocation or decoded sample allocation.
//! Field references and vanilla observations: research/audio/duration-metadata.md.
fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        d.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        d.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub fn duration(path: &str, bytes: &[u8]) -> Option<f32> {
    let d = if path.to_ascii_lowercase().ends_with(".fuz") || bytes.starts_with(b"FUZE") {
        super::decode::fuz_audio(bytes)?
    } else {
        bytes
    };
    if d.get(..4) != Some(b"RIFF") || !matches!(d.get(8..12)?, b"WAVE" | b"XWMA") {
        return None;
    }
    let end = (u32_at(d, 4)? as usize).checked_add(8)?;
    if end < 12 || end > d.len() {
        return None;
    }
    let mut at = 12usize;
    let (mut fmt, mut data_size, mut decoded_bytes) = (None, None, None);
    while at < end {
        let tag = d.get(at..at.checked_add(4)?)?;
        let size = u32_at(d, at.checked_add(4)?)? as usize;
        let start = at.checked_add(8)?;
        let next = start.checked_add(size)?;
        if next > end {
            return None;
        }
        let chunk = d.get(start..next)?;
        match tag {
            b"fmt " => fmt = Some(chunk),
            b"data" => data_size = Some(size as u64),
            b"dpds" => {
                if size == 0 || size % 4 != 0 {
                    return None;
                }
                let mut previous = 0u32;
                for word in chunk.chunks_exact(4) {
                    let value = u32::from_le_bytes(word.try_into().ok()?);
                    if value < previous {
                        return None;
                    }
                    previous = value;
                }
                decoded_bytes = Some(previous as u64);
            }
            _ => {}
        }
        at = next.checked_add(size & 1)?;
        if at > end {
            return None;
        }
    }
    let fmt = fmt?;
    let format = u16_at(fmt, 0)?;
    let channels = u16_at(fmt, 2)? as u64;
    let rate = u32_at(fmt, 4)? as u64;
    let alignment = u16_at(fmt, 12)? as u64;
    let bits = u16_at(fmt, 14)? as u64;
    let size = data_size?;
    if channels == 0 || rate == 0 || bits == 0 || bits % 8 != 0 {
        return None;
    }
    let frame_bytes = channels.checked_mul(bits / 8)?;
    let frames = match format {
        // Integer PCM / IEEE float. Compressed block alignment is not a frame size.
        1 | 3 => {
            if alignment != frame_bytes || size % alignment != 0 {
                return None;
            }
            size / alignment
        }
        // WMAudio2 / WMAudio3: dpds contains cumulative DECODED bytes.
        0x161 | 0x162 => {
            let bytes = decoded_bytes?;
            if bytes % frame_bytes != 0 {
                return None;
            }
            bytes / frame_bytes
        }
        _ => return None,
    };
    let seconds = frames as f64 / rate as f64;
    (seconds.is_finite() && seconds <= f32::MAX as f64).then_some(seconds as f32)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn riff(kind: &[u8; 4], fields: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut d = b"RIFF\0\0\0\0".to_vec();
        d.extend(kind);
        for (tag, bytes) in fields {
            d.extend(*tag);
            d.extend((bytes.len() as u32).to_le_bytes());
            d.extend(bytes);
            if bytes.len() % 2 != 0 {
                d.push(0);
            }
        }
        let size = (d.len() - 8) as u32;
        d[4..8].copy_from_slice(&size.to_le_bytes());
        d
    }
    fn format(tag: u16, channels: u16, rate: u32, alignment: u16, bits: u16) -> Vec<u8> {
        let mut d = tag.to_le_bytes().to_vec();
        d.extend(channels.to_le_bytes());
        d.extend(rate.to_le_bytes());
        d.extend(6000u32.to_le_bytes());
        d.extend(alignment.to_le_bytes());
        d.extend(bits.to_le_bytes());
        d
    }
    #[test]
    fn pcm_and_float_duration_uses_frames_not_bitrate() {
        for (tag, channels, bits) in [(1, 1, 8), (1, 2, 16), (1, 2, 24), (3, 2, 32)] {
            let align = channels * bits / 8;
            let d = riff(
                b"WAVE",
                &[
                    (b"JUNK", vec![42]),
                    (b"data", vec![0; 8000 * align as usize]),
                    (b"fmt ", format(tag, channels, 8000, align, bits)),
                ],
            );
            assert_eq!(duration("clip.wav", &d), Some(1.0));
        }
    }
    #[test]
    fn xwm_and_fuz_duration_needs_only_the_packet_table() {
        let table = [176400u32, 352800]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        // Deliberately not valid compressed audio: only header/table bytes are examined.
        let d = riff(
            b"XWMA",
            &[
                (b"fmt ", format(0x161, 2, 44100, 2230, 16)),
                (b"dpds", table),
                (b"data", vec![0xff; 4460]),
            ],
        );
        assert_eq!(duration("clip.xwm", &d), Some(2.0));
        let mut fuz = b"FUZE".to_vec();
        fuz.extend(1u32.to_le_bytes());
        fuz.extend(3u32.to_le_bytes());
        fuz.extend(b"lip");
        fuz.extend(&d);
        assert_eq!(duration("clip.fuz", &fuz), Some(2.0));
        for end in 0..d.len() {
            assert_eq!(duration("clip.xwm", &d[..end]), None);
        }
    }
    #[test]
    fn missing_or_malformed_metadata_does_not_guess_a_length() {
        assert_eq!(
            duration(
                "bad.xwm",
                &riff(
                    b"XWMA",
                    &[
                        (b"fmt ", format(0x161, 2, 44100, 2230, 16)),
                        (b"data", vec![0; 2230])
                    ]
                )
            ),
            None
        );
        for table in [
            vec![],
            vec![1, 2, 3],
            [200u32, 100]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        ] {
            assert_eq!(
                duration(
                    "bad.xwm",
                    &riff(
                        b"XWMA",
                        &[
                            (b"fmt ", format(0x161, 2, 44100, 2230, 16)),
                            (b"dpds", table),
                            (b"data", vec![0; 2230])
                        ]
                    )
                ),
                None
            );
        }
        for (channels, rate, align, bits) in [
            (0, 8000, 2, 16),
            (1, 0, 2, 16),
            (1, 8000, 0, 16),
            (1, 8000, 2, 0),
        ] {
            assert_eq!(
                duration(
                    "bad.wav",
                    &riff(
                        b"WAVE",
                        &[
                            (b"fmt ", format(1, channels, rate, align, bits)),
                            (b"data", vec![0; 4])
                        ]
                    )
                ),
                None
            );
        }
        assert_eq!(duration("bad.fuz", b"FUZE"), None);
    }
}
