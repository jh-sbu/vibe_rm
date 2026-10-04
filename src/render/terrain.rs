//! GPU landscape rendering.

use std::sync::Arc;

use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::world::terrain::{CELL_SIZE, Land, MAX_LAYERS, QUAD_VERTS, VERTS};

/// Texture repeats per cell.
const UV_REPEAT_PER_CELL: f32 = 6.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    uv: [f32; 2],
    w0: [f32; 4],
    w1: [f32; 4],
}

pub struct TerrainChunk {
    pub vbuf: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub center: Vec3,
    pub radius: f32,
}

pub struct TerrainPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bgl: wgpu::BindGroupLayout,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
}

impl TerrainPipeline {
    pub fn new(device: &wgpu::Device, frame_bgl: &wgpu::BindGroupLayout, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terrain.wgsl").into()),
        });
        let mut entries: Vec<wgpu::BindGroupLayoutEntry> = (0..16)
            .map(|i| wgpu::BindGroupLayoutEntry {
                binding: i,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 16,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("terrain"), entries: &entries });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain"),
            bind_group_layouts: &[Some(frame_bgl), Some(&bgl)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x2, 4 => Float32x4, 5 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TerrainVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let mut indices: Vec<u16> = Vec::new();
        let n = QUAD_VERTS as u16;
        for r in 0..n - 1 {
            for c in 0..n - 1 {
                let a = r * n + c;
                let b = a + 1;
                let cc = a + n + 1;
                let d = a + n;
                indices.extend_from_slice(&[a, b, cc, a, cc, d]);
            }
        }
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        TerrainPipeline { pipeline, bgl, ibuf, index_count: indices.len() as u32 }
    }
}

impl super::Renderer {
    /// Build the four quadrant chunks for a landscape cell. Textures must already be cached.
    pub fn build_terrain(&self, land: &Land) -> Vec<TerrainChunk> {
        let mut out = Vec::with_capacity(4);
        let origin = land.origin();
        for (q, quad) in land.quadrants.iter().enumerate() {
            let (qx, qy) = (q % 2, q / 2);
            let mut verts = Vec::with_capacity(QUAD_VERTS * QUAD_VERTS);
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for r in 0..QUAD_VERTS {
                for c in 0..QUAD_VERTS {
                    let gc = qx * 16 + c;
                    let gr = qy * 16 + r;
                    let gi = gr * VERTS + gc;
                    let step = CELL_SIZE / 32.0;
                    let p = Vec3::new(origin.x + gc as f32 * step, origin.y + gr as f32 * step, land.heights[gi]);
                    min = min.min(p);
                    max = max.max(p);
                    let mut w = [0f32; 8];
                    for (li, layer) in quad.layers.iter().enumerate().skip(1).take(MAX_LAYERS - 1) {
                        w[li - 1] = layer.opacity[r * QUAD_VERTS + c];
                    }
                    let uv = [gc as f32 / 32.0 * UV_REPEAT_PER_CELL, -(gr as f32) / 32.0 * UV_REPEAT_PER_CELL];
                    verts.push(TerrainVertex {
                        pos: p.to_array(),
                        normal: land.normals[gi].to_array(),
                        color: land.colors[gi].to_array(),
                        uv,
                        w0: [w[0], w[1], w[2], w[3]],
                        w1: [w[4], w[5], w[6], w[7]],
                    });
                }
            }
            let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terrain"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let mut views: Vec<Arc<super::texture::GpuTexture>> = Vec::with_capacity(16);
            for i in 0..MAX_LAYERS {
                let l = quad.layers.get(i);
                let d = l.and_then(|l| self.textures.get(&l.diffuse).flatten()).unwrap_or_else(|| self.white.clone());
                views.push(d);
            }
            for i in 0..MAX_LAYERS {
                let l = quad.layers.get(i);
                let n = l
                    .and_then(|l| l.normal.as_ref())
                    .and_then(|p| self.textures.get(p).flatten())
                    .unwrap_or_else(|| self.flat_normal.clone());
                views.push(n);
            }
            let mut entries: Vec<wgpu::BindGroupEntry> = views
                .iter()
                .enumerate()
                .map(|(i, t)| wgpu::BindGroupEntry { binding: i as u32, resource: wgpu::BindingResource::TextureView(&t.view) })
                .collect();
            entries.push(wgpu::BindGroupEntry { binding: 16, resource: wgpu::BindingResource::Sampler(&self.sampler) });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terrain"),
                layout: &self.terrain.bgl,
                entries: &entries,
            });
            out.push(TerrainChunk { vbuf, bind_group, center: (min + max) * 0.5, radius: (max - min).length() * 0.5 });
        }
        out
    }
}
