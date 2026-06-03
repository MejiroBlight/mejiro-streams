mod commands;
mod decoder;
mod protocol;
mod renderer;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

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
    /// Initialised wgpu renderer (None if GPU is unavailable).
    pub renderer: Mutex<Option<renderer::WgpuRenderer>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // --- plugins -----------------------------------------------------------
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // --- setup: initialise GPU renderer ------------------------------------
        .setup(|app| {
            let gpu = pollster::block_on(renderer::WgpuRenderer::new());
            match gpu {
                Ok(r) => {
                    eprintln!("[wgpu] Renderer initialised successfully");
                    app.manage(AppState {
                        current_time: Mutex::new(0),
                        video_path: Mutex::new(None),
                        ffmpeg_ctx: Mutex::new(None),
                        renderer: Mutex::new(Some(r)),
                    });
                }
                Err(e) => {
                    eprintln!("[wgpu] GPU renderer unavailable: {e}. Falling back to CPU path.");
                    app.manage(AppState {
                        current_time: Mutex::new(0),
                        video_path: Mutex::new(None),
                        ffmpeg_ctx: Mutex::new(None),
                        renderer: Mutex::new(None),
                    });
                }
            }
            Ok(())
        })
        // --- custom protocol: video-preview:// --------------------------------
        .register_uri_scheme_protocol("video-preview", |ctx, request| {
            let state = ctx.app_handle().state::<AppState>();
            protocol::handle(&request, &state)
        })
        // --- IPC commands -----------------------------------------------------
        .invoke_handler(tauri::generate_handler![
            commands::seek_frame,
            commands::open_video,
            commands::load_video_path,
            commands::get_current_time,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
