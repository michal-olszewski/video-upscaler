#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use engine_core::queue::BatchQueueManager;

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
    batch_queue: Arc<Mutex<BatchQueueManager>>,
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

#[derive(Deserialize)]
struct SaveDialogRequest {
    default_name: Option<String>,
}

#[derive(Deserialize)]
struct RemoveJobRequest {
    job_id: String,
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


fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to initialize tracing subscriber");

    let args: Vec<String> = std::env::args().collect();
    let is_headless = args.iter().any(|a| a == "--headless" || a == "--server-only");

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
        batch_queue: Arc::new(Mutex::new(BatchQueueManager::new())),
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
        .route("/api/dialog/open-file", post(dialog_open_file))
        .route("/api/dialog/open-files", post(dialog_open_files))
        .route("/api/dialog/save-file", post(dialog_save_file))
        .route("/api/queue", get(get_queue))
        .route("/api/queue/add", post(queue_add_job))
        .route("/api/queue/remove", post(queue_remove_job))
        .route("/api/queue/clear", post(queue_clear_completed))
        .route("/api/queue/start", post(queue_start))
        .route("/api/queue/pause", post(queue_pause))
        .route("/api/probe", post(probe_media))
        .route("/api/preview", post(generate_preview))
        .route("/api/upscale", post(start_upscale))
        .route("/api/status", get(get_status))
        .route("/api/cancel", post(cancel_upscale))
        .fallback_service(serve_dir)
        .layer(cors)
        .with_state(state);

    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let server_handle = std::thread::spawn(move || {
        runtime.block_on(async {
            let addr = "127.0.0.1:3001";
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind server at {}: {}", addr, e);
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            info!("Video Upscaler Studio API listening at http://{}", addr);
            let _ = ready_tx.send(true);
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Server error: {}", e);
            }
        });
    });

    let is_ready = ready_rx.recv().unwrap_or(false);
    if !is_ready {
        anyhow::bail!("Server failed to bind to 127.0.0.1:3001");
    }

    if is_headless {
        info!("Running in headless server mode (PID {})... Press Ctrl+C to stop.", std::process::id());
        server_handle.join().unwrap();
        return Ok(());
    }

    info!("Opening native OS application window (tao + wry)...");
    let event_loop = tao::event_loop::EventLoop::new();
    let window = tao::window::WindowBuilder::new()
        .with_title("⚡ Video Upscaler Studio Pro")
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(1080.0, 720.0))
        .build(&event_loop)?;

    let _webview = wry::WebViewBuilder::new()
        .with_url("http://127.0.0.1:3001")
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;
        match event {
            tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
            _ => {}
        }
    });
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

async fn execute_single_job(state: AppState, req: UpscaleJobRequest) {
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
    let batch_queue = Arc::clone(&state.batch_queue);
    let job_id = req.id.clone();

    tokio::spawn(async move {
        while let Some(prog) = rx.recv().await {
            let mut p = latest_prog.lock().await;
            *p = Some(prog.clone());
            let mut q = batch_queue.lock().await;
            let prog_job_id = prog.job_id.clone();
            q.update_job_progress(&prog_job_id, prog);


        }
    });

    let result = pipeline.execute(req, tx).await;
    {
        let mut active = active_pipe.lock().await;
        *active = None;
    }
    {
        let mut q = state.batch_queue.lock().await;
        match result {
            Ok(()) => q.mark_completed(&job_id),
            Err(e) => q.mark_failed(&job_id, e.to_string()),
        }
    }
}

fn trigger_queue_processing(state: AppState) {
    tokio::spawn(async move {
        loop {
            let next_req = {
                let mut q = state.batch_queue.lock().await;
                if !q.is_active() {
                    break;
                }
                q.pop_next_pending_job()
            };

            match next_req {
                Some(req) => {
                    execute_single_job(state.clone(), req).await;
                }
                None => {
                    break;
                }
            }
        }
    });
}

async fn start_upscale(
    State(state): State<AppState>,
    Json(req): Json<UpscaleJobRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    tokio::spawn(execute_single_job(state, req));
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

// ---------------------------------------------------------------
// Native OS File Dialogs
// ---------------------------------------------------------------

async fn dialog_open_file() -> impl IntoResponse {
    let path = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mkv", "mov", "avi", "webm", "flv", "m4v", "ts"])
            .pick_file()
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .unwrap_or(None);

    Json(serde_json::json!({ "path": path }))
}

async fn dialog_open_files() -> impl IntoResponse {
    let paths = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mkv", "mov", "avi", "webm", "flv", "m4v", "ts"])
            .pick_files()
            .map(|list| list.into_iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    Json(serde_json::json!({ "paths": paths }))
}

async fn dialog_save_file(Json(payload): Json<SaveDialogRequest>) -> impl IntoResponse {
    let default_name = payload.default_name.unwrap_or_else(|| "upscaled.mp4".to_string());
    let path = tokio::task::spawn_blocking(move || {
        rfd::FileDialog::new()
            .add_filter("MP4 Video", &["mp4"])
            .add_filter("MKV Video", &["mkv"])
            .set_file_name(&default_name)
            .save_file()
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .unwrap_or(None);

    Json(serde_json::json!({ "path": path }))
}

// ---------------------------------------------------------------
// Batch Processing Queue Handlers
// ---------------------------------------------------------------

async fn get_queue(State(state): State<AppState>) -> impl IntoResponse {
    let q = state.batch_queue.lock().await;
    Json(serde_json::json!({
        "jobs": q.list_jobs(),
        "is_active": q.is_active(),
        "active_job_id": q.get_active_job_id(),
    }))
}

async fn queue_add_job(
    State(state): State<AppState>,
    Json(req): Json<UpscaleJobRequest>,
) -> impl IntoResponse {
    let mut q = state.batch_queue.lock().await;
    let id = q.add_job(req);
    Json(serde_json::json!({ "status": "queued", "job_id": id }))
}

async fn queue_remove_job(
    State(state): State<AppState>,
    Json(payload): Json<RemoveJobRequest>,
) -> impl IntoResponse {
    let mut q = state.batch_queue.lock().await;
    let removed = q.remove_job(&payload.job_id);
    Json(serde_json::json!({ "status": if removed { "removed" } else { "busy_or_not_found" } }))
}

async fn queue_clear_completed(State(state): State<AppState>) -> impl IntoResponse {
    let mut q = state.batch_queue.lock().await;
    q.clear_completed();
    Json(serde_json::json!({ "status": "cleared" }))
}

async fn queue_start(State(state): State<AppState>) -> impl IntoResponse {
    let mut q = state.batch_queue.lock().await;
    q.set_active(true);
    let active_pipe = state.active_pipeline.lock().await;
    if active_pipe.is_none() {
        drop(q);
        drop(active_pipe);
        trigger_queue_processing(state);
    }

    Json(serde_json::json!({ "status": "queue_started" }))
}


async fn queue_pause(State(state): State<AppState>) -> impl IntoResponse {
    let mut q = state.batch_queue.lock().await;
    q.set_active(false);
    Json(serde_json::json!({ "status": "queue_paused" }))
}

