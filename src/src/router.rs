use crate::config::CorsConfig;
use crate::handlers::{app, backup, converter, dns, files, mihomo, stream, AppState};
use crate::ui::{static_handler, swagger_handler};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

pub fn build_cors_layer(cfg: &CorsConfig) -> CorsLayer {
    if !cfg.enabled {
        return CorsLayer::permissive();
    }

    let mut layer = CorsLayer::new();

    // Origins
    if cfg.allowed_origins.contains(&"*".to_string()) {
        layer = layer.allow_origin(Any);
    } else {
        let origins: Vec<HeaderValue> = cfg
            .allowed_origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        layer = layer.allow_origin(origins);
    }

    // Methods
    let methods: Vec<Method> = cfg
        .allowed_methods
        .iter()
        .filter_map(|m| Method::from_str(m).ok())
        .collect();
    layer = layer.allow_methods(methods);

    // Headers
    let headers: Vec<HeaderName> = cfg
        .allowed_headers
        .iter()
        .filter_map(|h| HeaderName::from_str(h).ok())
        .collect();
    layer = layer.allow_headers(headers);

    // Expose headers
    let expose: Vec<HeaderName> = cfg
        .expose_headers
        .iter()
        .filter_map(|h| HeaderName::from_str(h).ok())
        .collect();
    layer = layer.expose_headers(expose);

    if cfg.allow_credentials && !cfg.allowed_origins.contains(&"*".to_string()) {
        layer = layer.allow_credentials(true);
    }

    if cfg.max_age > 0 {
        layer = layer.max_age(Duration::from_secs(cfg.max_age as u64));
    }

    layer
}

pub fn create_router(state: Arc<AppState>, cors_config: &CorsConfig, enable_swagger: bool) -> Router {
    let api_router = Router::new()
        // Backup
        .route("/backup/create", post(backup::create_backup))
        .route("/backup/restore", post(backup::restore_backup))
        // Converter
        .route("/converter/parse", post(converter::parse_proxies))
        // DNS
        .route("/dns/lookup", post(dns::lookup_domain))
        // Mihomo Core
        .route("/mihomo/status", get(mihomo::get_status))
        .route("/mihomo/start", post(mihomo::start))
        .route("/mihomo/stop", post(mihomo::stop))
        .route("/mihomo/restart", post(mihomo::restart))
        .route("/mihomo/logs", get(stream::stream_mihomo_logs).delete(stream::clear_mihomo_logs))
        .route("/mihomo/memory", get(stream::stream_memory))
        .route("/mihomo/traffic", get(stream::stream_traffic))
        .route("/mihomo/connections", get(stream::stream_connections))
        .route("/mihomo/core-version", get(mihomo::get_core_version))
        .route("/mihomo/dashboard-info", get(mihomo::get_dashboard_info))
        .route("/mihomo/api/*path", get(mihomo::proxy_to_mihomo_api))
        // Active config
        .route("/mihomo/active-config", get(files::get_active_config_path).put(files::set_active_config_path))
        // Generic Mihomo files: configs, proxy-providers, rule-providers
        .route("/mihomo/:dir", get(files::get_files).post(files::create_file))
        .route("/mihomo/:dir/upload", post(files::upload_file))
        .route(
            "/mihomo/:dir/:filename",
            get(files::get_file_content)
                .put(files::update_file)
                .delete(files::delete_file),
        )
        .route("/mihomo/:dir/:filename/rename", put(files::rename_file))
        .route("/mihomo/:dir/:filename/download", get(files::download_file))
        // App config and system info
        .route("/app/config", get(app::get_config).put(app::update_config))
        .route("/app/logs", get(stream::stream_app_logs).delete(stream::clear_app_logs))
        .route("/app/ipv4", get(app::get_ipv4))
        .route("/app/ipv6", get(app::get_ipv6))
        .route("/app/geo/ipv4", get(app::get_geo_ipv4))
        .route("/app/geo/ipv6", get(app::get_geo_ipv6));

    let mut router = Router::new()
        .nest("/api/v1", api_router)
        .with_state(state);

    if enable_swagger {
        router = router
            .route("/docs", get(swagger_handler))
            .route("/docs/*path", get(swagger_handler));
    }

    let cors = build_cors_layer(cors_config);
    router = router.layer(cors);

    // Fallback static files & SPA
    router.fallback(static_handler)
}
