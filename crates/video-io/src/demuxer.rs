use crate::ffmpeg_bin::FfmpegLocator;
use crate::metadata::{MediaMetadata, MediaProbe};
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use tracing::info;

pub struct VideoDemuxer {
    metadata: MediaMetadata,
    child: Child,
    stdout: ChildStdout,
    frame_bytes: usize,
    current_frame: u64,
}

impl VideoDemuxer {
    /// Spawns an FFmpeg decode process streaming uncompressed RGB24 frames from stdout
    pub fn open<P: AsRef<Path>>(input_path: P) -> Result<Self> {
        let path = input_path.as_ref().to_path_buf();
        let metadata = MediaProbe::probe_file(&path)?;

        let width = metadata.video_stream.width;
        let height = metadata.video_stream.height;
        let frame_bytes = width * height * 3;

        let ffmpeg_bin = FfmpegLocator::find_ffmpeg()?;
        info!(
            "Demuxing video {:?} ({}x{}, {:.2} fps) using {:?}",
            path, width, height, metadata.video_stream.frame_rate, ffmpeg_bin
        );

        let mut child = Command::new(&ffmpeg_bin)
            .args([
                "-hwaccel",
                "auto",
                "-i",
            ])
            .arg(&path)
            .args([
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-an", // Ignore audio in frame extraction stream
                "-sn",
                "-v",
                "error",
                "-",
            ])
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
        })
    }

    pub fn metadata(&self) -> &MediaMetadata {
        &self.metadata
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
