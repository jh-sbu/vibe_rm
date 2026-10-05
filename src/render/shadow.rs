//! Sun shadows: cascaded shadow maps fitted to slices of the view frustum.

use glam::{Mat4, Vec3, Vec4};

use super::model::{SkinVertex, Vertex};
use super::{Camera, InstanceData, SkinInstanceData};

pub const CASCADES: usize = 4;
pub const SIZE: u32 = 1024;
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Where each cascade ends (view depth); the first starts at the near plane.
pub const SPLITS: [f32; CASCADES] = [600.0, 1800.0, 5000.0, 12000.0];
/// The smallest caster (bounding radius) each cascade draws.
pub const MIN_CASTER: [f32; CASCADES] = [0.0, 0.0, 64.0, 160.0];
/// How far beyond a cascade's slice casters are looked for, towards the sun
/// (mountains and trees between the slice and the sun).
const CASTER_REACH: f32 = 12000.0;

/// One cascade: what it sees from the sun, and the sphere around its slice of the
/// view frustum (light view space centre, radius).
#[derive(Clone, Copy, Debug)]
pub struct Cascade {
    pub view_proj: Mat4,
    pub view: Mat4,
    pub radius: f32,
}

impl Cascade {
    /// Whether a world-space sphere can cast into this cascade.
    pub fn sees(&self, c: Vec3, r: f32) -> bool {
        let p = self.view.transform_point3(c);
        // Looking down -Z from beyond the slice: anything between the sun and the slice.
        p.x.abs() < self.radius + r && p.y.abs() < self.radius + r && -p.z > -r && -p.z < 2.0 * self.radius + CASTER_REACH + r
    }
}

/// Fit the cascades to the camera for a sun shining from `sun_dir` (towards it).
pub fn cascades(camera: &Camera, aspect: f32, sun_dir: Vec3) -> [Cascade; CASCADES] {
    let fwd = camera.forward();
    let right = fwd.cross(Vec3::Z).normalize_or(Vec3::X);
    let up = right.cross(fwd);
    let ty = (camera.fov_y * 0.5).tan();
    let tx = ty * aspect;
    let l = sun_dir.normalize_or(Vec3::Z);
    // A fixed orientation for the light (only its position follows the camera), so
    // texel snapping keeps shadow edges still as the camera moves.
    let light_up = if l.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let rot = Mat4::look_to_rh(Vec3::ZERO, -l, light_up);
    let mut near = 5.0;
    std::array::from_fn(|i| {
        let far = SPLITS[i];
        let corners = [near, far].into_iter().flat_map(|d| {
            let c = camera.position + fwd * d;
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(sx, sy)| c + right * (sx * tx * d) + up * (sy * ty * d))
        });
        let corners: Vec<Vec3> = corners.collect();
        let centre = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        // Round the radius so the cascade's size (and texel size) stays put.
        let radius = (corners.iter().map(|c| c.distance(centre)).fold(0.0, f32::max) / 64.0).ceil() * 64.0;
        near = far;
        // Snap the centre to the texel grid in light space.
        let texel = 2.0 * radius / SIZE as f32;
        let mut lc = rot.transform_point3(centre);
        lc.x = (lc.x / texel).floor() * texel;
        lc.y = (lc.y / texel).floor() * texel;
        let centre = rot.inverse().transform_point3(lc);
        let eye = centre + l * (radius + CASTER_REACH);
        let view = Mat4::look_to_rh(eye, -l, light_up);
        let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.0, 2.0 * radius + CASTER_REACH);
        Cascade { view_proj: proj * view, view, radius }
    })
}

pub struct ShadowMaps {
    pub array_view: wgpu::TextureView,
    pub layers: Vec<wgpu::TextureView>,
    pub sampler: wgpu::Sampler,
    pub bgl: wgpu::BindGroupLayout,
    pub uniforms: Vec<wgpu::Buffer>,
    /// Static casters: depth only, or alpha-tested through their diffuse texture.
    pub depth_pipeline: wgpu::RenderPipeline,
    pub static_pipeline: wgpu::RenderPipeline,
    pub skinned_pipeline: wgpu::RenderPipeline,
    pub terrain_pipeline: wgpu::RenderPipeline,
}

impl ShadowMaps {
    pub fn new(device: &wgpu::Device, material_bgl: &wgpu::BindGroupLayout, terrain_stride: u64) -> ShadowMaps {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow maps"),
            size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: CASCADES as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = (0..CASCADES as u32)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow pass"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let uniforms = (0..CASCADES)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("shadow cascade"),
                    size: 64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });
        let with_material = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow"),
            bind_group_layouts: &[Some(&bgl), Some(material_bgl)],
            immediate_size: 0,
        });
        let terrain_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow terrain"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let depth = || wgpu::DepthStencilState {
            format: FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            // Slope-scaled bias keeps lit surfaces from shadowing themselves.
            bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.5, clamp: 0.0 },
        };
        // Position and UV of the object vertex formats (see `object.wgsl`).
        let vertex_attrs = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 48, shader_location: 4 },
        ];
        let inst_attrs = wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];
        let skin_attrs = [
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 48, shader_location: 4 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 72, shader_location: 11 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 88, shader_location: 12 },
        ];
        let skin_inst_attrs = wgpu::vertex_attr_array![13 => Uint32];
        let pipeline = |label: &str, layout: &wgpu::PipelineLayout, entry: &str, buffers: &[Option<wgpu::VertexBufferLayout>], fragment: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(entry), compilation_options: Default::default(), buffers },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth()),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState { module: &shader, entry_point: Some("fs_alpha"), compilation_options: Default::default(), targets: &[] }),
                multiview_mask: None,
                cache: None,
            })
        };
        let static_buffers = [
            Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &vertex_attrs }),
            Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<InstanceData>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &inst_attrs }),
        ];
        let depth_pipeline = pipeline("shadow depth", &with_material, "vs_static", &static_buffers, false);
        let static_pipeline = pipeline("shadow static", &with_material, "vs_static", &static_buffers, true);
        let skinned_pipeline = pipeline(
            "shadow skinned",
            &with_material,
            "vs_skinned",
            &[
                Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<SkinVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &skin_attrs }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SkinInstanceData>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &skin_inst_attrs,
                }),
            ],
            true,
        );
        let terrain_attrs = wgpu::vertex_attr_array![0 => Float32x3];
        let terrain_pipeline = pipeline(
            "shadow terrain",
            &terrain_layout,
            "vs_terrain",
            &[Some(wgpu::VertexBufferLayout { array_stride: terrain_stride, step_mode: wgpu::VertexStepMode::Vertex, attributes: &terrain_attrs })],
            false,
        );
        ShadowMaps { array_view, layers, sampler, bgl, uniforms, depth_pipeline, static_pipeline, skinned_pipeline, terrain_pipeline }
    }
}

/// Frame uniform data for sampling the cascades: their matrices, where each ends,
/// and (enabled, texel size, cascade radii...) parameters.
pub fn frame_data(c: &[Cascade; CASCADES], enabled: bool) -> ([[[f32; 4]; 4]; CASCADES], [f32; 4], [f32; 4]) {
    let mats = c.map(|c| c.view_proj.to_cols_array_2d());
    let params = Vec4::new(if enabled { 1.0 } else { 0.0 }, 1.0 / SIZE as f32, c[0].radius, c[CASCADES - 1].radius);
    (mats, SPLITS, params.to_array())
}
