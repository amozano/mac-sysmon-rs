// Copyright 2026 Ashton Mozano / mac-sysmon-rs contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::models::*;
use crate::process_ops;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tracing::{error, warn};

#[derive(RustEmbed)]
#[folder = "dist/"]
struct Asset;

#[derive(Clone)]
pub struct AppState {
    pub latest_metrics: Arc<RwLock<SystemMetrics>>,
    pub broadcast_tx: broadcast::Sender<SystemMetrics>,
}

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .route("/api/system", get(get_system_metrics))
        .route("/api/processes", get(get_processes))
        .route("/api/processes/:pid", get(get_process_detail))
        .route("/api/processes/:pid/kill", post(kill_process))
        .route("/ws", get(ws_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(app_state)
}

async fn get_system_metrics(State(state): State<AppState>) -> Json<ApiResponse<SystemMetrics>> {
    let metrics = state.latest_metrics.read().await.clone();
    Json(ApiResponse {
        success: true,
        message: "System metrics snapshot retrieved".to_string(),
        data: Some(metrics),
    })
}

async fn get_processes(State(state): State<AppState>) -> Json<ApiResponse<Vec<ProcessMetrics>>> {
    let metrics = state.latest_metrics.read().await;
    Json(ApiResponse {
        success: true,
        message: "Process list retrieved".to_string(),
        data: Some(metrics.processes.clone()),
    })
}

async fn get_process_detail(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<Json<ApiResponse<ProcessDetail>>, (StatusCode, Json<ApiResponse<()>>)> {
    match process_ops::get_process_detail(pid, state.latest_metrics).await {
        Ok(detail) => Ok(Json(ApiResponse {
            success: true,
            message: format!("Process {} details retrieved", pid),
            data: Some(detail),
        })),
        Err(err) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                message: err,
                data: None,
            }),
        )),
    }
}

async fn kill_process(
    Path(pid): Path<u32>,
    Json(payload): Json<KillProcessRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let sig_str = payload.signal.as_deref().unwrap_or("SIGTERM");
    match process_ops::kill_process(pid, Some(sig_str)) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Signal {} delivered successfully to PID {}", sig_str, pid),
                data: None,
            }),
        ),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: err,
                data: None,
            }),
        ),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();

    // Send immediate snapshot on connect
    {
        let snapshot = state.latest_metrics.read().await.clone();
        if let Ok(json) = serde_json::to_string(&snapshot) {
            if let Err(e) = sender.send(Message::Text(json)).await {
                warn!("Failed to send initial WS snapshot: {}", e);
                return;
            }
        }
    }

    // Task to forward broadcast metrics to the websocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(metrics) = rx.recv().await {
            match serde_json::to_string(&metrics) {
                Ok(json) => {
                    if let Err(_) = sender.send(Message::Text(json)).await {
                        // Client disconnected
                        break;
                    }
                }
                Err(e) => {
                    error!("Error serializing metrics for WS: {}", e);
                }
            }
        }
    });

    // Task to receive client messages (pings, close, etc.)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // If either task completes, abort the other
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Asset::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
                .body(axum::body::Body::from(content.data))
                .unwrap()
        }
        None => {
            // SPA fallback to index.html if not found
            if let Some(content) = Asset::get("index.html") {
                Response::builder()
                    .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
                    .body(axum::body::Body::from(content.data))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(axum::body::Body::from("404 Not Found - Assets not built"))
                    .unwrap()
            }
        }
    }
}
