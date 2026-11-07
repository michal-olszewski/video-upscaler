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

        // Check alongside current executable (bundle or portable directory)
        if let Some(p) = Self::find_alongside_exe("ffmpeg") {
            return Ok(p);
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
            for candidate in &[
                "C:\\ffmpeg\\bin\\ffmpeg.exe",
                "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
            ] {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let mut candidates = vec![
                // MacPorts (standard prefix /opt/local)
                "/opt/local/bin/ffmpeg".to_string(),
                "/opt/local/sbin/ffmpeg".to_string(),
                // Homebrew (Apple Silicon & Intel)
                "/opt/homebrew/bin/ffmpeg".to_string(),
                "/usr/local/bin/ffmpeg".to_string(),
                "/usr/bin/ffmpeg".to_string(),
            ];
            if !home.is_empty() {
                candidates.push(format!("{}/.local/bin/ffmpeg", home));
                candidates.push(format!("{}/bin/ffmpeg", home));
                candidates.push(format!("{}/macports/bin/ffmpeg", home));
            }
            for candidate in candidates {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }

            // Fallback: Query the user's login shell to pick up custom MacPorts or shell profiles
            if let Some(p) = Self::find_via_login_shell("ffmpeg") {
                return Ok(p);
            }
        }

        #[cfg(target_os = "macos")]
        anyhow::bail!(
            "ffmpeg executable not found. Please install FFmpeg using MacPorts ('sudo port install ffmpeg') or Homebrew ('brew install ffmpeg'), or set the FFMPEG_PATH environment variable."
        );

        #[cfg(not(target_os = "macos"))]
        anyhow::bail!("ffmpeg executable not found. Please ensure FFmpeg is installed or set FFMPEG_PATH.");
    }

    /// Discovers the path to the ffprobe executable on the system
    pub fn find_ffprobe() -> Result<PathBuf> {
        if let Ok(path_str) = std::env::var("FFPROBE_PATH") {
            let p = PathBuf::from(path_str);
            if p.exists() {
                return Ok(p);
            }
        }

        // Check alongside current executable (bundle or portable directory)
        if let Some(p) = Self::find_alongside_exe("ffprobe") {
            return Ok(p);
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
            for candidate in &[
                "C:\\ffmpeg\\bin\\ffprobe.exe",
                "C:\\Program Files\\ffmpeg\\bin\\ffprobe.exe",
            ] {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let mut candidates = vec![
                // MacPorts (standard prefix /opt/local)
                "/opt/local/bin/ffprobe".to_string(),
                "/opt/local/sbin/ffprobe".to_string(),
                // Homebrew (Apple Silicon & Intel)
                "/opt/homebrew/bin/ffprobe".to_string(),
                "/usr/local/bin/ffprobe".to_string(),
                "/usr/bin/ffprobe".to_string(),
            ];
            if !home.is_empty() {
                candidates.push(format!("{}/.local/bin/ffprobe", home));
                candidates.push(format!("{}/bin/ffprobe", home));
                candidates.push(format!("{}/macports/bin/ffprobe", home));
            }
            for candidate in candidates {
                let p = PathBuf::from(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }

            // Fallback: Query the user's login shell to pick up custom MacPorts or shell profiles
            if let Some(p) = Self::find_via_login_shell("ffprobe") {
                return Ok(p);
            }
        }

        #[cfg(target_os = "macos")]
        anyhow::bail!(
            "ffprobe executable not found. Please install FFmpeg using MacPorts ('sudo port install ffmpeg') or Homebrew ('brew install ffmpeg'), or set the FFPROBE_PATH environment variable."
        );

        #[cfg(not(target_os = "macos"))]
        anyhow::bail!("ffprobe executable not found. Please ensure FFprobe is installed or set FFPROBE_PATH.");
    }

    /// Checks for a binary alongside the current executable or inside the app bundle
    fn find_alongside_exe(bin_stem: &str) -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;

        let candidates = [
            // Exact name in same directory (e.g. Contents/MacOS/ffprobe or ./ffprobe)
            exe_dir.join(bin_stem),
            exe_dir.join(format!("{}.exe", bin_stem)),
            // In a bin/ subdirectory
            exe_dir.join("bin").join(bin_stem),
            exe_dir.join("bin").join(format!("{}.exe", bin_stem)),
            // macOS App Bundle Resources (e.g. Contents/Resources/bin/ffprobe or Contents/Resources/ffprobe)
            exe_dir.join("../Resources").join(bin_stem),
            exe_dir.join("../Resources/bin").join(bin_stem),
        ];

        for c in candidates {
            if c.exists() {
                return Some(c);
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    fn find_via_login_shell(bin_name: &str) -> Option<PathBuf> {
        // When launched via GUI (Finder/LaunchServices), PATH is restricted to /usr/bin:/bin.
        // Querying the user's login shell loads ~/.zprofile, ~/.zshrc where MacPorts, Homebrew,
        // or custom toolchains are registered in PATH.
        if let Ok(output) = std::process::Command::new("/bin/zsh")
            .args(["-l", "-c", &format!("which {}", bin_name)])
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    let p = PathBuf::from(path_str);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        None
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
