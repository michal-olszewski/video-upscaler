use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use engine_core::temporal::TemporalMode;
use engine_core::{JobProgress, JobState, PreviewEngine, UpscaleJobRequest, UpscalePipeline};
use inference_runtime::{ExecutionBackend, HardwareManager};
use models::{ModelDescriptor, ModelDownloadProgress, ModelDownloader, ModelRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use video_io::MediaProbe;

#[derive(Clone)]
struct AppState {
    active_pipeline: Arc<Mutex<Option<Arc<UpscalePipeline>>>>,
    latest_progress: Arc<Mutex<Option<JobProgress>>>,
    model_downloads: Arc<Mutex<HashMap<String, ModelDownloadProgress>>>,
}

#[derive(Deserialize)]
struct ProbeRequest {
    path: String,
}

#[derive(Deserialize)]
struct PreviewRequest {
    path: String,
    timestamp_seconds: Option<f64>,
    model_id: String,
    device_id: Option<u32>,
    film_grain: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    temporal_mode: Option<TemporalMode>,
}


#[derive(Deserialize)]
struct DownloadModelRequest {
    model_id: String,
}

#[derive(Serialize)]
struct ModelCatalogItem {
    #[serde(flatten)]
    descriptor: ModelDescriptor,
    is_cached: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing subscriber");

    info!("Starting Video Upscaler Studio Desktop Service...");

    // Print hardware discovery
    let devices = HardwareManager::probe_devices();
    info!("Discovered {} compute device(s):", devices.len());
    for dev in &devices {
        info!(" - [{}] {} ({:?})", dev.id, dev.name, dev.vendor);
    }

    let state = AppState {
        active_pipeline: Arc::new(Mutex::new(None)),
        latest_progress: Arc::new(Mutex::new(None)),
        model_downloads: Arc::new(Mutex::new(HashMap::new())),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let serve_dir = tower_http::services::ServeDir::new("apps/desktop/ui/dist")
        .append_index_html_on_directories(true);

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/devices", get(list_devices))
        .route("/api/models", get(list_models))
        .route("/api/models/download", post(download_model))
        .route("/api/models/downloads", get(get_downloads))
        .route("/api/probe", post(probe_media))
        .route("/api/preview", post(generate_preview))
        .route("/api/upscale", post(start_upscale))
        .route("/api/status", get(get_status))
        .route("/api/cancel", post(cancel_upscale))
        .fallback_service(serve_dir)
        .layer(cors)
        .with_state(state);

    let addr = "127.0.0.1:3001";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Video Upscaler Studio API listening at http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "online", "version": "0.1.0" }))
}

async fn list_devices() -> impl IntoResponse {
    Json(HardwareManager::probe_devices())
}

async fn list_models() -> impl IntoResponse {
    let list = ModelRegistry::list_models()
        .into_iter()
        .map(|m| {
            let cached = ModelDownloader::is_cached(&m.id);
            ModelCatalogItem {
                descriptor: m,
                is_cached: cached,
            }
        })
        .collect::<Vec<_>>();
    Json(list)
}

async fn download_model(
    State(state): State<AppState>,
    Json(payload): Json<DownloadModelRequest>,
) -> impl IntoResponse {
    let model_id = payload.model_id.clone();
    let downloads = Arc::clone(&state.model_downloads);

    tokio::task::spawn_blocking(move || {
        let downloads_cb = Arc::clone(&downloads);
        let _ = ModelDownloader::download_model(&model_id, move |prog| {
            let mut map = downloads_cb.blocking_lock();
            map.insert(prog.model_id.clone(), prog);
        });
    });

    Json(serde_json::json!({ "status": "download_started", "model_id": payload.model_id }))
}

async fn get_downloads(State(state): State<AppState>) -> impl IntoResponse {
    let map = state.model_downloads.lock().await;
    Json(map.clone())
}


async fn probe_media(Json(payload): Json<ProbeRequest>) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    match MediaProbe::probe_file(&payload.path) {
        Ok(meta) => Ok(Json(meta)),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

async fn generate_preview(
    Json(payload): Json<PreviewRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let backend = ExecutionBackend::DirectMl {
        device_id: payload.device_id.unwrap_or(0),
    };
    let timestamp = payload.timestamp_seconds.unwrap_or(0.0);
    let grain = payload.film_grain.unwrap_or(0.1);

    match PreviewEngine::generate_preview(&payload.path, timestamp, &payload.model_id, backend, grain) {
        Ok(preview) => Ok(Json(preview)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e.to_string() }),
        )),
    }
}

async fn start_upscale(
    State(state): State<AppState>,
    Json(req): Json<UpscaleJobRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let pipeline = Arc::new(UpscalePipeline::new());
    {
        let mut active = state.active_pipeline.lock().await;
        *active = Some(Arc::clone(&pipeline));
        let mut prog = state.latest_progress.lock().await;
        *prog = Some(JobProgress::new(req.id.clone(), 0));
    }

    let (tx, mut rx) = mpsc::channel::<JobProgress>(64);
    let latest_prog = Arc::clone(&state.latest_progress);
    let active_pipe = Arc::clone(&state.active_pipeline);

    tokio::spawn(async move {
        while let Some(prog) = rx.recv().await {
            let mut p = latest_prog.lock().await;
            *p = Some(prog);
        }
    });

    let pipeline_run = Arc::clone(&pipeline);
    tokio::spawn(async move {
        let _ = pipeline_run.execute(req, tx).await;
        let mut active = active_pipe.lock().await;
        *active = None;
    });

    Ok(Json(serde_json::json!({ "status": "started" })))
}

async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let prog = state.latest_progress.lock().await;
    match &*prog {
        Some(p) => Json(p.clone()),
        None => Json(JobProgress {
            job_id: "".to_string(),
            state: JobState::Queued,
            current_frame: 0,
            total_frames: 0,
            percentage: 0.0,
            current_fps: 0.0,
            eta_seconds: 0,
            error_message: None,
        }),
    }
}

async fn cancel_upscale(State(state): State<AppState>) -> impl IntoResponse {
    let active = state.active_pipeline.lock().await;
    if let Some(ref p) = *active {
        p.cancel();
        Json(serde_json::json!({ "status": "cancelling" }))
    } else {
        Json(serde_json::json!({ "status": "no_active_job" }))
    }
}
