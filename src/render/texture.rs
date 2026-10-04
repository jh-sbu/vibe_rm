//! GPU textures and a path-keyed cache.

use std::collections::HashMap;
use std::sync::Arc;

use super::dds;

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

pub fn upload_dds(device: &wgpu::Device, queue: &wgpu::Queue, d: &dds::Dds, label: &str) -> GpuTexture {
    let (bw, _) = d.format.block();
    // Block-compressed textures must have block-aligned base dimensions;
    // drop mips that would violate that by skipping leading levels if needed.
    let mut first = 0;
    while bw > 1 && (d.mip_size(first).0 % bw != 0 || d.mip_size(first).1 % bw != 0) && first + 1 < d.mips {
        first += 1;
    }
    let (w, h) = d.mip_size(first);
    let mut mips = d.mips - first;
    // Mips smaller than a block are fine for wgpu, but each mip must exist.
    if bw > 1 {
        // wgpu requires every mip's *physical* size to be valid, which always holds.
        mips = mips.max(1);
    }
    let layers = d.layers;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: layers },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: d.format.wgpu(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let (b, bytes) = d.format.block();
    let mut offset = 0usize;
    for layer in 0..layers {
        for level in 0..d.mips {
            let size = d.mip_bytes(level);
            if level >= first {
                let (mw, mh) = d.mip_size(level);
                let data = &d.data[offset..offset + size];
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level - first,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(mw.div_ceil(b) * bytes),
                        rows_per_image: Some(mh.div_ceil(b)),
                    },
                    wgpu::Extent3d {
                        width: mw.div_ceil(b) * b,
                        height: mh.div_ceil(b) * b,
                        depth_or_array_layers: 1,
                    },
                );
            }
            offset += size;
        }
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(if d.cube && layers == 6 {
            wgpu::TextureViewDimension::Cube
        } else if layers > 1 {
            wgpu::TextureViewDimension::D2Array
        } else {
            wgpu::TextureViewDimension::D2
        }),
        ..Default::default()
    });
    GpuTexture { texture, view }
}

pub fn solid(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4], label: &str) -> GpuTexture {
    let d = dds::Dds {
        width: 1,
        height: 1,
        mips: 1,
        layers: 1,
        cube: false,
        format: dds::Format::Rgba8,
        data: rgba.to_vec(),
    };
    upload_dds(device, queue, &d, label)
}

#[derive(Default)]
pub struct TextureCache {
    map: HashMap<String, Option<Arc<GpuTexture>>>,
}

impl TextureCache {
    pub fn get(&self, path: &str) -> Option<Option<Arc<GpuTexture>>> {
        self.map.get(path).cloned()
    }
    pub fn contains(&self, path: &str) -> bool {
        self.map.contains_key(path)
    }
    pub fn insert(&mut self, path: String, t: Option<Arc<GpuTexture>>) {
        self.map.insert(path, t);
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
