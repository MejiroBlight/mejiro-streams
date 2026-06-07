use crate::{state::AppState, worker_thread};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri_specta::{Builder, collect_commands};
use std::path::PathBuf;

pub enum CommandResponse {
    VideoInfo(Option<VideoInfo>),
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Serialize, specta::Type, Clone)]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load a video from a file path supplied directly (e.g. drag-and-drop).
#[tauri::command]
#[specta::specta]
pub async fn load_video_path(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<VideoInfo, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }

    state.worker_thread.write().await.as_ref()
        .ok_or("Worker thread not initialized".to_string())?
        .tx
        .send(worker_thread::WorkerMessage::LoadVideo(p))
        .await.map_err(|e| format!("Failed to send message to worker thread: {e}"))?;

    match tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            if let Some(response) = state.worker_thread.write().await.as_mut().unwrap().rx.recv().await {
                match response {
                    CommandResponse::VideoInfo(info) => {
                        if let Some(info) = info {
                            return Ok(info);
                        } else {
                            return Err("Failed to load video: No video info returned".to_string());
                        }
                    }
                }
            } else {
                return Err("Worker thread channel closed".to_string());
            }
        }
    }).await {
        Ok(result) => result,
        Err(_) => Err("Timed out waiting for worker thread response".to_string()),
    }
}

/// Return the current seek position (ms).
#[tauri::command]
#[specta::specta]
pub async fn get_current_time(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    Ok(state.timeline_state.read().await.current_time)
}

#[tauri::command]
#[specta::specta]
pub async fn start_frame_server(state: tauri::State<'_, AppState>) ->Result<String, String> {
    if state.worker_thread.read().await.is_some() {
        return Err("Worker thread already running".to_string());
    }
    worker_thread::FrameServer::start(state).await;
    Ok("Frame server started".to_string())
}

pub fn commands_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        get_current_time,
        load_video_path,
        start_frame_server,
    ])
}
