use crate::ffmpeg_bin::FfmpegLocator;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

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
    pub field_order: Option<String>,
}

impl VideoStreamInfo {
    /// Determines whether the video stream is high bit depth (10-bit, 12-bit, or 16-bit)
    pub fn is_high_bit_depth(&self) -> bool {
        self.pixel_format.contains("10")
            || self.pixel_format.contains("12")
            || self.pixel_format.contains("16")
            || self.pixel_format.contains("p010")
            || self.codec_name.to_lowercase().contains("prores")
            || self.codec_name.to_lowercase().contains("dnxhd")
    }

    /// Determines whether the video stream contains High Dynamic Range (HDR) metadata
    pub fn is_hdr(&self) -> bool {
        self.color_transfer.as_deref() == Some("smpte2084")
            || self.color_transfer.as_deref() == Some("arib-std-b67")
            || self.color_primaries.as_deref() == Some("bt2020")
    }

    /// Determines whether the video stream is interlaced
    pub fn is_interlaced(&self) -> bool {
        match self.field_order.as_deref() {
            Some("tt") | Some("bb") | Some("tb") | Some("bt") | Some("interlaced") => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleStreamInfo {
    pub index: usize,
    pub codec_name: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub file_path: String,
    pub format_name: String,
    pub duration_seconds: f64,
    pub video_stream: VideoStreamInfo,
    pub audio_streams: Vec<AudioStreamInfo>,
    pub subtitle_streams: Vec<SubtitleStreamInfo>,
}

pub struct MediaProbe;

impl MediaProbe {
    /// Probes media file metadata using ffprobe JSON output
    pub fn probe_file<P: AsRef<Path>>(file_path: P) -> Result<MediaMetadata> {
        let path = file_path.as_ref();
        if !path.exists() {
            anyhow::bail!("Media file does not exist: {:?}", path);
        }

        let ffprobe_bin = FfmpegLocator::find_ffprobe()?;

        let output = Command::new(&ffprobe_bin)
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(path)
            .output()
            .with_context(|| format!("Failed to execute ffprobe at {:?}", ffprobe_bin))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffprobe failed on {:?}: {}", path, err_msg);
        }

        let json_text = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&json_text).context("Failed to parse ffprobe JSON output")?;

        let format_obj = parsed
            .get("format")
            .context("Missing 'format' section in ffprobe output")?;

        let format_name = format_obj
            .get("format_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let total_duration = format_obj
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let streams = parsed
            .get("streams")
            .and_then(|v| v.as_array())
            .context("Missing or invalid 'streams' array in ffprobe output")?;

        let mut video_info = None;
        let mut audio_streams = Vec::new();
        let mut subtitle_streams = Vec::new();

        for (idx, stream) in streams.iter().enumerate() {
            let codec_type = stream
                .get("codec_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let codec_name = stream
                .get("codec_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let tags = stream.get("tags");
            let language = tags
                .and_then(|t| t.get("language"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            match codec_type {
                "video" if video_info.is_none() => {
                    let width = stream
                        .get("width")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let height = stream
                        .get("height")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;

                    let fps_str = stream
                        .get("r_frame_rate")
                        .or_else(|| stream.get("avg_frame_rate"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("24/1");

                    let frame_rate = Self::parse_framerate(fps_str).unwrap_or(24.0);

                    let nb_frames = stream
                        .get("nb_frames")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .or_else(|| {
                            if total_duration > 0.0 && frame_rate > 0.0 {
                                Some((total_duration * frame_rate).round() as u64)
                            } else {
                                None
                            }
                        });

                    let pixel_format = stream
                        .get("pix_fmt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("yuv420p")
                        .to_string();

                    let color_space = stream
                        .get("color_space")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let color_primaries = stream
                        .get("color_primaries")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let color_transfer = stream
                        .get("color_transfer")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let field_order = stream
                        .get("field_order")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    video_info = Some(VideoStreamInfo {
                        index: idx,
                        codec_name,
                        width,
                        height,
                        frame_rate,
                        total_frames: nb_frames,
                        duration_seconds: total_duration,
                        pixel_format,
                        color_space,
                        color_primaries,
                        color_transfer,
                        field_order,
                    });
                }
                "audio" => {
                    let sample_rate = stream
                        .get("sample_rate")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(48000);

                    let channels = stream
                        .get("channels")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(2) as u32;

                    audio_streams.push(AudioStreamInfo {
                        index: idx,
                        codec_name,
                        sample_rate,
                        channels,
                        language,
                    });
                }
                "subtitle" => {
                    subtitle_streams.push(SubtitleStreamInfo {
                        index: idx,
                        codec_name,
                        language,
                    });
                }
                _ => {}
            }
        }

        let video_stream = video_info.context("No video stream found in media file")?;

        Ok(MediaMetadata {
            file_path: path.to_string_lossy().to_string(),
            format_name,
            duration_seconds: total_duration,
            video_stream,
            audio_streams,
            subtitle_streams,
        })
    }

    fn parse_framerate(fps_str: &str) -> Option<f64> {
        let parts: Vec<&str> = fps_str.split('/').collect();
        if parts.len() == 2 {
            let num: f64 = parts[0].parse().ok()?;
            let den: f64 = parts[1].parse().ok()?;
            if den > 0.0 {
                return Some(num / den);
            }
        } else if let Ok(val) = fps_str.parse::<f64>() {
            return Some(val);
        }
        None
    }
}
