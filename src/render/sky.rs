//! Sky dome rendering driven by the evaluated weather state.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::texture::GpuTexture;
use crate::world::weather::SkyState;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyUniform {
    inv_view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    upper: [f32; 4],
    lower: [f32; 4],
    horizon: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    /// The incoming weather's four layers, then the outgoing one's.
    cloud_color: [[f32; 4]; 8],
    params: [f32; 4],
    /// x: the outgoing weather's layer count.
    params2: [f32; 4],
}

/// Cloud layers drawn per weather (two weathers during a transition).
const LAYERS: usize = 4;

pub struct SkyRenderer {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    state: Option<SkyState>,
    cloud_count: usize,
    outgoing_count: usize,
}

impl SkyRenderer {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        for b in 1..=(1 + 2 * LAYERS as u32) {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: b,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2 + 2 * LAYERS as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky"),
            entries: &entries,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky"),
            size: std::mem::size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SkyRenderer {
            pipeline,
            bgl,
            ubuf,
            bind_group: None,
            state: None,
            cloud_count: 0,
            outgoing_count: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.bind_group.is_some() && self.state.is_some()
    }

    pub fn disable(&mut self) {
        self.state = None;
    }

    pub fn set_state(&mut self, state: SkyState) {
        self.state = Some(state);
    }

    /// The sun's texture and the cloud layers' of the incoming weather and
    /// the outgoing one (up to [`LAYERS`] each).
    pub fn set_textures(
        &mut self,
        device: &wgpu::Device,
        sampler: &wgpu::Sampler,
        sun: Arc<GpuTexture>,
        clouds: Vec<Arc<GpuTexture>>,
        outgoing: Vec<Arc<GpuTexture>>,
        fallback: Arc<GpuTexture>,
    ) {
        self.cloud_count = clouds.len().min(LAYERS);
        self.outgoing_count = outgoing.len().min(LAYERS);
        let layers: Vec<Arc<GpuTexture>> = (0..2 * LAYERS)
            .map(|i| {
                let (list, j) = if i < LAYERS {
                    (&clouds, i)
                } else {
                    (&outgoing, i - LAYERS)
                };
                list.get(j).cloned().unwrap_or_else(|| fallback.clone())
            })
            .collect();
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.ubuf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&sun.view),
            },
        ];
        for (i, t) in layers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: 2 + i as u32,
                resource: wgpu::BindingResource::TextureView(&t.view),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: 2 + 2 * LAYERS as u32,
            resource: wgpu::BindingResource::Sampler(sampler),
        });
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky"),
            layout: &self.bgl,
            entries: &entries,
        }));
    }

    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass,
        view_proj: Mat4,
        cam_pos: Vec3,
        time: f32,
    ) {
        let (Some(bg), Some(st)) = (&self.bind_group, &self.state) else {
            return;
        };
        let mut cloud_color = [[0f32; 4]; 8];
        for (i, c) in st.clouds.iter().take(LAYERS).enumerate() {
            cloud_color[i] = [c.1.x, c.1.y, c.1.z, c.2];
        }
        for (i, c) in st.outgoing_clouds.iter().take(LAYERS).enumerate() {
            cloud_color[LAYERS + i] = [c.1.x, c.1.y, c.1.z, c.2];
        }
        let u = SkyUniform {
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cam_pos: cam_pos.extend(1.0).to_array(),
            upper: st.sky_upper.extend(1.0).to_array(),
            lower: st.sky_lower.extend(1.0).to_array(),
            horizon: st.horizon.extend(1.0).to_array(),
            sun_dir: st.sun_dir.extend(st.sun_visible).to_array(),
            sun_color: st.sun_color.extend(1.0).to_array(),
            cloud_color,
            params: [time, 0.3, self.cloud_count as f32, st.stars],
            params2: [
                if st.outgoing_clouds.is_empty() {
                    0.0
                } else {
                    self.outgoing_count as f32
                },
                0.0,
                0.0,
                0.0,
            ],
        };
        queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }
}
