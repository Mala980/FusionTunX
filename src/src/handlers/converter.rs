use crate::converter::{fetch_subscription, parse_link, parse_subscription, Proxy};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct ParseRequest {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Serialize)]
pub struct ParseResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Vec<Proxy>>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn parse_proxies(Json(req): Json<ParseRequest>) -> impl IntoResponse {
    let mut proxies = Vec::new();

    if !req.content.is_empty() && req.content != "string" {
        match parse_subscription(&req.content) {
            Ok(p) => proxies = p,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": e,
                        "count": 0
                    })),
                );
            }
        }
    } else if !req.url.is_empty() {
        if req.url.starts_with("http://") || req.url.starts_with("https://") {
            match fetch_subscription(&req.url).await {
                Ok(p) => proxies = p,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": e,
                            "count": 0
                        })),
                    );
                }
            }
        } else if req.url.starts_with("vmess://")
            || req.url.starts_with("vless://")
            || req.url.starts_with("trojan://")
            || req.url.starts_with("ss://")
        {
            match parse_link(&req.url) {
                Ok(p) => proxies.push(p),
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": e,
                            "count": 0
                        })),
                    );
                }
            }
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": "invalid URL: must be http(s):// subscription or vmess/vless/trojan/ss link",
                    "count": 0
                })),
            );
        }
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "either 'url' or 'content' is required",
                "count": 0
            })),
        );
    }

    let count = proxies.len();
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "proxies": proxies,
            "count": count
        })),
    )
}
