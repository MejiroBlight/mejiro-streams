use std::num::NonZeroU32;
use super::context::GpuContext;


pub struct ReadPixel {
    width: u32,
    height: u32,
    
    // Yプレーン用とUVプレーン用の、CPU読み出し用バッファ
    buffer_y: wgpu::Buffer,
    buffer_uv: wgpu::Buffer,
    
    // wgpu側のパディングを含んだ1行のバイト数
    padded_bytes_per_row_y: u32,
    padded_bytes_per_row_uv: u32,
    
    // パディングを剥ぎ取った、本来のYUVバイナリの各サイズ
    unpadded_size_y: usize,
    unpadded_size_uv: usize,
}

impl ReadPixel {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let device = &ctx.device;
        // --- 1. wgpuのアライメント（256の倍数）を計算 ---
        // Yは1ピクセル1バイト（R8Unorm）
        let bytes_per_row_y = width;
        let padded_bytes_per_row_y = (bytes_per_row_y + 255) & !255; // 255を足して下位8bitを落とす（256倍数化）
        
        // UVは縦横半分だが、1ピクセル2バイト（Rg8Unorm）なので、1行のバイト幅は width/2 * 2 = width と同じになる
        let bytes_per_row_uv = width; 
        let padded_bytes_per_row_uv = (bytes_per_row_uv + 255) & !255;

        // --- 2. GPUからコピーを受け取るバッファの作成 ---
        let buffer_y = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Read Buffer Y"),
            size: (padded_bytes_per_row_y * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let buffer_uv = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Read Buffer UV"),
            size: (padded_bytes_per_row_uv * (height / 2)) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // パディングがない本来のYUVのバイトサイズ
        let unpadded_size_y = (width * height) as usize;
        let unpadded_size_uv = (width * (height / 2)) as usize;

        Self {
            width, height,
            buffer_y, buffer_uv,
            padded_bytes_per_row_y, padded_bytes_per_row_uv,
            unpadded_size_y, unpadded_size_uv,
        }
    }

    /// 1. GPUに対して「テクスチャからバッファへコピーせよ」という命令をエンコードする
    pub fn enqueue_copy(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture_y: &wgpu::Texture,
        texture_uv: &wgpu::Texture,
    ) {
        // Yプレーンのコピー
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture { texture: texture_y, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyBuffer { buffer: &self.buffer_y, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(self.padded_bytes_per_row_y), rows_per_image: None } },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );

        // UVプレーンのコピー
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture { texture: texture_uv, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::ImageCopyBuffer { buffer: &self.buffer_uv, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(self.padded_bytes_per_row_uv), rows_per_image: None } },
            wgpu::Extent3d { width: self.width / 2, height: self.height / 2, depth_or_array_layers: 1 },
        );
    }

    /// 2. GPUの計算終了を待ち、パディングを剥ぎ取って1本の連続したNV12バイナリ（Vec<u8>）にして取り出す
    pub async fn download_pixels(&self, ctx: &GpuContext) -> Vec<u8> {
        let device = &ctx.device;
        // バッファのスライスを取得
        let slice_y = self.buffer_y.slice(..);
        let slice_uv = self.buffer_uv.slice(..);

        // 非同期でマップ要求（CPUから読める状態にするリクエスト）
        let (tx_y, mut rx_y) = tokio::sync::oneshot::channel();
        let (tx_uv, mut rx_uv) = tokio::sync::oneshot::channel();

        slice_y.map_async(wgpu::MapMode::Read, move |res| { let _ = tx_y.send(res); });
        slice_uv.map_async(wgpu::MapMode::Read, move |res| { let _ = tx_uv.send(res); });

        // GPUの処理を強制的に進めて待機
        device.poll(wgpu::Maintain::Wait);

        // マップ完了の確認
        rx_y.await.expect("Yバッファのマップに失敗しました").expect("Yバッファのマップに失敗しました");
        rx_uv.await.expect("UVバッファのマップに失敗しました").expect("UVバッファのマップに失敗しました");

        // 最終成果物を格納する、パディングなしのぴったりサイズのVecを確保
        let mut final_nv12 = vec![0u8; self.unpadded_size_y + self.unpadded_size_uv];

        {
            // GPUメモリのデータにアクセス
            let data_y = slice_y.get_mapped_range();
            let data_uv = slice_uv.get_mapped_range();

            // --- Yプレーンのコピー（パディング除去） ---
            let src_stride_y = self.padded_bytes_per_row_y as usize;
            let dst_stride_y = self.width as usize;
            for row in 0..self.height as usize {
                let src_start = row * src_stride_y;
                let dst_start = row * dst_stride_y;
                final_nv12[dst_start..dst_start + dst_stride_y]
                    .copy_from_slice(&data_y[src_start..src_start + dst_stride_y]);
            }

            // --- UVプレーンのコピー（パディング除去） ---
            let src_stride_uv = self.padded_bytes_per_row_uv as usize;
            let dst_stride_uv = self.width as usize; // 横幅/2 * 2バイト = width
            let uv_offset_in_final = self.unpadded_size_y; // Yデータのすぐ後ろに配置
            
            for row in 0..(self.height / 2) as usize {
                let src_start = row * src_stride_uv;
                let dst_start = uv_offset_in_final + (row * dst_stride_uv);
                final_nv12[dst_start..dst_start + dst_stride_uv]
                    .copy_from_slice(&data_uv[src_start..src_start + dst_stride_uv]);
            }
        } // ここで data_y と data_uv のスコープが終わり、アンマップ可能になる

        // 次のフレームのためにバッファをアンマップ（ロック解除）
        self.buffer_y.unmap();
        self.buffer_uv.unmap();

        final_nv12
    }
}