use crate::handlers::app::AppState;
use axum::extract::{Multipart, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Local;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::json;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tar::{Archive, Builder};
use walkdir::WalkDir;

pub async fn create_backup(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };

    let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("fusiontunx-backup-{}.tar.gz", timestamp);

    let mut tar_gz_buffer = Vec::new();
    {
        let enc = GzEncoder::new(&mut tar_gz_buffer, Compression::default());
        let mut tar = Builder::new(enc);

        let base_path = Path::new(&working_dir);
        if base_path.exists() {
            for entry in WalkDir::new(base_path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(rel_path) = path.strip_prefix(base_path) {
                        if let Ok(mut f) = File::open(path) {
                            let _ = tar.append_file(rel_path, &mut f);
                        }
                    }
                }
            }
        }
        let _ = tar.finish();
    }

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/gzip"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"backup.tar.gz\"")),
    );
    headers.insert("Content-Description", HeaderValue::from_static("File Transfer"));
    headers.insert("Content-Transfer-Encoding", HeaderValue::from_static("binary"));

    (StatusCode::OK, headers, tar_gz_buffer)
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };

    let mut file_bytes = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("backup") || field.file_name().is_some() {
            if let Ok(data) = field.bytes().await {
                file_bytes = data.to_vec();
                break;
            }
        }
    }

    if file_bytes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no backup file provided"})),
        );
    }

    let gz = GzDecoder::new(Cursor::new(file_bytes));
    let mut archive = Archive::new(gz);

    let target_dir = Path::new(&working_dir);
    if let Err(e) = fs::create_dir_all(target_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to create working directory: {}", e)})),
        );
    }

    if let Err(e) = archive.unpack(target_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("failed to unpack backup: {}", e)})),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Backup restored successfully"
        })),
    )
}
