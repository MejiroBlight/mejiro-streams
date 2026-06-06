use crate::{commands::{CommandResponse}, gpu::context::GpuContext, worker_thread::WorkerMessage};

use std::{sync::Arc};
use tauri::ipc::Channel;
use tokio::sync::RwLock;

/// Shared application state passed to every command and protocol handler.
pub struct AppState {
    pub gpu_ctx: Arc<GpuContext>,
    pub worker_thread: Arc<RwLock<Option<ThreadHandler<WorkerMessage, CommandResponse>>>>,
    pub timeline_state: Arc<RwLock<TimelineState>>,
}

#[derive(Default)]
pub struct TimelineState {
    pub current_time: u64,
    pub video_info: Option<crate::commands::VideoInfo>,
}

pub struct ThreadHandler<S, R>{
    pub tx: tokio::sync::mpsc::Sender<S>,
    pub rx: tokio::sync::mpsc::Receiver<R>,
    pub handle: tauri::async_runtime::JoinHandle<()>,
}