use crate::ffmpeg_bin::FfmpegLocator;
use crate::metadata::{MediaMetadata, MediaProbe};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeinterlaceMode {
    Disabled,
    Auto,           // Auto-detect if stream is interlaced; apply bwdif same-rate
    BwdifSameRate,  // Bob-Weaver deinterlacing (1 frame per frame: mode=0, parity=-1, deint=0)
    BwdifBob,       // Bob-Weaver deinterlacing (1 frame per field: mode=1, parity=-1, deint=0, 2x framerate)
    Yadif,          // Yet Another DeInterlacing Filter (mode=0, parity=-1, deint=0)
}

impl Default for DeinterlaceMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemuxOptions {
    pub high_bit_depth: bool,
    pub deinterlace_mode: DeinterlaceMode,
    pub denoise_strength: f32, // 0.0 = disabled, 0.0..=1.0
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
}

impl Default for DemuxOptions {
    fn default() -> Self {
        Self {
            high_bit_depth: false,
            deinterlace_mode: DeinterlaceMode::Disabled,
            denoise_strength: 0.0,
            start_time: None,
            end_time: None,
        }
    }
}

/// Generates an FFmpeg video filter string (-vf) for deinterlacing and archival denoising
pub fn build_video_filter(options: &DemuxOptions, is_stream_interlaced: bool) -> Option<String> {
    let mut filters = Vec::new();

    match options.deinterlace_mode {
        DeinterlaceMode::Disabled => {}
        DeinterlaceMode::Auto => {
            if is_stream_interlaced {
                filters.push("bwdif=mode=0:parity=-1:deint=0".to_string());
            }
        }
        DeinterlaceMode::BwdifSameRate => {
            filters.push("bwdif=mode=0:parity=-1:deint=0".to_string());
        }
        DeinterlaceMode::BwdifBob => {
            filters.push("bwdif=mode=1:parity=-1:deint=0".to_string());
        }
        DeinterlaceMode::Yadif => {
            filters.push("yadif=mode=0:parity=-1:deint=0".to_string());
        }
    }

    if options.denoise_strength > 0.001 {
        let s = options.denoise_strength.clamp(0.0, 1.0);
        let ls = (s * 8.0).max(1.0);
        let cs = (s * 6.0).max(1.0);
        let lt = (s * 12.0).max(1.0);
        let ct = (s * 9.0).max(1.0);
        filters.push(format!("hqdn3d={:.1}:{:.1}:{:.1}:{:.1}", ls, cs, lt, ct));
    }

    if filters.is_empty() {
        None
    } else {
        Some(filters.join(","))
    }
}

pub struct VideoDemuxer {
    metadata: MediaMetadata,
    child: Child,
    stdout: ChildStdout,
    frame_bytes: usize,
    current_frame: u64,
    high_bit_depth: bool,
}

impl VideoDemuxer {
    /// Spawns an FFmpeg decode process streaming uncompressed RGB frames from stdout.
    /// Auto-detects 10-bit / HDR source footage and enables 16-bit RGB48LE decoding.
    pub fn open<P: AsRef<Path>>(input_path: P) -> Result<Self> {
        let path = input_path.as_ref().to_path_buf();
        let metadata = MediaProbe::probe_file(&path)?;
        let high_bit = metadata.video_stream.is_high_bit_depth();
        let options = DemuxOptions {
            high_bit_depth: high_bit,
            ..Default::default()
        };
        Self::open_internal(path, metadata, options)
    }

    /// Spawns an FFmpeg decode process with explicit high bit depth configuration
    pub fn open_with_bit_depth<P: AsRef<Path>>(input_path: P, high_bit_depth: bool) -> Result<Self> {
        let path = input_path.as_ref().to_path_buf();
        let metadata = MediaProbe::probe_file(&path)?;
        let options = DemuxOptions {
            high_bit_depth,
            ..Default::default()
        };
        Self::open_internal(path, metadata, options)
    }

    /// Spawns an FFmpeg decode process with complete demux options (deinterlace, denoise, bit depth)
    pub fn open_with_options<P: AsRef<Path>>(input_path: P, options: DemuxOptions) -> Result<Self> {
        let path = input_path.as_ref().to_path_buf();
        let metadata = MediaProbe::probe_file(&path)?;
        Self::open_internal(path, metadata, options)
    }

    fn open_internal(path: std::path::PathBuf, metadata: MediaMetadata, options: DemuxOptions) -> Result<Self> {
        let width = metadata.video_stream.width;
        let height = metadata.video_stream.height;
        let bytes_per_pixel = if options.high_bit_depth { 6 } else { 3 };
        let frame_bytes = width * height * bytes_per_pixel;
        let pix_fmt = if options.high_bit_depth { "rgb48le" } else { "rgb24" };

        let ffmpeg_bin = FfmpegLocator::find_ffmpeg()?;
        let vf_filter = build_video_filter(&options, metadata.video_stream.is_interlaced());

        info!(
            "Demuxing video {:?} ({}x{}, {:.2} fps, {}) with filter: {:?} using {:?}",
            path, width, height, metadata.video_stream.frame_rate, pix_fmt, vf_filter, ffmpeg_bin
        );

        let mut cmd = Command::new(&ffmpeg_bin);
        cmd.args(["-hwaccel", "auto"]);

        if let Some(st) = options.start_time {
            if st > 0.001 {
                cmd.args(["-ss", &format!("{:.3}", st)]);
            }
        }

        cmd.args(["-i"]).arg(&path);

        if let Some(et) = options.end_time {
            let st = options.start_time.unwrap_or(0.0);
            let duration = (et - st).max(0.01);
            cmd.args(["-t", &format!("{:.3}", duration)]);
        }

        if let Some(ref vf) = vf_filter {
            cmd.args(["-vf", vf]);
        }

        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            pix_fmt,
            "-an", // Ignore audio in frame extraction stream
            "-sn",
            "-v",
            "error",
            "-",
        ]);

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn ffmpeg decoder for {:?}", path))?;

        let stdout = child.stdout.take().context("Failed to capture ffmpeg stdout pipe")?;

        Ok(Self {
            metadata,
            child,
            stdout,
            frame_bytes,
            current_frame: 0,
            high_bit_depth: options.high_bit_depth,
        })
    }

    pub fn metadata(&self) -> &MediaMetadata {
        &self.metadata
    }

    pub fn is_high_bit_depth(&self) -> bool {
        self.high_bit_depth
    }

    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Reads the next decoded raw RGB24 frame. Returns Ok(None) at EOF.
    pub fn read_frame(&mut self) -> Result<Option<Vec<u8>>> {
        let mut buffer = vec![0u8; self.frame_bytes];
        let mut bytes_read = 0;

        while bytes_read < self.frame_bytes {
            match self.stdout.read(&mut buffer[bytes_read..]) {
                Ok(0) => {
                    if bytes_read == 0 {
                        // Clean EOF
                        return Ok(None);
                    } else {
                        anyhow::bail!(
                            "Unexpected EOF: expected {} bytes for frame {}, got {}",
                            self.frame_bytes,
                            self.current_frame,
                            bytes_read
                        );
                    }
                }
                Ok(n) => {
                    bytes_read += n;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }

        self.current_frame += 1;
        Ok(Some(buffer))
    }
}

impl Drop for VideoDemuxer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
