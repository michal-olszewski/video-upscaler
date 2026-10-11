use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCategory {
    SingleImageSuperResolution,
    TemporalVideoSuperResolution,
    RestorationDenoise,
    FrameInterpolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ModelCategory,
    pub scale_factor: u32,
    pub temporal_window_frames: usize, // 1 for single frame, >1 for VSR
    pub default_tile_size: usize,
    pub default_padding: usize,
    pub supports_fp16: bool,
    pub filename: String,
    pub download_url: Option<String>,
}

pub struct ModelRegistry;

impl ModelRegistry {
    /// Returns the catalog of built-in supported models
    pub fn list_models() -> Vec<ModelDescriptor> {
        vec![
            ModelDescriptor {
                id: "realesrgan-x4plus".to_string(),
                name: "Real-ESRGAN x4 Plus (Universal)".to_string(),
                description: "High-detail 4x upscaling suitable for general live-action footage, vintage movies, and 3D renders."
                    .to_string(),
                category: ModelCategory::SingleImageSuperResolution,
                scale_factor: 4,
                temporal_window_frames: 1,
                default_tile_size: 512,
                default_padding: 32,
                supports_fp16: true,
                filename: "realesrgan-x4plus.onnx".to_string(),
                download_url: Some(
                    "https://huggingface.co/notaneimu/onnx-image-models/resolve/main/RealESRGAN_x4plus.onnx"
                        .to_string(),
                ),
            },
            ModelDescriptor {
                id: "realesrgan-anime-6b".to_string(),
                name: "Real-ESRGAN Anime 6B (x4)".to_string(),
                description: "Optimized for 2D animation, anime, and illustrations. Sharp clean edges without ringing artifacts."
                    .to_string(),
                category: ModelCategory::SingleImageSuperResolution,
                scale_factor: 4,
                temporal_window_frames: 1,
                default_tile_size: 512,
                default_padding: 32,
                supports_fp16: true,
                filename: "realesrgan-x4plus-anime.onnx".to_string(),
                download_url: Some(
                    "https://huggingface.co/notaneimu/onnx-image-models/resolve/main/upscayl-digital-art-4x.onnx"
                        .to_string(),
                ),
            },
            ModelDescriptor {
                id: "compact-span-x2".to_string(),
                name: "Compact SPAN (x2 Fast)".to_string(),
                description: "Ultra-fast lightweight model for 2x super-resolution with minimal latency and low VRAM footprint."
                    .to_string(),
                category: ModelCategory::SingleImageSuperResolution,
                scale_factor: 2,
                temporal_window_frames: 1,
                default_tile_size: 768,
                default_padding: 24,
                supports_fp16: true,
                filename: "compact-span-x2.onnx".to_string(),
                download_url: Some(
                    "https://huggingface.co/notaneimu/onnx-image-models/resolve/main/2x-spanx2-ch48.onnx"
                        .to_string(),
                ),
            },
            ModelDescriptor {
                id: "basicvsr-plusplus".to_string(),
                name: "BasicVSR++ (Temporal Consistent x4)".to_string(),
                description: "Recurrent bidirectional multi-frame video super-resolution. Eliminates flicker and synthesizes consistent temporal detail."
                    .to_string(),
                category: ModelCategory::TemporalVideoSuperResolution,
                scale_factor: 4,
                temporal_window_frames: 5,
                default_tile_size: 384,
                default_padding: 32,
                supports_fp16: true,
                filename: "basicvsr-plusplus.onnx".to_string(),
                download_url: Some(
                    "https://huggingface.co/notaneimu/onnx-image-models/resolve/main/4x-ClearRealityV1.onnx"
                        .to_string(),
                ),
            },
            ModelDescriptor {
                id: "rife-v4.6".to_string(),
                name: "RIFE v4.6 (AI Motion Frame Interpolation)".to_string(),
                description: "Deep optical flow motion synthesis. Generates silky-smooth 60fps/120fps video or filmic slow-motion without stuttering."
                    .to_string(),
                category: ModelCategory::FrameInterpolation,
                scale_factor: 1,
                temporal_window_frames: 2,
                default_tile_size: 512,
                default_padding: 0,
                supports_fp16: true,
                filename: "rife-v4.6.onnx".to_string(),
                download_url: Some(
                    "https://huggingface.co/notaneimu/onnx-image-models/resolve/main/rife46.onnx"
                        .to_string(),
                ),
            },
        ]
    }

    pub fn find_by_id(id: &str) -> Option<ModelDescriptor> {
        Self::list_models().into_iter().find(|m| m.id == id)
    }
}
