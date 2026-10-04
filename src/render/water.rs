//! Water planes.

use std::sync::Arc;

use glam::Vec3;
use wgpu::util::DeviceExt;

use super::texture::GpuTexture;

#[derive(Debug, Clone)]
pub struct WaterParams {
    pub shallow: Vec3,
    pub deep: Vec3,
    pub reflection: Vec3,
    pub fresnel: f32,
    pub reflectivity: f32,
    pub sun_power: f32,
    pub noise_texture: String,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WaterUniform {
    shallow: [f32; 4],
    deep: [f32; 4],
    reflection: [f32; 4],
    params: [f32; 4],
}

pub struct WaterPlane {
    pub vbuf: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub center: Vec3,
    pub radius: f32,
}

pub struct WaterPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bgl: wgpu::BindGroupLayout,
}

impl WaterPipeline {
    pub fn new(device: &wgpu::Device, frame_bgl: &wgpu::BindGroupLayout, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/water.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water"),
            bind_group_layouts: &[Some(frame_bgl), Some(&bgl)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: 12, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs }],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        WaterPipeline { pipeline, bgl }
    }
}

impl super::Renderer {
    /// A square water plane covering `[x0, x0+size] x [y0, y0+size]` at height `z`.
    pub fn build_water(&self, x0: f32, y0: f32, size: f32, z: f32, params: &WaterParams) -> WaterPlane {
        let v: [[f32; 3]; 6] = [
            [x0, y0, z],
            [x0 + size, y0, z],
            [x0 + size, y0 + size, z],
            [x0, y0, z],
            [x0 + size, y0 + size, z],
            [x0, y0 + size, z],
        ];
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water"),
            contents: bytemuck::cast_slice(&v),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let u = WaterUniform {
            shallow: params.shallow.extend(1.0).to_array(),
            deep: params.deep.extend(1.0).to_array(),
            reflection: params.reflection.extend(1.0).to_array(),
            params: [params.fresnel, params.reflectivity, params.sun_power, 0.0],
        };
        let ubuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let noise: Arc<GpuTexture> = self.textures.get(&params.noise_texture).flatten().unwrap_or_else(|| self.flat_normal.clone());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water"),
            layout: &self.water.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&noise.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: ubuf.as_entire_binding() },
            ],
        });
        WaterPlane {
            vbuf,
            bind_group,
            center: Vec3::new(x0 + size * 0.5, y0 + size * 0.5, z),
            radius: size * 0.75,
        }
    }
}
