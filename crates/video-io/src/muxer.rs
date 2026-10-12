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
        Self::open_with_trim(
            output_path,
            out_width,
            out_height,
            frame_rate,
            profile,
            audio_source_path,
            None,
            None,
        )
    }

    /// Spawns an FFmpeg encoding process with optional trim start and duration for audio synchronization
    pub fn open_with_trim<P: AsRef<Path>, Q: AsRef<Path>>(
        output_path: P,
        out_width: usize,
        out_height: usize,
        frame_rate: f64,
        profile: &OutputEncodingProfile,
        audio_source_path: Option<Q>,
        trim_start: Option<f64>,
        trim_duration: Option<f64>,
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
            if let Some(st) = trim_start {
                if st > 0.001 {
                    cmd.args(["-ss", &format!("{:.3}", st)]);
                }
            }
            if let Some(dur) = trim_duration {
                cmd.args(["-t", &format!("{:.3}", dur)]);
            }
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

        // Apply target geometry & aspect ratio scaling filter if specified
        if let Some(ref geom) = profile.geometry_profile {
            let (target_w, target_h) = geom.resolution.dimensions(out_width, out_height);
            if (target_w, target_h) != (out_width, out_height) {
                let filter_str = match geom.fit_mode {
                    crate::codecs::FitMode::Letterbox => {
                        format!(
                            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:{}",
                            target_w, target_h, target_w, target_h, geom.matte_color
                        )
                    }
                    crate::codecs::FitMode::CenterCrop => {
                        format!(
                            "scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                            target_w, target_h, target_w, target_h
                        )
                    }
                    crate::codecs::FitMode::Stretch => {
                        format!("scale={}:{}", target_w, target_h)
                    }
                };
                info!("Applying target geometry filter: {}", filter_str);
                cmd.args(["-vf", &filter_str]);
            }
        }

        // Map audio & subtitles from input 1 if available
        if has_audio_source {
            let audio_prof = profile.audio_profile.as_ref();
            let codec = audio_prof.map(|p| &p.codec).unwrap_or(if profile.copy_audio {
                &crate::codecs::AudioCodec::Copy
            } else {
                &crate::codecs::AudioCodec::Mute
            });

            match codec {
                crate::codecs::AudioCodec::Mute => {
                    cmd.arg("-an");
                }
                crate::codecs::AudioCodec::Copy => {
                    cmd.args(["-map", "1:a?", "-c:a", "copy"]);
                }
                crate::codecs::AudioCodec::Aac => {
                    let b = audio_prof.map(|p| p.bitrate_kbps).unwrap_or(192);
                    cmd.args(["-map", "1:a?", "-c:a", "aac", "-b:a", &format!("{}k", b)]);
                }
                crate::codecs::AudioCodec::Opus => {
                    let b = audio_prof.map(|p| p.bitrate_kbps).unwrap_or(128);
                    cmd.args(["-map", "1:a?", "-c:a", "libopus", "-b:a", &format!("{}k", b)]);
                }
                crate::codecs::AudioCodec::Flac => {
                    cmd.args(["-map", "1:a?", "-c:a", "flac"]);
                }
                crate::codecs::AudioCodec::PcmS16le => {
                    cmd.args(["-map", "1:a?", "-c:a", "pcm_s16le"]);
                }
                crate::codecs::AudioCodec::PcmS24le => {
                    cmd.args(["-map", "1:a?", "-c:a", "pcm_s24le"]);
                }
            }

            // Loudness Normalization Pass (EBU R128)
            if let Some(ap) = audio_prof {
                if ap.loudness_normalization
                    && ap.codec != crate::codecs::AudioCodec::Copy
                    && ap.codec != crate::codecs::AudioCodec::Mute
                {
                    cmd.args(["-af", &format!("loudnorm=I={:.1}:LRA=7:TP=-1.5", ap.target_lufs)]);
                }
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
