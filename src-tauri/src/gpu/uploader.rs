use crate::decoder;

use super::context::GpuContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputFormat {
    Nv12,
    Rgba,
}

pub struct Uploader {
    width: u32,
    height: u32,
    format: InputFormat,

    pub textures: Vec<wgpu::Texture>,
    pub views: Vec<wgpu::TextureView>,
}

impl Uploader{
    pub fn new(ctx: &GpuContext, width: u32, height: u32, format: InputFormat) -> Self {
        let device = &ctx.device;
        let mut textures: Vec<wgpu::Texture> = Vec::new();
        let mut views: Vec<wgpu::TextureView> = Vec::new();
        match format {
            InputFormat::Nv12 => {
                // 1. Yプレーン (等倍, R8Unorm)
                let tex_y = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Generic Input Y"),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                // 2. UVプレーン (縦横半分, Rg8Unorm)
                let tex_uv = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Generic Input UV"),
                    size: wgpu::Extent3d { width: width / 2, height: height / 2, depth_or_array_layers: 1 },
                    mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rg8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                views.push(tex_y.create_view(&Default::default()));
                views.push(tex_uv.create_view(&Default::default()));
                textures.push(tex_y);
                textures.push(tex_uv);
            }
            InputFormat::Rgba => {
                // 1. RGBAプレーン (等倍, Rgba8Unorm)
                let tex_rgba = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Generic Input RGBA"),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                views.push(tex_rgba.create_view(&Default::default()));
                textures.push(tex_rgba);
            }
        }
        Self {
            width,
            height,
            format,
            textures,
            views,
        }
    }

    /// メインのアップロードメソッド（引数で指定された通りに処理する）
    pub fn upload(
        &mut self,
        ctx: &GpuContext,
        frame: &decoder::FrameData,
    )  -> &[wgpu::TextureView] {
        let queue = &ctx.queue;
        let width = frame.width;
        let height = frame.height;
        let byte_per_row = match self.format {
            InputFormat::Nv12 => width, // YもUVも1ピクセルあたり1バイト（UVは2チャンネルで2バイトだが、横幅半分なので結果的に同じ）
            InputFormat::Rgba => width * 4, // RGBAは1ピクセル4バイト
        };
        let row_per_image = height;

        // 2. フォーマットに応じてデータを流し込む
        match self.format {
            InputFormat::Nv12 => {
                let y_size = (width * height) as usize; // Yプレーンのサイズ
                let uv_size = (width * height / 2) as usize; // UVプレ
                // Yプレーンの転送
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.textures[0],
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &frame.nv12_pixels[..y_size], // Yプレーンのデータは先頭からy_sizeバイト分
                    wgpu::ImageDataLayout {
                        offset: 0, 
                        bytes_per_row: Some(byte_per_row),
                        rows_per_image: Some(row_per_image),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
                // UVプレーンの転送
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.textures[1],
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &frame.nv12_pixels[y_size..y_size + uv_size], // UVプレーンのデータはYプレーンの後ろからuv_sizeバイト分
                    wgpu::ImageDataLayout {
                        offset: 0, 
                        bytes_per_row: Some(byte_per_row / 2), // UVは横幅半分
                        rows_per_image: Some(row_per_image / 2), // UVは縦幅半分
                    },
                    wgpu::Extent3d { width: width / 2, height: height / 2, depth_or_array_layers: 1 },
                );
            }
            InputFormat::Rgba => {
                // RGBAプレーンの転送 (RGBAの場合、通常プレーン0に全てのデータが入っています)
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.textures[0],
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    [].as_ref(), // ここはRGBAデータのスライスを指定する必要があります。例: &frame.rgba_pixels
                    wgpu::ImageDataLayout {
                        offset: 0, 
                        bytes_per_row: Some(byte_per_row),
                        rows_per_image: Some(row_per_image),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
            }
        }
        &self.views
    }
}