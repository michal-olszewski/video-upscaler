use inference_runtime::ExecutionBackend;
use serde::{Deserialize, Serialize};
use video_io::OutputEncodingProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Analyzing,
    Processing,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

use crate::interpolation::InterpolationMode;
use crate::temporal::TemporalMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpscaleJobRequest {
    pub id: String,
    pub input_path: String,
    pub output_path: String,
    pub model_id: String,
    pub backend: ExecutionBackend,
    pub tile_size: Option<usize>,
    pub tile_padding: Option<usize>,
    pub film_grain: f32,
    #[serde(default)]
    pub temporal_mode: Option<TemporalMode>,
    #[serde(default)]
    pub interpolation_mode: Option<InterpolationMode>,
    #[serde(default)]
    pub deinterlace_mode: Option<video_io::DeinterlaceMode>,
    #[serde(default)]
    pub denoise_strength: Option<f32>,
    #[serde(default)]
    pub trim_start: Option<f64>,
    #[serde(default)]
    pub trim_end: Option<f64>,
    pub encoding_profile: OutputEncodingProfile,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub job_id: String,
    pub state: JobState,
    pub current_frame: u64,
    pub total_frames: u64,
    pub percentage: f32,
    pub current_fps: f64,
    pub eta_seconds: u64,
    pub error_message: Option<String>,
}

impl JobProgress {
    pub fn new(job_id: String, total_frames: u64) -> Self {
        Self {
            job_id,
            state: JobState::Queued,
            current_frame: 0,
            total_frames,
            percentage: 0.0,
            current_fps: 0.0,
            eta_seconds: 0,
            error_message: None,
        }
    }
}
