#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod config;
mod config_store;
mod icon;
mod paths;
mod printer;
mod scale;
mod system_printer;
mod tray;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::{future::Future, sync::Arc};
use tracing::info;

use config_store::{AgentConfigUpdate, ConfigStore, ConfigStoreError};
use printer::PrinterManager;
use scale::{ScaleError, ScaleManager, SUPPORTED_PROTOCOLS};

#[derive(Clone)]
struct AppState {
    config_store: Arc<ConfigStore>,
    printer: Arc<PrinterManager>,
    scale: Arc<ScaleManager>,
    version: &'static str,
}

fn main() {
    init_tracing();

    let config_store = Arc::new(ConfigStore::load());
    let scale = Arc::new(ScaleManager::new(Arc::clone(&config_store)));

    let state = AppState {
        printer: Arc::new(PrinterManager::new(Arc::clone(&config_store))),
        scale,
        config_store,
        version: env!("CARGO_PKG_VERSION"),
    };

    tray::run(state);
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "efact_hardware_agent=info".into());
    let log_dir = paths::log_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::never(log_dir, "agent.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(non_blocking)
        .init();

    std::mem::forget(guard);
}

pub(crate) async fn run_server(
    state: AppState,
    port: u16,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/config", get(get_config).put(put_config))
        .route("/printers", get(list_printers))
        .route("/print", axum::routing::post(print_raw))
        .route("/scale/protocols", get(list_scale_protocols))
        .route("/scale/ports", get(list_scale_ports))
        .route("/scale/status", get(scale_status))
        .route("/scale/weight", get(scale_weight))
        .layer(axum::middleware::from_fn(cors_and_pna))
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    info!("efact-hardware-agent listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .unwrap();

    Ok(())
}

async fn cors_and_pna(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let is_preflight = req.method() == Method::OPTIONS;

    if is_preflight {
        let mut res = StatusCode::NO_CONTENT.into_response();
        let h = res.headers_mut();
        h.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
        h.insert(
            "Access-Control-Allow-Methods",
            HeaderValue::from_static("GET, PUT, POST, OPTIONS"),
        );
        h.insert(
            "Access-Control-Allow-Headers",
            HeaderValue::from_static("*"),
        );
        h.insert(
            "Access-Control-Allow-Private-Network",
            HeaderValue::from_static("true"),
        );
        return res;
    }

    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    h.insert(
        "Access-Control-Allow-Private-Network",
        HeaderValue::from_static("true"),
    );
    response
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let scale_status = state.scale.status();
    Json(json!({
        "status": "ok",
        "version": state.version,
        "config_path": state.config_store.path().display().to_string(),
        "services": {
            "printer": { "available": true },
            "scale": {
                "enabled": scale_status.enabled,
                "connected": scale_status.connected,
                "protocol": scale_status.protocol,
                "port": scale_status.port,
            }
        }
    }))
}

async fn get_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "config": state.config_store.get(),
        "path": state.config_store.path().display().to_string(),
    }))
}

async fn put_config(
    State(state): State<AppState>,
    Json(update): Json<AgentConfigUpdate>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let current = state.config_store.get();
    let next = update.merge_into(current);

    match state.config_store.save(next) {
        Ok(path) => {
            state.scale.reload();
            Ok(Json(json!({
                "ok": true,
                "path": path.display().to_string(),
                "config": state.config_store.get(),
                "message": "Configuración guardada. La balanza se reconectó con los nuevos parámetros.",
            })))
        }
        Err(ConfigStoreError::Io(err)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        )),
        Err(ConfigStoreError::Serialize(err)) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        )),
    }
}

async fn list_printers(State(state): State<AppState>) -> Json<Value> {
    let printers = state.printer.list();
    Json(json!({ "printers": printers }))
}

async fn print_raw(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "empty body" })),
        ));
    }

    state.printer.print(&body).map_err(|e| {
        tracing::error!("Print error: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_scale_protocols() -> Json<Value> {
    Json(json!({ "protocols": SUPPORTED_PROTOCOLS }))
}

async fn list_scale_ports(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "ports": state.scale.list_ports() }))
}

async fn scale_status(State(state): State<AppState>) -> Json<Value> {
    Json(json!(state.scale.status()))
}

async fn scale_weight(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match state.scale.weight() {
        Ok(reading) => Ok(Json(json!(reading))),
        Err(ScaleError::Disabled) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": ScaleError::Disabled.to_string() })),
        )),
        Err(ScaleError::NoReading) => Ok(Json(json!({
            "kg": null,
            "value": null,
            "stable": false,
            "connected": state.scale.status().connected,
            "protocol": state.scale.status().protocol,
            "port": state.scale.status().port,
            "raw": null,
            "updated_at_ms": 0,
        }))),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        )),
    }
}
