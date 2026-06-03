mod commands;
mod decoder;
mod protocol;
mod state;
mod gpu;
mod worker_thread;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // --- plugins -----------------------------------------------------------
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // --- setup: initialise GPU renderer ------------------------------------
        .setup(|app| {
            let gpu_ctx = pollster::block_on(gpu::context::GpuContext::new());
            match gpu_ctx {
                Ok(r) => {
                    eprintln!("[wgpu] Renderer initialised successfully");
                    app.manage( AppState {
                        current_time: Mutex::new(0),
                        video_path: Mutex::new(None),
                        ffmpeg_ctx: Mutex::new(None),
                        gpu_ctx: Mutex::new(Some(r)),
                        pipelines: Mutex::new(None),
                    });
                }
                Err(e) => {
                    eprintln!("[wgpu] GPU renderer unavailable: {e}. Falling back to CPU path.");
                    app.manage(AppState {
                        current_time: Mutex::new(0),
                        video_path: Mutex::new(None),
                        ffmpeg_ctx: Mutex::new(None),
                        gpu_ctx: Mutex::new(None),
                        pipelines: Mutex::new(None),
                    });
                }
            }
            Ok(())
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
