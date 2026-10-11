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
    pub fn cache_dir() -> PathBuf {
        PathBuf::from("models/cache")
    }

    /// Checks if a model's weights file is already downloaded and present in cache
    pub fn is_cached(model_id: &str) -> bool {
        if let Some(desc) = ModelRegistry::find_by_id(model_id) {
            let path = Self::cache_dir().join(&desc.filename);
            path.exists() && std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false)
        } else {
            false
        }
    }

    /// Downloads a model with progress callbacks
    pub fn download_model<F>(model_id: &str, on_progress: F) -> Result<PathBuf>
    where
        F: Fn(ModelDownloadProgress),
    {
        let desc = ModelRegistry::find_by_id(model_id)
            .context(format!("Model not found in registry: {}", model_id))?;

        let cache_dir = Self::cache_dir();
        std::fs::create_dir_all(&cache_dir)?;
        let final_path = cache_dir.join(&desc.filename);

        if final_path.exists() {
            let len = std::fs::metadata(&final_path)?.len();
            on_progress(ModelDownloadProgress {
                model_id: model_id.to_string(),
                downloaded_bytes: len,
                total_bytes: len,
                percentage: 100.0,
                is_completed: true,
                error: None,
            });
            return Ok(final_path);
        }

        let download_url = desc.download_url.as_deref().unwrap_or(
            "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth",
        );

        info!("Downloading model weights from {} -> {:?}", download_url, final_path);

        let part_path = cache_dir.join(format!("{}.part", desc.filename));

        let res = (|| -> Result<()> {
            let response = ureq::get(download_url)
                .call()
                .with_context(|| format!("Failed to initiate HTTP GET to {}", download_url))?;

            let total_bytes: u64 = response
                .headers()
                .get("content-length")
                .and_then(|h| h.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);

            let mut reader = response.into_body().into_reader();
            let mut file = File::create(&part_path)?;

            let mut downloaded: u64 = 0;
            let mut buf = [0u8; 64 * 1024];

            loop {
                let bytes_read = reader.read(&mut buf)?;
                if bytes_read == 0 {
                    break;
                }
                file.write_all(&buf[..bytes_read])?;
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
                std::fs::rename(&part_path, &final_path)?;
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
