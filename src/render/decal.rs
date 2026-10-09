//! Projected decals: each decal's box drawn over the opaque scene, its
//! fragments finding the surface behind them in the depth buffer and showing
//! the decal's texture where that surface lies in the box
//! (`vs_decal` / `fs_decal` in `object.wgsl`).

use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use super::{GpuMaterial, InstanceData};

/// A decal placed in a cell.
pub struct GpuDecal {
    pub ref_id: u32,
    pub hidden: bool,
    pub material: Arc<GpuMaterial>,
    /// The unit box about the origin to the decal's box in the world.
    pub transform: Mat4,
    /// Its subtexture: uv offset (xy) and scale (zw).
    pub uv: Vec4,
    pub tint: Vec4,
    pub lights: [u16; 8],
    pub center: Vec3,
    pub radius: f32,
}

impl GpuDecal {
    pub(super) fn instance(&self) -> InstanceData {
        InstanceData {
            model: self.transform.to_cols_array_2d(),
            lights: super::pack_lights(self.lights),
            tint: self.tint.to_array(),
            params: self.uv.to_array(),
        }
    }
}

pub struct DecalPipeline {
    pub pipeline: wgpu::RenderPipeline,
    depth_bgl: wgpu::BindGroupLayout,
    pub depth_bg: wgpu::BindGroup,
    pub cube: wgpu::Buffer,
}

/// A unit cube's 36 corners (12 triangles, wound outwards).
fn cube() -> Vec<[f32; 3]> {
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut v = Vec::new();
    for (n, u, w) in faces {
        let (n, u, w) = (Vec3::from(n), Vec3::from(u), Vec3::from(w));
        let c = |a: f32, b: f32| ((n + u * a + w * b) * 0.5).to_array();
        v.extend([c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0)]);
        v.extend([c(-1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)]);
    }
    v
}

impl DecalPipeline {
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        frame_bgl: &wgpu::BindGroupLayout,
        material_bgl: &wgpu::BindGroupLayout,
        color_format: wgpu::TextureFormat,
        depth: &wgpu::TextureView,
    ) -> Self {
        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("decal depth"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("decal"),
            bind_group_layouts: &[Some(frame_bgl), Some(material_bgl), Some(&depth_bgl)],
            immediate_size: 0,
        });
        let inst_attrs = wgpu::vertex_attr_array![
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Uint32x4,
            15 => Float32x4, 16 => Float32x4
        ];
        let pos_attrs = wgpu::vertex_attr_array![0 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("decal"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_decal"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 12,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &pos_attrs,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<InstanceData>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &inst_attrs,
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                // The box's far side: drawn with the camera inside it too.
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            // The depth buffer is read in the shader, attached read-only.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_decal"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let cube = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decal box"),
            contents: bytemuck::cast_slice(&cube()),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let depth_bg = Self::bind_depth(device, &depth_bgl, depth);
        DecalPipeline {
            pipeline,
            depth_bgl,
            depth_bg,
            cube,
        }
    }

    fn bind_depth(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        depth: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("decal depth"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth),
            }],
        })
    }

    /// The depth buffer was remade (resized).
    pub fn rebind(&mut self, device: &wgpu::Device, depth: &wgpu::TextureView) {
        self.depth_bg = Self::bind_depth(device, &self.depth_bgl, depth);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cube_winds_outwards() {
        let v = super::cube();
        assert_eq!(v.len(), 36);
        for t in v.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(glam::Vec3::from);
            let n = (b - a).cross(c - a);
            let centre = (a + b + c) / 3.0;
            assert!(n.dot(centre) > 0.0, "{t:?}");
        }
    }
}
