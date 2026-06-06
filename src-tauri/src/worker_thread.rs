use std::{path::PathBuf, sync::Arc};

use tauri::{UriSchemeResponder, http::response, http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE}};
use tokio::sync::RwLock;

use crate::{commands::CommandResponse, decoder, gpu, state::{AppState, ThreadHandler, TimelineState}};


pub enum WorkerMessage {
    SeekFrame(u64, UriSchemeResponder), // frame number
    LoadVideo(PathBuf), // video path
}

pub struct FrameServer {
    rx: tokio::sync::mpsc::Receiver<WorkerMessage>,
    tx: tokio::sync::mpsc::Sender<CommandResponse>,
    ffmpeg_ctx: Option<decoder::Decorder>,
    timeline_state: Arc<RwLock<TimelineState>>,
    gpu_ctx: Arc<gpu::context::GpuContext>,
    pipelines: Option<Pipelines>,
}

pub struct Pipelines {
    pub nv12_upload: gpu::nv12_uploader::Nv12Uploader,
    pub flip: gpu::flip_filter::FlipFilter,
    pub read_pixel: gpu::read_pixel::RgbaToNv12ComputeConverter,
}

impl FrameServer {

    pub async fn start(state: tauri::State<'_, AppState>){
        let (main_tx, worker_rx) = tokio::sync::mpsc::channel(100);
        let (worker_tx, main_rx) = tokio::sync::mpsc::channel(100);
        let timeline_state = state.timeline_state.clone();
        let gpu_ctx = state.gpu_ctx.clone();
        let handle = tauri::async_runtime::spawn(async move{
            let mut server = FrameServer {
                rx: worker_rx,
                tx: worker_tx,
                timeline_state,
                gpu_ctx,
                ffmpeg_ctx: None,
                pipelines: None,
            };
            loop {
                server.thread_loop().await;
            }
        });
        state.worker_thread.write().await.replace(ThreadHandler {
            tx: main_tx,
            rx: main_rx,
            handle,
        });
        eprintln!("Frame server started");
    }

    async fn thread_loop(&mut self) {
        while let Some(message) = self.rx.recv().await {
            match message {
                WorkerMessage::SeekFrame(time_ms, responder) => {
                    self.timeline_state.write().await.current_time = time_ms;
                    let result = self.decode_and_send_frame(time_ms).await;
                    match result {
                        Ok(frame_data) => {
                            let response = response::Builder::new()
                                .status(200)
                                .header(CONTENT_TYPE, "application/octet-stream")
                                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "http://localhost:1420") 
                                .body(frame_data)
                                .unwrap();
                            responder.respond(response);
                        },
                        Err(e) => {
                            eprintln!("{e}");
                            let response = response::Builder::new()
                                .status(500)
                                .header(CONTENT_TYPE, "text/plain")
                                .header(ACCESS_CONTROL_ALLOW_ORIGIN, "http://localhost:1420") 
                                .body(e.into_bytes())
                                .unwrap();
                            responder.respond(response);
                        }
                    }
                },
                WorkerMessage::LoadVideo(path) => {
                    match decoder::Decorder::new(&path) {
                        Ok(ctx) => {
                            self.ffmpeg_ctx.replace(ctx);
                            let info = self.ffmpeg_ctx.as_ref().unwrap().get_video_info();
                            self.timeline_state.write().await.video_info.replace(info.clone());
                            self.init_pipelines(info.width, info.height).await;
                            let _ = self.tx.send(CommandResponse::VideoInfo(Some(info))).await;
                        },
                        Err(e) => {
                            eprintln!("Error loading video: {e}");
                            let _ = self.tx.send(CommandResponse::VideoInfo(None)).await;
                        },
                    }
                }
            }
        }
    }

    async fn init_pipelines(&mut self, width: u32, height: u32){
        let ctx = &self.gpu_ctx;
        self.pipelines.replace(Pipelines {
            nv12_upload: gpu::nv12_uploader::Nv12Uploader::new(ctx, width, height),
            flip: gpu::flip_filter::FlipFilter::new(ctx, width, height),
            read_pixel: gpu::read_pixel::RgbaToNv12ComputeConverter::new(ctx, width, height),
        });
    }

    async fn decode_and_send_frame(&mut self, time_ms: u64) -> Result<Vec<u8>, String> {
        match (&mut self.ffmpeg_ctx, &mut self.pipelines) {
            (Some(decoder), Some(pipelines)) => {
                match decoder.decode_frame(time_ms) {
                    Ok(frame) => {
                        let mut encoder = self.gpu_ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Frame Render Encoder"),
                        });
                        let v = pipelines.nv12_upload.upload(&self.gpu_ctx,&mut encoder, &frame);
                        let flipped_v = pipelines.flip.execute(&self.gpu_ctx, &mut encoder, &v);
                        self.gpu_ctx.queue.submit(Some(encoder.finish()));
                        pipelines.read_pixel.process_and_download(&self.gpu_ctx, flipped_v).await.map_err(|e| e.to_string())
                    },
                    Err(e) => Err(format!("Error decoding frame: {e}")),
                }
            },
            _ => Err("Decoder or pipelines not initialized".to_string()),
        }
    }
}

