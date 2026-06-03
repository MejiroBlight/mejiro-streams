
enum StreamCommand {
    SeekFrame(u64), // frame number
    Pause,
    Play,
    LoadVideo(String), // video path
}

pub async fn worker_thread_loop(mut receiver: tokio::sync::mpsc::Receiver<StreamCommand>) {
    while let Some(command) = receiver.recv().await {
        match command {
            StreamCommand::SeekFrame(frame_num) => {
                // Handle seeking to the specified frame
            }
            StreamCommand::Pause => {
                // Handle pausing the stream
            }
            StreamCommand::Play => {
                // Handle resuming playback
            }
            StreamCommand::LoadVideo(path) => {
                // Handle loading a new video
            }
        }
    }
}

