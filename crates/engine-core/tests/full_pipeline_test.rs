use engine_core::{JobProgress, JobState, UpscaleJobRequest, UpscalePipeline};
use inference_runtime::ExecutionBackend;
use tokio::sync::mpsc;
use video_io::{FfmpegLocator, MediaProbe, OutputEncodingProfile};

#[tokio::test]
async fn test_full_video_upscale_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_engine_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_in = temp_dir.join("input_160x120.mp4");
    let test_out = temp_dir.join("upscaled_320x240.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate synthetic test video (1 sec, 30fps, 160x120) with audio
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
            "sine=frequency=880:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_in)
        .output()
        .expect("Failed to create test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Set up upscale job request targeting Compact SPAN (2x) on DirectML Adapter 0
    let request = UpscaleJobRequest {
        id: "job-test-001".to_string(),
        input_path: test_in.to_string_lossy().to_string(),
        output_path: test_out.to_string_lossy().to_string(),
        model_id: "compact-span-x2".to_string(),
        backend: ExecutionBackend::DirectMl { device_id: 0 },
        tile_size: Some(128),
        tile_padding: Some(16),
        film_grain: 0.12,
        temporal_mode: Some(engine_core::temporal::TemporalMode::Fast3Frame),
        interpolation_mode: None,
        deinterlace_mode: None,
        denoise_strength: None,
        trim_start: None,
        trim_end: None,
        encoding_profile: OutputEncodingProfile::default(),
    };


    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let pipeline = UpscalePipeline::new();

    // Spawn pipeline runner task
    let pipeline_handle = tokio::spawn(async move {
        pipeline.execute(request, tx).await
    });

    let mut final_progress = None;
    while let Some(progress) = rx.recv().await {
        println!(
            "Progress: {:.1}% | Frame {}/{} | {:.1} fps",
            progress.percentage, progress.current_frame, progress.total_frames, progress.current_fps
        );
        final_progress = Some(progress);
    }

    let result = pipeline_handle.await.expect("Task join error");
    assert!(result.is_ok(), "Pipeline execution failed: {:?}", result.err());

    let last_prog = final_progress.expect("Expected at least one progress update");
    assert_eq!(last_prog.state, JobState::Completed);
    assert_eq!(last_prog.percentage, 100.0);

    // 3. Verify output video resolution and streams
    assert!(test_out.exists(), "Output video file must exist");
    let out_meta = MediaProbe::probe_file(&test_out).expect("Failed to probe output video");

    assert_eq!(out_meta.video_stream.width, 320, "Width should be 2x upscaled (320)");
    assert_eq!(out_meta.video_stream.height, 240, "Height should be 2x upscaled (240)");
    assert!(!out_meta.audio_streams.is_empty(), "Audio stream should be preserved");

    // Clean up
    let _ = std::fs::remove_file(test_in);
    let _ = std::fs::remove_file(test_out);
}

#[tokio::test]
async fn test_full_10bit_hdr10_upscale_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_engine_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_in = temp_dir.join("input_hdr_160x120.mp4");
    let test_out = temp_dir.join("upscaled_hdr_320x240.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate synthetic 10-bit test video (10 frames, 160x120) with audio
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.33:size=160x120:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.33",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_in)
        .output()
        .expect("Failed to create test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Set up upscale job request targeting 10-bit HDR10 export profile
    let hdr_profile = OutputEncodingProfile::hdr10(
        video_io::HardwareEncoder::SoftwareCpu(video_io::VideoCodecType::H265Hevc),
        22,
    );

    let request = UpscaleJobRequest {
        id: "job-hdr-test-002".to_string(),
        input_path: test_in.to_string_lossy().to_string(),
        output_path: test_out.to_string_lossy().to_string(),
        model_id: "compact-span-x2".to_string(),
        backend: ExecutionBackend::DirectMl { device_id: 0 },
        tile_size: Some(128),
        tile_padding: Some(16),
        film_grain: 0.05,
        temporal_mode: Some(engine_core::temporal::TemporalMode::Fast3Frame),
        interpolation_mode: None,
        deinterlace_mode: None,
        denoise_strength: None,
        trim_start: None,
        trim_end: None,
        encoding_profile: hdr_profile,
    };

    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let pipeline = UpscalePipeline::new();

    let pipeline_handle = tokio::spawn(async move {
        pipeline.execute(request, tx).await
    });

    let mut final_progress = None;
    while let Some(progress) = rx.recv().await {
        final_progress = Some(progress);
    }

    let result = pipeline_handle.await.expect("Task join error");
    assert!(result.is_ok(), "Pipeline execution failed: {:?}", result.err());

    let last_prog = final_progress.expect("Expected progress");
    assert_eq!(last_prog.state, JobState::Completed);

    // 3. Verify output video is 10-bit HDR10 and 2x resolution
    assert!(test_out.exists(), "Output HDR file must exist");
    let out_meta = MediaProbe::probe_file(&test_out).expect("Failed to probe output video");

    assert_eq!(out_meta.video_stream.width, 320);
    assert_eq!(out_meta.video_stream.height, 240);
    assert_eq!(out_meta.video_stream.pixel_format, "yuv420p10le");
    assert!(out_meta.video_stream.is_high_bit_depth());
    assert!(out_meta.video_stream.is_hdr());

    let _ = std::fs::remove_file(test_in);
    let _ = std::fs::remove_file(test_out);
}

#[tokio::test]
async fn test_motion_interpolation_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_engine_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_in = temp_dir.join("input_interp_160x120.mp4");
    let test_out = temp_dir.join("interp_output_48fps.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate 10-frame synthetic test video at 24 fps
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.416:size=160x120:rate=24",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=500:duration=0.416",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_in)
        .output()
        .expect("Failed to create test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Set up upscale job request targeting 2x Motion Interpolation (24 fps -> 48 fps)
    let request = UpscaleJobRequest {
        id: "job-interp-test-003".to_string(),
        input_path: test_in.to_string_lossy().to_string(),
        output_path: test_out.to_string_lossy().to_string(),
        model_id: "compact-span-x2".to_string(),
        backend: ExecutionBackend::DirectMl { device_id: 0 },
        tile_size: Some(128),
        tile_padding: Some(16),
        film_grain: 0.0,
        temporal_mode: Some(engine_core::temporal::TemporalMode::Disabled),
        interpolation_mode: Some(engine_core::interpolation::InterpolationMode::DoubleFps),
        deinterlace_mode: None,
        denoise_strength: None,
        trim_start: None,
        trim_end: None,
        encoding_profile: OutputEncodingProfile::default(),
    };

    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let pipeline = UpscalePipeline::new();

    let pipeline_handle = tokio::spawn(async move {
        pipeline.execute(request, tx).await
    });

    let mut final_progress = None;
    while let Some(progress) = rx.recv().await {
        final_progress = Some(progress);
    }

    let result = pipeline_handle.await.expect("Task join error");
    assert!(result.is_ok(), "Pipeline execution failed: {:?}", result.err());

    let last_prog = final_progress.expect("Expected progress");
    assert_eq!(last_prog.state, JobState::Completed);

    // 3. Verify output video has doubled framerate (48 fps) and doubled frames
    assert!(test_out.exists(), "Output interpolated file must exist");
    let out_meta = MediaProbe::probe_file(&test_out).expect("Failed to probe output video");

    assert_eq!(out_meta.video_stream.width, 320);
    assert_eq!(out_meta.video_stream.height, 240);
    assert!(
        (out_meta.video_stream.frame_rate - 48.0).abs() < 1.0,
        "Expected output frame rate ~48 fps, got {:.2}",
        out_meta.video_stream.frame_rate
    );
    assert!(!out_meta.audio_streams.is_empty(), "Audio track must be preserved");

    let _ = std::fs::remove_file(test_in);
    let _ = std::fs::remove_file(test_out);
}

#[tokio::test]
async fn test_deinterlace_and_denoise_upscale_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_engine_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_in = temp_dir.join("input_deint_160x120.mp4");
    let test_out = temp_dir.join("deint_denoised_output.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate 10-frame synthetic test video at 30 fps
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.33:size=160x120:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=600:duration=0.33",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_in)
        .output()
        .expect("Failed to create test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Set up upscale job request with bwdif deinterlacing and hqdn3d denoising
    let request = UpscaleJobRequest {
        id: "job-deint-test-004".to_string(),
        input_path: test_in.to_string_lossy().to_string(),
        output_path: test_out.to_string_lossy().to_string(),
        model_id: "compact-span-x2".to_string(),
        backend: ExecutionBackend::DirectMl { device_id: 0 },
        tile_size: Some(128),
        tile_padding: Some(16),
        film_grain: 0.05,
        temporal_mode: Some(engine_core::temporal::TemporalMode::Fast3Frame),
        interpolation_mode: None,
        deinterlace_mode: Some(video_io::DeinterlaceMode::BwdifSameRate),
        denoise_strength: Some(0.35),
        trim_start: None,
        trim_end: None,
        encoding_profile: OutputEncodingProfile::default(),
    };

    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let pipeline = UpscalePipeline::new();

    let pipeline_handle = tokio::spawn(async move {
        pipeline.execute(request, tx).await
    });

    let mut final_progress = None;
    while let Some(progress) = rx.recv().await {
        final_progress = Some(progress);
    }

    let result = pipeline_handle.await.expect("Task join error");
    assert!(result.is_ok(), "Pipeline execution failed: {:?}", result.err());

    let last_prog = final_progress.expect("Expected progress");
    assert_eq!(last_prog.state, JobState::Completed);

    // 3. Verify output video is 2x resolution and audio is intact
    assert!(test_out.exists(), "Output deinterlaced file must exist");
    let out_meta = MediaProbe::probe_file(&test_out).expect("Failed to probe output video");

    assert_eq!(out_meta.video_stream.width, 320);
    assert_eq!(out_meta.video_stream.height, 240);
    assert!(!out_meta.audio_streams.is_empty(), "Audio track must be preserved");

    let _ = std::fs::remove_file(test_in);
    let _ = std::fs::remove_file(test_out);
}

#[tokio::test]
async fn test_trimmed_range_upscale_pipeline() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_engine_tests");
    let _ = std::fs::create_dir_all(&temp_dir);

    let test_in = temp_dir.join("input_full_1sec.mp4");
    let test_out = temp_dir.join("trimmed_upscaled.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate 1.0-second test video (30 fps = 30 frames) with tone audio
    let gen_status = std::process::Command::new(&ffmpeg_bin)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1.0:size=160x120:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1.0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&test_in)
        .output()
        .expect("Failed to create test video");

    assert!(gen_status.status.success(), "FFmpeg generation failed");

    // 2. Set up upscale job request targeting trimmed range: 0.2s to 0.6s (0.4s duration = ~12 frames)
    let request = UpscaleJobRequest {
        id: "job-trim-test-005".to_string(),
        input_path: test_in.to_string_lossy().to_string(),
        output_path: test_out.to_string_lossy().to_string(),
        model_id: "compact-span-x2".to_string(),
        backend: ExecutionBackend::DirectMl { device_id: 0 },
        tile_size: Some(128),
        tile_padding: Some(16),
        film_grain: 0.0,
        temporal_mode: Some(engine_core::temporal::TemporalMode::Disabled),
        interpolation_mode: None,
        deinterlace_mode: None,
        denoise_strength: None,
        trim_start: Some(0.2),
        trim_end: Some(0.6),
        encoding_profile: OutputEncodingProfile::default(),
    };

    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let pipeline = UpscalePipeline::new();

    let pipeline_handle = tokio::spawn(async move {
        pipeline.execute(request, tx).await
    });

    let mut final_progress = None;
    while let Some(progress) = rx.recv().await {
        final_progress = Some(progress);
    }

    let result = pipeline_handle.await.expect("Task join error");
    assert!(result.is_ok(), "Pipeline execution failed: {:?}", result.err());

    let last_prog = final_progress.expect("Expected progress");
    assert_eq!(last_prog.state, JobState::Completed);

    // 3. Verify output trimmed video properties
    assert!(test_out.exists(), "Output trimmed file must exist");
    let out_meta = MediaProbe::probe_file(&test_out).expect("Failed to probe output video");

    assert_eq!(out_meta.video_stream.width, 320);
    assert_eq!(out_meta.video_stream.height, 240);
    assert!(
        out_meta.duration_seconds < 0.7,
        "Expected trimmed duration < 0.7s, got {:.2}s",
        out_meta.duration_seconds
    );
    assert!(!out_meta.audio_streams.is_empty(), "Audio track must be preserved in trimmed output");

    let _ = std::fs::remove_file(test_in);
    let _ = std::fs::remove_file(test_out);
}


