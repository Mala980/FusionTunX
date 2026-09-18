use crate::handlers::app::AppState;
use axum::extract::{Path as AxumPath, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.mihomo_service.get_status().await;
    Json(json!({
        "success": true,
        "data": {
            "running": status == "running"
        }
    }))
}

pub async fn start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mihomo_service.start().await {
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Mihomo service started"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
    }
}

pub async fn stop(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mihomo_service.stop(true).await {
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Mihomo service stopped"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
    }
}

pub async fn restart(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mihomo_service.restart().await {
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Mihomo service restarted"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
    }
}

pub async fn get_core_version(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let core_path = { state.mihomo_service.config.read().await.mihomo.core_path.clone() };
    if core_path.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "error": "core path not configured"})),
        );
    }

    match Command::new(&core_path).arg("-v").output() {
        Ok(output) => {
            let ver_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let final_ver = if ver_str.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                ver_str
            };
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "data": { "version": final_ver }
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": format!("failed to get core version: {}", e)
            })),
        ),
    }
}

pub async fn get_dashboard_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.mihomo_service.config.read().await.clone();
    let ui_path = format!("{}/ui", cfg.mihomo.working_dir);

    let mut available_dashboards = Vec::new();
    if let Ok(entries) = fs::read_dir(&ui_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    available_dashboards.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }

    let mut port = "9090".to_string();
    if !cfg.mihomo.api_url.is_empty() {
        let parts: Vec<&str> = cfg.mihomo.api_url.split(':').collect();
        if parts.len() >= 3 {
            port = parts[2].trim_end_matches('/').to_string();
        }
    }

    Json(json!({
        "success": true,
        "data": {
            "port": port,
            "secret": cfg.mihomo.api_secret,
            "dashboards": available_dashboards
        }
    }))
}

pub async fn proxy_to_mihomo_api(
    State(state): State<Arc<AppState>>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let status = state.mihomo_service.get_status().await;
    if status != "running" {
        return (
            StatusCode::BAD_REQUEST,
            HeaderMap::new(),
            serde_json::to_vec(&json!({
                "success": false,
                "error": "mihomo is not running",
                "status": status
            }))
            .unwrap_or_default(),
        );
    }

    let cfg = state.mihomo_service.config.read().await.clone();
    let normalized_path = if path.starts_with('/') { path } else { format!("/{}", path) };
    let target_url = format!("{}{}", cfg.mihomo.api_url, normalized_path);

    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                serde_json::to_vec(&json!({"success": false, "error": format!("failed to build client: {}", e)}))
                    .unwrap_or_default(),
            );
        }
    };

    let mut req_builder = client.get(&target_url);
    if !cfg.mihomo.api_secret.is_empty() {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", cfg.mihomo.api_secret));
    }

    match req_builder.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::OK);
            let mut headers = HeaderMap::new();
            if let Some(ct) = resp.headers().get(CONTENT_TYPE) {
                headers.insert(CONTENT_TYPE, ct.clone());
            }
            let body = resp.bytes().await.unwrap_or_default().to_vec();
            (status, headers, body)
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            HeaderMap::new(),
            serde_json::to_vec(&json!({"success": false, "error": format!("Failed to connect to Mihomo API: {}", e)}))
                .unwrap_or_default(),
        ),
    }
}
