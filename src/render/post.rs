//! Image space effects drawn over the finished frame: the cinematic values
//! (saturation, brightness, contrast), tint, fade colour, blur and double
//! vision that image space modifiers and screen fades drive
//! (`crate::imagespace`). With nothing to apply the frame renders straight to
//! its target.

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostUniform {
    /// Saturation, brightness, contrast, 1 when the target holds linear values.
    cinematic: [f32; 4],
    /// Tint colour and amount.
    tint: [f32; 4],
    /// Fade colour and amount.
    fade: [f32; 4],
    /// Blur radius and double vision offset in pixels, texel size.
    blur: [f32; 4],
}

/// What the frame gets drawn through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostEffect {
    pub saturation: f32,
    pub brightness: f32,
    pub contrast: f32,
    /// Colour the image leans to (by luminance), and how far.
    pub tint: [f32; 4],
    /// Colour laid over the image, and how much.
    pub fade: [f32; 4],
    /// Blur radius (720 lines' pixels).
    pub blur: f32,
    /// Double vision strength.
    pub double_vision: f32,
}

impl Default for PostEffect {
    fn default() -> Self {
        PostEffect {
            saturation: 1.0,
            brightness: 1.0,
            contrast: 1.0,
            tint: [1.0, 1.0, 1.0, 0.0],
            fade: [0.0, 0.0, 0.0, 0.0],
            blur: 0.0,
            double_vision: 0.0,
        }
    }
}

impl PostEffect {
    pub fn is_neutral(&self) -> bool {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        near(self.saturation, 1.0)
            && near(self.brightness, 1.0)
            && near(self.contrast, 1.0)
            && self.tint[3] < 1e-3
            && self.fade[3] < 1e-3
            && self.blur < 1e-2
            && self.double_vision < 1e-2
    }
}

pub struct PostPass {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// The frame rendered before the effects: (size, its view, bind group).
    target: Option<([u32; 2], wgpu::TextureView, wgpu::BindGroup)>,
    format: wgpu::TextureFormat,
}

impl PostPass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post"),
            size: std::mem::size_of::<PostUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        PostPass {
            pipeline,
            bgl,
            ubuf,
            sampler,
            target: None,
            format,
        }
    }

    /// The texture to render the frame into before the effects.
    pub fn target(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let size = [width, height];
        if self.target.as_ref().is_none_or(|t| t.0 != size) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pre-post"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.ubuf.as_entire_binding(),
                    },
                ],
            });
            self.target = Some((size, view, bg));
        }
        self.target.as_ref().unwrap().1.clone()
    }

    /// Draw the frame rendered into [`PostPass::target`] onto `out` through `fx`.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        fx: &PostEffect,
    ) {
        let Some((size, _, bg)) = &self.target else {
            return;
        };
        // Effects work on display values: sRGB and float targets hold linear ones.
        let linear = self.format.is_srgb()
            || matches!(
                self.format,
                wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
            );
        let scale = size[1] as f32 / 720.0;
        let u = PostUniform {
            cinematic: [
                fx.saturation,
                fx.brightness,
                fx.contrast,
                if linear { 1.0 } else { 0.0 },
            ],
            tint: fx.tint,
            fade: fx.fade,
            blur: [
                fx.blur * scale,
                fx.double_vision * 8.0 * scale,
                1.0 / size[0] as f32,
                1.0 / size[1] as f32,
            ],
        };
        queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bg, &[]);
        pass.draw(0..3, 0..1);
    }
}
