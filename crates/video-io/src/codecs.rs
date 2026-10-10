use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodecType {
    H264,
    H265Hevc,
    Av1,
    ProRes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareEncoder {
    NvidiaNvenc(VideoCodecType),
    AmdAmf(VideoCodecType),
    AppleVideoToolbox(VideoCodecType),
    SoftwareCpu(VideoCodecType),
}

impl HardwareEncoder {
    pub fn ffmpeg_encoder_name(&self) -> &'static str {
        match self {
            Self::NvidiaNvenc(VideoCodecType::H264) => "h264_nvenc",
            Self::NvidiaNvenc(VideoCodecType::H265Hevc) => "hevc_nvenc",
            Self::NvidiaNvenc(VideoCodecType::Av1) => "av1_nvenc",
            Self::AmdAmf(VideoCodecType::H264) => "h264_amf",
            Self::AmdAmf(VideoCodecType::H265Hevc) => "hevc_amf",
            Self::AmdAmf(VideoCodecType::Av1) => "av1_amf",
            Self::AppleVideoToolbox(VideoCodecType::H264) => "h264_videotoolbox",
            Self::AppleVideoToolbox(VideoCodecType::H265Hevc) => "hevc_videotoolbox",
            Self::SoftwareCpu(VideoCodecType::H264) => "libx264",
            Self::SoftwareCpu(VideoCodecType::H265Hevc) => "libx265",
            Self::SoftwareCpu(VideoCodecType::Av1) => "libsvtav1",
            Self::SoftwareCpu(VideoCodecType::ProRes) | Self::AppleVideoToolbox(VideoCodecType::ProRes) | Self::NvidiaNvenc(VideoCodecType::ProRes) | Self::AmdAmf(VideoCodecType::ProRes) => "prores_ks",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEncodingProfile {
    pub encoder: HardwareEncoder,
    pub crf: u32,
    pub bitrate_kbps: Option<u32>,
    pub preset: String, // e.g. "p7" for nvenc, "slow" for x264/x265
    pub pixel_format: String, // e.g. "yuv420p", "yuv420p10le"
    pub copy_audio: bool,
    pub copy_subtitles: bool,
}

impl Default for OutputEncodingProfile {
    fn default() -> Self {
        Self {
            encoder: HardwareEncoder::SoftwareCpu(VideoCodecType::H265Hevc),
            crf: 18,
            bitrate_kbps: None,
            preset: "medium".to_string(),
            pixel_format: "yuv420p".to_string(),
            copy_audio: true,
            copy_subtitles: true,
        }
    }
}
