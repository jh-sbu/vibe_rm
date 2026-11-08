//! The face's tint texture, composited from the race's tint masks by the
//! actor's tint layers, as the pre-built `facetint` textures are
//! (`research/facegen.md`, "Tints"): the skin tone (`QNAM`) everywhere,
//! then each layer other than the skin tone's, in order, blended toward its
//! colour by mask × value.

use crate::{TintLayer, TintMask};

/// An RGB image, 0..1, row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<[f32; 3]>,
}

impl Image {
    /// Nearest sample at texture coordinates in 0..1.
    fn sample(&self, u: f32, v: f32) -> [f32; 3] {
        let x = ((u * self.width as f32) as usize).min(self.width - 1);
        let y = ((v * self.height as f32) as usize).min(self.height - 1);
        self.rgb[y * self.width + x]
    }

    /// RGBA8, opaque.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.rgb
            .iter()
            .flat_map(|c| {
                let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                [b(c[0]), b(c[1]), b(c[2]), 255]
            })
            .collect()
    }
}

/// The tint mask type of the skin tone (`TINP` 6).
pub const MASK_SKIN_TONE: u16 = 6;

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn rgb565(v: u16) -> [f32; 3] {
    [
        (v >> 11) as f32 / 31.0,
        ((v >> 5) & 63) as f32 / 63.0,
        (v & 31) as f32 / 31.0,
    ]
}

/// The top mip of a DDS texture as RGB: uncompressed (by its bit masks),
/// DXT1 or DXT5 (colour only). Other formats give `None`.
pub fn decode_dds(d: &[u8]) -> Option<Image> {
    if d.get(..4) != Some(b"DDS ") {
        return None;
    }
    let height = u32_at(d, 12)? as usize;
    let width = u32_at(d, 16)? as usize;
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return None;
    }
    let fourcc = d.get(84..88)?;
    let mut rgb = vec![[0.0; 3]; width * height];
    match fourcc {
        b"\0\0\0\0" => {
            let bits = u32_at(d, 88)? as usize;
            let bpp = bits / 8;
            if !(3..=4).contains(&bpp) {
                return None;
            }
            let masks = [u32_at(d, 92)?, u32_at(d, 96)?, u32_at(d, 100)?];
            let px = d.get(128..128 + width * height * bpp)?;
            for (i, p) in px.chunks_exact(bpp).enumerate() {
                let mut v = [0u8; 4];
                v[..bpp].copy_from_slice(p);
                let v = u32::from_le_bytes(v);
                for (c, &m) in masks.iter().enumerate() {
                    if m != 0 {
                        rgb[i][c] = ((v & m) >> m.trailing_zeros()) as f32
                            / (m >> m.trailing_zeros()) as f32;
                    }
                }
            }
        }
        b"DXT1" | b"DXT5" => {
            let block = if fourcc == b"DXT1" { 8 } else { 16 };
            let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
            let data = d.get(128..128 + bw * bh * block)?;
            for (bi, b) in data.chunks_exact(block).enumerate() {
                let c = &b[block - 8..];
                let c0 = u16::from_le_bytes([c[0], c[1]]);
                let c1 = u16::from_le_bytes([c[2], c[3]]);
                let (p0, p1) = (rgb565(c0), rgb565(c1));
                let mix =
                    |a: f32, b: f32| -> [f32; 3] { std::array::from_fn(|k| p0[k] * a + p1[k] * b) };
                let pal = if c0 > c1 || block == 16 {
                    [p0, p1, mix(2.0 / 3.0, 1.0 / 3.0), mix(1.0 / 3.0, 2.0 / 3.0)]
                } else {
                    [p0, p1, mix(0.5, 0.5), [0.0; 3]]
                };
                let idx = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                let (bx, by) = (bi % bw, bi / bw);
                for i in 0..16 {
                    let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                    if x < width && y < height {
                        rgb[y * width + x] = pal[((idx >> (2 * i)) & 3) as usize];
                    }
                }
            }
        }
        _ => return None,
    }
    Some(Image { width, height, rgb })
}

/// Composite a face tint of `size` × `size`: the skin tone, then each layer
/// whose mask the race has (other than the skin tone's), blended toward its
/// colour by the mask's red channel × the layer's value. `mask` reads a
/// mask texture by path.
pub fn composite(
    size: usize,
    skin: [f32; 3],
    layers: &[TintLayer],
    masks: &[TintMask],
    mut mask: impl FnMut(&str) -> Option<std::sync::Arc<Image>>,
) -> Image {
    let mut out = Image {
        width: size,
        height: size,
        rgb: vec![skin; size * size],
    };
    for l in layers {
        let Some(m) = masks.iter().find(|m| m.index == l.index) else {
            continue;
        };
        if m.kind == MASK_SKIN_TONE || l.value <= 0.0 {
            continue;
        }
        let Some(img) = mask(&m.file) else { continue };
        let color = l.color.map(|c| c as f32 / 255.0);
        for y in 0..size {
            let v = (y as f32 + 0.5) / size as f32;
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32;
                let a = (img.sample(u, v)[0] * l.value).clamp(0.0, 1.0);
                let p = &mut out.rgb[y * size + x];
                for k in 0..3 {
                    p[k] += (color[k] - p[k]) * a;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dds_header(w: u32, h: u32, fourcc: &[u8; 4]) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[..4].copy_from_slice(b"DDS ");
        d[12..16].copy_from_slice(&h.to_le_bytes());
        d[16..20].copy_from_slice(&w.to_le_bytes());
        d[84..88].copy_from_slice(fourcc);
        d
    }

    #[test]
    fn decodes_bgr24_by_its_masks_and_dxt1_blocks() {
        let mut d = dds_header(1, 1, b"\0\0\0\0");
        d[88..92].copy_from_slice(&24u32.to_le_bytes());
        d[92..96].copy_from_slice(&0xff0000u32.to_le_bytes());
        d[96..100].copy_from_slice(&0xff00u32.to_le_bytes());
        d[100..104].copy_from_slice(&0xffu32.to_le_bytes());
        d.extend([0, 0, 255]); // B G R
        assert_eq!(decode_dds(&d).unwrap().rgb, vec![[1.0, 0.0, 0.0]]);

        let mut d = dds_header(4, 4, b"DXT1");
        d.extend(0xffffu16.to_le_bytes()); // white
        d.extend(0u16.to_le_bytes()); // black
        d.extend(0x5555_5555u32.to_le_bytes()); // every texel index 1
        let img = decode_dds(&d).unwrap();
        assert_eq!(img.rgb.len(), 16);
        assert!(img.rgb.iter().all(|p| *p == [0.0; 3]));
    }

    #[test]
    fn layers_blend_over_the_skin_tone_by_mask_and_value() {
        let masks = vec![
            TintMask {
                index: 1,
                file: "skin".into(),
                kind: MASK_SKIN_TONE,
                ..Default::default()
            },
            TintMask {
                index: 2,
                file: "half".into(),
                kind: 1,
                ..Default::default()
            },
        ];
        let layers = [
            TintLayer {
                index: 1,
                color: [0, 0, 0, 0],
                value: 1.0,
                preset: 0,
            },
            TintLayer {
                index: 2,
                color: [255, 0, 0, 0],
                value: 0.5,
                preset: 0,
            },
        ];
        let white = std::sync::Arc::new(Image {
            width: 1,
            height: 1,
            rgb: vec![[1.0; 3]],
        });
        let out = composite(2, [0.0, 0.0, 1.0], &layers, &masks, |_| Some(white.clone()));
        // The skin tone layer is the base, not blended again; the lip
        // layer goes halfway to red.
        assert_eq!(out.rgb[0], [0.5, 0.0, 0.5]);
    }
}
