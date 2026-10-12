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
    #[serde(default)]
    pub audio_profile: Option<AudioEncodingProfile>,
    #[serde(default)]
    pub geometry_profile: Option<TargetGeometryProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    Copy,
    Aac,
    Opus,
    Flac,
    PcmS16le,
    PcmS24le,
    Mute,
}

impl Default for AudioCodec {
    fn default() -> Self {
        Self::Copy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEncodingProfile {
    #[serde(default)]
    pub codec: AudioCodec,
    #[serde(default = "default_audio_bitrate")]
    pub bitrate_kbps: u32,
    #[serde(default)]
    pub loudness_normalization: bool,
    #[serde(default = "default_lufs")]
    pub target_lufs: f32, // default -14.0 LUFS
}

fn default_audio_bitrate() -> u32 {
    192
}

fn default_lufs() -> f32 {
    -14.0
}

impl Default for AudioEncodingProfile {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Copy,
            bitrate_kbps: 192,
            loudness_normalization: false,
            target_lufs: -14.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetResolution {
    NativeScale,
    Preset720p,         // 1280x720
    Preset1080p,        // 1920x1080
    Preset1440p,        // 2560x1440
    Preset4KUHD,        // 3840x2160
    Preset8KUHD,        // 7680x4320
    SocialVertical9_16, // 1080x1920 (Reels / Shorts / TikTok)
    SocialSquare1_1,    // 1080x1080
    CinemaWidescreen,   // 3840x1606 (2.39:1)
    Custom { width: u32, height: u32 },
}

impl Default for TargetResolution {
    fn default() -> Self {
        Self::NativeScale
    }
}

impl TargetResolution {
    pub fn dimensions(&self, in_w: usize, in_h: usize) -> (usize, usize) {
        match self {
            Self::NativeScale => (in_w, in_h),
            Self::Preset720p => (1280, 720),
            Self::Preset1080p => (1920, 1080),
            Self::Preset1440p => (2560, 1440),
            Self::Preset4KUHD => (3840, 2160),
            Self::Preset8KUHD => (7680, 4320),
            Self::SocialVertical9_16 => (1080, 1920),
            Self::SocialSquare1_1 => (1080, 1080),
            Self::CinemaWidescreen => (3840, 1606),
            Self::Custom { width, height } => (*width as usize, *height as usize),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    Letterbox,  // Pads with black/matte bars while preserving aspect ratio
    CenterCrop, // Crops excess to fill target aspect ratio
    Stretch,    // Stretches to fill target aspect ratio
}

impl Default for FitMode {
    fn default() -> Self {
        Self::Letterbox
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetGeometryProfile {
    #[serde(default)]
    pub resolution: TargetResolution,
    #[serde(default)]
    pub fit_mode: FitMode,
    #[serde(default = "default_matte_color")]
    pub matte_color: String,
}

fn default_matte_color() -> String {
    "black".to_string()
}

impl Default for TargetGeometryProfile {
    fn default() -> Self {
        Self {
            resolution: TargetResolution::NativeScale,
            fit_mode: FitMode::Letterbox,
            matte_color: "black".to_string(),
        }
    }
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
            audio_profile: None,
            geometry_profile: None,
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
            audio_profile: None,
            geometry_profile: None,
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
            audio_profile: None,
            geometry_profile: None,
        }
    }

    /// Checks if this profile represents HDR content
    pub fn is_hdr(&self) -> bool {
        self.color_transfer.as_deref() == Some("smpte2084")
            || self.color_transfer.as_deref() == Some("arib-std-b67")
            || self.color_primaries.as_deref() == Some("bt2020")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_resolution_dimensions() {
        assert_eq!(TargetResolution::NativeScale.dimensions(640, 480), (640, 480));
        assert_eq!(TargetResolution::Preset720p.dimensions(640, 480), (1280, 720));
        assert_eq!(TargetResolution::Preset1080p.dimensions(640, 480), (1920, 1080));
        assert_eq!(TargetResolution::Preset1440p.dimensions(640, 480), (2560, 1440));
        assert_eq!(TargetResolution::Preset4KUHD.dimensions(640, 480), (3840, 2160));
        assert_eq!(TargetResolution::Preset8KUHD.dimensions(640, 480), (7680, 4320));
        assert_eq!(TargetResolution::SocialVertical9_16.dimensions(640, 480), (1080, 1920));
        assert_eq!(TargetResolution::SocialSquare1_1.dimensions(640, 480), (1080, 1080));
        assert_eq!(TargetResolution::CinemaWidescreen.dimensions(640, 480), (3840, 1606));
        assert_eq!(
            TargetResolution::Custom { width: 1920, height: 800 }.dimensions(640, 480),
            (1920, 800)
        );
    }

    #[test]
    fn test_audio_profile_defaults_and_serialization() {
        let default_audio = AudioEncodingProfile::default();
        assert_eq!(default_audio.codec, AudioCodec::Copy);
        assert_eq!(default_audio.bitrate_kbps, 192);
        assert!(!default_audio.loudness_normalization);
        assert_eq!(default_audio.target_lufs, -14.0);

        let json = serde_json::to_string(&default_audio).unwrap();
        let parsed: AudioEncodingProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.codec, AudioCodec::Copy);
        assert_eq!(parsed.target_lufs, -14.0);

        let custom_json = r#"{"codec":"Aac","bitrate_kbps":320,"loudness_normalization":true,"target_lufs":-14.0}"#;
        let custom: AudioEncodingProfile = serde_json::from_str(custom_json).unwrap();
        assert_eq!(custom.codec, AudioCodec::Aac);
        assert_eq!(custom.bitrate_kbps, 320);
        assert!(custom.loudness_normalization);
        assert_eq!(custom.target_lufs, -14.0);
    }

    #[test]
    fn test_geometry_profile_defaults_and_serialization() {
        let default_geom = TargetGeometryProfile::default();
        assert_eq!(default_geom.resolution, TargetResolution::NativeScale);
        assert_eq!(default_geom.fit_mode, FitMode::Letterbox);
        assert_eq!(default_geom.matte_color, "black");

        let json = serde_json::to_string(&default_geom).unwrap();
        let parsed: TargetGeometryProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.fit_mode, FitMode::Letterbox);

        let vertical_json = r##"{"resolution":"SocialVertical9_16","fit_mode":"CenterCrop","matte_color":"#101010"}"##;
        let vertical: TargetGeometryProfile = serde_json::from_str(vertical_json).unwrap();
        assert_eq!(vertical.resolution, TargetResolution::SocialVertical9_16);
        assert_eq!(vertical.fit_mode, FitMode::CenterCrop);
        assert_eq!(vertical.matte_color, "#101010");
    }
}

