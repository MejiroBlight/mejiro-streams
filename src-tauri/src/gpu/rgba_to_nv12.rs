use super::context::GpuContext;

/// RGBA から NV12 へのコンピュートパスを保持するリソース群。
pub struct RgbaToNv12Converter {
    width: u32,
    height: u32,
    
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    pub texture_y: wgpu::Texture,
    pub texture_uv: wgpu::Texture,
    pub view_y: wgpu::TextureView,
    pub view_uv: wgpu::TextureView,
}

impl RgbaToNv12Converter {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // シェーダーが要求するレイアウトの定義
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RGBA to NV12 Layout"),
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
            ],
        });

        // 先ほどのシェーダーコードを読み込む
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RGBA to NV12 Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rgba_to_nv12.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RGBA to NV12 Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 3. パイプラインの組み立て（MRT構成）
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("RGBA to NV12 Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                // ★重要: シェーダーの @location(0), @location(1) に対応する2つのターゲットを並べる
                targets: &[
                    // @location(0) -> Yプレーン用 (R8Unorm)
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R8Unorm,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // @location(1) -> UVプレーン用 (Rg8Unorm)
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rg8Unorm,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 4. 出力先となるNV12の器（テクスチャ）を作成する
        //    ※最終的にCPUへコピー(COPY_SRC)するため、usageに指定を入れておく
        let texture_y = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output Texture Y"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_uv = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output Texture UV"),
            size: wgpu::Extent3d { width: width / 2, height: height / 2, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view_y = texture_y.create_view(&Default::default());
        let view_uv = texture_uv.create_view(&Default::default());

        Self {
            width, height, pipeline, bind_group_layout, sampler,
            texture_y, texture_uv, view_y, view_uv,
        }
    }

    /// 前のステージから受け取ったRGBAビューをNV12テクスチャに逆変換して書き込む
    /// 戻り値として、出力が完了したYとUVのテクスチャへの参照を返す（次のread_pixelステージ用）
    pub fn execute<'a>(
        &'a self,
        ctx: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        input_rgba_view: &wgpu::TextureView,
    ) -> (&'a wgpu::Texture, &'a wgpu::Texture) {
        
        let device = &ctx.device;
        // 入力RGBAのバインドグループを生成
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RGBA to NV12 Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_rgba_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // レンダーパスを開始（MRT設定）
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("RGBA to NV12 Render Pass"),
                // ★重要: pipelineのtargets配列と同じ順番で、出力先Viewを指定する
                color_attachments: &[
                    // location(0) -> Yプレーンのビューへ
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.view_y,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    // location(1) -> UVプレーンのビューへ
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.view_uv,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                ..Default::default()
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..4, 0..1); // 四角形を描画
        }

        // 次のread_pixel（CPU取り出し）ステージがコピーコマンドを発行できるように、
        // 生成されたテクスチャの実体をペアで返す
        (&self.texture_y, &self.texture_uv)
    }
}