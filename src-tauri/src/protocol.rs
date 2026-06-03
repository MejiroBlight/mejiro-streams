use crate::{decoder, state::AppState};
use tauri::http::{Request, Response};


pub async fn handle(
    request: &Request<Vec<u8>>,
    state: &tauri::State<'_, AppState>,
) -> Response<Vec<u8>> {
    use std::time::Instant;
    let start = Instant::now();
    let uri = request.uri().to_string();
    let time_ms = parse_time_ms(&uri);

    // Try to decode using persistent FFmpeg context
    let decode_start = Instant::now();
    let frame = {
        let mut ctx_guard = state.ffmpeg_ctx.lock().unwrap();
        match ctx_guard.as_mut() {
            Some(ctx) => {
                // Decode using persistent context
                match decoder::decode_frame_persistent(
                    &mut ctx.ictx,
                    ctx.stream_index,
                    &mut ctx.decoder,
                    time_ms,
                ) {
                    Ok(f) => f,
                    Err(e) => return error_response(&format!("Decode error: {e}")),
                }
            }
            None => {
                // No persistent context – fall back to slow path
                let video_path = {
                    let guard = state.video_path.lock().unwrap();
                    match guard.clone() {
                        Some(p) => p,
                        None => return error_response("No video loaded"),
                    }
                };
                match decoder::decode_frame(&video_path, time_ms){
                    Ok(f) => f,
                    Err(e) => return error_response(&format!("Decode error (fallback): {e}")),
                }
            }
        }
    };
    let decode_ms = decode_start.elapsed().as_millis();
    eprintln!("[protocol] Decode took {}ms", decode_ms);

    // Render via wgpu (composite pass)
    let render_start = Instant::now();
    let binary = match (state.gpu_ctx.lock().unwrap().as_mut(), state.pipelines.lock().unwrap().as_mut()) {
        (Some(gpu_ctx), Some(pipelines)) => {
            let mut encoder = gpu_ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Render Encoder"),
            });
            let v = pipelines.upload.upload(gpu_ctx, &frame);
            let rgba_v = pipelines.nv12_to_rgba.execute(gpu_ctx, &mut encoder, v);
            let flipped_v = pipelines.flip.execute(gpu_ctx, &mut encoder, &rgba_v);
            let (out_y, out_uv) = pipelines.rgba_to_nv12.execute(gpu_ctx, &mut encoder, &flipped_v);
            pipelines.read_pixel.enqueue_copy(&mut encoder, &out_y, &out_uv);
            gpu_ctx.queue.submit(Some(encoder.finish()));
            pipelines.read_pixel.download_pixels(gpu_ctx).await
        }
        _ => {
            eprintln!("[protocol] GPU unavailable, falling back to CPU path for rendering");
            frame.nv12_pixels.clone()
        }
    };
    let render_ms = render_start.elapsed().as_millis();
    eprintln!("[protocol] Render took {}ms", render_ms);
    eprintln!("[protocol] Total processing time: {}ms", start.elapsed().as_millis());
    success_response(binary)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_time_ms(uri: &str) -> u64 {
    uri.split('?')
        .nth(1)
        .and_then(|query| {
            query
                .split('&')
                .find(|p| p.starts_with("t="))
                .and_then(|p| p[2..].parse().ok())
        })
        .unwrap_or(0)
}

fn error_response(msg: &str) -> Response<Vec<u8>> {
    eprintln!("[protocol] {msg}");
    Response::builder()
        .status(500)
        .header("Content-Type", "text/plain")
        .body(msg.as_bytes().to_vec())
        .unwrap()
}

fn success_response(data: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("Content-Type", "application/octet-stream")
        .body(data)
        .unwrap()
}
