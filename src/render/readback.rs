//! Strip GPU row padding and convert screenshots to opaque RGBA8.

pub(super) fn rgba8(
    data: &[u8],
    size: (u32, u32),
    bytes_per_row: u32,
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    use wgpu::TextureFormat as F;
    let width = size.0 as usize;
    let mut out = vec![0; width * size.1 as usize * 4];
    let unorm = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    for (row, rgba) in data
        .chunks_exact(bytes_per_row as usize)
        .zip(out.chunks_exact_mut(width * 4))
    {
        // Select the conversion once per row, not once per pixel. The common
        // 8-bit formats can copy the entire row before fixing alpha/channels.
        match format {
            F::Rgba8Unorm | F::Rgba8UnormSrgb => {
                rgba.copy_from_slice(&row[..width * 4]);
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel[3] = 255;
                }
            }
            F::Bgra8Unorm | F::Bgra8UnormSrgb => {
                rgba.copy_from_slice(&row[..width * 4]);
                for pixel in rgba.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
            }
            F::Rgb10a2Unorm => {
                for (src, dst) in row.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
                    let v = u32::from_le_bytes(src.try_into().unwrap());
                    let c = |shift: u32| ((v >> shift & 0x3ff) >> 2) as u8;
                    dst.copy_from_slice(&[c(0), c(10), c(20), 255]);
                }
            }
            F::Rgba16Float => {
                for (src, dst) in row.chunks_exact(8).zip(rgba.chunks_exact_mut(4)) {
                    let c = |i: usize| {
                        unorm(f16_to_f32(u16::from_le_bytes([src[i * 2], src[i * 2 + 1]])))
                    };
                    dst.copy_from_slice(&[c(0), c(1), c(2), 255]);
                }
            }
            _ => {
                let bpp = format.block_copy_size(None).unwrap_or(4) as usize;
                for (src, dst) in row.chunks_exact(bpp).zip(rgba.chunks_exact_mut(4)) {
                    dst.copy_from_slice(&[src[0], src[1], src[2], 255]);
                }
            }
        }
    }
    out
}

fn f16_to_f32(h: u16) -> f32 {
    let (sign, exp, man) = (
        (h >> 15) as u32,
        (h >> 10 & 0x1f) as i32,
        (h & 0x3ff) as f32,
    );
    let mag = match exp {
        0 => man * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    };
    if sign == 1 { -mag } else { mag }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::TextureFormat as F;

    fn padded_rows(format: F, first: &[u8], second: &[u8], expected: &[u8]) {
        let mut bytes = vec![0xdd; 512];
        bytes[..first.len()].copy_from_slice(first);
        bytes[256..256 + second.len()].copy_from_slice(second);
        assert_eq!(rgba8(&bytes, (2, 2), 256, format), expected);
    }

    #[test]
    fn byte_formats_strip_padding_swap_bgra_and_force_opaque_alpha() {
        for format in [F::Rgba8Unorm, F::Rgba8UnormSrgb] {
            padded_rows(
                format,
                &[1, 2, 3, 0, 4, 5, 6, 17],
                &[7, 8, 9, 255, 10, 11, 12, 128],
                &[1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
            );
        }
        for format in [F::Bgra8Unorm, F::Bgra8UnormSrgb] {
            padded_rows(
                format,
                &[3, 2, 1, 0, 6, 5, 4, 17],
                &[9, 8, 7, 255, 12, 11, 10, 128],
                &[1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
            );
        }
    }

    #[test]
    fn wide_formats_preserve_channel_conversion_and_opaque_alpha() {
        let pixel = |r: u32, g: u32, b: u32| (r | g << 10 | b << 20).to_le_bytes();
        let row = [pixel(1023, 512, 3), pixel(0, 4, 1020)].concat();
        padded_rows(
            F::Rgb10a2Unorm,
            &row,
            &row,
            &[255, 128, 0, 255, 0, 1, 255, 255].repeat(2),
        );
        let row: Vec<_> = [0x3c00u16, 0x3800, 0xbc00, 0, 0x4000, 1, 0x7c00, 0x3c00]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        padded_rows(
            F::Rgba16Float,
            &row,
            &row,
            &[255, 128, 0, 255, 255, 0, 255, 255].repeat(2),
        );
    }
}
