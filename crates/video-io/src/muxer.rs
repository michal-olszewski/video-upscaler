use crate::codecs::{HardwareEncoder, OutputEncodingProfile};
use crate::ffmpeg_bin::FfmpegLocator;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use tracing::info;

pub struct VideoMuxer {
    child: Child,
    stdin: Option<ChildStdin>,
    out_width: usize,
    out_height: usize,
    expected_frame_bytes: usize,
    frames_written: u64,
}

impl VideoMuxer {
    /// Spawns an FFmpeg encoding process taking raw RGB24 frames from stdin and writing an encoded MP4/MKV
    pub fn open<P: AsRef<Path>, Q: AsRef<Path>>(
        output_path: P,
        out_width: usize,
        out_height: usize,
        frame_rate: f64,
        profile: &OutputEncodingProfile,
        audio_source_path: Option<Q>,
    ) -> Result<Self> {
        let out_path = output_path.as_ref().to_path_buf();
        let ffmpeg_bin = FfmpegLocator::find_ffmpeg()?;
        let encoder_name = profile.encoder.ffmpeg_encoder_name();

        info!(
            "Initializing Video Muxer -> {:?} ({}x{} @ {:.2} fps) using encoder {}",
            out_path, out_width, out_height, frame_rate, encoder_name
        );

        let mut cmd = Command::new(&ffmpeg_bin);
        cmd.args(["-y"]); // Overwrite output file if exists

        // Input 0: Raw video stream from stdin (RGB48LE for high bit depth / HDR, RGB24 for 8-bit standard)
        let (raw_pix_fmt, bytes_per_px) = if profile.high_bit_depth {
            ("rgb48le", 6)
        } else {
            ("rgb24", 3)
        };

        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            raw_pix_fmt,
            "-s",
            &format!("{}x{}", out_width, out_height),
            "-r",
            &format!("{:.4}", frame_rate),
            "-i",
            "-",
        ]);

        // Input 1: Original file for audio & subtitles passthrough if provided
        let has_audio_source = audio_source_path.is_some();
        if let Some(ref audio_path) = audio_source_path {
            cmd.arg("-i").arg(audio_path.as_ref());
        }

        // Map video stream from input 0
        cmd.args(["-map", "0:v:0"]);

        // Video codec & parameters
        cmd.args(["-c:v", encoder_name]);

        // Apply hardware specific flags
        match &profile.encoder {
            HardwareEncoder::NvidiaNvenc(crate::codecs::VideoCodecType::Av1) => {
                cmd.args(["-preset", &profile.preset, "-cq", &profile.crf.to_string()]);
            }
            HardwareEncoder::NvidiaNvenc(_) => {
                cmd.args(["-preset", &profile.preset, "-cq", &profile.crf.to_string()]);
            }
            HardwareEncoder::AmdAmf(crate::codecs::VideoCodecType::Av1) => {
                cmd.args(["-quality", "quality", "-rc", "cqvbr", "-qp_i", &profile.crf.to_string()]);
            }
            HardwareEncoder::AmdAmf(_) => {
                cmd.args(["-quality", "quality", "-rc", "cqvbr", "-qp_i", &profile.crf.to_string()]);
            }
            HardwareEncoder::AppleVideoToolbox(_) => {
                cmd.args(["-q:v", &profile.crf.to_string()]);
            }
            HardwareEncoder::SoftwareCpu(crate::codecs::VideoCodecType::Av1) => {
                let svt_preset = match profile.preset.as_str() {
                    "ultrafast" | "fast" => "10",
                    "slow" | "veryslow" => "4",
                    _ => "6",
                };
                cmd.args(["-crf", &profile.crf.to_string(), "-preset", svt_preset]);
            }
            HardwareEncoder::SoftwareCpu(crate::codecs::VideoCodecType::ProRes) => {
                cmd.args(["-profile:v", "3"]); // ProRes 422 HQ
            }
            HardwareEncoder::SoftwareCpu(crate::codecs::VideoCodecType::H265Hevc) => {
                cmd.args(["-crf", &profile.crf.to_string(), "-preset", &profile.preset]);
                if profile.is_hdr() {
                    let mut params = Vec::new();
                    if let Some(ref prim) = profile.color_primaries {
                        params.push(format!("colorprim={}", prim));
                    }
                    if let Some(ref trc) = profile.color_transfer {
                        params.push(format!("transfer={}", trc));
                    }
                    if let Some(ref spc) = profile.color_space {
                        params.push(format!("colormatrix={}", spc));
                    }
                    if !params.is_empty() {
                        cmd.args(["-x265-params", &params.join(":")]);
                    }
                }
            }
            HardwareEncoder::SoftwareCpu(_) => {
                cmd.args(["-crf", &profile.crf.to_string(), "-preset", &profile.preset]);
            }
        }

        // Forward color space and HDR10 metadata flags
        if let Some(ref primaries) = profile.color_primaries {
            cmd.args(["-color_primaries", primaries]);
        }
        if let Some(ref transfer) = profile.color_transfer {
            cmd.args(["-color_trc", transfer]);
        }
        if let Some(ref space) = profile.color_space {
            cmd.args(["-colorspace", space]);
        }

        // Tag hvc1 for HEVC streams in MP4/MOV containers for Apple/DaVinci/DirectShow compatibility
        let is_mp4_or_mov = out_path.extension().map(|e| e == "mp4" || e == "mov").unwrap_or(false);
        if is_mp4_or_mov && (encoder_name.contains("hevc") || encoder_name.contains("265")) {
            cmd.args(["-tag:v", "hvc1"]);
        }

        // Pixel format for output (e.g. yuv420p, yuv420p10le, p010le, yuv422p10le)
        cmd.args(["-pix_fmt", &profile.pixel_format]);

        // Map audio & subtitles from input 1 if available
        if has_audio_source {
            if profile.copy_audio {
                cmd.args(["-map", "1:a?", "-c:a", "copy"]);
            }
            if profile.copy_subtitles {
                cmd.args(["-map", "1:s?", "-c:s", "copy"]);
            }
        }

        // Faststart for MP4 web and preview playback
        if is_mp4_or_mov {
            cmd.args(["-movflags", "+faststart"]);
        }

        cmd.arg(&out_path);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn FFmpeg encoder process for {:?}", out_path))?;

        let stdin = child.stdin.take().context("Failed to open FFmpeg stdin pipe")?;

        Ok(Self {
            child,
            stdin: Some(stdin),
            out_width,
            out_height,
            expected_frame_bytes: out_width * out_height * bytes_per_px,
            frames_written: 0,
        })
    }

    /// Writes an uncompressed raw RGB frame (RGB24 or RGB48LE) into the encoder stdin pipe
    pub fn write_frame(&mut self, rgb_bytes: &[u8]) -> Result<()> {
        if rgb_bytes.len() != self.expected_frame_bytes {
            anyhow::bail!(
                "Frame byte count mismatch: expected {} bytes ({}x{}), got {}",
                self.expected_frame_bytes,
                self.out_width,
                self.out_height,
                rgb_bytes.len()
            );
        }

        if let Some(ref mut stdin) = self.stdin {
            stdin
                .write_all(rgb_bytes)
                .context("Failed to write video frame to encoder stdin pipe")?;
            self.frames_written += 1;
            Ok(())
        } else {
            anyhow::bail!("Cannot write frame: muxer is already closed")
        }
    }

    /// Finalizes encoding, closes stdin pipe, and waits for container muxing to finish
    pub fn finish(mut self) -> Result<u64> {
        // Drop stdin to signal EOF to FFmpeg
        self.stdin.take();

        let status = self
            .child
            .wait()
            .context("Failed waiting for FFmpeg encoder to complete")?;

        if !status.success() {
            anyhow::bail!("FFmpeg encoder exited with error: {:?}", status.code());
        }

        Ok(self.frames_written)
    }
}
