mod commands;
mod decoder;
pub mod export;
mod gpu;
mod state;
mod worker_thread;

use std::sync::Arc;

use pollster::FutureExt;
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
            app.manage(AppState {
                gpu_ctx: Arc::new(gpu_ctx.unwrap()),
                worker_thread: Arc::new(RwLock::new(None)),
                timeline_state: Arc::new(RwLock::new(state::TimelineState::default())),
            });
            Ok(())
        })
        // --- IPC commands -----------------------------------------------------
        .invoke_handler(commands::commands_builder().invoke_handler())
        .register_asynchronous_uri_scheme_protocol("tauri", move |app, request, responder| {
            let uri = request.uri();
            let path = uri.path();
            let query = uri.query();

            let frame_num = query
                .and_then(|q| q.split('&').find(|s| s.starts_with("num=")))
                .and_then(|s| s.split('=').nth(1))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);

            eprintln!("Received request for frame {frame_num} at path: {path}");

            let state = app.app_handle().state::<AppState>();
            let _ = state
                .worker_thread
                .write()
                .block_on()
                .as_ref()
                .ok_or("Worker thread not initialized".to_string())
                .unwrap()
                .tx
                .send(worker_thread::WorkerMessage::SeekFrame(
                    frame_num as u64,
                    responder,
                ))
                .block_on()
                .map_err(|e| eprintln!("Failed to send message to worker thread: {e}"));
        })
        .on_window_event(|window, e| {
            if let tauri::WindowEvent::Destroyed = e {
                if window.label() == "main" {
                    std::process::exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
