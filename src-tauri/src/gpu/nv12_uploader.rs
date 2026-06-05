use crate::decoder;
use super::context::GpuContext;

/// NV12生データの受け取りから、RGBAテクスチャへの変換までを担う統合モジュール
pub struct Nv12Uploader {
    width: u32,
    height: u32,
    
    pub input_buffer: wgpu::Buffer,    // NV12生データ用
    pub texture_rgba: wgpu::Texture,  // RGBA変換結果用
    pub view_rgba: wgpu::TextureView,
    pub config_buffer: wgpu::Buffer, // 変換に必要なパラメータをGPUに渡すためのバッファ
    
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Nv12Uploader {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;
        
        // 1. 入力バッファの作成
        let aligned_stride = (width + 255) & !255;
        let buffer_size = (aligned_stride * height as u32 + aligned_stride * height as u32 / 2) as u64;

        let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("NV12 Raw Input Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("NV12 Config Uniform Buffer"),
            size: 16, // vec4<u32> = 16 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 2. 出力RGBAテクスチャの作成
        let texture_rgba = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RGBA Output Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let view_rgba = texture_rgba.create_view(&Default::default());

        // 3. コンピュートパイプラインの構築
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("NV12 to RGBA Pipeline Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry{
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba8Unorm, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2, // 新規
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { 
                        ty: wgpu::BufferBindingType::Uniform, 
                        has_dynamic_offset: false, 
                        min_binding_size: None 
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&bind_group_layout], push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NV12 to RGBA Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("shaders/nv12_to_rgba_compute.wgsl"))),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("NV12 to RGBA Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "cs_main",
            compilation_options: Default::default(),
            cache: None,
        });

        Self { width, height, input_buffer, texture_rgba, view_rgba, pipeline, bind_group_layout, config_buffer}
    }

    /// フレームデータのアップロードと変換実行までを一気通貫で行う
    pub fn upload(&self, ctx: &GpuContext, encoder: &mut wgpu::CommandEncoder, frame: &decoder::FrameData) -> &wgpu::TextureView {
        // A. アップロード
        ctx.queue.write_buffer(&self.input_buffer, 0, &frame.pixels);

        let aligned_stride = (self.width + 255) & !255;

        ctx.queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&[aligned_stride, 0, 0, 0]));

        // B. 変換実行
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.input_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.view_rgba) },
                wgpu::BindGroupEntry { binding: 2, resource: self.config_buffer.as_entire_binding() }, // 新規
            ],
        });

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups((self.width + 15) / 16, (self.height + 15) / 16, 1);

        &self.view_rgba
    }
}