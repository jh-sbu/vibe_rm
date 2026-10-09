//! Image space effects drawn over the finished frame: the cinematic values
//! (saturation, brightness, contrast), tint, fade colour, blur and double
//! vision that the base image space, its modifiers and screen fades drive
//! (`crate::imagespace`). With nothing to apply the frame renders straight to
//! its target. The frame is reduced to its average by a mip chain; contrast
//! works about the average's luminance.

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
    /// The index of the smallest mip (the frame's average colour); the rest unused.
    levels: [f32; 4],
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

/// The frame rendered before the effects.
struct Target {
    size: [u32; 2],
    /// Mip 0, rendered into.
    view: wgpu::TextureView,
    /// All the mips, for the post pass.
    bind_group: wgpu::BindGroup,
    /// Each mip below the first: (its view, the bind group sampling the one above).
    mips: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

pub struct PostPass {
    pipeline: wgpu::RenderPipeline,
    /// Halving a mip into the next.
    downsample: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    ubuf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    target: Option<Target>,
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
        let downsample = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post downsample"),
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
                entry_point: Some("fs_down"),
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
            downsample,
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
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let levels = 32 - width.max(height).max(1).leading_zeros();
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pre-post"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let mip = |level: u32| {
                tex.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            };
            let bind = |view: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("post"),
                    layout: &self.bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
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
                })
            };
            let views: Vec<wgpu::TextureView> = (0..levels).map(mip).collect();
            let mips = (1..levels as usize)
                .map(|i| (views[i].clone(), bind(&views[i - 1])))
                .collect();
            self.target = Some(Target {
                size,
                view: views[0].clone(),
                bind_group: bind(&tex.create_view(&Default::default())),
                mips,
            });
        }
        self.target.as_ref().unwrap().view.clone()
    }

    /// Draw the frame rendered into [`PostPass::target`] onto `out` through `fx`.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        fx: &PostEffect,
    ) {
        let Some(target) = &self.target else {
            return;
        };
        let size = target.size;
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
            levels: [target.mips.len() as f32, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
        for (view, bg) in &target.mips {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post downsample"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&self.downsample);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
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
        pass.set_bind_group(0, &target.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
