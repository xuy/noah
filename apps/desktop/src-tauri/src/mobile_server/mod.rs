mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State as AxumState;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::{Any, CorsLayer};

use crate::agent::orchestrator::{Orchestrator, PendingApprovals};

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub device_id: String,
    pub secret: String,
    pub device_name: String,
    pub paired_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub event: String,
    pub data: serde_json::Value,
}

pub struct PairingState {
    pub pending_token: Option<PendingToken>,
    pub paired_device: Option<PairedDevice>,
    pub failed_attempts: u32,
}

pub struct PendingToken {
    pub token: String,
    pub created_at: std::time::Instant,
}

pub struct MobileServerState {
    pub orchestrator: Arc<Mutex<Orchestrator>>,
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub pending_approvals: PendingApprovals,
    pub pairing: Mutex<PairingState>,
    pub sse_tx: broadcast::Sender<SseEvent>,
    pub app_dir: PathBuf,
    pub app_handle: Option<tauri::AppHandle>,
    pub port: std::sync::atomic::AtomicU16,
}

// ── Server setup ──

pub fn build_router(state: Arc<MobileServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/pair", post(handlers::pair))
        .route("/generate-qr", get(handlers::generate_qr))
        .route("/status", get(handlers::status))
        .route("/triage", post(handlers::triage))
        .route("/approve/{id}", post(handlers::approve))
        .route("/sessions", get(handlers::list_sessions))
        .route("/sessions/{id}/messages", get(handlers::session_messages))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state)
}

async fn auth_middleware(
    AxumState(state): AxumState<Arc<MobileServerState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    // Skip auth for /pair and /generate-qr
    let path = request.uri().path();
    if path == "/pair" || path == "/generate-qr" {
        return next.run(request).await;
    }

    let pairing = state.pairing.lock().await;
    let paired = match &pairing.paired_device {
        Some(d) => d.clone(),
        None => {
            return (StatusCode::UNAUTHORIZED, "Not paired").into_response();
        }
    };
    drop(pairing);

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Some(secret) = auth_header.strip_prefix("Bearer ") {
        if secret == paired.secret {
            return next.run(request).await;
        }
    }

    (StatusCode::UNAUTHORIZED, "Invalid secret").into_response()
}

/// Start the embedded HTTP server. Returns the port it bound to.
pub async fn start_server(state: Arc<MobileServerState>) -> anyhow::Result<u16> {
    let router = build_router(state);

    // Try ports 7892-7899
    for port in 7892..=7899 {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                eprintln!("[mobile-server] Listening on 0.0.0.0:{}", port);
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, router).await {
                        eprintln!("[mobile-server] Server error: {}", e);
                    }
                });
                return Ok(port);
            }
            Err(_) => continue,
        }
    }

    anyhow::bail!("Could not bind to any port in 7892-7899")
}

/// Generate a pairing token and return QR data.
pub async fn generate_pairing_data(
    state: &Arc<MobileServerState>,
    port: u16,
) -> anyhow::Result<PairingQrData> {
    use rand::Rng;

    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let mut pairing = state.pairing.lock().await;
    pairing.pending_token = Some(PendingToken {
        token: token.clone(),
        created_at: std::time::Instant::now(),
    });
    pairing.failed_attempts = 0;

    let qr_payload = serde_json::json!({
        "version": 1,
        "host": local_ip,
        "port": port,
        "token": token,
    });

    Ok(PairingQrData {
        host: local_ip,
        port,
        token,
        qr_json: qr_payload.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingQrData {
    pub host: String,
    pub port: u16,
    pub token: String,
    pub qr_json: String,
}
