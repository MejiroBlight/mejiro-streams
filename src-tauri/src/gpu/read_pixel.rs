use super::context::GpuContext;

/// RGBA から NV12 へのコンピュートパスと、CPU引き戻しバッファを一本化したマネージャー
pub struct RgbaToNv12ComputeConverter {
    width: u32,
    height: u32,
    
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,

    // 計算用の中間Storageテクスチャ (フォーマットはR32Uintで固定)
    storage_texture_y: wgpu::Texture,
    storage_texture_uv: wgpu::Texture,
    view_y: wgpu::TextureView,
    view_uv: wgpu::TextureView,

    // 最終成果物が1本になって入る、CPU読み出し用の一体型バッファ
    output_cpu_buffer: wgpu::Buffer,
    aligned_stride: u32,
    y_plane_size: u64,
}

impl RgbaToNv12ComputeConverter {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;

        // 1. wgpuの256バイトの掟に従い、アライメント後の1行あたりのバイト幅を計算 (1920なら2048)
        let aligned_stride = (width + 255) & !255; 
        let y_plane_size = (aligned_stride * height) as u64;
        let uv_plane_size = (aligned_stride * (height / 2)) as u64;
        let total_buffer_size = y_plane_size + uv_plane_size;

        // 2. バインディングレイアウトの定義 (入力RGBA1枚 ＋ 出力Storageテクスチャ2枚)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RGBA to NV12 Compute Layout"),
            entries: &[
                // Binding 0: 入力 RGBA テクスチャ (Read専用)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 1: 出力 Y プレーン (R32Uint Storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Binding 2: 出力 UV プレーン (R32Uint Storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // 3. コンピュートシェーダーの読み込み
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RGBA to NV12 Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rgba_to_nv12_compute.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RGBA to NV12 Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("RGBA to NV12 Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        });

        // 4. STORAGE書き込み用中間テクスチャの作成
        // ★修正: 横幅を 4バイト(R32Uint)に合わせて width / 4 にする
        let storage_texture_y = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Storage Texture Y (R32Uint)"),
            size: wgpu::Extent3d { 
                width: width / 4, // 1920 / 4 = 480 ➔ 1行のバイト数は 480 * 4 = 1920バイト！
                height, 
                depth_or_array_layers: 1 
            },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let storage_texture_uv = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Storage Texture UV (R32Uint)"),
            // UVプレーンも1行あたり width バイト（960画素×2バイト）なので、480ピクセルで一致します
            size: wgpu::Extent3d { 
                width: width / 4, // 1920 / 4 = 480
                height: height / 2, 
                depth_or_array_layers: 1 
            },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view_y = storage_texture_y.create_view(&Default::default());
        let view_uv = storage_texture_uv.create_view(&Default::default());

        // 5. 最終成果物が1本になって入る、CPU読み出し用の一体型巨大バッファ
        let output_cpu_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output NV12 Unified CPU Buffer"),
            size: total_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            width, height, pipeline, bind_group_layout,
            storage_texture_y, storage_texture_uv, view_y, view_uv,
            output_cpu_buffer, aligned_stride, y_plane_size, 
        }
    }

    /// 【一本化された実行関数】
    /// 1. コンピュートシェーダーで高速NV12変換
    /// 2. copy_texture_to_buffer を使って、GPU内で本来のNV12（1バイト/2バイト）のサイズとして1本のバッファに結合パッキング
    /// 3. メモリをCPUへ引き戻して Vec<u8> として返却
    pub async fn process_and_download(
        &self,
        ctx: &GpuContext,
        input_rgba_view: &wgpu::TextureView,
    ) -> Result<Vec<u8>, String> {
        let device = &ctx.device;
        let queue = &ctx.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // バインドグループの作成
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RGBA to NV12 Compute Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(input_rgba_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.view_y) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.view_uv) },
            ],
        });

        // --- ステージ1: コンピュートシェーダーによる超並列YUV変換 ---
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("RGBA to NV12 Compute Pass"),
                ..Default::default()
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);

            // 16x16タイルのスレッドを dispatch
            let workgroups_x = (self.width + 15) / 16;
            let workgroups_y = (self.height + 15) / 16;
            cpass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // --- ステージ2: 2枚の出力を、1本のバッファ（前半Y、後半UV）へGPU内で超高速合流パッキング ---
        // ★ここで wgpu に「テクスチャの中身はR32Uint(4バイト)だけど、コピーする時はR8Unorm(1バイト)のサイズとして扱って！」と嘘をつくことで
        // 余分なゼロバイトが消え去り、ギチギチにパッキングされたNV12バッファが完成します。

        // Yプレーンのコピー (横幅の指定を width / 4 に合わせる)
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.storage_texture_y, 
                mip_level: 0, 
                origin: wgpu::Origin3d::ZERO, 
                aspect: wgpu::TextureAspect::All 
            },
            wgpu::ImageCopyBuffer{ 
                buffer: &self.output_cpu_buffer, 
                layout: wgpu::ImageDataLayout { 
                    offset: 0, 
                    bytes_per_row: Some(self.aligned_stride), 
                    rows_per_image: None 
                } 
            },
            wgpu::Extent3d { 
                width: self.width / 4, 
                height: self.height, 
                depth_or_array_layers: 1 
            }, // ★ width / 4
        );

        // UVプレーンのコピー
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture { 
                texture: &self.storage_texture_uv, 
                mip_level: 0, 
                origin: wgpu::Origin3d::ZERO, 
                aspect: wgpu::TextureAspect::All 
            },
            wgpu::ImageCopyBuffer { 
                buffer: &self.output_cpu_buffer, 
                layout: wgpu::ImageDataLayout { 
                    offset: self.y_plane_size, 
                    bytes_per_row: Some(self.aligned_stride), rows_per_image: None
                } 
            },
            wgpu::Extent3d { 
                width: self.width / 4, 
                height: self.height / 2, 
                depth_or_array_layers: 1 
            }, // ★ width / 4
        );

        // コマンドの送信
        queue.submit(Some(encoder.finish()));

        // --- ステージ3: CPUへの引き戻し ---
        let slice = self.output_cpu_buffer.slice(..);
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        slice.map_async(wgpu::MapMode::Read, move |res| { let _ = tx.send(res); });
        device.poll(wgpu::Maintain::Wait);
        rx.await.map_err(|e| format!("Channel error: {e}"))?.map_err(|e| format!("Buffer map error: {e}"))?;

        let final_nv12 = slice.get_mapped_range().to_vec(); // ここで初めて、GPUが書き込んだNV12データがCPU側に現れる

        self.output_cpu_buffer.unmap();

        // --- ★ パディングを剥ぎ取る処理を追加 ---
        let width = self.width as usize;
        let height = self.height as usize;
        let stride = self.aligned_stride as usize; // 2048

        // パディングなしの本来のNV12サイズ (1920 * 1080 * 1.5)
        let mut packed_nv12 = Vec::with_capacity(width * height + (width * height / 2));

        // 1. Yプレーンのパディング剥ぎ取り
        for y in 0..height {
            let start = y * stride;
            packed_nv12.extend_from_slice(&final_nv12[start..start + width]);
        }

        // 2. UVプレーンのパディング剥ぎ取り (Yプレーンの直後から始まる)
        let y_plane_end = self.y_plane_size as usize;
        for y in 0..(height / 2) {
            let start = y_plane_end + (y * stride);
            packed_nv12.extend_from_slice(&final_nv12[start..start + width]);
        }

        Ok(packed_nv12)
    }
}