use crate::handlers::app::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub async fn stream_mihomo_logs(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_mihomo_logs_socket(socket, state))
}

async fn handle_mihomo_logs_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let status = state.mihomo_service.get_status().await;
    if status != "running" {
        let _ = socket.send(Message::Text("ERROR: mihomo is not running".into())).await;
        return;
    }

    let log_file = { state.mihomo_service.config.read().await.mihomo.log_file.clone() };
    stream_file(socket, log_file).await;
}

pub async fn clear_mihomo_logs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let log_file = { state.mihomo_service.config.read().await.mihomo.log_file.clone() };
    if log_file.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "log file not configured"})),
        );
    }

    match OpenOptions::new().write(true).truncate(true).open(&log_file) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"message": "Mihomo logs cleared successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to clear log file: {}", e)})),
        ),
    }
}

pub async fn stream_app_logs(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let log_file = { state.mihomo_service.config.read().await.logging.file.clone() };
    ws.on_upgrade(move |socket| stream_file(socket, log_file))
}

pub async fn clear_app_logs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let log_file = { state.mihomo_service.config.read().await.logging.file.clone() };
    if log_file.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "log file not configured"})),
        );
    }

    match OpenOptions::new().write(true).truncate(true).open(&log_file) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"message": "Application logs cleared successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to clear log file: {}", e)})),
        ),
    }
}

async fn stream_file(mut socket: WebSocket, log_file: String) {
    if log_file.is_empty() {
        let _ = socket.send(Message::Text("ERROR: log file not configured".into())).await;
        return;
    }

    if !Path::new(&log_file).exists() {
        let _ = socket.send(Message::Text(format!("ERROR: log file does not exist: {}", log_file).into())).await;
        return;
    }

    let mut file = match File::open(&log_file) {
        Ok(f) => f,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("ERROR: failed to open log file: {}", e).into())).await;
            return;
        }
    };

    // Initial read last 4KB
    let mut current_size = 0u64;
    if let Ok(metadata) = file.metadata() {
        let file_size = metadata.len();
        let start_pos = if file_size > 4096 { file_size - 4096 } else { 0 };

        if file.seek(SeekFrom::Start(start_pos)).is_ok() {
            let mut initial_data = vec![0u8; (file_size - start_pos) as usize];
            if let Ok(n) = file.read(&mut initial_data) {
                if n > 0 {
                    let content = String::from_utf8_lossy(&initial_data[..n]);
                    let lines: Vec<&str> = content.split('\n').collect();
                    let start_idx = if start_pos > 0 && !lines.is_empty() { 1 } else { 0 };

                    for line in &lines[start_idx..] {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            if socket.send(Message::Text(trimmed.to_string().into())).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        }
        current_size = file_size;
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    loop {
        ticker.tick().await;

        let metadata = match fs::metadata(&log_file) {
            Ok(m) => m,
            Err(_) => return,
        };

        let new_size = metadata.len();
        if new_size < current_size {
            let _ = file.seek(SeekFrom::Start(0));
            current_size = 0;
        }

        if new_size == current_size {
            continue;
        }

        let bytes_to_read = (new_size - current_size) as usize;
        let mut buffer = vec![0u8; bytes_to_read];
        if file.seek(SeekFrom::Start(current_size)).is_ok() {
            if let Ok(n) = file.read(&mut buffer) {
                if n > 0 {
                    let content = String::from_utf8_lossy(&buffer[..n]);
                    for line in content.split('\n') {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            if socket.send(Message::Text(trimmed.to_string().into())).await.is_err() {
                                return;
                            }
                        }
                    }
                    current_size += n as u64;
                }
            }
        }
    }
}

pub async fn stream_traffic(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_mihomo_stream_api(socket, state, "/traffic"))
}

pub async fn stream_memory(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_mihomo_stream_api(socket, state, "/memory"))
}

pub async fn stream_connections(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_connections_poll(socket, state))
}

async fn stream_mihomo_stream_api(mut socket: WebSocket, state: Arc<AppState>, endpoint: &'static str) {
    let status = state.mihomo_service.get_status().await;
    if status != "running" {
        let _ = socket.send(Message::Text("ERROR: mihomo is not running".into())).await;
        return;
    }

    let cfg = state.mihomo_service.config.read().await.clone();
    let url = format!("{}{}", cfg.mihomo.api_url, endpoint);

    let client = reqwest::Client::new();
    let mut req_builder = client.get(&url);
    if !cfg.mihomo.api_secret.is_empty() {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", cfg.mihomo.api_secret));
    }

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("ERROR: failed to connect to Mihomo API: {}", e).into())).await;
            return;
        }
    };

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        if socket.send(Message::Text(trimmed.to_string().into())).await.is_err() {
                            return;
                        }
                    }
                }
            }
            Err(_) => return,
        }
    }
}

async fn stream_connections_poll(mut socket: WebSocket, state: Arc<AppState>) {
    let status = state.mihomo_service.get_status().await;
    if status != "running" {
        let _ = socket.send(Message::Text("ERROR: mihomo is not running".into())).await;
        return;
    }

    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return,
    };

    loop {
        ticker.tick().await;

        let cfg = state.mihomo_service.config.read().await.clone();
        let url = format!("{}/connections", cfg.mihomo.api_url);

        let mut req_builder = client.get(&url);
        if !cfg.mihomo.api_secret.is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", cfg.mihomo.api_secret));
        }

        if let Ok(resp) = req_builder.send().await {
            if let Ok(bytes) = resp.bytes().await {
                let s = String::from_utf8_lossy(&bytes).to_string();
                if socket.send(Message::Text(s.into())).await.is_err() {
                    return;
                }
            }
        }
    }
}
