use video_io::{
    FfmpegLocator, HardwareEncoder, MediaProbe, OutputEncodingProfile, VideoCodecType,
    VideoDemuxer, VideoMuxer,
};

#[test]
fn test_ffmpeg_binary_discovery() {
    let ffmpeg = FfmpegLocator::find_ffmpeg();
    assert!(ffmpeg.is_ok(), "Failed to locate ffmpeg: {:?}", ffmpeg.err());

    let ffprobe = FfmpegLocator::find_ffprobe();
    assert!(ffprobe.is_ok(), "Failed to locate ffprobe: {:?}", ffprobe.err());
}

#[test]
fn test_video_demux_and_mux_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_input = temp_dir.join("synthetic_input.mp4");
    let test_output = temp_dir.join("synthetic_output.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("ffmpeg must be found");

    // 1. Generate a 1-second 30fps 160x120 test video with a sine wave audio track
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=160x120:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_input)
        .output()
        .expect("Failed to generate synthetic test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Probe metadata
    let metadata = MediaProbe::probe_file(&test_input).expect("Failed to probe test video");
    assert_eq!(metadata.video_stream.width, 160);
    assert_eq!(metadata.video_stream.height, 120);
    assert!((metadata.video_stream.frame_rate - 30.0).abs() < 0.1);
    assert!(!metadata.audio_streams.is_empty(), "Audio stream should be detected");

    // 3. Demux video frames
    let mut demuxer = VideoDemuxer::open(&test_input).expect("Failed to open demuxer");
    let mut frames = Vec::new();

    while let Some(frame) = demuxer.read_frame().expect("Frame read error") {
        assert_eq!(frame.len(), 160 * 120 * 3);
        frames.push(frame);
    }

    assert_eq!(frames.len(), 30, "Expected exactly 30 frames for a 1-second video");

    // 4. Mux frames to 2x resolution (320x240) with software fallback profile
    let mut profile = OutputEncodingProfile::default();
    profile.encoder = HardwareEncoder::SoftwareCpu(VideoCodecType::H264);
    profile.crf = 20;

    let mut muxer = VideoMuxer::open(
        &test_output,
        320,
        240,
        30.0,
        &profile,
        Some(&test_input),
    )
    .expect("Failed to open muxer");

    for frame in frames {
        // Simple 2x nearest-neighbor upscale for test
        let mut upscaled = vec![0u8; 320 * 240 * 3];
        for y in 0..240 {
            let src_y = y / 2;
            for x in 0..320 {
                let src_x = x / 2;
                let src_idx = (src_y * 160 + src_x) * 3;
                let dst_idx = (y * 320 + x) * 3;
                upscaled[dst_idx] = frame[src_idx];
                upscaled[dst_idx + 1] = frame[src_idx + 1];
                upscaled[dst_idx + 2] = frame[src_idx + 2];
            }
        }
        muxer.write_frame(&upscaled).expect("Write frame error");
    }

    let written = muxer.finish().expect("Finish muxing error");
    assert_eq!(written, 30);

    // 5. Verify output video metadata
    let out_meta = MediaProbe::probe_file(&test_output).expect("Probe output video error");
    assert_eq!(out_meta.video_stream.width, 320);
    assert_eq!(out_meta.video_stream.height, 240);
    assert!(!out_meta.audio_streams.is_empty(), "Audio track should be preserved in output");

    // Clean up temporary test files
    let _ = std::fs::remove_file(test_input);
    let _ = std::fs::remove_file(test_output);
}

#[test]
fn test_10bit_hdr10_pipeline_muxing() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_output = temp_dir.join("synthetic_hdr10_output.mp4");

    // 1. Configure 10-bit HDR10 profile (BT.2020 + SMPTE 2084 PQ)
    let profile = OutputEncodingProfile::hdr10(
        HardwareEncoder::SoftwareCpu(VideoCodecType::H265Hevc),
        24,
    );
    assert!(profile.is_hdr());
    assert!(profile.high_bit_depth);
    assert_eq!(profile.pixel_format, "yuv420p10le");

    // 2. Open muxer for 10-bit HDR10 (160x120 @ 30fps)
    let mut muxer = VideoMuxer::open(
        &test_output,
        160,
        120,
        30.0,
        &profile,
        None::<&std::path::Path>,
    )
    .expect("Failed to open 10-bit HDR10 muxer");

    // 3. Generate 10 frames of 16-bit RGB48LE gradient data (160 * 120 * 6 bytes per frame)
    let frame_bytes_count = 160 * 120 * 6;
    let mut rgb48_frame = vec![0u8; frame_bytes_count];
    for y in 0..120 {
        for x in 0..160 {
            let idx = (y * 160 + x) * 6;
            // 10-bit value mapped into 16-bit range [0..65535]
            let val = ((x * 65535) / 160) as u16;
            rgb48_frame[idx..idx + 2].copy_from_slice(&val.to_le_bytes());
            rgb48_frame[idx + 2..idx + 4].copy_from_slice(&val.to_le_bytes());
            rgb48_frame[idx + 4..idx + 6].copy_from_slice(&val.to_le_bytes());
        }
    }

    for _ in 0..10 {
        muxer.write_frame(&rgb48_frame).expect("Failed to write 10-bit frame");
    }

    let written = muxer.finish().expect("Failed to finish 10-bit muxing");
    assert_eq!(written, 10);

    // 4. Verify probe detects 10-bit depth and HDR metadata tags
    let out_meta = MediaProbe::probe_file(&test_output).expect("Probe 10-bit HDR10 output error");
    assert_eq!(out_meta.video_stream.width, 160);
    assert_eq!(out_meta.video_stream.height, 120);
    assert!(
        out_meta.video_stream.is_high_bit_depth(),
        "Expected high bit depth stream, got pix_fmt: {}",
        out_meta.video_stream.pixel_format
    );
    assert_eq!(out_meta.video_stream.pixel_format, "yuv420p10le");
    assert!(out_meta.video_stream.is_hdr(), "Expected stream to be flagged as HDR");

    let _ = std::fs::remove_file(test_output);
}

