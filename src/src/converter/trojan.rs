use super::types::{Proxy, ProxyType};
use std::collections::HashMap;
use url::form_urlencoded;

fn url_decode(s: &str) -> String {
    form_urlencoded::parse(format!("k={}", s).as_bytes())
        .find(|(k, _)| k == "k")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| s.to_string())
}

pub fn parse_trojan(link: &str) -> Result<Proxy, String> {
    if !link.starts_with("trojan://") {
        return Err("invalid trojan link".to_string());
    }

    let mut link = link.trim_start_matches("trojan://");

    let mut remark = String::new();
    if let Some(idx) = link.find('#') {
        remark = url_decode(&link[idx + 1..]);
        link = &link[..idx];
    }

    let mut query = "";
    if let Some(idx) = link.find('?') {
        query = &link[idx + 1..];
        link = &link[..idx];
    }

    let parts: Vec<&str> = link.split('@').collect();
    if parts.len() != 2 {
        return Err("invalid trojan link format".to_string());
    }

    let password = parts[0].to_string();
    let server_port = parts[1];

    let server_parts: Vec<&str> = server_port.split(':').collect();
    if server_parts.len() != 2 {
        return Err("invalid server:port format".to_string());
    }

    let server = server_parts[0].to_string();
    let port = server_parts[1].parse::<i32>().map_err(|e| format!("invalid port: {}", e))?;

    let mut proxy = Proxy {
        name: if remark.is_empty() { format!("{}:{}", server, port) } else { remark },
        proxy_type: ProxyType::Trojan,
        server,
        port,
        uuid: None,
        password: Some(password),
        cipher: None,
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
        alter_id: None,
        plugin: None,
        plugin_opts: None,
        skip_cert_verify: None,
    };

    if !query.is_empty() {
        let params: HashMap<String, String> = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();

        if let Some(security) = params.get("security") {
            if security == "tls" {
                proxy.tls = Some(true);
            }
        }

        if let Some(sni) = params.get("sni") {
            if !sni.is_empty() {
                proxy.sni = Some(sni.clone());
            }
        }

        if let Some(alpn) = params.get("alpn") {
            if !alpn.is_empty() {
                proxy.alpn = Some(alpn.split(',').map(|s| s.trim().to_string()).collect());
            }
        }

        if let Some(network) = params.get("type") {
            if !network.is_empty() {
                proxy.network = Some(network.clone());

                let path = params.get("path").map(|p| url_decode(p));
                let host = params.get("host").cloned();

                match network.as_str() {
                    "ws" | "websocket" => {
                        proxy.ws_path = path;
                        if let Some(h) = host {
                            let mut headers = HashMap::new();
                            headers.insert("Host".to_string(), h);
                            proxy.ws_headers = Some(headers);
                        }
                    }
                    "grpc" => {
                        if let Some(service_name) = params.get("serviceName") {
                            proxy.grpc_service_name = Some(service_name.clone());
                        }
                    }
                    "splithttp" => {
                        proxy.splithttp_path = path;
                    }
                    "xhttp" => {
                        proxy.xhttp_path = path;
                    }
                    "httpupgrade" => {
                        proxy.httpupgrade_path = path;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(proxy)
}
