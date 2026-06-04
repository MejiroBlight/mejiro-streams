use crate::decoder;

use super::context::GpuContext;

pub struct Uploader {
    width: u32,
    height: u32,

    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Uploader{
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;
        // 1. RGBAプレーン (等倍, Rgba8Unorm)
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Generic Input RGBA"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());

        Self {
            width,
            height,
            texture,
            view
        }
    }

    /// メインのアップロードメソッド（引数で指定された通りに処理する）
    pub fn upload(
        &mut self,
        ctx: &GpuContext,
        frame: &decoder::FrameData,
    )  -> &wgpu::TextureView {
        let queue = &ctx.queue;
        let width = self.width;
        let height = self.height;
        // RGBAの場合は、単純に全体をRGBAテクスチャに書き込む
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture, // RGBAテクスチャ
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4), // RGBAは1ピクセル4バイト
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
        &self.view
    }
}