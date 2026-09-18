use super::types::{Proxy, ProxyType};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;

pub fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let clean = input.trim();
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(clean) {
        return Ok(bytes);
    }
    if let Ok(bytes) = URL_SAFE.decode(clean) {
        return Ok(bytes);
    }
    if let Ok(bytes) = STANDARD_NO_PAD.decode(clean) {
        return Ok(bytes);
    }
    if let Ok(bytes) = STANDARD.decode(clean) {
        return Ok(bytes);
    }
    Err("Failed to decode base64".to_string())
}

fn get_string_value(v: &Option<Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

pub fn parse_vmess(link: &str) -> Result<Proxy, String> {
    if !link.starts_with("vmess://") {
        return Err("invalid vmess link".to_string());
    }

    let encoded = link.trim_start_matches("vmess://");
    let decoded_bytes = decode_base64(encoded).map_err(|e| format!("failed to decode vmess link: {}", e))?;
    let json_val: Value = serde_json::from_slice(&decoded_bytes)
        .map_err(|e| format!("failed to parse vmess config: {}", e))?;

    let ps = json_val.get("ps").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let add = json_val.get("add").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let port_str = get_string_value(&json_val.get("port").cloned());
    let port = port_str.parse::<i32>().map_err(|e| format!("invalid port: {}", e))?;

    let id = json_val.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let aid_str = get_string_value(&json_val.get("aid").cloned());
    let alter_id = aid_str.parse::<i32>().unwrap_or(0);

    let scy = json_val.get("scy").and_then(|v| v.as_str()).unwrap_or("auto").to_string();
    let cipher = if scy.is_empty() { "auto".to_string() } else { scy };

    let mut proxy = Proxy {
        name: if ps.is_empty() { format!("{}:{}", add, port) } else { ps },
        proxy_type: ProxyType::VMess,
        server: add,
        port,
        uuid: Some(id),
        password: None,
        cipher: Some(cipher),
        udp: Some(true),
        tls: None,
        sni: None,
        alpn: None,
        network: None,
        ws_path: None,
        ws_headers: None,
        grpc_service_name: None,
        splithttp_path: None,
        xhttp_path: None,
        httpupgrade_path: None,
        flow: None,
        alter_id: Some(alter_id),
        plugin: None,
        plugin_opts: None,
        skip_cert_verify: None,
    };

    if let Some(tls) = json_val.get("tls").and_then(|v| v.as_str()) {
        if tls == "tls" {
            proxy.tls = Some(true);
        }
    }

    if let Some(sni) = json_val.get("sni").and_then(|v| v.as_str()) {
        if !sni.is_empty() {
            proxy.sni = Some(sni.to_string());
        }
    }

    if let Some(alpn_val) = json_val.get("alpn") {
        if let Some(s) = alpn_val.as_str() {
            if !s.is_empty() {
                proxy.alpn = Some(s.split(',').map(|x| x.trim().to_string()).collect());
            }
        } else if let Some(arr) = alpn_val.as_array() {
            proxy.alpn = Some(
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect(),
            );
        }
    }

    let net = json_val.get("net").and_then(|v| v.as_str()).unwrap_or("");
    let path = json_val.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let host = json_val.get("host").and_then(|v| v.as_str()).unwrap_or("");

    if !net.is_empty() {
        proxy.network = Some(net.to_string());
        match net {
            "ws" | "websocket" => {
                if !path.is_empty() {
                    proxy.ws_path = Some(path.to_string());
                }
                if !host.is_empty() {
                    let mut headers = HashMap::new();
                    headers.insert("Host".to_string(), host.to_string());
                    proxy.ws_headers = Some(headers);
                }
            }
            "grpc" => {
                if !path.is_empty() {
                    proxy.grpc_service_name = Some(path.to_string());
                }
            }
            "splithttp" => {
                if !path.is_empty() {
                    proxy.splithttp_path = Some(path.to_string());
                }
            }
            "xhttp" => {
                if !path.is_empty() {
                    proxy.xhttp_path = Some(path.to_string());
                }
            }
            "httpupgrade" => {
                if !path.is_empty() {
                    proxy.httpupgrade_path = Some(path.to_string());
                }
            }
            _ => {}
        }
    }

    Ok(proxy)
}
