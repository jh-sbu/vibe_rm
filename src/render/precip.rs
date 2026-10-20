//! Rain and snow: a weather's precipitation (SPGD) as particles in a box
//! around the camera. Nothing is simulated: each particle's place comes from
//! its index and the time (falling at the gravity velocity, wrapping round
//! the box, snow turning about its falling centre), so the box follows the
//! camera while the particles stay put in the world. The wind carries them
//! all sideways together, and rain streaks along its slanted fall. Particles
//! under a roof are left out: the renderer draws what lies above the camera
//! into a top-down depth map (`shadow::precipitation_map`) and a particle
//! below the highest surface over it isn't drawn.

use std::sync::Arc;

use bytemuck::Zeroable;
use glam::{DVec2, Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

use super::texture::GpuTexture;
use crate::world::weather::Precipitation;

/// Particles per unit of SPGD density (made up: the density's scale isn't
/// public; the storm's 2 gives 5000 drops in its 1400 unit box).
const PARTICLES_PER_DENSITY: f32 = 2500.0;
const MAX_PARTICLES: u32 = 32768;
/// Units per unit of SPGD particle size (made up: rain's 0.35 x 2 makes
/// streaks 3.5 units wide and 20 long, snow's 1.15 flakes 11.5 across).
const SIZE_SCALE: f32 = 10.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PrecipUniform {
    /// Size x, size y, box size, snow (0 / 1).
    shape: [f32; 4],
    /// The fall so far (wrapped to the box), the turn so far (degrees),
    /// start rotation range, gravity velocity.
    motion: [f32; 4],
    /// Centre offset min, max; subtextures x, y.
    offsets: [f32; 4],
    /// Colour, alpha.
    color: [f32; 4],
    /// How far the wind has carried them (wrapped to the box), and the
    /// wind's velocity.
    wind: [f32; 4],
}

/// The occlusion map as the shader reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct OcclusionUniform {
    view_proj: [[f32; 4]; 4],
    /// 1 when the map is current, its depth range in units, how far under a
    /// surface a particle must be to be sheltered by it (units).
    params: [f32; 4],
}

/// How far under the highest surface over it a particle must lie to be out
/// of the rain (units): keeps drops at a roof's edge and on its own top out of
/// it (made up).
const SHELTER_BIAS: f32 = 12.0;

/// One weather's precipitation, ready to draw.
struct Layer {
    params: Precipitation,
    ubuf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// How much of it is falling (0..1).
    intensity: f32,
}

pub struct PrecipRenderer {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    /// The incoming weather's and the outgoing one's.
    layers: [Option<Layer>; 2],
    color: Vec3,
    wind: Vec2,
    /// How far the wind has carried everything.
    drift: DVec2,
    occlusion_buf: wgpu::Buffer,
    occlusion_bg: wgpu::BindGroup,
}

impl PrecipRenderer {
    pub fn new(
        device: &wgpu::Device,
        frame_bgl: &wgpu::BindGroupLayout,
        color_format: wgpu::TextureFormat,
        occlusion_map: &wgpu::TextureView,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("precipitation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/precip.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("precipitation"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let occlusion_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("precipitation occlusion"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let occlusion_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("precipitation occlusion"),
            contents: bytemuck::bytes_of(&OcclusionUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let occlusion_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("precipitation occlusion"),
            layout: &occlusion_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: occlusion_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(occlusion_map),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("precipitation"),
            bind_group_layouts: &[Some(frame_bgl), Some(&bgl), Some(&occlusion_bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("precipitation"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
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
        PrecipRenderer {
            pipeline,
            bgl,
            layers: [None, None],
            color: Vec3::ONE,
            wind: Vec2::ZERO,
            drift: DVec2::ZERO,
            occlusion_buf,
            occlusion_bg,
        }
    }

    /// Half the box of the widest layer falling: how far the occlusion map
    /// has to reach round the camera (0 with nothing falling).
    pub fn reach(&self) -> f32 {
        self.layers
            .iter()
            .flatten()
            .filter(|l| count(l) > 0)
            .map(|l| l.params.box_size.max(1.0) * 0.5)
            .fold(0.0, f32::max)
    }

    /// The occlusion map drawn from `view_proj`, `range` units deep, or none
    /// (nothing is sheltered).
    pub fn set_occlusion(&self, queue: &wgpu::Queue, map: Option<(Mat4, f32)>) {
        let u = match map {
            Some((view_proj, range)) => OcclusionUniform {
                view_proj: view_proj.to_cols_array_2d(),
                params: [1.0, range, SHELTER_BIAS, 0.0],
            },
            None => OcclusionUniform::zeroed(),
        };
        queue.write_buffer(&self.occlusion_buf, 0, bytemuck::bytes_of(&u));
    }

    /// The precipitation of the incoming weather (`slot` 0) or the outgoing
    /// one (1), with its texture; `None` clears it.
    pub fn set_layer(
        &mut self,
        device: &wgpu::Device,
        sampler: &wgpu::Sampler,
        slot: usize,
        layer: Option<(Precipitation, Arc<GpuTexture>)>,
    ) {
        if let (Some(cur), Some((p, _))) = (&self.layers[slot], &layer)
            && cur.params == *p
        {
            return;
        }
        self.layers[slot] = layer.map(|(params, tex)| {
            let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("precipitation"),
                contents: bytemuck::bytes_of(&PrecipUniform::zeroed()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("precipitation"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubuf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            Layer {
                params,
                ubuf,
                bind_group,
                intensity: 0.0,
            }
        });
    }

    /// How much of each layer falls (0..1) and the light it's lit by.
    pub fn set_intensity(&mut self, intensities: [f32; 2], color: Vec3) {
        for (l, i) in self.layers.iter_mut().zip(intensities) {
            if let Some(l) = l {
                l.intensity = i.clamp(0.0, 1.0);
            }
        }
        self.color = color;
    }

    /// The wind blowing now (units a second) over `dt` seconds.
    pub fn blow(&mut self, wind: Vec2, dt: f32) {
        self.wind = wind;
        // Wrapped at a distance every box size divides, to keep it small.
        self.drift = (self.drift + wind.as_dvec2() * dt as f64).rem_euclid(DVec2::splat(1.0e7));
    }

    /// The layers' particle counts now.
    pub fn particle_counts(&self) -> [u32; 2] {
        self.layers.each_ref().map(|l| l.as_ref().map_or(0, count))
    }

    /// Draw into a pass whose frame bind group (group 0) is set.
    pub fn draw(&self, queue: &wgpu::Queue, pass: &mut wgpu::RenderPass, time: f32) {
        for l in self.layers.iter().flatten() {
            let n = count(l);
            if n == 0 {
                continue;
            }
            let p = &l.params;
            let box_size = p.box_size.max(1.0);
            // Kept small for the shader's precision.
            let fall = (time as f64 * p.gravity as f64).rem_euclid(box_size as f64) as f32;
            let turn = (time as f64 * p.rotation_velocity as f64).rem_euclid(360.0) as f32;
            let u = PrecipUniform {
                shape: [
                    p.size.0 * SIZE_SCALE,
                    p.size.1 * SIZE_SCALE,
                    box_size,
                    p.snow as u32 as f32,
                ],
                motion: [fall, turn, p.start_rotation, p.gravity],
                offsets: [
                    p.center_offset.0,
                    p.center_offset.1,
                    p.subtextures.0 as f32,
                    p.subtextures.1 as f32,
                ],
                color: self.color.extend(1.0).to_array(),
                wind: [
                    self.drift.x.rem_euclid(box_size as f64) as f32,
                    self.drift.y.rem_euclid(box_size as f64) as f32,
                    self.wind.x,
                    self.wind.y,
                ],
            };
            queue.write_buffer(&l.ubuf, 0, bytemuck::bytes_of(&u));
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(1, &l.bind_group, &[]);
            pass.set_bind_group(2, &self.occlusion_bg, &[]);
            pass.draw(0..6, 0..n);
        }
    }
}

/// Particles drawn for a layer: its density's worth, as much as is falling.
fn count(l: &Layer) -> u32 {
    let full = (l.params.density * PARTICLES_PER_DENSITY).min(MAX_PARTICLES as f32);
    (full * l.intensity) as u32
}
