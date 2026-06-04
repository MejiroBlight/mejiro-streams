mod commands;
mod decoder;
mod state;
mod gpu;
mod worker_thread;
pub mod export;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::RwLock;

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
            if gpu_ctx.is_err() {
                panic!("Failed to initialize GPU context: {:?}", gpu_ctx.err());
            }
            app.manage(AppState{
                gpu_ctx: Arc::new(gpu_ctx.unwrap()),
                worker_thread: Arc::new(RwLock::new(None)),
                timeline_state: Arc::new(RwLock::new(state::TimelineState::default())),
            });
            Ok(())
        })
        // --- IPC commands -----------------------------------------------------
        .invoke_handler(commands::commands_builder().invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
