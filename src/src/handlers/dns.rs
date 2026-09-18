use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::IpAddr;

#[derive(Deserialize)]
pub struct LookupRequest {
    pub domain: String,
}

pub async fn lookup_domain(Json(req): Json<LookupRequest>) -> impl IntoResponse {
    let domain = req.domain.trim();
    if domain.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": "invalid request: domain is required"
            })),
        );
    }

    let host_port = format!("{}:0", domain);
    match tokio::net::lookup_host(&host_port).await {
        Ok(addrs) => {
            let mut ipv4 = Vec::new();
            let mut ipv6 = Vec::new();

            for addr in addrs {
                match addr.ip() {
                    IpAddr::V4(v4) => {
                        let s = v4.to_string();
                        if !ipv4.contains(&s) {
                            ipv4.push(s);
                        }
                    }
                    IpAddr::V6(v6) => {
                        let s = v6.to_string();
                        if !ipv6.contains(&s) {
                            ipv6.push(s);
                        }
                    }
                }
            }

            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "domain": domain,
                    "ipv4": ipv4,
                    "ipv6": ipv6
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "domain": domain,
                "error": format!("failed to lookup domain: {}", e)
            })),
        ),
    }
}
