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
            Self::AppleVideoToolbox(VideoCodecType::Av1) => "libsvtav1",
            Self::SoftwareCpu(VideoCodecType::H264) => "libx264",
            Self::SoftwareCpu(VideoCodecType::H265Hevc) => "libx265",
            Self::SoftwareCpu(VideoCodecType::Av1) => "libsvtav1",
            Self::SoftwareCpu(VideoCodecType::ProRes)
            | Self::AppleVideoToolbox(VideoCodecType::ProRes)
            | Self::NvidiaNvenc(VideoCodecType::ProRes)
            | Self::AmdAmf(VideoCodecType::ProRes) => "prores_ks",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEncodingProfile {
    pub encoder: HardwareEncoder,
    pub crf: u32,
    pub bitrate_kbps: Option<u32>,
    pub preset: String, // e.g. "p7" for nvenc, "slow" for x264/x265
    pub pixel_format: String, // e.g. "yuv420p", "yuv420p10le", "p010le", "yuv422p10le"
    pub copy_audio: bool,
    pub copy_subtitles: bool,
    #[serde(default)]
    pub color_primaries: Option<String>, // e.g. "bt2020", "bt709"
    #[serde(default)]
    pub color_transfer: Option<String>, // e.g. "smpte2084", "arib-std-b67", "bt709"
    #[serde(default)]
    pub color_space: Option<String>, // e.g. "bt2020nc", "bt709"
    #[serde(default)]
    pub high_bit_depth: bool, // enables 10-bit/12-bit/16-bit intermediate RGB48 pipeline
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
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            high_bit_depth: false,
        }
    }
}

impl OutputEncodingProfile {
    /// Creates a 10-bit HDR10 mastering profile (BT.2020 color primaries + SMPTE 2084 PQ transfer)
    pub fn hdr10(encoder: HardwareEncoder, crf: u32) -> Self {
        Self {
            encoder,
            crf,
            bitrate_kbps: None,
            preset: "medium".to_string(),
            pixel_format: "yuv420p10le".to_string(),
            copy_audio: true,
            copy_subtitles: true,
            color_primaries: Some("bt2020".to_string()),
            color_transfer: Some("smpte2084".to_string()),
            color_space: Some("bt2020nc".to_string()),
            high_bit_depth: true,
        }
    }

    /// Creates an Apple ProRes 422 HQ 10-bit professional mastering profile
    pub fn prores_422_hq() -> Self {
        Self {
            encoder: HardwareEncoder::SoftwareCpu(VideoCodecType::ProRes),
            crf: 16,
            bitrate_kbps: None,
            preset: "medium".to_string(),
            pixel_format: "yuv422p10le".to_string(),
            copy_audio: true,
            copy_subtitles: true,
            color_primaries: Some("bt709".to_string()),
            color_transfer: Some("bt709".to_string()),
            color_space: Some("bt709".to_string()),
            high_bit_depth: true,
        }
    }

    /// Checks if this profile represents HDR content
    pub fn is_hdr(&self) -> bool {
        self.color_transfer.as_deref() == Some("smpte2084")
            || self.color_transfer.as_deref() == Some("arib-std-b67")
            || self.color_primaries.as_deref() == Some("bt2020")
    }
}
