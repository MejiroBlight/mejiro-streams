use std::{path::Path};

use ffmpeg_next::{ threading::Config};

/// Decoded video frame as RGBA pixels.
pub struct FrameData {
    pub width: u32,
    pub height: u32,
    pub rgba_pixels: Vec<u8>,
}

/// Video metadata.
pub struct VideoInfo {
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
}

/// Initialize FFmpeg context for a new video file.
/// Returns a tuple of (Input context, stream_index, Video decoder, VideoInfo).
pub fn init_ffmpeg(
    path: &Path,
) -> Result<
    (
        ffmpeg_next::format::context::Input,
        usize,
        ffmpeg_next::codec::decoder::Video,
        VideoInfo,
    ),
    String,
> {
    ffmpeg_next::init().map_err(|e| e.to_string())?;

    ffmpeg_next::log::set_level(ffmpeg_next::log::Level::Quiet);

    let ictx = ffmpeg_next::format::input(&path).map_err(|e| e.to_string())?;

    let stream = ictx
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .ok_or_else(|| "No video stream found".to_string())?;

    let stream_index = stream.index();
    let ctx = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())
        .map_err(|e| e.to_string())?;
    let mut decoder = ctx.decoder().video().map_err(|e| e.to_string())?;
    decoder.set_threading(Config{
        kind: ffmpeg_next::threading::Type::Frame,
        count: 0, // auto-detect
    });
    let width = decoder.width();
    let height = decoder.height();

    let duration_us = ictx.duration();
    let duration_ms = if duration_us > 0 {
        (duration_us / 1000) as u64
    } else {
        let tb = stream.time_base();
        let sd = stream.duration();
        if sd > 0 && tb.denominator() > 0 {
            (sd as f64 * f64::from(tb) * 1000.0) as u64
        } else {
            0
        }
    };

    Ok((
        ictx,
        stream_index,
        decoder,
        VideoInfo {
            duration_ms,
            width,
            height,
        },
    ))
}

/// Decode a frame using persistent FFmpeg context.
/// This avoids re-initializing FFmpeg on every seek.
pub fn decode_frame_persistent(
    ictx: &mut ffmpeg_next::format::context::Input,
    stream_index: usize,
    decoder: &mut ffmpeg_next::codec::decoder::Video,
    time_ms: u64,
) -> Result<FrameData, String> {
    use std::time::Instant;
    let start = Instant::now();

    // Get time_base from the stream
    let stream = ictx
        .streams()
        .nth(stream_index)
        .ok_or_else(|| "Stream not found".to_string())?;
    let time_base = stream.time_base();
    let time_base_f64 = f64::from(time_base);

    let width = decoder.width();
    let height = decoder.height();

    let mut scaler = ffmpeg_next::software::scaling::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg_next::format::Pixel::RGBA,
        width,
        height,
        ffmpeg_next::software::scaling::Flags::BILINEAR,
    )
    .map_err(|e| e.to_string())?;

    // Seek to target position
    if time_ms > 0 {
        let seek_ts_us = time_ms as i64 * 1000;
        ictx.seek(seek_ts_us, ..seek_ts_us)
            .map_err(|e| e.to_string())?;
        eprintln!("[decoder] Seeked to {}ms ({}us)", time_ms, seek_ts_us);
    }
    decoder.flush();
    let mut raw_frame = ffmpeg_next::frame::Video::empty();
    let mut rgba_frame = ffmpeg_next::frame::Video::empty();

    for (stream, packet) in ictx.packets(){

        if stream.index() == stream_index{

            match decoder.send_packet(&packet) {
                Ok(_) => {
                    continue;
                }
                Err(ref e) if e == &ffmpeg_next::Error::Other { errno: ffmpeg_next::util::error::EAGAIN } => {
                    
                    while decoder.receive_frame(&mut raw_frame).is_ok(){
                        
                        let frame_ms = (raw_frame.pts().unwrap_or(0) as f64 * time_base_f64 * 1000.0) as u64;

                        if frame_ms >= time_ms {
                            scaler.run(&raw_frame, &mut rgba_frame).map_err(|e| e.to_string())?;
                            let result = extract_pixels(&rgba_frame, width, height);
                            let total_ms = start.elapsed().as_millis();
                            eprintln!("[decoder] Persistent decode took {}ms", total_ms);
                            return Ok(result);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[decoder] Error sending packet: {e}");
                    continue;
                }
            }
        }
    }

    let _ = decoder.send_eof();
    while decoder.receive_frame(&mut raw_frame).is_ok() {
        scaler
            .run(&raw_frame, &mut rgba_frame)
            .map_err(|e| e.to_string())?;
        let result = extract_pixels(&rgba_frame, width, height);
        let total_ms = start.elapsed().as_millis();
        eprintln!("[decoder] Persistent decode (flush) took {}ms", total_ms);
        return Ok(result);
    }

    Err("Failed to decode any frame".to_string())
}

/// Returns metadata (duration, dimensions) for the given video file.
/// Falls back to slow path if context is not available.
pub fn get_video_info(path: &Path) -> Result<VideoInfo, String> {
    let (_, _, _, info) = init_ffmpeg(path)?;
    Ok(info)
}

/// Decode a single frame at `time_ms` milliseconds from the given video file.
/// Falls back to slow path if context is not available.
pub fn decode_frame(path: &Path, time_ms: u64) -> Result<FrameData, String> {
    let (mut ictx, stream_index, mut decoder, _info) = init_ffmpeg(path)?;
    decode_frame_persistent(&mut ictx, stream_index, &mut decoder, time_ms)
}

fn extract_pixels(frame: &ffmpeg_next::frame::Video, width: u32, height: u32) -> FrameData {
    let data = frame.data(0);
    let stride = frame.stride(0);
    let row_bytes = width as usize * 4;

    let mut pixels = Vec::with_capacity(row_bytes * height as usize);
    for y in 0..height as usize {
        let row_start = y * stride;
        let row_end = row_start + row_bytes;
        pixels.extend_from_slice(&data[row_start..row_end]);
    }

    FrameData {
        width,
        height,
        rgba_pixels: pixels,
    }
}
