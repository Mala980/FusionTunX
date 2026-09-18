use crate::config::{save, Config, MihomoConfig};
use crate::service::MihomoService;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

pub struct AppState {
    pub mihomo_service: Arc<MihomoService>,
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.mihomo_service.config.read().await;
    Json(json!({
        "mihomo": cfg.mihomo,
        "logging": {
            "level": cfg.logging.level
        }
    }))
}

#[derive(Deserialize)]
pub struct UpdateConfigLogging {
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub mihomo: Option<MihomoConfig>,
    pub logging: Option<UpdateConfigLogging>,
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfigRequest>,
) -> impl IntoResponse {
    let mut needs_restart = false;
    let status = state.mihomo_service.get_status().await;

    {
        let mut cfg = state.mihomo_service.config.write().await;
        if let Some(m) = payload.mihomo {
            needs_restart = m.auto_restart && status == "running";
            cfg.mihomo = m;
        }
        if let Some(l) = payload.logging {
            if let Some(lvl) = l.level {
                if !lvl.is_empty() {
                    cfg.logging.level = lvl;
                }
            }
        }
        if let Err(e) = save(&cfg, &state.mihomo_service.config_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": format!("failed to save config: {}", e)
                })),
            );
        }
    }

    if needs_restart {
        if let Err(e) = state.mihomo_service.restart().await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": format!("config updated but failed to restart mihomo: {}", e)
                })),
            );
        }
        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Configuration updated and mihomo restarted successfully"
            })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Configuration updated successfully. Restart application to apply logging changes."
        })),
    )
}

pub async fn get_ipv4() -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to create request"})),
            )
        }
    };

    match client.get("https://api-ipv4.ip.sb/ip").header("User-Agent", "Mozilla/5.0").send().await {
        Ok(resp) => {
            if let Ok(ip) = resp.text().await {
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "data": { "ip": ip.trim() }
                    })),
                )
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"success": false, "error": "Failed to read IPv4 response"})),
                )
            }
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"success": false, "error": "Failed to get IPv4 address"})),
        ),
    }
}

pub async fn get_ipv6() -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to create request"})),
            )
        }
    };

    match client.get("https://api-ipv6.ip.sb/ip").header("User-Agent", "Mozilla/5.0").send().await {
        Ok(resp) => {
            if let Ok(ip) = resp.text().await {
                (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "data": { "ip": ip.trim() }
                    })),
                )
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"success": false, "error": "Failed to read IPv6 response"})),
                )
            }
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"success": false, "error": "Failed to get IPv6 address"})),
        ),
    }
}

pub async fn get_geo_ipv4() -> impl IntoResponse {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to create request"})),
            )
        }
    };

    match client.get("https://api-ipv4.ip.sb/geoip").header("User-Agent", "Mozilla/5.0").send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => (StatusCode::OK, Json(json!({"success": true, "data": data}))),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to parse geolocation data"})),
            ),
        },
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"success": false, "error": "Failed to get IPv4 geolocation"})),
        ),
    }
}

pub async fn get_geo_ipv6() -> impl IntoResponse {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to create request"})),
            )
        }
    };

    match client.get("https://api-ipv6.ip.sb/geoip").header("User-Agent", "Mozilla/5.0").send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => (StatusCode::OK, Json(json!({"success": true, "data": data}))),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to parse geolocation data"})),
            ),
        },
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"success": false, "error": "Failed to get IPv6 geolocation"})),
        ),
    }
}
