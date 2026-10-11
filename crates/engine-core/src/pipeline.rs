use crate::job::{JobProgress, JobState, UpscaleJobRequest};
use anyhow::{Context, Result};
use image_processing::{ColorPipeline, TileSpec, TilingEngine};
use inference_runtime::InferenceSessionWrapper;
use models::ModelRegistry;
use ndarray::Array4;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::info;
use video_io::{MediaProbe, VideoDemuxer, VideoMuxer};

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

        let model_path = format!("models/cache/{}", model_desc.filename);
        let session = InferenceSessionWrapper::new(&model_path, request.backend.clone())?;
        let scale = model_desc.scale_factor as usize;

        // Probe input media metadata
        let metadata = MediaProbe::probe_file(&request.input_path)
            .context("Failed to probe input video metadata")?;

        let in_w = metadata.video_stream.width;
        let in_h = metadata.video_stream.height;
        let out_w = in_w * scale;
        let out_h = in_h * scale;
        let fps = metadata.video_stream.frame_rate;
        let total_frames = metadata.video_stream.total_frames.unwrap_or(1).max(1);

        info!(
            "Job {}: Upscaling {}x{} -> {}x{} @ {:.2} fps ({} total frames) using {}",
            request.id, in_w, in_h, out_w, out_h, fps, total_frames, model_desc.name
        );

        let tile_size = request.tile_size.unwrap_or(model_desc.default_tile_size);
        let tile_padding = request
            .tile_padding
            .unwrap_or(model_desc.default_padding);

        let tiling_spec = TileSpec {
            tile_size,
            padding: tile_padding,
            scale,
        };
        let tiling_engine = TilingEngine::new(tiling_spec);

        // Initialize Demuxer and Muxer
        let mut demuxer = VideoDemuxer::open(&request.input_path)
            .context("Failed to open video demuxer")?;

        let mut muxer = VideoMuxer::open(
            &request.output_path,
            out_w,
            out_h,
            fps,
            &request.encoding_profile,
            Some(&request.input_path),
        )
        .context("Failed to initialize video muxer")?;

        let mut progress = JobProgress::new(request.id.clone(), total_frames);
        progress.state = JobState::Processing;

        let start_time = Instant::now();
        let mut frame_idx = 0u64;

        while let Some(raw_frame) = demuxer.read_frame()? {
            if self.cancelled.load(Ordering::SeqCst) {
                progress.state = JobState::Cancelled;
                let _ = progress_tx.send(progress).await;
                info!("Pipeline cancelled by user.");
                return Ok(());
            }

            // 1. Convert raw RGB bytes to NCHW float tensor [0.0, 1.0]
            let in_tensor = ColorPipeline::rgb_bytes_to_nchw(&raw_frame, in_w, in_h);

            // 2. Partition into tiles
            let tile_coords = tiling_engine.compute_tiles(in_w, in_h);
            let mut tile_outputs = Vec::with_capacity(tile_coords.len());

            for coord in tile_coords {
                let mut tile_in = Array4::<f32>::zeros((1, 3, coord.in_h, coord.in_w));
                for y in 0..coord.in_h {
                    for x in 0..coord.in_w {
                        for c in 0..3 {
                            tile_in[[0, c, y, x]] = in_tensor[[0, c, coord.in_y + y, coord.in_x + x]];
                        }
                    }
                }

                // 3. Inference on tile (via DirectML / ONNX Session)
                let tile_out = session.run_inference(&tile_in)?;
                tile_outputs.push((coord, tile_out));
            }

            // 4. Stitch tiles back using 2D Hann window blending
            let mut upscaled = tiling_engine.stitch_tiles(&tile_outputs, out_w, out_h);

            // 5. Apply natural film grain if requested
            if request.film_grain > 0.0 {
                ColorPipeline::apply_film_grain(&mut upscaled, request.film_grain);
            }

            // 6. Convert back to RGB bytes and write to hardware encoder
            let out_bytes = ColorPipeline::nchw_to_rgb_bytes(&upscaled);
            muxer.write_frame(&out_bytes)?;

            frame_idx += 1;

            // Emit progress updates
            let elapsed = start_time.elapsed().as_secs_f64();
            let current_fps = if elapsed > 0.0 {
                frame_idx as f64 / elapsed
            } else {
                0.0
            };
            let remaining = total_frames.saturating_sub(frame_idx);
            let eta = if current_fps > 0.0 {
                (remaining as f64 / current_fps) as u64
            } else {
                0
            };

            progress.current_frame = frame_idx;
            progress.percentage = ((frame_idx as f32 / total_frames as f32) * 100.0).min(100.0);
            progress.current_fps = current_fps;
            progress.eta_seconds = eta;

            let _ = progress_tx.send(progress.clone()).await;
        }

        // Finalize encoding & container muxing
        muxer.finish().context("Failed to finalize video output container")?;

        progress.state = JobState::Completed;
        progress.percentage = 100.0;
        let _ = progress_tx.send(progress).await;

        info!(
            "Job {} successfully processed {} frames in {:.2?}",
            request.id, frame_idx, start_time.elapsed()
        );

        Ok(())
    }
}
