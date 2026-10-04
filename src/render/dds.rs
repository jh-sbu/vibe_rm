//! Minimal DDS container parsing, producing data ready for GPU upload.

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc6h,
    Bc7,
    Rgba8,
    Bgra8,
}

impl Format {
    /// (block width/height in texels, bytes per block)
    pub fn block(self) -> (u32, u32) {
        match self {
            Format::Bc1 | Format::Bc4 => (4, 8),
            Format::Bc2 | Format::Bc3 | Format::Bc5 | Format::Bc6h | Format::Bc7 => (4, 16),
            Format::Rgba8 | Format::Bgra8 => (1, 4),
        }
    }
    pub fn wgpu(self) -> wgpu::TextureFormat {
        use wgpu::TextureFormat as F;
        match self {
            Format::Bc1 => F::Bc1RgbaUnorm,
            Format::Bc2 => F::Bc2RgbaUnorm,
            Format::Bc3 => F::Bc3RgbaUnorm,
            Format::Bc4 => F::Bc4RUnorm,
            Format::Bc5 => F::Bc5RgUnorm,
            Format::Bc6h => F::Bc6hRgbUfloat,
            Format::Bc7 => F::Bc7RgbaUnorm,
            Format::Rgba8 => F::Rgba8Unorm,
            Format::Bgra8 => F::Bgra8Unorm,
        }
    }
}

pub struct Dds {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    pub layers: u32,
    pub cube: bool,
    pub format: Format,
    /// Tightly packed data: for each layer, each mip.
    pub data: Vec<u8>,
}

impl Dds {
    pub fn mip_size(&self, level: u32) -> (u32, u32) {
        ((self.width >> level).max(1), (self.height >> level).max(1))
    }
    pub fn mip_bytes(&self, level: u32) -> usize {
        let (w, h) = self.mip_size(level);
        let (b, bytes) = self.format.block();
        (w.div_ceil(b) * h.div_ceil(b) * bytes) as usize
    }
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

pub fn parse(d: &[u8]) -> Result<Dds> {
    if d.len() < 128 || &d[0..4] != b"DDS " {
        bail!("not a DDS file");
    }
    let height = u32_at(d, 12);
    let width = u32_at(d, 16);
    let mips = u32_at(d, 28).max(1);
    let pf_flags = u32_at(d, 80);
    let fourcc = &d[84..88];
    let bit_count = u32_at(d, 88);
    let rmask = u32_at(d, 92);
    let amask = u32_at(d, 104);
    let caps2 = u32_at(d, 112);
    let mut cube = caps2 & 0x200 != 0;
    let mut layers = if cube { 6 } else { 1 };
    let mut offset = 128;
    let mut convert = Conv::None;
    let format = if pf_flags & 0x4 != 0 {
        match fourcc {
            b"DXT1" => Format::Bc1,
            b"DXT2" | b"DXT3" => Format::Bc2,
            b"DXT4" | b"DXT5" => Format::Bc3,
            b"ATI1" | b"BC4U" => Format::Bc4,
            b"ATI2" | b"BC5U" => Format::Bc5,
            b"DX10" => {
                offset += 20;
                let dxgi = u32_at(d, 128);
                if u32_at(d, 136) & 0x4 != 0 {
                    cube = true;
                }
                layers = u32_at(d, 140).max(1) * if cube { 6 } else { 1 };
                match dxgi {
                    70..=72 => Format::Bc1,
                    73..=75 => Format::Bc2,
                    76..=78 => Format::Bc3,
                    79..=81 => Format::Bc4,
                    82..=84 => Format::Bc5,
                    94..=96 => Format::Bc6h,
                    97..=99 => Format::Bc7,
                    27..=29 => Format::Rgba8,
                    87 | 90 | 91 => Format::Bgra8,
                    88 | 92 | 93 => {
                        convert = Conv::Bgrx;
                        Format::Rgba8
                    }
                    61 => {
                        convert = Conv::R8;
                        Format::Rgba8
                    }
                    other => bail!("unsupported DXGI format {other}"),
                }
            }
            other => bail!("unsupported fourcc {:?}", String::from_utf8_lossy(other)),
        }
    } else if bit_count == 32 {
        if rmask == 0x00FF_0000 {
            if amask == 0 {
                convert = Conv::Bgrx;
                Format::Rgba8
            } else {
                Format::Bgra8
            }
        } else {
            Format::Rgba8
        }
    } else if bit_count == 24 {
        convert = if rmask == 0x00FF_0000 { Conv::Bgr } else { Conv::Rgb };
        Format::Rgba8
    } else if bit_count == 8 {
        convert = Conv::R8;
        Format::Rgba8
    } else {
        bail!("unsupported uncompressed DDS ({bit_count} bpp, flags {pf_flags:#x})");
    };

    let mut dds = Dds { width, height, mips, layers, cube, format, data: Vec::new() };
    let src_bpp = |level: u32| -> usize {
        let (w, h) = ((width >> level).max(1), (height >> level).max(1));
        match convert.src_bytes_per_pixel() {
            Some(bpp) => (w * h) as usize * bpp,
            None => {
                let (b, bytes) = format.block();
                (w.div_ceil(b) * h.div_ceil(b) * bytes) as usize
            }
        }
    };
    let mut total = 0;
    for _ in 0..layers {
        for m in 0..mips {
            total += src_bpp(m);
        }
    }
    if d.len() < offset + total {
        // Some textures declare more mips than they contain; trim.
        let mut avail = d.len() - offset.min(d.len());
        let mut ok_mips = 0;
        let per_layer: Vec<usize> = (0..mips).map(src_bpp).collect();
        let layer_total = |n: u32| per_layer[..n as usize].iter().sum::<usize>() * layers as usize;
        for m in 1..=mips {
            if layer_total(m) <= avail {
                ok_mips = m;
            }
        }
        if ok_mips == 0 || layers > 1 {
            bail!("truncated DDS");
        }
        dds.mips = ok_mips;
        avail = layer_total(ok_mips);
        total = avail;
    }
    let src = &d[offset..offset + total];
    dds.data = match convert {
        Conv::None => src.to_vec(),
        conv => {
            let (w, h) = (width as usize, height as usize);
            let mut out = Vec::new();
            let mut p = 0;
            for _ in 0..layers {
                for m in 0..dds.mips {
                    let n = ((w >> m).max(1)) * ((h >> m).max(1));
                    let bytes = src_bpp(m);
                    out.extend(conv.apply(&src[p..p + bytes], n));
                    p += bytes;
                }
            }
            out
        }
    };
    Ok(dds)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Conv {
    None,
    Bgrx,
    Bgr,
    Rgb,
    R8,
}

impl Conv {
    fn src_bytes_per_pixel(self) -> Option<usize> {
        match self {
            Conv::None => None,
            Conv::Bgrx => Some(4),
            Conv::Bgr | Conv::Rgb => Some(3),
            Conv::R8 => Some(1),
        }
    }
    fn apply(self, s: &[u8], n: usize) -> Vec<u8> {
        match self {
            Conv::None => s.to_vec(),
            Conv::Bgrx => bgrx_to_rgba(s, n),
            Conv::Bgr => bgr_to_rgba(s, n),
            Conv::Rgb => rgb_to_rgba(s, n),
            Conv::R8 => r8_to_rgba(s, n),
        }
    }
}

fn bgrx_to_rgba(s: &[u8], n: usize) -> Vec<u8> {
    let mut o = Vec::with_capacity(n * 4);
    for p in s.chunks_exact(4) {
        o.extend_from_slice(&[p[2], p[1], p[0], 255]);
    }
    o
}
fn bgr_to_rgba(s: &[u8], n: usize) -> Vec<u8> {
    let mut o = Vec::with_capacity(n * 4);
    for p in s.chunks_exact(3) {
        o.extend_from_slice(&[p[2], p[1], p[0], 255]);
    }
    o
}
fn rgb_to_rgba(s: &[u8], n: usize) -> Vec<u8> {
    let mut o = Vec::with_capacity(n * 4);
    for p in s.chunks_exact(3) {
        o.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    o
}
fn r8_to_rgba(s: &[u8], n: usize) -> Vec<u8> {
    let mut o = Vec::with_capacity(n * 4);
    for &v in &s[..n.min(s.len())] {
        o.extend_from_slice(&[v, v, v, 255]);
    }
    o
}
