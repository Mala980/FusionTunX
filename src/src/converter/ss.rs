use super::types::{Proxy, ProxyType};
use super::vmess::decode_base64;
use std::collections::HashMap;
use url::form_urlencoded;

fn url_decode(s: &str) -> String {
    form_urlencoded::parse(format!("k={}", s).as_bytes())
        .find(|(k, _)| k == "k")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| s.to_string())
}

pub fn parse_ss(link: &str) -> Result<Proxy, String> {
    if !link.starts_with("ss://") {
        return Err("invalid ss link".to_string());
    }

    let mut link = link.trim_start_matches("ss://");

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

    let method: String;
    let password: String;
    let server: String;
    let port: i32;

    if link.contains('@') {
        let parts: Vec<&str> = link.split('@').collect();
        if parts.len() != 2 {
            return Err("invalid ss link format".to_string());
        }

        let userinfo = parts[0];
        let server_port = parts[1];

        let decoded = decode_base64(userinfo).map_err(|e| format!("failed to decode userinfo: {}", e))?;
        let userinfo_str = String::from_utf8_lossy(&decoded);
        let method_pw: Vec<&str> = userinfo_str.splitn(2, ':').collect();
        if method_pw.len() != 2 {
            return Err("invalid method:password format".to_string());
        }

        method = method_pw[0].to_string();
        password = method_pw[1].to_string();

        let server_parts: Vec<&str> = server_port.split(':').collect();
        if server_parts.len() != 2 {
            return Err("invalid server:port format".to_string());
        }

        server = server_parts[0].to_string();
        port = server_parts[1].parse::<i32>().map_err(|e| format!("invalid port: {}", e))?;
    } else {
        let decoded = decode_base64(link).map_err(|e| format!("failed to decode ss link: {}", e))?;
        let decoded_str = String::from_utf8_lossy(&decoded);

        let parts: Vec<&str> = decoded_str.splitn(2, '@').collect();
        if parts.len() != 2 {
            return Err("invalid ss link format".to_string());
        }

        let method_pw: Vec<&str> = parts[0].splitn(2, ':').collect();
        if method_pw.len() != 2 {
            return Err("invalid method:password format".to_string());
        }

        method = method_pw[0].to_string();
        password = method_pw[1].to_string();

        let server_parts: Vec<&str> = parts[1].split(':').collect();
        if server_parts.len() != 2 {
            return Err("invalid server:port format".to_string());
        }

        server = server_parts[0].to_string();
        port = server_parts[1].parse::<i32>().map_err(|e| format!("invalid port: {}", e))?;
    }

    let mut proxy = Proxy {
        name: if remark.is_empty() { format!("{}:{}", server, port) } else { remark },
        proxy_type: ProxyType::SS,
        server,
        port,
        uuid: None,
        password: Some(password),
        cipher: Some(method),
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

        if let Some(plugin_str) = params.get("plugin") {
            let mut plugin_parts = plugin_str.splitn(2, ';');
            if let Some(p) = plugin_parts.next() {
                proxy.plugin = Some(p.to_string());
            }

            if let Some(opts_str) = plugin_parts.next() {
                let mut opts = HashMap::new();
                for opt in opts_str.split(';') {
                    let mut kv = opt.splitn(2, '=');
                    if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                        opts.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                    }
                }
                if !opts.is_empty() {
                    proxy.plugin_opts = Some(opts);
                }
            }
        }
    }

    Ok(proxy)
}
