use crate::ffmpeg_bin::FfmpegLocator;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StabilizationMode {
    Disabled,
    SmoothMotion,
    TripodStatic,
}

impl Default for StabilizationMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StabilizationBorderMode {
    AutoZoom,
    Mirror,
    Black,
}

impl Default for StabilizationBorderMode {
    fn default() -> Self {
        Self::AutoZoom
    }
}

fn default_smoothing() -> u32 {
    30
}

fn default_shakiness() -> u8 {
    6
}

fn default_accuracy() -> u8 {
    9
}

fn default_stepsize() -> u8 {
    6
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilizationOptions {
    pub mode: StabilizationMode,
    #[serde(default = "default_smoothing")]
    pub smoothing: u32,
    #[serde(default = "default_shakiness")]
    pub shakiness: u8,
    #[serde(default = "default_accuracy")]
    pub accuracy: u8,
    #[serde(default = "default_stepsize")]
    pub stepsize: u8,
    #[serde(default)]
    pub border_mode: StabilizationBorderMode,
    #[serde(default)]
    pub zoom_percent: f32,
}

impl Default for StabilizationOptions {
    fn default() -> Self {
        Self {
            mode: StabilizationMode::Disabled,
            smoothing: 30,
            shakiness: 6,
            accuracy: 9,
            stepsize: 6,
            border_mode: StabilizationBorderMode::AutoZoom,
            zoom_percent: 0.0,
        }
    }
}

impl StabilizationOptions {
    pub fn is_enabled(&self) -> bool {
        self.mode != StabilizationMode::Disabled
    }

    /// Helper to convert a file path to FFmpeg's filter escaped string
    pub fn escape_ffmpeg_path(path: &Path) -> String {
        let p_str = path.to_string_lossy().replace('\\', "/");
        p_str.replace(':', "\\:")
    }

    /// Formats the vidstabdetect filter for Pass 1
    pub fn build_detect_filter(&self, trf_path: &Path) -> String {
        let escaped = Self::escape_ffmpeg_path(trf_path);
        let shakiness = self.shakiness.clamp(1, 10);
        let accuracy = self.accuracy.clamp(1, 15);
        let stepsize = self.stepsize.clamp(1, 32);
        let tripod = if self.mode == StabilizationMode::TripodStatic { 1 } else { 0 };

        format!(
            "vidstabdetect=stepsize={}:shakiness={}:accuracy={}:tripod={}:result='{}'",
            stepsize, shakiness, accuracy, tripod, escaped
        )
    }

    /// Formats the vidstabtransform filter for Pass 2
    pub fn build_transform_filter(&self, trf_path: &Path) -> String {
        let escaped = Self::escape_ffmpeg_path(trf_path);
        let smoothing = self.smoothing.clamp(1, 120);
        let tripod = if self.mode == StabilizationMode::TripodStatic { 1 } else { 0 };

        let (optzoom, crop) = match self.border_mode {
            StabilizationBorderMode::AutoZoom => (1, "keep"),
            StabilizationBorderMode::Mirror => (0, "keep"),
            StabilizationBorderMode::Black => (0, "black"),
        };

        let zoom = self.zoom_percent.clamp(0.0, 50.0);

        format!(
            "vidstabtransform=input='{}':smoothing={}:optzoom={}:tripod={}:crop={}:zoom={:.1}:interpol=bicubic",
            escaped, smoothing, optzoom, tripod, crop, zoom
        )
    }
}

pub struct VideoStabilizer;

impl VideoStabilizer {
    /// Runs Pass 1 motion vector analysis using vidstabdetect, outputting to `transforms_file`
    pub fn analyze_motion<P1: AsRef<Path>, P2: AsRef<Path>>(
        input_path: P1,
        transforms_file: P2,
        options: &StabilizationOptions,
        start_time: Option<f64>,
        end_time: Option<f64>,
    ) -> Result<()> {
        let ffmpeg_bin = FfmpegLocator::find_ffmpeg()?;
        let in_p = input_path.as_ref();
        let trf_p = transforms_file.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = trf_p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let detect_filter = options.build_detect_filter(trf_p);
        info!(
            "Running Pass 1 camera motion analysis on {:?} with filter: {}",
            in_p, detect_filter
        );

        let mut cmd = Command::new(&ffmpeg_bin);
        cmd.args(["-y", "-hwaccel", "auto"]);

        if let Some(st) = start_time {
            if st > 0.001 {
                cmd.args(["-ss", &format!("{:.3}", st)]);
            }
        }

        cmd.args(["-i"]).arg(in_p);

        if let Some(et) = end_time {
            let st = start_time.unwrap_or(0.0);
            let duration = (et - st).max(0.01);
            cmd.args(["-t", &format!("{:.3}", duration)]);
        }

        cmd.args(["-vf", &detect_filter, "-f", "null", "-"]);

        let output = cmd.output().with_context(|| {
            format!("Failed to execute FFmpeg vidstabdetect pass for {:?}", in_p)
        })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("vidstabdetect analysis failed: {}", err);
        }

        if !trf_p.exists() {
            anyhow::bail!("Expected transforms file {:?} was not produced by FFmpeg", trf_p);
        }

        Ok(())
    }

    /// Deterministically gets or calculates the motion transforms file for an input video
    pub fn get_or_generate_transforms<P: AsRef<Path>>(
        input_path: P,
        options: &StabilizationOptions,
        start_time: Option<f64>,
        end_time: Option<f64>,
    ) -> Result<PathBuf> {
        let in_p = input_path.as_ref();
        let file_stem = in_p.file_stem().and_then(|s| s.to_str()).unwrap_or("video");

        let st_tag = start_time.map(|s| format!("_s{:.1}", s)).unwrap_or_default();
        let et_tag = end_time.map(|e| format!("_e{:.1}", e)).unwrap_or_default();
        let mode_tag = match options.mode {
            StabilizationMode::SmoothMotion => "smooth",
            StabilizationMode::TripodStatic => "tripod",
            StabilizationMode::Disabled => "none",
        };
        let file_name = format!(
            "vstab_{}_{}{}{}_sh{}_ac{}.trf",
            file_stem, mode_tag, st_tag, et_tag, options.shakiness, options.accuracy
        );
        let temp_trf = std::env::temp_dir().join("video_upscaler_cache").join(file_name);

        if temp_trf.exists() && std::fs::metadata(&temp_trf).map(|m| m.len() > 0).unwrap_or(false) {
            info!("Reusing cached motion transforms file: {:?}", temp_trf);
            return Ok(temp_trf);
        }

        Self::analyze_motion(in_p, &temp_trf, options, start_time, end_time)?;
        Ok(temp_trf)
    }
}
