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
