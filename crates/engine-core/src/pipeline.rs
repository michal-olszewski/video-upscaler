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

    /// Runs the video upscale pipeline asynchronously using decoupled producer-consumer stages with backpressure
    pub async fn execute(
        &self,
        request: UpscaleJobRequest,
        progress_tx: mpsc::Sender<JobProgress>,
    ) -> Result<()> {
        info!("Starting decoupled streaming upscale pipeline for job: {}", request.id);

        let model_desc = ModelRegistry::find_by_id(&request.model_id)
            .context(format!("Model not found: {}", request.model_id))?;

        let model_path = format!("models/cache/{}", model_desc.filename);
        let session = Arc::new(InferenceSessionWrapper::new(&model_path, request.backend.clone())?);
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
            "Job {}: Streaming {}x{} -> {}x{} @ {:.2} fps ({} total frames) using {}",
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
        let tiling_engine = Arc::new(TilingEngine::new(tiling_spec));

        // Buffer limits in-flight uncompressed frames to prevent VRAM / RAM exhaustion
        const RING_BUFFER_CAPACITY: usize = 8;
        let (decode_tx, mut decode_rx) = mpsc::channel::<(u64, Vec<u8>)>(RING_BUFFER_CAPACITY);
        let (infer_tx, mut infer_rx) = mpsc::channel::<(u64, Vec<u8>)>(RING_BUFFER_CAPACITY);

        let cancelled_decode = Arc::clone(&self.cancelled);
        let input_path = request.input_path.clone();

        // ----------------------------------------------------
        // STAGE 1: Video Decode & Demux Worker (Producer)
        // ----------------------------------------------------
        let decode_task = tokio::task::spawn_blocking(move || -> Result<u64> {
            let mut demuxer = VideoDemuxer::open(&input_path)?;
            let mut count = 0u64;

            while let Some(raw_frame) = demuxer.read_frame()? {
                if cancelled_decode.load(Ordering::SeqCst) {
                    break;
                }
                if decode_tx.blocking_send((count, raw_frame)).is_err() {
                    break;
                }
                count += 1;
            }
            Ok(count)
        });

        // ----------------------------------------------------
        // STAGE 2: Tiling, DirectML Inference & Reconstruction
        // ----------------------------------------------------
        let cancelled_infer = Arc::clone(&self.cancelled);
        let session_clone = Arc::clone(&session);
        let tiling_clone = Arc::clone(&tiling_engine);
        let film_grain = request.film_grain;

        let infer_task = tokio::spawn(async move {
            while let Some((frame_idx, raw_frame)) = decode_rx.recv().await {
                if cancelled_infer.load(Ordering::SeqCst) {
                    break;
                }

                // 1. Normalization
                let in_tensor = ColorPipeline::rgb_bytes_to_nchw(&raw_frame, in_w, in_h);

                // 2. Tiling & Tensor Inference
                let tile_coords = tiling_clone.compute_tiles(in_w, in_h);
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

                    let tile_out = session_clone.run_inference(&tile_in)?;
                    tile_outputs.push((coord, tile_out));
                }

                // 3. 2D Hann reconstruction
                let mut upscaled = tiling_clone.stitch_tiles(&tile_outputs, out_w, out_h);

                // 4. Procedural film grain
                if film_grain > 0.0 {
                    ColorPipeline::apply_film_grain(&mut upscaled, film_grain);
                }

                let out_bytes = ColorPipeline::nchw_to_rgb_bytes(&upscaled);
                if infer_tx.send((frame_idx, out_bytes)).await.is_err() {
                    break;
                }
            }
            Ok::<(), anyhow::Error>(())
        });

        // ----------------------------------------------------
        // STAGE 3: Hardware Encode & Stream Muxer (Consumer)
        // ----------------------------------------------------
        let output_path = request.output_path.clone();
        let encoding_profile = request.encoding_profile.clone();
        let audio_src = request.input_path.clone();
        let job_id = request.id.clone();
        let cancelled_mux = Arc::clone(&self.cancelled);

        let mut progress = JobProgress::new(job_id.clone(), total_frames);
        progress.state = JobState::Processing;
        let _ = progress_tx.send(progress.clone()).await;

        let mut muxer = VideoMuxer::open(
            &output_path,
            out_w,
            out_h,
            fps,
            &encoding_profile,
            Some(&audio_src),
        )?;

        let start_time = Instant::now();
        let mut processed_frames = 0u64;

        while let Some((_idx, upscaled_bytes)) = infer_rx.recv().await {
            if cancelled_mux.load(Ordering::SeqCst) {
                progress.state = JobState::Cancelled;
                let _ = progress_tx.send(progress).await;
                info!("Pipeline cancelled during encode.");
                return Ok(());
            }

            muxer.write_frame(&upscaled_bytes)?;
            processed_frames += 1;

            let elapsed = start_time.elapsed().as_secs_f64();
            let current_fps = if elapsed > 0.0 {
                processed_frames as f64 / elapsed
            } else {
                0.0
            };
            let remaining = total_frames.saturating_sub(processed_frames);
            let eta = if current_fps > 0.0 {
                (remaining as f64 / current_fps) as u64
            } else {
                0
            };

            progress.current_frame = processed_frames;
            progress.percentage = ((processed_frames as f32 / total_frames as f32) * 100.0).min(100.0);
            progress.current_fps = current_fps;
            progress.eta_seconds = eta;

            let _ = progress_tx.send(progress.clone()).await;
        }

        // Wait for workers to cleanly finish
        let _ = decode_task.await?;
        let _ = infer_task.await?;

        // Finalize video container
        muxer.finish().context("Failed to finalize video output container")?;

        progress.state = JobState::Completed;
        progress.percentage = 100.0;
        let _ = progress_tx.send(progress).await;

        info!(
            "Job {}: Streamed {} frames in {:.2?} ({:.1} fps avg)",
            job_id,
            processed_frames,
            start_time.elapsed(),
            if start_time.elapsed().as_secs_f64() > 0.0 {
                processed_frames as f64 / start_time.elapsed().as_secs_f64()
            } else {
                0.0
            }
        );

        Ok(())
    }
}
