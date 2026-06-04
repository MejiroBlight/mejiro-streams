use std::{path::Path};

use ffmpeg_next::{ threading::Config};

use crate::commands::VideoInfo;

/// Decoded video frame as RGBA pixels.
pub struct FrameData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub struct Decorder{
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub ictx: ffmpeg_next::format::context::Input,
    pub stream_index: usize,
    pub decoder: ffmpeg_next::codec::decoder::Video,
    pub path: String,
}

impl Decorder{
    pub fn new(path: &Path) -> Result<Self, String> {
        ffmpeg_next::init().map_err(|e| e.to_string())?;
        ffmpeg_next::log::set_level(ffmpeg_next::log::Level::Quiet);

        let ictx = ffmpeg_next::format::input(&path).map_err(|e| e.to_string())?;

        let stream = ictx
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or_else(|| "No video stream found".to_string())?;

        let stream_index = stream.index();

        let ctx = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| e.to_string())?;

        let mut decoder = ctx.decoder().video().map_err(|e| e.to_string())?;
        decoder.set_threading(Config{
            kind: ffmpeg_next::threading::Type::Frame,
            count: 0, // auto-detect
        });

        let width = decoder.width();
        let height = decoder.height();

        let duration_us = ictx.duration();
        let duration_ms = if duration_us > 0 {
            (duration_us / 1000) as u64
        } else {
            let tb = stream.time_base();
            let sd = stream.duration();
            if sd > 0 && tb.denominator() > 0 {
                (sd as f64 * f64::from(tb) * 1000.0) as u64
            } else {
                0
            }
        };

        Ok(Self {
            width,
            height,
            duration_ms,
            ictx,
            stream_index,
            decoder,
            path: path.to_string_lossy().to_string(),
        })
    }

    pub fn decode_frame(&mut self, time_ms: u64) -> Result<FrameData, String> {
        let start = std::time::Instant::now();

        let stream = self
            .ictx
            .streams()
            .nth(self.stream_index)
            .ok_or_else(|| "Stream not found".to_string())?;
        let time_base = stream.time_base();
        let time_base_f64 = f64::from(time_base);

        let mut scaler = ffmpeg_next::software::scaling::Context::get(
            self.decoder.format(),
            self.width,
            self.height,
            ffmpeg_next::format::Pixel::NV12,
            self.width,
            self.height,
            ffmpeg_next::software::scaling::Flags::BILINEAR,
        ).map_err(|e| e.to_string())?;

        if time_ms > 0 {
            let seek_ts_us = time_ms as i64 * 1000;
            self.ictx.seek(seek_ts_us, ..seek_ts_us).map_err(|e| e.to_string())?;
            eprintln!("[decoder] Seeked to {}ms ({}us)", time_ms, seek_ts_us);
        }

        self.decoder.flush();

        let mut raw_frame = ffmpeg_next::frame::Video::empty();
        let mut nv12_frame = ffmpeg_next::frame::Video::empty();

        for (stream, packet) in self.ictx.packets() {
            if stream.index() == self.stream_index {
                match self.decoder.send_packet(&packet) {
                    Ok(_) => continue,
                    Err(ref e) if e == &ffmpeg_next::Error::Other { errno: ffmpeg_next::util::error::EAGAIN } => {
                        while self.decoder.receive_frame(&mut raw_frame).is_ok() {
                            let frame_ms = (raw_frame.pts().unwrap_or(0) as f64 * time_base_f64 * 1000.0) as u64;
                            if frame_ms >= time_ms {
                                scaler.run(&raw_frame, &mut nv12_frame).map_err(|e| e.to_string())?;
                                let result = self.extract_pixels(&nv12_frame);
                                let total_ms = start.elapsed().as_millis();
                                eprintln!("[decoder] Decode took {}ms", total_ms);
                                return Ok(result);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[decoder] Error sending packet: {e}");
                        continue;
                    }
                }
            }
        }

        let _ = self.decoder.send_eof();
        while self.decoder.receive_frame(&mut raw_frame).is_ok() {
            scaler.run(&raw_frame, &mut nv12_frame).map_err(|e| e.to_string())?;
            let result = self.extract_pixels(&nv12_frame);
            let total_ms = start.elapsed().as_millis();
            eprintln!("[decoder] Decode (flush) took {}ms", total_ms);
            return Ok(result);
        }

        Err("Failed to decode any frame".to_string())
    }

    pub fn extract_pixels(&self, frame: &ffmpeg_next::frame::Video) -> FrameData {
        let width = self.width as usize;
        let height = self.height as usize;

        // 256バイト単位のアライメント（1920ピクセルなら2048バイト）
        let aligned_stride = (width + 255) & !255;

        // 必要なサイズを計算
        let y_size = aligned_stride * height;
        let uv_size = aligned_stride * (height / 2);
        let total_size = y_size + uv_size;

        // バッファを確保
        let mut pixels = vec![0u8; total_size];

        // 1. Yプレーンのコピー
        let y_data = frame.data(0);
        let y_stride = frame.stride(0);
        for y in 0..height {
            let src_start = y * y_stride;
            let dest_start = y * aligned_stride;
            // 横幅は width 分だけコピーし、余ったパディング領域は vec! の初期値 0 のまま残す
            pixels[dest_start..dest_start + width].copy_from_slice(&y_data[src_start..src_start + width]);
        }

        // 2. UVプレーンのコピー
        let uv_data = frame.data(1);
        let uv_stride = frame.stride(1);
        let uv_height = height / 2;
        for y in 0..uv_height {
            let src_start = y * uv_stride;
            let dest_start = y_size + (y * aligned_stride);
            pixels[dest_start..dest_start + width].copy_from_slice(&uv_data[src_start..src_start + width]);
        }

        FrameData {
            width: self.width,
            height: self.height,
            pixels, // ★このサイズ(total_size)をそのままGPUバッファ確保に使います
        }
    }

    pub fn get_video_info(&self) -> VideoInfo {
        VideoInfo {
            duration_ms: self.duration_ms,
            width: self.width,
            height: self.height,
            path: self.path.clone(),
        }
    }
}