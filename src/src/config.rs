use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub mihomo: MihomoConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_port() -> String {
    std::env::var("PORT").unwrap_or_else(|_| "8080".to_string())
}

fn default_host() -> String {
    std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string())
}

fn default_mode() -> String {
    std::env::var("GIN_MODE").unwrap_or_else(|_| "release".to_string())
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            mode: default_mode(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    Tun,
    TProxy,
    Redirect,
    Disable,
}

impl Default for RoutingMode {
    fn default() -> Self {
        RoutingMode::Disable
    }
}

impl RoutingMode {
    pub fn as_str(&self) -> &str {
        match self {
            RoutingMode::Tun => "tun",
            RoutingMode::TProxy => "tproxy",
            RoutingMode::Redirect => "redirect",
            RoutingMode::Disable => "disable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    #[serde(default)]
    pub tcp: RoutingMode,
    #[serde(default)]
    pub udp: RoutingMode,
    #[serde(default = "default_tun_device")]
    pub tun_device: String,
}

fn default_tun_device() -> String {
    "Meta".to_string()
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            tcp: RoutingMode::Redirect,
            udp: RoutingMode::Tun,
            tun_device: default_tun_device(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MihomoConfig {
    #[serde(default = "default_core_path")]
    pub core_path: String,
    #[serde(default = "default_config_path")]
    pub config_path: String,
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_true")]
    pub auto_restart: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_log_file")]
    pub log_file: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_secret: String,
    #[serde(default)]
    pub routing: RoutingConfig,
}

fn default_core_path() -> String {
    "/usr/bin/mihomo".to_string()
}

fn default_config_path() -> String {
    "/etc/fusiontunx/configs/config.yaml".to_string()
}

fn default_working_dir() -> String {
    "/etc/fusiontunx".to_string()
}

fn default_true() -> bool {
    true
}

fn default_log_file() -> String {
    "/var/log/mihomo.log".to_string()
}

impl Default for MihomoConfig {
    fn default() -> Self {
        Self {
            core_path: default_core_path(),
            config_path: default_config_path(),
            working_dir: default_working_dir(),
            auto_restart: true,
            auto_start: false,
            log_file: default_log_file(),
            api_url: String::new(),
            api_secret: String::new(),
            routing: RoutingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_app_log_file")]
    pub file: String,
    #[serde(default = "default_max_size")]
    pub max_size: i32,
    #[serde(default = "default_max_backups")]
    pub max_backups: i32,
    #[serde(default = "default_max_age")]
    pub max_age: i32,
}

fn default_log_level() -> String {
    std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
}

fn default_app_log_file() -> String {
    "/var/log/fusiontunx.log".to_string()
}

fn default_max_size() -> i32 {
    100
}

fn default_max_backups() -> i32 {
    3
}

fn default_max_age() -> i32 {
    28
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_app_log_file(),
            max_size: default_max_size(),
            max_backups: default_max_backups(),
            max_age: default_max_age(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_rate_limit")]
    pub rate_limit: i32,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
    #[serde(default = "default_true")]
    pub enable_swagger: bool,
    #[serde(default)]
    pub cors: CorsConfig,
}

fn default_rate_limit() -> i32 {
    100
}

fn default_timeout() -> i32 {
    30
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rate_limit: default_rate_limit(),
            timeout: default_timeout(),
            enable_swagger: true,
            cors: CorsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_origins")]
    pub allowed_origins: Vec<String>,
    #[serde(default = "default_methods")]
    pub allowed_methods: Vec<String>,
    #[serde(default = "default_headers")]
    pub allowed_headers: Vec<String>,
    #[serde(default = "default_expose_headers")]
    pub expose_headers: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_credentials: bool,
    #[serde(default = "default_cors_max_age")]
    pub max_age: i32,
}

fn default_origins() -> Vec<String> {
    vec![
        "http://localhost:3000".to_string(),
        "http://192.168.2.1:3000".to_string(),
        "http://127.0.0.1:3000".to_string(),
    ]
}

fn default_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}

fn default_headers() -> Vec<String> {
    vec![
        "Content-Type".to_string(),
        "Authorization".to_string(),
        "X-Requested-With".to_string(),
    ]
}

fn default_expose_headers() -> Vec<String> {
    vec!["Content-Length".to_string()]
}

fn default_cors_max_age() -> i32 {
    3600
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_origins: default_origins(),
            allowed_methods: default_methods(),
            allowed_headers: default_headers(),
            expose_headers: default_expose_headers(),
            allow_credentials: true,
            max_age: default_cors_max_age(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            mihomo: MihomoConfig::default(),
            logging: LoggingConfig::default(),
            api: ApiConfig::default(),
        }
    }
}

pub fn load(path: &str) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    if !Path::new(path).exists() {
        return create_default_config(path);
    }

    let data = fs::read_to_string(path)?;
    let mut config: Config = serde_yaml::from_str(&data)?;

    if !config.mihomo.config_path.is_empty() {
        if let Ok((api_url, secret)) = parse_mihomo_config(&config.mihomo.config_path) {
            config.mihomo.api_url = api_url;
            config.mihomo.api_secret = secret;
        }
    }

    Ok(config)
}

pub fn save(config: &Config, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = serde_yaml::to_string(config)?;
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, data)?;
    Ok(())
}

pub fn create_default_config(path: &str) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::default();
    save(&config, path)?;
    Ok(config)
}

#[derive(Deserialize)]
struct MihomoExternalConfig {
    #[serde(rename = "external-controller", default)]
    external_controller: Option<String>,
    #[serde(default)]
    secret: Option<String>,
}

pub fn parse_mihomo_config(config_path: &str) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let data = fs::read_to_string(config_path)?;
    let val: MihomoExternalConfig = serde_yaml::from_str(&data)?;

    let mut controller = val.external_controller.unwrap_or_else(|| "127.0.0.1:9090".to_string());
    if controller.is_empty() {
        controller = "127.0.0.1:9090".to_string();
    }

    let api_url = if !controller.starts_with("http://") && !controller.starts_with("https://") {
        format!("http://{}", controller)
    } else {
        controller
    };

    let secret = val.secret.unwrap_or_default();
    Ok((api_url, secret))
}
