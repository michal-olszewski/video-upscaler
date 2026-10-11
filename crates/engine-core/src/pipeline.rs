use crate::job::{JobProgress, JobState, UpscaleJobRequest};
use anyhow::{Context, Result};
use image_processing::TileSpec;
use models::ModelRegistry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::info;

pub struct UpscalePipeline {
    cancelled: Arc<AtomicBool>,
}

impl UpscalePipeline {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Runs the video upscale pipeline asynchronously, streaming progress updates via the channel
    pub async fn execute(
        &self,
        request: UpscaleJobRequest,
        progress_tx: mpsc::Sender<JobProgress>,
    ) -> Result<()> {
        info!("Starting upscale pipeline for job: {}", request.id);

        let model_desc = ModelRegistry::find_by_id(&request.model_id)
            .context(format!("Model not found: {}", request.model_id))?;

        let tile_size = request.tile_size.unwrap_or(model_desc.default_tile_size);
        let tile_padding = request
            .tile_padding
            .unwrap_or(model_desc.default_padding);

        let tiling_spec = TileSpec {
            tile_size,
            padding: tile_padding,
            scale: model_desc.scale_factor as usize,
        };
        let _tiling_engine = image_processing::TilingEngine::new(tiling_spec);

        let model_path = format!("models/cache/{}", model_desc.filename);
        let _session = inference_runtime::InferenceSessionWrapper::new(model_path, request.backend.clone())?;

        // Total frames estimated or parsed from video metadata
        let total_frames = 100u64; // Placeholder for initialization
        let mut progress = JobProgress::new(request.id.clone(), total_frames);
        progress.state = JobState::Processing;

        let start_time = Instant::now();

        for frame_idx in 0..total_frames {
            if self.cancelled.load(Ordering::SeqCst) {
                progress.state = JobState::Cancelled;
                let _ = progress_tx.send(progress).await;
                info!("Pipeline cancelled by user.");
                return Ok(());
            }

            // Pipeline Step:
            // 1. Decode Frame (via video-io)
            // 2. Preprocess / Normalize to NCHW
            // 3. Tile & Run Inference via session.run_inference()
            // 4. Stitch with Hann Window weights
            // 5. Apply film grain if requested
            // 6. Encode Frame (via video-io hardware encoder)

            let elapsed = start_time.elapsed().as_secs_f64();
            let fps = if elapsed > 0.0 {
                (frame_idx + 1) as f64 / elapsed
            } else {
                0.0
            };
            let remaining = total_frames.saturating_sub(frame_idx + 1);
            let eta = if fps > 0.0 {
                (remaining as f64 / fps) as u64
            } else {
                0
            };

            progress.current_frame = frame_idx + 1;
            progress.percentage = (progress.current_frame as f32 / total_frames as f32) * 100.0;
            progress.current_fps = fps;
            progress.eta_seconds = eta;

            let _ = progress_tx.send(progress.clone()).await;

            // Yield to async executor
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        progress.state = JobState::Completed;
        let _ = progress_tx.send(progress).await;

        info!("Job {} completed successfully in {:?}", request.id, start_time.elapsed());
        Ok(())
    }
}
