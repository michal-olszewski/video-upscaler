use engine_core::PreviewEngine;
use inference_runtime::ExecutionBackend;
use video_io::FfmpegLocator;

#[test]
fn test_single_frame_preview_generation() {
    let temp_dir = std::env::temp_dir().join("video_upscaler_preview_tests");
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_video = temp_dir.join("preview_input.mp4");

    let ffmpeg_bin = FfmpegLocator::find_ffmpeg().expect("FFmpeg must be found");

    // 1. Generate test video
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
        .arg(&test_video)
        .output()
        .expect("Failed to create preview test video");

    assert!(gen_status.status.success());

    // 2. Extract and upscale preview frame
    let preview = PreviewEngine::generate_preview(
        &test_video,
        0.0,
        "compact-span-x2",
        ExecutionBackend::DirectMl { device_id: 0 },
        0.1,
    )
    .expect("Failed to generate preview");

    assert_eq!(preview.in_width, 160);
    assert_eq!(preview.in_height, 120);
    assert_eq!(preview.out_width, 320);
    assert_eq!(preview.out_height, 240);
    assert!(preview.original_data_url.starts_with("data:image/png;base64,"));
    assert!(preview.upscaled_data_url.starts_with("data:image/png;base64,"));
    assert!(preview.original_data_url.len() > 100);
    assert!(preview.upscaled_data_url.len() > 100);

    let _ = std::fs::remove_file(test_video);
}
