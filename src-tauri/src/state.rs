use crate::gpu::{self, context::GpuContext};

use std::{path::PathBuf, sync::Mutex};

/// Persistent FFmpeg decoder context to avoid re-initializing on every frame.
pub struct PersistentDecoder {
    pub ictx: ffmpeg_next::format::context::Input,
    pub stream_index: usize,
    pub decoder: ffmpeg_next::codec::decoder::Video,
}

/// Shared application state passed to every command and protocol handler.
pub struct AppState {
    /// Current seek position in milliseconds.
    pub current_time: Mutex<u64>,
    /// Path to the currently loaded video file.
    pub video_path: Mutex<Option<PathBuf>>,
    /// Persistent FFmpeg context (re-initialized when video changes).
    pub ffmpeg_ctx: Mutex<Option<PersistentDecoder>>,
    /// GPU renderer instance (None if GPU initialization failed, in which case we fall back to CPU path).
    pub gpu_ctx: Mutex<Option<GpuContext>>,
    /// GPU pipelines (flip, format conversion, readback) - initialized lazily when GPU is available.
    pub pipelines: Mutex<Option<Pipelines>>,
}

pub struct Pipelines {
    pub upload: gpu::uploader::Uploader,
    pub flip: gpu::flip_filter::FlipFilter,
    pub rgba_to_nv12: gpu::rgba_to_nv12::RgbaToNv12Converter,
    pub nv12_to_rgba: gpu::nv12_to_rgba::Nv12RgbaConverter,
    pub read_pixel: gpu::read_pixel::ReadPixel,
}