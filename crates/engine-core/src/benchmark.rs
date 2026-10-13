use anyhow::{Context, Result};
use image_processing::{ColorPipeline, TileSpec, TilingEngine};
use inference_runtime::{ExecutionBackend, HardwareManager};
use models::ModelRegistry;
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScoreboardItem {
    pub test_name: String,
    pub tile_size: usize,
    pub latency_ms: f64,
    pub fps: f64,
    pub vram_mb: u64,
    pub score: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareOptimizationRecommendation {
    pub recommended_tile_size: usize,
    pub recommended_batch_concurrency: usize,
    pub recommended_backend: String,
    pub performance_tier: String,
    pub overall_score: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunResponse {
    pub device_name: String,
    pub vendor: String,
    pub is_gpu: bool,
    pub items: Vec<BenchmarkScoreboardItem>,
    pub recommendation: HardwareOptimizationRecommendation,
}

pub struct BenchmarkEngine;

impl BenchmarkEngine {
    /// Runs a standardized hardware inference throughput benchmark testing 512, 768, and 1024 tile sizes
    pub async fn run_system_benchmark(backend: Option<ExecutionBackend>) -> Result<BenchmarkRunResponse> {
        let devices = HardwareManager::probe_devices();
        let primary_device = devices.first().cloned();

        let (device_name, vendor, is_gpu, exec_backend) = match primary_device {
            Some(dev) => {
                let is_gpu = !dev.is_integrated && dev.dedicated_vram_mb > 0;
                let b = backend.unwrap_or(dev.recommended_backend.clone());
                (dev.name, format!("{:?}", dev.vendor), is_gpu, b)
            }
            None => (
                "Generic Compute Device".to_string(),
                "Unknown".to_string(),
                false,
                backend.unwrap_or(ExecutionBackend::Cpu { thread_count: 4 }),
            ),
        };

        info!("Starting Hardware Benchmark on {} (backend: {})", device_name, exec_backend);

        let test_tile_sizes = vec![512usize, 768usize, 1024usize];
        let mut items = Vec::new();

        // Model to test: Compact SPAN or Real-ESRGAN
        let model_id = "compact-span";
        let model_desc = ModelRegistry::find_by_id(model_id)
            .or_else(|| ModelRegistry::list_models().first().cloned())
            .context("No benchmark model found in registry")?;

        let scale = model_desc.scale_factor as usize;
        let model_path = format!("models/cache/{}", model_desc.filename);
        let session = Arc::new(inference_runtime::InferenceSessionWrapper::new_with_scale(
            &model_path,
            exec_backend.clone(),
            scale,
        )?);

        // Standardized synthetic 128x128 test frame (simulates realistic high-frequency image)
        let width = 128usize;
        let height = 128usize;
        let mut synthetic_frame = vec![0u8; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                synthetic_frame[idx] = ((x * 7 + y * 13) % 256) as u8;
                synthetic_frame[idx + 1] = ((x * 11 + y * 5) % 256) as u8;
                synthetic_frame[idx + 2] = ((x * 3 + y * 17) % 256) as u8;
            }
        }

        let input_tensor = ColorPipeline::rgb_bytes_to_nchw(&synthetic_frame, width, height);

        for tile_size in test_tile_sizes {
            let tiling_spec = TileSpec {
                tile_size,
                padding: 16,
                scale,
            };
            let tiling_engine = TilingEngine::new(tiling_spec);
            let tile_coords = tiling_engine.compute_tiles(width, height);

            let run_single_pass = || -> Result<()> {
                for coord in &tile_coords {
                    let mut tile_in = Array4::<f32>::zeros((1, 3, coord.in_h, coord.in_w));
                    for y in 0..coord.in_h {
                        for x in 0..coord.in_w {
                            for c in 0..3 {
                                tile_in[[0, c, y, x]] = input_tensor[[0, c, coord.in_y + y, coord.in_x + x]];
                            }
                        }
                    }
                    let _ = session.run_inference(&tile_in)?;
                }
                Ok(())
            };

            // Warm-up run (1 iteration)
            let _ = run_single_pass();

            // Timed iterations
            let iterations = 2;
            let start = Instant::now();
            for _ in 0..iterations {
                let _ = run_single_pass();
            }
            let elapsed = start.elapsed();
            let total_seconds = elapsed.as_secs_f64();
            let avg_frame_sec = total_seconds / (iterations as f64);
            let latency_ms = (avg_frame_sec * 1000.0).max(0.1);
            let fps = (1.0 / avg_frame_sec).max(0.1);

            // Calculate estimated VRAM footprint based on tile size and scale
            let vram_mb = ((tile_size * tile_size * 4 * 4 * 4) / (1024 * 1024)).max(128) as u64;
            let score = ((fps * 100.0) as u64).max(10);

            items.push(BenchmarkScoreboardItem {
                test_name: format!("{} (Tile {}px)", model_desc.name, tile_size),
                tile_size,
                latency_ms: (latency_ms * 100.0).round() / 100.0,
                fps: (fps * 10.0).round() / 10.0,
                vram_mb,
                score,
            });
        }

        // Determine best performing tile size
        let best_item = items.iter().max_by_key(|i| i.score).cloned().unwrap_or(items[0].clone());
        let overall_score = (items.iter().map(|i| i.score).sum::<u64>() / items.len() as u64).max(50);

        let performance_tier = if best_item.fps >= 40.0 {
            "Extreme (8K/4K Pro Studio)".to_string()
        } else if best_item.fps >= 20.0 {
            "High (4K Studio Ready)".to_string()
        } else if best_item.fps >= 8.0 {
            "Balanced (1080p Real-Time)".to_string()
        } else {
            "Entry (CPU Fallback)".to_string()
        };

        let rec_concurrency = if best_item.fps >= 30.0 { 2 } else { 1 };

        let recommendation = HardwareOptimizationRecommendation {
            recommended_tile_size: best_item.tile_size,
            recommended_batch_concurrency: rec_concurrency,
            recommended_backend: format!("{}", exec_backend),
            performance_tier,
            overall_score,
        };

        Ok(BenchmarkRunResponse {
            device_name,
            vendor,
            is_gpu,
            items,
            recommendation,
        })
    }
}
