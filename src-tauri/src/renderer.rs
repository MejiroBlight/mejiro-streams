/// wgpu offscreen renderer.
///
/// Takes raw RGBA pixels, uploads them to the GPU, runs the composite
/// shader, reads the result back and returns it as raw RGBA bytes.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    // Cached GPU resources (reused if dimensions match)
    cached_width: u32,
    cached_height: u32,
    cached_input_tex: Option<wgpu::Texture>,
    cached_output_tex: Option<wgpu::Texture>,
    cached_readback_buf: Option<wgpu::Buffer>,
    cached_bytes_per_row: u32,
}

impl WgpuRenderer {
    /// Initialise the GPU device and build the render pipeline.
    /// This is async; call it once at startup with `pollster::block_on`.
    pub async fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "No suitable GPU adapter found".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

        // --- bind group layout --------------------------------------------------
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("frame_bind_group_layout"),
                entries: &[
                    // binding 0 : input texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // binding 1 : sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // --- shader -------------------------------------------------------------
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/composite.wgsl").into(),
            ),
        });

        // --- pipeline -----------------------------------------------------------
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // --- sampler ------------------------------------------------------------
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("frame_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            sampler,
            cached_width: 0,
            cached_height: 0,
            cached_input_tex: None,
            cached_output_tex: None,
            cached_readback_buf: None,
            cached_bytes_per_row: 0,
        })
    }

    /// Upload `rgba_pixels` to the GPU, run the composite shader, and return
    /// the result as a flat RGBA byte vector.
    pub fn render_frame(
        &mut self,
        rgba_pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        // Check if we need to allocate new GPU resources
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row_raw = 4 * width;
        let bytes_per_row = (bytes_per_row_raw + align - 1) / align * align;
        let buf_size = (bytes_per_row * height) as u64;

        if self.cached_width != width
            || self.cached_height != height
            || self.cached_input_tex.is_none()
            || self.cached_output_tex.is_none()
            || self.cached_readback_buf.is_none()
        {
            // Dimensions changed or first call – allocate new GPU resources
            self.cached_width = width;
            self.cached_height = height;
            self.cached_bytes_per_row = bytes_per_row;

            // Input texture
            self.cached_input_tex = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("input_texture_cached"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }));

            // Output texture
            self.cached_output_tex = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("output_texture_cached"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));

            // Readback buffer
            self.cached_readback_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback_buffer_cached"),
                size: buf_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }

        let input_tex = self
            .cached_input_tex
            .as_ref()
            .ok_or("Failed to cache input texture")?;
        let output_tex = self
            .cached_output_tex
            .as_ref()
            .ok_or("Failed to cache output texture")?;
        let readback_buf = self
            .cached_readback_buf
            .as_ref()
            .ok_or("Failed to cache readback buffer")?;

        // --- write input texture -----------------------------------------------
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: input_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let input_view = input_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = output_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // --- encode render pass ------------------------------------------------
        let mut encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_encoder"),
                });

        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &bind_group, &[]);
            rp.draw(0..4, 0..1); // triangle strip → fullscreen quad
        }

        // --- copy texture → buffer and submit ----------------------------------
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: output_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: readback_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.cached_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // --- map buffer and read pixels ----------------------------------------
        let buf_slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        buf_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{:?}", e))?;

        let mapped = buf_slice.get_mapped_range();
        let row_bytes = (4 * width) as usize;
        let mut pixels = Vec::with_capacity(row_bytes * height as usize);
        for y in 0..height as usize {
            let row_start = y * self.cached_bytes_per_row as usize;
            pixels.extend_from_slice(&mapped[row_start..row_start + row_bytes]);
        }
        drop(mapped);
        readback_buf.unmap();

        Ok(pixels)
    }
}
