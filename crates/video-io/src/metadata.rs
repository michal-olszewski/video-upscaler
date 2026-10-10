use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStreamInfo {
    pub index: usize,
    pub codec_name: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStreamInfo {
    pub index: usize,
    pub codec_name: String,
    pub width: usize,
    pub height: usize,
    pub frame_rate: f64,
    pub total_frames: Option<u64>,
    pub duration_seconds: f64,
    pub pixel_format: String,
    pub color_space: Option<String>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub file_path: String,
    pub format_name: String,
    pub duration_seconds: f64,
    pub video_stream: VideoStreamInfo,
    pub audio_streams: Vec<AudioStreamInfo>,
}
