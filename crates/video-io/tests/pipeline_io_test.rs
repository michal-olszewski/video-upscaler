use video_io::{
    build_video_filter, AudioCodec, AudioEncodingProfile, DeinterlaceMode, DemuxOptions,
    FfmpegLocator, FitMode, HardwareEncoder, MediaProbe, OutputEncodingProfile,
    StabilizationBorderMode, StabilizationMode, StabilizationOptions, TargetGeometryProfile,
    TargetResolution, VideoCodecType, VideoDemuxer, VideoMuxer, VideoStabilizer,
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

#[test]
fn test_deinterlace_and_denoise_demuxing() {
    // 1. Verify filter string generation logic
    let filter_none = build_video_filter(&DemuxOptions::default(), false);
    assert_eq!(filter_none, None);

    let opts_bwdif = DemuxOptions {
        deinterlace_mode: DeinterlaceMode::BwdifSameRate,
        ..Default::default()
    };
    assert_eq!(
        build_video_filter(&opts_bwdif, false),
        Some("bwdif=mode=0:parity=-1:deint=0".to_string())
    );

    let opts_denoise = DemuxOptions {
        denoise_strength: 0.5,
        ..Default::default()
    };
    assert_eq!(
        build_video_filter(&opts_denoise, false),
        Some("hqdn3d=4.0:3.0:6.0:4.5".to_string())
    );

    let opts_combined = DemuxOptions {
        deinterlace_mode: DeinterlaceMode::BwdifSameRate,
        denoise_strength: 0.5,
        ..Default::default()
    };
    assert_eq!(
        build_video_filter(&opts_combined, false),
        Some("bwdif=mode=0:parity=-1:deint=0,hqdn3d=4.0:3.0:6.0:4.5".to_string())
    );

    // 2. Test actual demuxing with ffmpeg on a synthetic test video
    let temp_dir = std::env::temp_dir().join("video_upscaler_tests");
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_input = temp_dir.join("synthetic_interlace_test.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("ffmpeg must be found");
    let status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "testsrc=duration=0.5:size=160x120:rate=30",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
        ])
        .arg(&test_input)
        .status()
        .expect("Failed to execute ffmpeg");
    assert!(status.success());

    let mut demuxer = VideoDemuxer::open_with_options(&test_input, opts_combined)
        .expect("Failed to open demuxer with deinterlace and denoise options");

    let mut count = 0;
    while let Some(frame) = demuxer.read_frame().expect("Read frame failed") {
        assert_eq!(frame.len(), 160 * 120 * 3);
        count += 1;
    }
    assert_eq!(count, 15, "Expected 15 frames for 0.5s @ 30fps with same-rate deinterlacer");

    let _ = std::fs::remove_file(test_input);
}

#[test]
fn test_phase19_audio_mastering_and_loudness_normalization() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_audio_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_input = temp_dir.join("audio_test_input.mp4");
    let test_output_norm = temp_dir.join("audio_test_norm.mp4");
    let test_output_mute = temp_dir.join("audio_test_mute.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("ffmpeg must be found");

    // 1. Generate 0.5s synthetic test video with 1kHz tone
    let status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "testsrc=duration=0.5:size=160x120:rate=30",
            "-f", "lavfi",
            "-i", "sine=frequency=1000:duration=0.5",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
        ])
        .arg(&test_input)
        .status()
        .expect("Failed to create test audio video");
    assert!(status.success());

    // 2. Test AAC mastering with EBU R128 loudness normalization (-14 LUFS)
    let mut profile_norm = OutputEncodingProfile::default();
    profile_norm.encoder = HardwareEncoder::SoftwareCpu(VideoCodecType::H264);
    profile_norm.audio_profile = Some(AudioEncodingProfile {
        codec: AudioCodec::Aac,
        bitrate_kbps: 192,
        loudness_normalization: true,
        target_lufs: -14.0,
    });

    let mut muxer_norm = VideoMuxer::open(
        &test_output_norm,
        160,
        120,
        30.0,
        &profile_norm,
        Some(&test_input),
    )
    .expect("Failed to open muxer with loudness normalization");

    let dummy_frame = vec![128u8; 160 * 120 * 3];
    for _ in 0..15 {
        muxer_norm.write_frame(&dummy_frame).expect("Write frame failed");
    }
    muxer_norm.finish().expect("Finish muxer failed");

    let probed_norm = MediaProbe::probe_file(&test_output_norm).expect("Probe failed on normalized file");
    assert_eq!(probed_norm.audio_streams.len(), 1, "Expected 1 audio stream");
    assert_eq!(probed_norm.audio_streams[0].codec_name, "aac");

    // 3. Test Mute audio mastering
    let mut profile_mute = OutputEncodingProfile::default();
    profile_mute.encoder = HardwareEncoder::SoftwareCpu(VideoCodecType::H264);
    profile_mute.audio_profile = Some(AudioEncodingProfile {
        codec: AudioCodec::Mute,
        bitrate_kbps: 0,
        loudness_normalization: false,
        target_lufs: -14.0,
    });

    let mut muxer_mute = VideoMuxer::open(
        &test_output_mute,
        160,
        120,
        30.0,
        &profile_mute,
        Some(&test_input),
    )
    .expect("Failed to open muxer with mute profile");

    for _ in 0..15 {
        muxer_mute.write_frame(&dummy_frame).expect("Write frame failed");
    }
    muxer_mute.finish().expect("Finish mute muxer failed");

    let probed_mute = MediaProbe::probe_file(&test_output_mute).expect("Probe failed on muted file");
    assert_eq!(probed_mute.audio_streams.len(), 0, "Expected 0 audio streams when muted");

    let _ = std::fs::remove_file(test_input);
    let _ = std::fs::remove_file(test_output_norm);
    let _ = std::fs::remove_file(test_output_mute);
}

#[test]
fn test_phase20_target_geometry_and_aspect_ratio_fitting() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_geom_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_output_letterbox = temp_dir.join("geom_letterbox_720p.mp4");
    let test_output_crop = temp_dir.join("geom_crop_custom.mp4");

    // 1. Test Preset720p with Letterbox fitting (160x120 input -> 1280x720 letterboxed output)
    let mut profile_box = OutputEncodingProfile::default();
    profile_box.encoder = HardwareEncoder::SoftwareCpu(VideoCodecType::H264);
    profile_box.geometry_profile = Some(TargetGeometryProfile {
        resolution: TargetResolution::Preset720p,
        fit_mode: FitMode::Letterbox,
        matte_color: "black".to_string(),
    });

    let mut muxer_box = VideoMuxer::open(
        &test_output_letterbox,
        160,
        120,
        30.0,
        &profile_box,
        None::<&std::path::Path>,
    )
    .expect("Failed to open muxer with letterbox geometry");

    let dummy_frame = vec![200u8; 160 * 120 * 3];
    for _ in 0..10 {
        muxer_box.write_frame(&dummy_frame).expect("Write frame failed");
    }
    muxer_box.finish().expect("Finish letterbox muxer failed");

    let probed_box = MediaProbe::probe_file(&test_output_letterbox).expect("Probe failed on letterbox file");
    assert_eq!(probed_box.video_stream.width, 1280, "Expected width 1280");
    assert_eq!(probed_box.video_stream.height, 720, "Expected height 720");

    // 2. Test Custom resolution with CenterCrop fitting (160x120 -> 200x200 square)
    let mut profile_crop = OutputEncodingProfile::default();
    profile_crop.encoder = HardwareEncoder::SoftwareCpu(VideoCodecType::H264);
    profile_crop.geometry_profile = Some(TargetGeometryProfile {
        resolution: TargetResolution::Custom { width: 200, height: 200 },
        fit_mode: FitMode::CenterCrop,
        matte_color: "black".to_string(),
    });

    let mut muxer_crop = VideoMuxer::open(
        &test_output_crop,
        160,
        120,
        30.0,
        &profile_crop,
        None::<&std::path::Path>,
    )
    .expect("Failed to open muxer with center crop geometry");

    for _ in 0..10 {
        muxer_crop.write_frame(&dummy_frame).expect("Write frame failed");
    }
    muxer_crop.finish().expect("Finish crop muxer failed");

    let probed_crop = MediaProbe::probe_file(&test_output_crop).expect("Probe failed on crop file");
    assert_eq!(probed_crop.video_stream.width, 200, "Expected width 200");
    assert_eq!(probed_crop.video_stream.height, 200, "Expected height 200");

    let _ = std::fs::remove_file(test_output_letterbox);
    let _ = std::fs::remove_file(test_output_crop);
}

#[test]
fn test_stabilization_filter_generation() {
    let dummy_trf = std::path::PathBuf::from("C:\\temp\\motion_transforms.trf");
    let options = StabilizationOptions {
        mode: StabilizationMode::SmoothMotion,
        smoothing: 25,
        shakiness: 7,
        accuracy: 10,
        stepsize: 6,
        border_mode: StabilizationBorderMode::AutoZoom,
        zoom_percent: 2.5,
    };

    let detect = options.build_detect_filter(&dummy_trf);
    assert!(detect.contains("vidstabdetect"));
    assert!(detect.contains("shakiness=7"));
    assert!(detect.contains("accuracy=10"));
    assert!(detect.contains("motion_transforms.trf"));

    let transform = options.build_transform_filter(&dummy_trf);
    assert!(transform.contains("vidstabtransform"));
    assert!(transform.contains("smoothing=25"));
    assert!(transform.contains("optzoom=1"));
    assert!(transform.contains("zoom=2.5"));

    // Test Tripod mode filter
    let tripod_options = StabilizationOptions {
        mode: StabilizationMode::TripodStatic,
        smoothing: 0,
        shakiness: 8,
        accuracy: 12,
        stepsize: 4,
        border_mode: StabilizationBorderMode::Black,
        zoom_percent: 0.0,
    };
    let tripod_detect = tripod_options.build_detect_filter(&dummy_trf);
    assert!(tripod_detect.contains("tripod=1"));
    let tripod_transform = tripod_options.build_transform_filter(&dummy_trf);
    assert!(tripod_transform.contains("tripod=1"));
    assert!(tripod_transform.contains("crop=black"));
}

#[test]
fn test_stabilization_two_pass_execution() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_stab_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_input = temp_dir.join("stab_synthetic_input.mp4");
    let transforms_trf = temp_dir.join("stab_transforms.trf");
    let ffmpeg_bin = match FfmpegLocator::find_ffmpeg() {
        Ok(bin) => bin,
        Err(_) => return, // Skip test if ffmpeg is not available on environment
    };

    // Generate 1-second 30fps test video
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=160x120:rate=30",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&test_input)
        .output();

    if gen_status.is_err() || !gen_status.as_ref().unwrap().status.success() {
        return;
    }

    let stab_options = StabilizationOptions {
        mode: StabilizationMode::SmoothMotion,
        smoothing: 15,
        shakiness: 5,
        accuracy: 8,
        stepsize: 6,
        border_mode: StabilizationBorderMode::AutoZoom,
        zoom_percent: 0.0,
    };

    // Pass 1: analyze motion
    let analyze_res = VideoStabilizer::analyze_motion(&test_input, &transforms_trf, &stab_options, None, None);
    assert!(analyze_res.is_ok(), "Motion analysis failed: {:?}", analyze_res.err());
    assert!(transforms_trf.exists(), "Transforms file must exist");

    // Pass 2: Demux frames with stabilization transform filter
    let demux_opts = DemuxOptions {
        stabilization: Some(stab_options),
        stabilization_transforms_path: Some(transforms_trf.clone()),
        ..Default::default()
    };

    let demux_res = VideoDemuxer::open_with_options(&test_input, demux_opts);
    assert!(demux_res.is_ok(), "Failed to open demuxer with stabilization: {:?}", demux_res.err());
    let mut demuxer = demux_res.unwrap();

    let mut frame_count = 0;
    while let Ok(Some(frame)) = demuxer.read_frame() {
        assert_eq!(frame.len(), 160 * 120 * 3);
        frame_count += 1;
    }
    assert_eq!(frame_count, 30, "Expected 30 stabilized frames");

    let _ = std::fs::remove_file(test_input);
    let _ = std::fs::remove_file(transforms_trf);
}



