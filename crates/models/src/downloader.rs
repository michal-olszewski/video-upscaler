use crate::registry::ModelRegistry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use tracing::info;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub is_completed: bool,
    pub error: Option<String>,
}

pub struct ModelDownloader;

impl ModelDownloader {
    /// Returns the OS-appropriate persistent directory for caching model weights
    pub fn cache_dir() -> PathBuf {
        // 1. Explicit override via MODELS_CACHE_DIR
        if let Ok(dir_str) = std::env::var("MODELS_CACHE_DIR") {
            let p = PathBuf::from(dir_str);
            if !p.as_os_str().is_empty() {
                return p;
            }
        }

        // 2. Check if local ./models/cache already exists (e.g. running from repo root in dev mode)
        let local_cache = PathBuf::from("models").join("cache");
        if local_cache.is_dir() {
            return local_cache;
        }

        // 3. Check adjacent to current executable (portable or macOS bundle)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let exe_cache = exe_dir.join("models").join("cache");
                if exe_cache.is_dir() {
                    return exe_cache;
                }
                let mac_bundle_models = exe_dir.join("..").join("Resources").join("models");
                if mac_bundle_models.is_dir() {
                    return mac_bundle_models;
                }
            }
        }

        // 4. Standard OS User Application Data Directory
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local_app_data).join("VideoUpscaler").join("models");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("VideoUpscaler")
                    .join("models");
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
                return PathBuf::from(xdg).join("VideoUpscaler").join("models");
            }
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("VideoUpscaler")
                    .join("models");
            }
        }

        // Fallback
        PathBuf::from("models").join("cache")
    }

    /// Finds the location of a model file if it exists (checking cache, bundled, or current dir)
    pub fn get_model_path(model_id: &str) -> Option<PathBuf> {
        let desc = ModelRegistry::find_by_id(model_id)?;
        Self::get_model_file_path(&desc.filename)
    }

    /// Resolves an existing model file by filename across known search paths
    pub fn get_model_file_path(filename: &str) -> Option<PathBuf> {
        // 1. Check primary cache_dir
        let p = Self::cache_dir().join(filename);
        if p.exists() && std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
            return Some(p);
        }

        // 2. Check local ./models/cache/
        let p_local = PathBuf::from("models").join("cache").join(filename);
        if p_local.exists() && std::fs::metadata(&p_local).map(|m| m.len() > 0).unwrap_or(false) {
            return Some(p_local);
        }

        // 3. Check alongside current executable or inside macOS bundle
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let p_exe = exe_dir.join("models").join("cache").join(filename);
                if p_exe.exists() && std::fs::metadata(&p_exe).map(|m| m.len() > 0).unwrap_or(false) {
                    return Some(p_exe);
                }
                let p_mac = exe_dir.join("..").join("Resources").join("models").join(filename);
                if p_mac.exists() && std::fs::metadata(&p_mac).map(|m| m.len() > 0).unwrap_or(false) {
                    return Some(p_mac);
                }
            }
        }

        None
    }

    /// Resolves the target path for a model (existing file or default cache destination)
    pub fn resolve_model_target_path(filename: &str) -> PathBuf {
        if let Some(existing) = Self::get_model_file_path(filename) {
            existing
        } else {
            Self::cache_dir().join(filename)
        }
    }

    /// Checks if a model's weights file is already downloaded and present in cache
    pub fn is_cached(model_id: &str) -> bool {
        Self::get_model_path(model_id).is_some()
    }

    /// Downloads a model with progress callbacks
    pub fn download_model<F>(model_id: &str, on_progress: F) -> Result<PathBuf>
    where
        F: Fn(ModelDownloadProgress),
    {
        let desc = match ModelRegistry::find_by_id(model_id) {
            Some(d) => d,
            None => {
                let err_msg = format!("Model not found in registry: {}", model_id);
                on_progress(ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    is_completed: false,
                    error: Some(err_msg.clone()),
                });
                anyhow::bail!(err_msg);
            }
        };

        // If already cached, notify completion immediately and return path
        if let Some(existing) = Self::get_model_file_path(&desc.filename) {
            let len = std::fs::metadata(&existing).map(|m| m.len()).unwrap_or(0);
            on_progress(ModelDownloadProgress {
                model_id: model_id.to_string(),
                downloaded_bytes: len,
                total_bytes: len,
                percentage: 100.0,
                is_completed: true,
                error: None,
            });
            return Ok(existing);
        }

        let cache_dir = Self::cache_dir();
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            let err_msg = format!("Failed to create model cache directory {:?}: {}", cache_dir, e);
            on_progress(ModelDownloadProgress {
                model_id: model_id.to_string(),
                downloaded_bytes: 0,
                total_bytes: 0,
                percentage: 0.0,
                is_completed: false,
                error: Some(err_msg.clone()),
            });
            anyhow::bail!(err_msg);
        }

        let final_path = cache_dir.join(&desc.filename);
        let part_path = cache_dir.join(format!("{}.part", desc.filename));

        let download_url = match desc.download_url.as_deref() {
            Some(url) => url,
            None => {
                let err_msg = format!("No download URL configured for model {}", model_id);
                on_progress(ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    is_completed: false,
                    error: Some(err_msg.clone()),
                });
                anyhow::bail!(err_msg);
            }
        };

        info!("Downloading model weights from {} -> {:?}", download_url, final_path);

        let res = (|| -> Result<()> {
            let response = ureq::get(download_url)
                .header("User-Agent", "VideoUpscalerStudio/1.0")
                .call()
                .with_context(|| format!("Failed to initiate HTTP GET to {}", download_url))?;

            let total_bytes: u64 = response
                .headers()
                .get("content-length")
                .and_then(|h| h.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);

            let mut reader = response.into_body().into_reader();
            let mut file = File::create(&part_path)
                .with_context(|| format!("Failed to create partial download file at {:?}", part_path))?;

            let mut downloaded: u64 = 0;
            let mut buf = [0u8; 64 * 1024];

            loop {
                let bytes_read = reader
                    .read(&mut buf)
                    .with_context(|| "Failed reading download stream")?;
                if bytes_read == 0 {
                    break;
                }
                file.write_all(&buf[..bytes_read])
                    .with_context(|| "Failed writing to partial download file")?;
                downloaded += bytes_read as u64;

                let percentage = if total_bytes > 0 {
                    (downloaded as f32 / total_bytes as f32) * 100.0
                } else {
                    0.0
                };

                on_progress(ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes,
                    percentage: percentage.min(100.0),
                    is_completed: false,
                    error: None,
                });
            }

            file.flush()?;
            Ok(())
        })();

        match res {
            Ok(()) => {
                std::fs::rename(&part_path, &final_path)
                    .with_context(|| format!("Failed to rename {:?} to {:?}", part_path, final_path))?;
                let final_len = std::fs::metadata(&final_path)?.len();
                on_progress(ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded_bytes: final_len,
                    total_bytes: final_len,
                    percentage: 100.0,
                    is_completed: true,
                    error: None,
                });
                Ok(final_path)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part_path);
                let err_msg = e.to_string();
                on_progress(ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    is_completed: false,
                    error: Some(err_msg.clone()),
                });
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cached_returns_false_for_nonexistent() {
        assert!(!ModelDownloader::is_cached("nonexistent-test-model-xyz"));
    }

    #[test]
    fn test_model_download_progress_serialization() {
        let prog = ModelDownloadProgress {
            model_id: "test-model".to_string(),
            downloaded_bytes: 1024,
            total_bytes: 2048,
            percentage: 50.0,
            is_completed: false,
            error: None,
        };
        let serialized = serde_json::to_string(&prog).unwrap();
        assert!(serialized.contains("50.0"));
    }

    #[test]
    fn test_cache_dir_resolution() {
        let dir = ModelDownloader::cache_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_registry_models_have_valid_urls() {
        let models = ModelRegistry::list_models();
        assert!(!models.is_empty());
        for m in models {
            assert!(m.download_url.is_some());
            let url = m.download_url.unwrap();
            assert!(url.starts_with("https://"));
            assert!(url.ends_with(".onnx"));
        }
    }
}
