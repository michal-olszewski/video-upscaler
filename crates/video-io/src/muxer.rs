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

        // Input 0: Raw video stream from stdin
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
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
            HardwareEncoder::NvidiaNvenc(_) => {
                cmd.args(["-preset", &profile.preset, "-cq", &profile.crf.to_string()]);
            }
            HardwareEncoder::AmdAmf(_) => {
                // AMF uses quality/speed presets and qp/crf
                cmd.args(["-quality", "quality", "-rc", "cqvbr", "-qp_i", &profile.crf.to_string()]);
            }
            HardwareEncoder::AppleVideoToolbox(_) => {
                cmd.args(["-q:v", &profile.crf.to_string()]);
            }
            HardwareEncoder::SoftwareCpu(_) => {
                cmd.args(["-crf", &profile.crf.to_string(), "-preset", &profile.preset]);
            }
        }

        // Pixel format for output (e.g. yuv420p for standard compatibility)
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
        if out_path.extension().map(|e| e == "mp4" || e == "mov").unwrap_or(false) {
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
            expected_frame_bytes: out_width * out_height * 3,
            frames_written: 0,
        })
    }

    /// Writes an uncompressed RGB24 frame into the encoder stdin pipe
    pub fn write_frame(&mut self, rgb_bytes: &[u8]) -> Result<()> {
        if rgb_bytes.len() != self.expected_frame_bytes {
            anyhow::bail!(
                "Frame byte count mismatch: expected {} bytes ({}x{}x3), got {}",
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
