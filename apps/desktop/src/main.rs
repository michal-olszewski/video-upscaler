use inference_runtime::HardwareManager;
use models::ModelRegistry;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing subscriber");

    info!("Starting Video Upscaler Pro Engine...");

    // Hardware discovery
    let devices = HardwareManager::probe_devices();
    info!("Detected {} compute device(s):", devices.len());
    for dev in &devices {
        info!(
            " - [{}] {} ({:?}) - Recommended: {}",
            dev.id, dev.name, dev.vendor, dev.recommended_backend
        );
    }

    // List models
    let available_models = ModelRegistry::list_models();
    info!("Registered {} enhancement models:", available_models.len());
    for m in &available_models {
        info!(" - {} ({}x) - {}", m.name, m.scale_factor, m.description);
    }

    Ok(())
}
