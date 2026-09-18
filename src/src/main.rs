#[macro_use]
mod logger;
mod config;
mod converter;
mod handlers;
mod router;
mod service;
mod ui;

use handlers::AppState;
use router::create_router;
use service::{MihomoService, NftablesService};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut config_path = "/etc/fusiontunx/app.yaml".to_string();

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "-c" || args[i] == "-config" || args[i] == "--config" {
            if i + 1 < args.len() {
                config_path = args[i + 1].clone();
                i += 1;
            }
        }
        i += 1;
    }

    let cfg = match config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config from {}: {}", config_path, e);
            std::process::exit(1);
        }
    };

    if let Err(e) = logger::init(&cfg.logging.level, &cfg.logging.file) {
        eprintln!("Failed to initialize logger: {}", e);
        std::process::exit(1);
    }

    let nftables = Arc::new(NftablesService::new());
    let cfg_lock = Arc::new(RwLock::new(cfg.clone()));
    let mihomo_service = Arc::new(MihomoService::new(
        cfg_lock.clone(),
        config_path.clone(),
        nftables.clone(),
    ));

    if let Err(e) = mihomo_service.restore_state().await {
        log_warn!("Failed to restore mihomo state: {}", e);
    }

    let state = Arc::new(AppState {
        mihomo_service: mihomo_service.clone(),
    });

    let app = create_router(state, &cfg.api.cors, cfg.api.enable_swagger);

    let address_str = format!("{}:{}", cfg.server.host, cfg.server.port);
    let addr: SocketAddr = address_str.parse().unwrap_or_else(|_| {
        format!("0.0.0.0:{}", cfg.server.port)
            .parse()
            .expect("Valid bind address")
    });

    log_info!("Starting server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log_error!("Failed to bind server on {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(mihomo_service.clone()));

    if let Err(e) = server.await {
        log_error!("Server error: {}", e);
    }

    log_info!("Server exited");
    Ok(())
}

async fn shutdown_signal(mihomo_service: Arc<MihomoService>) {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to listen for terminate signal")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log_info!("Shutting down server...");

    if mihomo_service.get_status().await == "running" {
        log_info!("Stopping mihomo service...");
        if let Err(e) = mihomo_service.stop(false).await {
            log_warn!("Failed to stop mihomo: {}", e);
        }
    }
}
