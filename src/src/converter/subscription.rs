use super::parser::parse_links;
use super::types::Proxy;
use super::vmess::decode_base64;
use std::time::Duration;

pub async fn fetch_subscription(url: &str) -> Result<Vec<Proxy>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed to build client: {}", e))?;

    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("failed to fetch subscription: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("subscription returned status {}", resp.status()));
    }

    let body_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("failed to read subscription body: {}", e))?;

    let content = String::from_utf8_lossy(&body_bytes).to_string();
    parse_subscription(&content)
}

pub fn parse_subscription(content: &str) -> Result<Vec<Proxy>, String> {
    let text = if let Ok(decoded) = decode_base64(content) {
        String::from_utf8_lossy(&decoded).to_string()
    } else {
        content.to_string()
    };

    let lines: Vec<String> = text
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    parse_links(&lines)
}
