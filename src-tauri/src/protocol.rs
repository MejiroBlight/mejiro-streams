use crate::{decoder, AppState};
use image::{codecs::jpeg::JpegEncoder, ImageEncoder};
use tauri::http::{Request, Response};

/// Handle a request to `video-preview://localhost/frame?t=<ms>`.
///
/// Pipeline: seek → FFmpeg decode → wgpu composite → JPEG encode → HTTP response.
pub fn handle(
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
    let rendered = {
        let mut guard = state.renderer.lock().unwrap();
        match guard.as_mut() {
            Some(r) => match r.render_frame(&frame.rgba_pixels, frame.width, frame.height) {
                Ok(pixels) => pixels,
                Err(e) => return error_response(&format!("Render error: {e}")),
            },
            None => {
                // wgpu not available – pass raw pixels directly
                frame.rgba_pixels.clone()
            }
        }
    };
    let render_ms = render_start.elapsed().as_millis();
    eprintln!("[protocol] Render took {}ms", render_ms);

    // Encode to JPEG
    let encode_start = Instant::now();
    let result = match encode_jpeg(&rendered, frame.width, frame.height) {
        Ok(jpeg) => {
            let encode_ms = encode_start.elapsed().as_millis();
            let total_ms = start.elapsed().as_millis();
            eprintln!("[protocol] JPEG encode took {}ms | Total: {}ms (decode={}ms, render={}ms)", 
                encode_ms, total_ms, decode_ms, render_ms);
            Response::builder()
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "no-store")
                .body(jpeg)
                .unwrap()
        },
        Err(e) => error_response(&format!("JPEG encode error: {e}")),
    };
    result
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

fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    // Convert RGBA → RGB (JPEG does not support alpha)
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or("Failed to build RgbaImage")?;
    let rgb_img = image::DynamicImage::ImageRgba8(img).to_rgb8();

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 70)
        .write_image(
            rgb_img.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| e.to_string())?;

    Ok(out)
}
