use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use image::{ImageBuffer, Rgb};
use image_processing::{ColorPipeline, TileSpec, TilingEngine};
use inference_runtime::{ExecutionBackend, InferenceSessionWrapper};
use models::ModelRegistry;
use ndarray::Array4;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use video_io::{FfmpegLocator, MediaProbe};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewComparisonResult {
    pub original_data_url: String,
    pub upscaled_data_url: String,
    pub in_width: usize,
    pub in_height: usize,
    pub out_width: usize,
    pub out_height: usize,
    pub timestamp_seconds: f64,
}

pub struct PreviewEngine;

impl PreviewEngine {
    /// Extracts a single frame at timestamp_seconds and runs high-fidelity super-resolution preview
    pub fn generate_preview<P: AsRef<Path>>(
        video_path: P,
        timestamp_seconds: f64,
        model_id: &str,
        backend: ExecutionBackend,
        film_grain: f32,
    ) -> Result<PreviewComparisonResult> {
        let path = video_path.as_ref();
        let metadata = MediaProbe::probe_file(path)?;

        let in_w = metadata.video_stream.width;
        let in_h = metadata.video_stream.height;
        let frame_bytes = in_w * in_h * 3;

        let ffmpeg_bin = FfmpegLocator::find_ffmpeg()?;

        // 1. Extract single frame via FFmpeg
        let mut child = Command::new(&ffmpeg_bin)
            .args([
                "-ss",
                &format!("{:.3}", timestamp_seconds),
                "-i",
            ])
            .arg(path)
            .args([
                "-vframes",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-v",
                "error",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn ffmpeg preview extractor on {:?}", path))?;

        let mut stdout = child.stdout.take().context("Failed to capture stdout pipe")?;
        let mut raw_frame = vec![0u8; frame_bytes];
        stdout.read_exact(&mut raw_frame).context("Failed to read raw preview frame")?;
        let _ = child.wait();

        // 2. Load model session & tiling engine
        let model_desc = ModelRegistry::find_by_id(model_id)
            .context(format!("Model not found: {}", model_id))?;
        let scale = model_desc.scale_factor as usize;
        let out_w = in_w * scale;
        let out_h = in_h * scale;

        let model_path = format!("models/cache/{}", model_desc.filename);
        let session = InferenceSessionWrapper::new_with_scale(&model_path, backend, scale)?;

        let tiling_spec = TileSpec {
            tile_size: model_desc.default_tile_size,
            padding: model_desc.default_padding,
            scale,
        };
        let tiling_engine = TilingEngine::new(tiling_spec);

        // 3. Process frame with Tiling & DirectML
        let in_tensor = ColorPipeline::rgb_bytes_to_nchw(&raw_frame, in_w, in_h);
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
            let tile_out = session.run_inference(&tile_in)?;
            tile_outputs.push((coord, tile_out));
        }

        let mut upscaled = tiling_engine.stitch_tiles(&tile_outputs, out_w, out_h);

        if film_grain > 0.0 {
            ColorPipeline::apply_film_grain(&mut upscaled, film_grain);
        }

        let upscaled_bytes = ColorPipeline::nchw_to_rgb_bytes(&upscaled);

        // 4. Encode both images as PNG and format as Base64 Data URLs
        let orig_png = Self::rgb_to_png_base64(&raw_frame, in_w as u32, in_h as u32)?;
        let upscaled_png = Self::rgb_to_png_base64(&upscaled_bytes, out_w as u32, out_h as u32)?;

        Ok(PreviewComparisonResult {
            original_data_url: format!("data:image/png;base64,{}", orig_png),
            upscaled_data_url: format!("data:image/png;base64,{}", upscaled_png),
            in_width: in_w,
            in_height: in_h,
            out_width: out_w,
            out_height: out_h,
            timestamp_seconds,
        })
    }

    fn rgb_to_png_base64(rgb_bytes: &[u8], width: u32, height: u32) -> Result<String> {
        let img: ImageBuffer<Rgb<u8>, _> =
            ImageBuffer::from_raw(width, height, rgb_bytes.to_vec())
                .context("Failed to construct image buffer from raw RGB bytes")?;

        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .context("Failed to encode image as PNG")?;

        Ok(BASE64.encode(buf.into_inner()))
    }
}
