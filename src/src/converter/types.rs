use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    VMess,
    VLess,
    Trojan,
    #[serde(rename = "ss")]
    SS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: ProxyType,
    pub server: String,
    pub port: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    #[serde(rename = "ws-path", skip_serializing_if = "Option::is_none")]
    pub ws_path: Option<String>,

    #[serde(rename = "ws-headers", skip_serializing_if = "Option::is_none")]
    pub ws_headers: Option<HashMap<String, String>>,

    #[serde(rename = "grpc-service-name", skip_serializing_if = "Option::is_none")]
    pub grpc_service_name: Option<String>,

    #[serde(rename = "splithttp-path", skip_serializing_if = "Option::is_none")]
    pub splithttp_path: Option<String>,

    #[serde(rename = "xhttp-path", skip_serializing_if = "Option::is_none")]
    pub xhttp_path: Option<String>,

    #[serde(rename = "httpupgrade-path", skip_serializing_if = "Option::is_none")]
    pub httpupgrade_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,

    #[serde(rename = "alterId", skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,

    #[serde(rename = "plugin-opts", skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<HashMap<String, serde_json::Value>>,

    #[serde(rename = "skip-cert-verify", skip_serializing_if = "Option::is_none")]
    pub skip_cert_verify: Option<bool>,
}
