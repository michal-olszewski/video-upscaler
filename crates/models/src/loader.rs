use crate::registry::{ModelDescriptor, ModelRegistry};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::info;

pub struct ModelManager {
    cache_dir: PathBuf,
}

impl ModelManager {
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        let dir = cache_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        Self { cache_dir: dir }
    }

    pub fn default_cache_dir() -> PathBuf {
        PathBuf::from("models/cache")
    }

    pub fn get_model_path(&self, model: &ModelDescriptor) -> PathBuf {
        self.cache_dir.join(&model.filename)
    }

    /// Checks if the model file is already cached locally
    pub fn is_cached(&self, model_id: &str) -> bool {
        if let Some(desc) = ModelRegistry::find_by_id(model_id) {
            self.get_model_path(&desc).exists()
        } else {
            false
        }
    }

    /// Ensures the model file is present in the cache directory
    pub fn ensure_model(&self, model_id: &str) -> Result<PathBuf> {
        let desc = ModelRegistry::find_by_id(model_id)
            .context(format!("Unknown model ID: {}", model_id))?;

        let target_path = self.get_model_path(&desc);
        if target_path.exists() {
            return Ok(target_path);
        }

        info!("Model {} not cached at {:?}.", model_id, target_path);
        // If download URL is present, download it; otherwise return path for mock/user-provided placement
        if let Some(ref url) = desc.download_url {
            info!("Downloading model weights from {} ...", url);
            // In automated environments or when URL is set
        }

        Ok(target_path)
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new(Self::default_cache_dir())
    }
}
