use anyhow::Result;
use std::path::PathBuf;

pub struct FfmpegLocator;

impl FfmpegLocator {
    /// Discovers the path to the ffmpeg executable on the system
    pub fn find_ffmpeg() -> Result<PathBuf> {
        if let Ok(path_str) = std::env::var("FFMPEG_PATH") {
            let p = PathBuf::from(path_str);
            if p.exists() {
                return Ok(p);
            }
        }

        // Try direct command resolution
        if let Ok(output) = std::process::Command::new("ffmpeg").arg("-version").output() {
            if output.status.success() {
                return Ok(PathBuf::from("ffmpeg"));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(p) = Self::find_windows_winget_binary("ffmpeg.exe") {
                return Ok(p);
            }
        }

        #[cfg(target_os = "macos")]
        {
            for candidate in &["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/usr/bin/ffmpeg"] {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        anyhow::bail!("ffmpeg executable not found. Please ensure FFmpeg is installed or set FFMPEG_PATH.")
    }

    /// Discovers the path to the ffprobe executable on the system
    pub fn find_ffprobe() -> Result<PathBuf> {
        if let Ok(path_str) = std::env::var("FFPROBE_PATH") {
            let p = PathBuf::from(path_str);
            if p.exists() {
                return Ok(p);
            }
        }

        // Try direct command resolution
        if let Ok(output) = std::process::Command::new("ffprobe").arg("-version").output() {
            if output.status.success() {
                return Ok(PathBuf::from("ffprobe"));
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(p) = Self::find_windows_winget_binary("ffprobe.exe") {
                return Ok(p);
            }
        }

        #[cfg(target_os = "macos")]
        {
            for candidate in &["/opt/homebrew/bin/ffprobe", "/usr/local/bin/ffprobe", "/usr/bin/ffprobe"] {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        anyhow::bail!("ffprobe executable not found. Please ensure FFprobe is installed or set FFPROBE_PATH.")
    }

    #[cfg(target_os = "windows")]
    fn find_windows_winget_binary(bin_name: &str) -> Option<PathBuf> {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_dir = PathBuf::from(local_app_data).join("Microsoft\\WinGet\\Packages");
            if winget_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&winget_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("Gyan.FFmpeg") {
                            // Recursively search for binary in this package directory
                            if let Some(found) = Self::search_binary_recursive(&entry.path(), bin_name) {
                                return Some(found);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    fn search_binary_recursive(dir: &std::path::Path, target_name: &str) -> Option<PathBuf> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(found) = Self::search_binary_recursive(&path, target_name) {
                        return Some(found);
                    }
                } else if path.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(target_name)).unwrap_or(false) {
                    return Some(path);
                }
            }
        }
        None
    }
}
