//! wgpu-based forward renderer.

pub mod dds;
pub mod model;
pub mod sky;
pub mod terrain;
pub mod texture;
pub mod water;

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use model::{BlendMode, CpuModel, MaterialDesc, ShaderKind, SkinVertex, Vertex};
use texture::{GpuTexture, TextureCache};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const MAX_LIGHTS: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    ambient: [f32; 4],
    fog_near_color: [f32; 4],
    fog_far_color: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
    amb: [[f32; 4]; 6],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
    pub pos_radius: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    uv: [f32; 4],
    emissive: [f32; 4],
    specular: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
    falloff: [f32; 4],
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceData {
    model: [[f32; 4]; 4],
    lights: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinInstanceData {
    palette_base: u32,
    lights: [u32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PipelineKey {
    pub skinned: bool,
    pub blend: BlendMode,
    pub double_sided: bool,
    pub z_write: bool,
    pub z_test: bool,
}

pub struct GpuMaterial {
    pub bind_group: wgpu::BindGroup,
    pub key: PipelineKey,
}

pub struct GpuPart {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
    pub material: Arc<GpuMaterial>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

pub struct GpuSkinnedPart {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
    pub material: Arc<GpuMaterial>,
    pub bone_names: Vec<String>,
    pub skin_to_bone: Vec<Mat4>,
    pub name: String,
}

pub struct GpuModel {
    pub path: String,
    pub parts: Vec<GpuPart>,
    pub skinned: Vec<GpuSkinnedPart>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

pub struct Instance {
    pub model: Arc<GpuModel>,
    pub transform: Mat4,
    pub lights: [u16; 8],
    pub world_center: Vec3,
    pub world_radius: f32,
}

impl Instance {
    pub fn new(model: Arc<GpuModel>, transform: Mat4) -> Self {
        let world_center = transform.transform_point3(model.bound_center);
        let scale = transform.x_axis.truncate().length().max(transform.y_axis.truncate().length()).max(transform.z_axis.truncate().length());
        let world_radius = model.bound_radius * scale;
        Instance { model, transform, lights: [0xFFFF; 8], world_center, world_radius }
    }
}

/// Global lighting environment for a frame.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    pub fog_near_color: Vec3,
    pub fog_far_color: Vec3,
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_power: f32,
    pub fog_max: f32,
    pub clear_color: Vec3,
    /// Directional ambient (X+, X-, Y+, Y-, Z+, Z-); replaces `ambient` when set.
    pub dalc: Option<[Vec3; 6]>,
    /// Draw the weather sky instead of the clear colour.
    pub sky: bool,
}

impl Default for Environment {
    fn default() -> Self {
        Environment {
            sun_dir: Vec3::new(0.3, 0.4, 0.8).normalize(),
            sun_color: Vec3::splat(0.6),
            ambient: Vec3::splat(0.3),
            fog_near_color: Vec3::splat(0.5),
            fog_far_color: Vec3::splat(0.5),
            fog_near: 0.0,
            fog_far: 500_000.0,
            fog_power: 1.0,
            fog_max: 0.0,
            clear_color: Vec3::new(0.4, 0.5, 0.6),
            dalc: None,
            sky: false,
        }
    }
}

/// One skinned shape of an actor, with its bones mapped onto the actor's skeleton.
pub struct ActorMesh {
    pub model: Arc<GpuModel>,
    pub part: usize,
    /// Mesh bone index -> skeleton bone index.
    pub bone_map: Vec<usize>,
}

/// A posed, skinned character.
pub struct ActorInstance {
    pub meshes: Vec<ActorMesh>,
    /// Rigid (non-skinned) models attached to skeleton bones, e.g. weapons.
    pub attachments: Vec<(Arc<GpuModel>, usize, Mat4)>,
    pub transform: Mat4,
    /// Model-space bone matrices for the current pose.
    pub pose: Vec<Mat4>,
    pub lights: [u16; 8],
    pub radius: f32,
}

impl ActorInstance {
    pub fn center(&self) -> Vec3 {
        self.transform.transform_point3(Vec3::new(0.0, 0.0, 64.0))
    }
}

/// Identifies a loaded cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellKey {
    Interior(esp::FormId),
    Exterior(i32, i32),
}

/// Render data belonging to one loaded cell.
#[derive(Default)]
pub struct RenderCell {
    pub instances: Vec<Instance>,
    pub actors: Vec<ActorInstance>,
    pub terrain: Vec<terrain::TerrainChunk>,
    pub water: Vec<water::WaterPlane>,
}

#[derive(Default)]
pub struct Scene {
    pub cells: HashMap<CellKey, RenderCell>,
    pub lights: Vec<GpuLight>,
    pub env: Environment,
}

impl Scene {
    /// Debug: instances whose bounding sphere intersects a ray, nearest first.
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Vec<(f32, String)> {
        let mut hits = Vec::new();
        for i in self.instances() {
            let oc = i.world_center - origin;
            let t = oc.dot(dir);
            let d2 = oc.length_squared() - t * t;
            if t > 0.0 && d2 < i.world_radius * i.world_radius {
                hits.push((t, i.model.path.clone()));
            }
        }
        for a in self.cells.values().flat_map(|c| c.actors.iter()) {
            let oc = a.center() - origin;
            let t = oc.dot(dir);
            let d2 = oc.length_squared() - t * t;
            if t > 0.0 && d2 < a.radius * a.radius {
                let names: Vec<&str> = a.meshes.iter().map(|m| m.model.skinned[m.part].name.as_str()).collect();
                hits.push((t, format!("actor: {names:?}")));
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits
    }

    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.cells.values().flat_map(|c| c.instances.iter())
    }
    pub fn instance_count(&self) -> usize {
        self.cells.values().map(|c| c.instances.len()).sum()
    }

    /// Assign up to 8 point lights to each instance based on bounding-sphere overlap.
    pub fn assign_lights(&mut self) {
        let lights = &self.lights;
        for inst in self.cells.values_mut().flat_map(|c| c.instances.iter_mut()) {
            let mut cands: Vec<(f32, u16)> = Vec::new();
            for (i, l) in lights.iter().enumerate() {
                let p = Vec3::new(l.pos_radius[0], l.pos_radius[1], l.pos_radius[2]);
                let r = l.pos_radius[3];
                let d = p.distance(inst.world_center);
                if d < r + inst.world_radius {
                    let lum = l.color[0] + l.color[1] + l.color[2];
                    // Prefer bright, close lights.
                    let score = lum * (1.0 - (d / (r + inst.world_radius)).min(1.0));
                    cands.push((-score, i as u16));
                }
            }
            cands.sort_by(|a, b| a.0.total_cmp(&b.0));
            inst.lights = [0xFFFF; 8];
            for (slot, (_, i)) in cands.iter().take(8).enumerate() {
                inst.lights[slot] = *i;
            }
        }
        for a in self.cells.values_mut().flat_map(|c| c.actors.iter_mut()) {
            a.lights = pick_lights(lights, a.center(), a.radius);
        }
    }
}

fn pick_lights(lights: &[GpuLight], center: Vec3, radius: f32) -> [u16; 8] {
    let mut cands: Vec<(f32, u16)> = Vec::new();
    for (i, l) in lights.iter().enumerate() {
        let p = Vec3::new(l.pos_radius[0], l.pos_radius[1], l.pos_radius[2]);
        let r = l.pos_radius[3];
        let d = p.distance(center);
        if d < r + radius {
            let lum = l.color[0] + l.color[1] + l.color[2];
            cands.push((-(lum * (1.0 - (d / (r + radius)).min(1.0))), i as u16));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = [0xFFFF; 8];
    for (slot, (_, i)) in cands.iter().take(8).enumerate() {
        out[slot] = *i;
    }
    out
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.yaw.cos() * self.pitch.cos(), self.pitch.sin())
    }
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), -self.yaw.sin(), 0.0)
    }
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.position, self.forward(), Vec3::Z)
    }
    pub fn proj(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(self.fov_y, aspect, 5.0)
    }
}

struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    fn from_matrix(m: Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let norm = |p: Vec4| p / p.truncate().length();
        // Reverse-Z infinite: near plane is z <= w  (r3 - r2), no far plane.
        Frustum { planes: [norm(r3 + r0), norm(r3 - r0), norm(r3 + r1), norm(r3 - r1), norm(r3 - r2)] }
    }
    fn sphere_visible(&self, c: Vec3, r: f32) -> bool {
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= -r)
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub color_format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    depth_view: wgpu::TextureView,
    frame_buf: wgpu::Buffer,
    light_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    frame_bgl: wgpu::BindGroupLayout,
    palette_buf: wgpu::Buffer,
    palette_cap: usize,
    skin_instance_buf: wgpu::Buffer,
    skin_instance_cap: usize,
    material_bgl: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) terrain: terrain::TerrainPipeline,
    pub(crate) water: water::WaterPipeline,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    pub textures: TextureCache,
    pub(crate) white: Arc<GpuTexture>,
    pub(crate) flat_normal: Arc<GpuTexture>,
    pub(crate) black: Arc<GpuTexture>,
    pub stats: FrameStats,
    pub sky: sky::SkyRenderer,
    /// Seconds since start, for animated effects.
    pub time: f32,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FrameStats {
    pub draws: u32,
    pub instances: u32,
    pub culled: u32,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, color_format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("object"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/object.wgsl").into()),
        });
        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                tex_entry(0),
                tex_entry(1),
                tex_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("object"),
            bind_group_layouts: &[Some(&frame_bgl), Some(&material_bgl)],
            immediate_size: 0,
        });
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lights"),
            size: (MAX_LIGHTS * std::mem::size_of::<GpuLight>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_cap = 4096;
        let palette_buf = Self::make_palette(&device, palette_cap);
        let frame_bg = Self::make_frame_bg(&device, &frame_bgl, &frame_buf, &light_buf, &palette_buf);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("main"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let instance_cap = 16384;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (instance_cap * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let white = Arc::new(texture::solid(&device, &queue, [255, 255, 255, 255], "white"));
        let flat_normal = Arc::new(texture::solid(&device, &queue, [128, 128, 255, 0], "flat_normal"));
        let black = Arc::new(texture::solid(&device, &queue, [0, 0, 0, 255], "black"));
        let depth_view = Self::make_depth(&device, width, height);
        let skin_instance_buf = Self::make_vbuf(&device, 1024 * std::mem::size_of::<SkinInstanceData>(), "skin instances");
        let terrain = terrain::TerrainPipeline::new(&device, &frame_bgl, color_format);
        let sky = sky::SkyRenderer::new(&device, color_format);
        let water = water::WaterPipeline::new(&device, &frame_bgl, color_format);
        Renderer {
            water,
            sky,
            time: 0.0,
            terrain,
            device,
            queue,
            color_format,
            width,
            height,
            depth_view,
            frame_buf,
            light_buf,
            frame_bg,
            frame_bgl,
            palette_buf,
            palette_cap,
            skin_instance_buf,
            skin_instance_cap: 1024,
            material_bgl,
            pipeline_layout,
            shader,
            pipelines: HashMap::new(),
            sampler,
            instance_buf,
            instance_cap,
            textures: TextureCache::default(),
            white,
            flat_normal,
            black,
            stats: FrameStats::default(),
        }
    }

    fn make_palette(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bone palette"),
            size: (cap * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_vbuf(device: &wgpu::Device, size: usize, label: &str) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_frame_bg(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        palette: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: palette.as_entire_binding() },
            ],
        })
    }

    fn make_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
        self.depth_view = Self::make_depth(&self.device, self.width, self.height);
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&key) {
            let p = self.create_pipeline(key);
            self.pipelines.insert(key, p);
        }
        &self.pipelines[&key]
    }

    fn create_pipeline(&self, key: PipelineKey) -> wgpu::RenderPipeline {
        let vertex_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x2, 5 => Float32x4
        ];
        let inst_attrs = wgpu::vertex_attr_array![
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Uint32x4
        ];
        let skin_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x2, 5 => Float32x4,
            11 => Uint32x4, 12 => Float32x4
        ];
        let skin_inst_attrs = wgpu::vertex_attr_array![13 => Uint32, 14 => Uint32x4];
        let static_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<InstanceData>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &inst_attrs,
            }),
        ];
        let skinned_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SkinVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &skin_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SkinInstanceData>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &skin_inst_attrs,
            }),
        ];
        let blend = match key.blend {
            BlendMode::Opaque => None,
            BlendMode::Blend(src, dst) => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: blend_factor(src),
                    dst_factor: blend_factor(dst),
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        };
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("object"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some(if key.skinned { "vs_skinned" } else { "vs_main" }),
                compilation_options: Default::default(),
                buffers: if key.skinned { &skinned_buffers } else { &static_buffers },
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: if key.double_sided { None } else { Some(wgpu::Face::Back) },
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(key.z_write),
                depth_compare: Some(if key.z_test {
                    wgpu::CompareFunction::GreaterEqual
                } else {
                    wgpu::CompareFunction::Always
                }),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.color_format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Upload a parsed DDS into the texture cache.
    pub fn add_texture(&mut self, path: &str, d: Option<&dds::Dds>) {
        let t = d.map(|d| Arc::new(texture::upload_dds(&self.device, &self.queue, d, path)));
        self.textures.insert(path.to_owned(), t);
    }

    fn texture_or(&self, path: &Option<String>, fallback: &Arc<GpuTexture>) -> (Arc<GpuTexture>, bool) {
        match path.as_ref().and_then(|p| self.textures.get(p)).flatten() {
            Some(t) => (t, true),
            None => (fallback.clone(), false),
        }
    }

    pub fn create_material(&self, m: &MaterialDesc) -> GpuMaterial {
        let (diffuse, _) = self.texture_or(&m.diffuse, &self.white);
        let (normal, has_normal) = self.texture_or(&m.normal, &self.flat_normal);
        let (glow, has_glow) = self.texture_or(&m.glow, &self.black);
        let u = MaterialUniform {
            uv: [m.uv_offset.x, m.uv_offset.y, m.uv_scale.x, m.uv_scale.y],
            emissive: m.emissive.to_array(),
            specular: [m.specular.x, m.specular.y, m.specular.z, m.glossiness],
            params: [
                m.alpha,
                m.alpha_test.unwrap_or(-1.0),
                if has_normal && m.kind == ShaderKind::Lit { 1.0 } else { 0.0 },
                if has_glow && m.shader_type != 4 { 1.0 } else if has_glow { 2.0 } else { 0.0 },
            ],
            flags: [m.flags1, m.flags2, if m.kind == ShaderKind::Effect { 1 } else { 0 }, m.shader_type],
            falloff: m.falloff.to_array(),
            tint: m.tint.extend(1.0).to_array(),
        };
        let ubuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&diffuse.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&normal.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&glow.view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: ubuf.as_entire_binding() },
            ],
        });
        let key = PipelineKey {
            skinned: false,
            blend: m.blend,
            double_sided: m.double_sided,
            z_write: m.z_write || m.blend == BlendMode::Opaque,
            z_test: m.z_test || m.blend == BlendMode::Opaque,
        };
        GpuMaterial { bind_group, key }
    }

    pub fn upload_model(&self, cpu: &CpuModel) -> GpuModel {
        let mut parts = Vec::with_capacity(cpu.meshes.len());
        for m in &cpu.meshes {
            let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&m.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&m.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            parts.push(GpuPart {
                vbuf,
                ibuf,
                index_count: m.indices.len() as u32,
                material: Arc::new(self.create_material(&m.material)),
                bound_center: m.bound_center,
                bound_radius: m.bound_radius,
            });
        }
        let mut skinned = Vec::with_capacity(cpu.skinned.len());
        for m in &cpu.skinned {
            let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&m.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&m.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let mut mat = self.create_material(&m.material);
            mat.key.skinned = true;
            skinned.push(GpuSkinnedPart {
                vbuf,
                ibuf,
                index_count: m.indices.len() as u32,
                material: Arc::new(mat),
                bone_names: m.bone_names.clone(),
                skin_to_bone: m.skin_to_bone.clone(),
                name: m.name.clone(),
            });
        }
        GpuModel { path: String::new(), parts, skinned, bound_center: cpu.bound_center, bound_radius: cpu.bound_radius }
    }

    pub fn render(&mut self, scene: &Scene, camera: &Camera, target: &wgpu::TextureView) {
        let aspect = self.width as f32 / self.height as f32;
        let view_proj = camera.proj(aspect) * camera.view();
        let frustum = Frustum::from_matrix(view_proj);
        let env = &scene.env;
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos: camera.position.extend(1.0).to_array(),
            sun_dir: env.sun_dir.normalize_or(Vec3::Z).extend(0.0).to_array(),
            sun_color: env.sun_color.extend(1.0).to_array(),
            ambient: env.ambient.extend(1.0).to_array(),
            fog_near_color: env.fog_near_color.extend(1.0).to_array(),
            fog_far_color: env.fog_far_color.extend(1.0).to_array(),
            fog: [env.fog_near, env.fog_far, env.fog_power, env.fog_max],
            misc: [self.time, scene.lights.len() as f32, 0.0, 0.0],
            amb: match env.dalc {
                Some(d) => {
                    let mut a = [[0f32; 4]; 6];
                    for i in 0..6 {
                        a[i] = d[i].extend(1.0).to_array();
                    }
                    a
                }
                None => [[0f32; 4]; 6],
            },
        };
        self.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let nl = scene.lights.len().min(MAX_LIGHTS);
        if nl > 0 {
            self.queue.write_buffer(&self.light_buf, 0, bytemuck::cast_slice(&scene.lights[..nl]));
        }

        // Batch visible instances by (model, part).
        let mut stats = FrameStats::default();
        let mut opaque: HashMap<(*const GpuModel, usize), (&GpuPart, Vec<InstanceData>)> = HashMap::new();
        let mut blended: Vec<(f32, &GpuPart, InstanceData)> = Vec::new();
        for inst in scene.instances() {
            if !frustum.sphere_visible(inst.world_center, inst.world_radius) {
                stats.culled += 1;
                continue;
            }
            let mp = Arc::as_ptr(&inst.model);
            let l = inst.lights;
            let data = InstanceData { model: inst.transform.to_cols_array_2d(), lights: pack_lights(l) };
            for (pi, part) in inst.model.parts.iter().enumerate() {
                if part.material.key.blend == BlendMode::Opaque {
                    opaque.entry((mp, pi)).or_insert_with(|| (part, Vec::new())).1.push(data);
                } else {
                    let c = inst.transform.transform_point3(part.bound_center);
                    blended.push((c.distance_squared(camera.position), part, data));
                }
            }
        }
        // Rigid attachments of actors (weapons, etc.) go through the static path.
        for actor in scene.cells.values().flat_map(|c| c.actors.iter()) {
            if !frustum.sphere_visible(actor.center(), actor.radius) {
                continue;
            }
            for (model, bone, local) in &actor.attachments {
                let xf = actor.transform * actor.pose.get(*bone).copied().unwrap_or(Mat4::IDENTITY) * *local;
                let data = InstanceData { model: xf.to_cols_array_2d(), lights: pack_lights(actor.lights) };
                for (pi, part) in model.parts.iter().enumerate() {
                    if part.material.key.blend == BlendMode::Opaque {
                        opaque.entry((Arc::as_ptr(model), pi)).or_insert_with(|| (part, Vec::new())).1.push(data);
                    } else {
                        let c = xf.transform_point3(part.bound_center);
                        blended.push((c.distance_squared(camera.position), part, data));
                    }
                }
            }
        }
        blended.sort_by(|a, b| b.0.total_cmp(&a.0));

        // Skinned actors: build the bone palette.
        let mut palette: Vec<Mat4> = Vec::new();
        let mut skin_inst: Vec<SkinInstanceData> = Vec::new();
        let mut skin_draws: Vec<(&GpuSkinnedPart, u32)> = Vec::new();
        for actor in scene.cells.values().flat_map(|c| c.actors.iter()) {
            if !frustum.sphere_visible(actor.center(), actor.radius) {
                stats.culled += 1;
                continue;
            }
            for mesh in &actor.meshes {
                let Some(part) = mesh.model.skinned.get(mesh.part) else { continue };
                let base = palette.len() as u32;
                for (i, &b) in mesh.bone_map.iter().enumerate() {
                    let s2b = part.skin_to_bone.get(i).copied().unwrap_or(Mat4::IDENTITY);
                    // Bones missing from the skeleton stay in their bind position.
                    let m = match actor.pose.get(b) {
                        Some(bone) => actor.transform * *bone * s2b,
                        None => actor.transform,
                    };
                    palette.push(m);
                }
                if mesh.bone_map.is_empty() {
                    palette.push(actor.transform);
                }
                skin_inst.push(SkinInstanceData { palette_base: base, lights: pack_lights(actor.lights) });
                skin_draws.push((part, skin_inst.len() as u32 - 1));
            }
        }
        if palette.len() > self.palette_cap {
            self.palette_cap = palette.len().next_power_of_two();
            self.palette_buf = Self::make_palette(&self.device, self.palette_cap);
            self.frame_bg = Self::make_frame_bg(&self.device, &self.frame_bgl, &self.frame_buf, &self.light_buf, &self.palette_buf);
        }
        if !palette.is_empty() {
            self.queue.write_buffer(&self.palette_buf, 0, bytemuck::cast_slice(&palette));
        }
        if skin_inst.len() > self.skin_instance_cap {
            self.skin_instance_cap = skin_inst.len().next_power_of_two();
            self.skin_instance_buf =
                Self::make_vbuf(&self.device, self.skin_instance_cap * std::mem::size_of::<SkinInstanceData>(), "skin instances");
        }
        if !skin_inst.is_empty() {
            self.queue.write_buffer(&self.skin_instance_buf, 0, bytemuck::cast_slice(&skin_inst));
        }
        for (part, _) in &skin_draws {
            self.pipeline(part.material.key);
        }

        // Flatten into one instance buffer.
        let mut all: Vec<InstanceData> = Vec::new();
        let mut draws: Vec<(&GpuPart, std::ops::Range<u32>)> = Vec::new();
        let mut opaque_sorted: Vec<_> = opaque.into_values().collect();
        opaque_sorted.sort_by_key(|(part, _)| (part.material.key.double_sided, part.material.key.z_write));
        for (part, v) in opaque_sorted {
            let start = all.len() as u32;
            all.extend(v);
            draws.push((part, start..all.len() as u32));
        }
        let opaque_count = draws.len();
        for (_, part, data) in &blended {
            let start = all.len() as u32;
            all.push(*data);
            draws.push((part, start..start + 1));
        }
        if all.len() > self.instance_cap {
            self.instance_cap = all.len().next_power_of_two();
            self.instance_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: (self.instance_cap * std::mem::size_of::<InstanceData>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !all.is_empty() {
            self.queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&all));
        }
        // Make sure all pipelines exist before borrowing them in the pass.
        for (part, _) in &draws {
            self.pipeline(part.material.key);
        }

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let c = env.clear_color;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c.x as f64, g: c.y as f64, b: c.z as f64, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if env.sky && self.sky.is_active() {
                self.sky.draw(&self.queue, &mut pass, view_proj, camera.position, self.time);
            }
            pass.set_bind_group(0, &self.frame_bg, &[]);
            if scene.cells.values().any(|c| !c.terrain.is_empty()) {
                pass.set_pipeline(&self.terrain.pipeline);
                pass.set_index_buffer(self.terrain.ibuf.slice(..), wgpu::IndexFormat::Uint16);
                for chunk in scene.cells.values().flat_map(|c| c.terrain.iter()) {
                    if !frustum.sphere_visible(chunk.center, chunk.radius) {
                        continue;
                    }
                    pass.set_bind_group(1, &chunk.bind_group, &[]);
                    pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                    pass.draw_indexed(0..self.terrain.index_count, 0, 0..1);
                    stats.draws += 1;
                }
            }
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            let current: std::cell::Cell<Option<PipelineKey>> = std::cell::Cell::new(None);
            let mut draw = |pass: &mut wgpu::RenderPass, part: &GpuPart, range: std::ops::Range<u32>| {
                if current.get() != Some(part.material.key) {
                    pass.set_pipeline(&self.pipelines[&part.material.key]);
                    current.set(Some(part.material.key));
                }
                pass.set_bind_group(1, &part.material.bind_group, &[]);
                pass.set_vertex_buffer(0, part.vbuf.slice(..));
                pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..part.index_count, 0, range.clone());
                stats.draws += 1;
                stats.instances += range.end - range.start;
            };
            for (part, range) in &draws[..opaque_count] {
                draw(&mut pass, part, range.clone());
            }
            let mut skinned_draws = 0;
            if !skin_draws.is_empty() {
                pass.set_vertex_buffer(1, self.skin_instance_buf.slice(..));
                for (part, i) in &skin_draws {
                    if current.get() != Some(part.material.key) {
                        pass.set_pipeline(&self.pipelines[&part.material.key]);
                        current.set(Some(part.material.key));
                    }
                    pass.set_bind_group(1, &part.material.bind_group, &[]);
                    pass.set_vertex_buffer(0, part.vbuf.slice(..));
                    pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..part.index_count, 0, *i..*i + 1);
                    skinned_draws += 1;
                }
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            }
            if scene.cells.values().any(|c| !c.water.is_empty()) {
                pass.set_pipeline(&self.water.pipeline);
                for w in scene.cells.values().flat_map(|c| c.water.iter()) {
                    if !frustum.sphere_visible(w.center, w.radius) {
                        continue;
                    }
                    pass.set_bind_group(1, &w.bind_group, &[]);
                    pass.set_vertex_buffer(0, w.vbuf.slice(..));
                    pass.draw(0..6, 0..1);
                }
                current.set(None);
            }
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            for (part, range) in &draws[opaque_count..] {
                draw(&mut pass, part, range.clone());
            }
            drop(draw);
            stats.draws += skinned_draws;
        }
        self.queue.submit([enc.finish()]);
        self.stats = stats;
    }

    /// Render `frames` frames offscreen and return the average wall time per frame (GPU-synchronised).
    pub fn bench(&mut self, scene: &Scene, camera: &Camera, frames: u32) -> std::time::Duration {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench"),
            size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(scene, camera, &view);
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let t = std::time::Instant::now();
        let mut cpu = std::time::Duration::ZERO;
        for _ in 0..frames {
            let c = std::time::Instant::now();
            self.render(scene, camera, &view);
            cpu += c.elapsed();
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
        log::info!("bench: cpu {:?}/frame", cpu / frames.max(1));
        t.elapsed() / frames.max(1)
    }

    /// Render a frame into an offscreen texture and return RGBA8 pixels.
    pub fn render_to_image(&mut self, scene: &Scene, camera: &Camera, overlay: impl FnOnce(&mut Self, &wgpu::TextureView)) -> Vec<u8> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(scene, camera, &view);
        overlay(self, &view);
        let bpr = (self.width * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bpr * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: Default::default(), aspect: Default::default() },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(self.height) },
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("readback buffer mapped");
        let mut out = Vec::with_capacity((self.width * self.height * 4) as usize);
        let bgra = matches!(self.color_format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        for y in 0..self.height {
            let row = &data[(y * bpr) as usize..(y * bpr + self.width * 4) as usize];
            for px in row.chunks_exact(4) {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    out.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        out
    }
}

fn pack_lights(l: [u16; 8]) -> [u32; 4] {
    [
        l[0] as u32 | (l[1] as u32) << 16,
        l[2] as u32 | (l[3] as u32) << 16,
        l[4] as u32 | (l[5] as u32) << 16,
        l[6] as u32 | (l[7] as u32) << 16,
    ]
}

fn blend_factor(f: u16) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as B;
    match f {
        0 => B::One,
        1 => B::Zero,
        2 => B::Src,
        3 => B::OneMinusSrc,
        4 => B::Dst,
        5 => B::OneMinusDst,
        6 => B::SrcAlpha,
        7 => B::OneMinusSrcAlpha,
        8 => B::DstAlpha,
        9 => B::OneMinusDstAlpha,
        10 => B::SrcAlphaSaturated,
        _ => B::One,
    }
}
