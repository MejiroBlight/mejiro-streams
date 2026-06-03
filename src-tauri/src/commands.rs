use crate::{decoder, gpu::{self, uploader::InputFormat}, state::{AppState, PersistentDecoder, Pipelines}};
use serde::Serialize;
use tauri_specta::{Builder, collect_commands};
use std::path::PathBuf;
use tauri::AppHandle;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Serialize, specta::Type)]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Update the internal seek position and return the custom-protocol URL that
/// the frontend should assign to the `<img>` src attribute.
#[tauri::command]
#[specta::specta]
pub fn seek_frame(time_ms: u64, state: tauri::State<'_, AppState>) -> Result<String, String> {
    *state.current_time.lock().unwrap() = time_ms;

    // On Windows, Tauri custom protocols are served as http://<scheme>.localhost/
    // On macOS/Linux, they use <scheme>://localhost/
    #[cfg(target_os = "windows")]
    let url = format!("http://video-preview.localhost/frame?t={time_ms}");
    #[cfg(not(target_os = "windows"))]
    let url = format!("video-preview://localhost/frame?t={time_ms}");
    Ok(url)
}

/// Load a video from a file path supplied directly (e.g. drag-and-drop).
#[tauri::command]
#[specta::specta]
pub fn load_video_path(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<VideoInfo, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }

    // Initialize FFmpeg context for the new file
    let (ictx, stream_index, decoder, info) = decoder::init_ffmpeg(&p)?;

    // Persist context in shared state
    *state.video_path.lock().unwrap() = Some(p);
    *state.current_time.lock().unwrap() = 0;
    *state.ffmpeg_ctx.lock().unwrap() = Some(PersistentDecoder {
        ictx,
        stream_index,
        decoder,
    });

    if let Some(gpu_ctx) = state.gpu_ctx.lock().unwrap().as_mut() {
        state.pipelines.lock().unwrap().replace(Pipelines {
            upload: gpu::uploader::Uploader::new(gpu_ctx, info.width, info.height, InputFormat::Nv12),
            flip: gpu::flip_filter::FlipFilter::new(gpu_ctx, info.width, info.height),
            rgba_to_nv12: gpu::rgba_to_nv12::RgbaToNv12Converter::new(gpu_ctx, info.width, info.height),
            nv12_to_rgba: gpu::nv12_to_rgba::Nv12RgbaConverter::new(gpu_ctx, info.width, info.height),
            read_pixel: gpu::read_pixel::ReadPixel::new(gpu_ctx, info.width, info.height),
        });
    }

    Ok(VideoInfo {
        duration_ms: info.duration_ms,
        width: info.width,
        height: info.height,
        path,
    })
}

/// Return the current seek position (ms).
#[tauri::command]
#[specta::specta]
pub fn get_current_time(state: tauri::State<'_, AppState>) -> u64 {
    *state.current_time.lock().unwrap()
}

pub fn commands_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        get_current_time,
        load_video_path,
        seek_frame,
    ])
}
