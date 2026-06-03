use super::context::GpuContext;

/// NV12 から RGBA へのコンピュートパスを保持するリソース群。
pub struct Nv12RgbaConverter {
    width: u32,
    height: u32,
    
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    pub texture_rgba: wgpu::Texture,
    pub view_rgba: wgpu::TextureView,
}

impl Nv12RgbaConverter {
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
            label: Some("NV12 Convert Layout"),
            entries: &[
                // Binding 0: Yテクスチャ
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
                // Binding 1: UVテクスチャ
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
                // Binding 2: サンプラー
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // WGSLシェーダーコードの読み込み（以前紹介したNV12用のWGSL）
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NV12 to RGBA Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/nv12_to_rgba.wgsl").into()),
        });

        // パイプラインの組み立て
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NV12 Convert Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NV12 Convert Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main", // 頂点シェーダーの関数名
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main", // 前述した色変換ロジックの関数名
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm, // 出力先フォーマット
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 自分自身の出力先（キャンバス）テクスチャを作る
        let texture_rgba = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("NV12 Convert Output RGBA"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view_rgba = texture_rgba.create_view(&Default::default());

        Self {
            width, height, pipeline, bind_group_layout, sampler, texture_rgba, view_rgba
        }
    }

    pub fn execute(
        &self,
        ctx: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        input_views: &[wgpu::TextureView], // ★ UploadFrame.get_views() をそのまま受ける
    ) -> &wgpu::TextureView {
        let device = &ctx.device;

        // 安全チェック: NV12なので必ず2つのビュー（YとUV）が必要
        assert_eq!(input_views.len(), 2, "NV12 Converter requires exactly 2 input views (Y and UV)");
        let view_y = &input_views[0];
        let view_uv = &input_views[1];

        // 1. 今届いたテクスチャビューを、シェーダーのBinding(0), Binding(1)に割り当てる
        //    ※バインドグループは毎フレームこの瞬間に作って使い捨てるのがwgpuでは一般的かつ安全です
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("NV12 Convert Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view_y), // WGSLの texture_y
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view_uv), // WGSLの texture_uv
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler), // サンプラー
                },
            ],
        });
        // 2. レンダーパスを開始（描き先は自分の中間RGBAテクスチャ）
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NV12 to RGBA Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view_rgba, // ★ここにRGBAとして描画される
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            // 3. パイプラインと、今作ったバインドグループをセットして実行！
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1); // 頂点バッファなしのフルスクリーン描画（3頂点）
        }

        // 4. 描き終わった中間RGBAテクスチャのビューを、次のステージに「これ使って！」と返す
        &self.view_rgba
    }
}