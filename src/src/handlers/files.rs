use crate::config::save;
use crate::handlers::app::AppState;
use axum::extract::{Multipart, Path as AxumPath, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn normalize_dir(dir: &str) -> Option<&'static str> {
    match dir {
        "configs" => Some("configs"),
        "proxy-providers" | "proxy_providers" => Some("proxy_providers"),
        "rule-providers" | "rule_providers" => Some("rule_providers"),
        _ => None,
    }
}

fn is_path_safe(path: &Path, base: &Path) -> bool {
    if let (Ok(canonical_path), Ok(canonical_base)) = (path.canonicalize(), base.canonicalize()) {
        canonical_path.starts_with(canonical_base)
    } else {
        // If file doesn't exist yet, check parent
        if let Some(parent) = path.parent() {
            if let (Ok(canonical_parent), Ok(canonical_base)) = (parent.canonicalize(), base.canonicalize()) {
                return canonical_parent.starts_with(canonical_base);
            }
        }
        false
    }
}

fn check_yaml_ext(filename: &str) -> bool {
    filename.ends_with(".yaml") || filename.ends_with(".yml")
}

pub async fn get_files(
    State(state): State<Arc<AppState>>,
    AxumPath(dir): AxumPath<String>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))).into_response(),
    };

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let dir_path = Path::new(&working_dir).join(dir_name);

    if !dir_path.exists() {
        let _ = fs::create_dir_all(&dir_path);
    }

    match fs::read_dir(&dir_path) {
        Ok(entries) => {
            let mut file_names = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        file_names.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            Json(json!(file_names)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ).into_response(),
    }
}

pub async fn get_file_content(
    State(state): State<Arc<AppState>>,
    AxumPath((dir, filename)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    if !check_yaml_ext(&filename) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "only yaml files are allowed"})));
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let file_path = base_dir.join(&filename);

    if !is_path_safe(&file_path, &base_dir) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid filename"})));
    }

    match fs::read_to_string(&file_path) {
        Ok(content) => (StatusCode::OK, Json(json!({"content": content}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
pub struct CreateFileRequest {
    pub filename: String,
    pub content: String,
}

pub async fn create_file(
    State(state): State<Arc<AppState>>,
    AxumPath(dir): AxumPath<String>,
    Json(req): Json<CreateFileRequest>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    if !check_yaml_ext(&req.filename) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "only yaml files are allowed"})));
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let file_path = base_dir.join(&req.filename);

    if !base_dir.exists() {
        let _ = fs::create_dir_all(&base_dir);
    }

    if !is_path_safe(&file_path, &base_dir) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid filename"})));
    }

    if file_path.exists() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "file already exists"})));
    }

    match fs::write(&file_path, &req.content) {
        Ok(_) => (StatusCode::CREATED, Json(json!({"message": "File created successfully"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
pub struct UpdateFileRequest {
    pub content: String,
}

pub async fn update_file(
    State(state): State<Arc<AppState>>,
    AxumPath((dir, filename)): AxumPath<(String, String)>,
    Json(req): Json<UpdateFileRequest>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    if !check_yaml_ext(&filename) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "only yaml files are allowed"})));
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let file_path = base_dir.join(&filename);

    if !is_path_safe(&file_path, &base_dir) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid filename"})));
    }

    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "file does not exist"})));
    }

    if let Err(e) = fs::write(&file_path, &req.content) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})));
    }

    let cfg = state.mihomo_service.config.read().await.clone();
    let is_active = dir_name == "configs" && file_path == Path::new(&cfg.mihomo.config_path);
    let status = state.mihomo_service.get_status().await;

    if cfg.mihomo.auto_restart && status == "running" && is_active {
        if let Err(e) = state.mihomo_service.restart().await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("file updated but failed to restart mihomo: {}", e)})),
            );
        }
        return (
            StatusCode::OK,
            Json(json!({"message": "File updated and mihomo restarted successfully"})),
        );
    }

    (StatusCode::OK, Json(json!({"message": "File updated successfully"})))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    AxumPath((dir, filename)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    if !check_yaml_ext(&filename) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "only yaml files are allowed"})));
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let file_path = base_dir.join(&filename);

    if !is_path_safe(&file_path, &base_dir) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid filename"})));
    }

    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "file does not exist"})));
    }

    let active_path = { state.mihomo_service.config.read().await.mihomo.config_path.clone() };
    if dir_name == "configs" && file_path == Path::new(&active_path) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "cannot delete active config file"})),
        );
    }

    if let Err(e) = fs::remove_file(&file_path) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})));
    }

    (StatusCode::OK, Json(json!({"message": "File deleted successfully"})))
}

#[derive(Deserialize)]
pub struct RenameFileRequest {
    pub new_filename: String,
}

pub async fn rename_file(
    State(state): State<Arc<AppState>>,
    AxumPath((dir, filename)): AxumPath<(String, String)>,
    Json(req): Json<RenameFileRequest>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    if !check_yaml_ext(&filename) || !check_yaml_ext(&req.new_filename) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "only yaml files are allowed"})));
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let old_path = base_dir.join(&filename);
    let new_path = base_dir.join(&req.new_filename);

    if !is_path_safe(&old_path, &base_dir) || !is_path_safe(&new_path, &base_dir) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid filename"})));
    }

    if !old_path.exists() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "source file does not exist"})));
    }

    if new_path.exists() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "destination file already exists"})));
    }

    if let Err(e) = fs::rename(&old_path, &new_path) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})));
    }

    let mut cfg = state.mihomo_service.config.write().await;
    if dir_name == "configs" && old_path == Path::new(&cfg.mihomo.config_path) {
        cfg.mihomo.config_path = new_path.to_string_lossy().to_string();
        if let Err(e) = save(&cfg, &state.mihomo_service.config_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("file renamed but failed to update app config: {}", e)})),
            );
        }
    }

    (StatusCode::OK, Json(json!({"message": "File renamed successfully"})))
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    AxumPath((dir, filename)): AxumPath<(String, String)>,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, HeaderMap::new(), Vec::new()),
    };

    if !check_yaml_ext(&filename) {
        return (StatusCode::BAD_REQUEST, HeaderMap::new(), Vec::new());
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let file_path = base_dir.join(&filename);

    if !is_path_safe(&file_path, &base_dir) || !file_path.exists() {
        return (StatusCode::NOT_FOUND, HeaderMap::new(), Vec::new());
    }

    let data = match fs::read(&file_path) {
        Ok(d) => d,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), Vec::new()),
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));

    (StatusCode::OK, headers, data)
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    AxumPath(dir): AxumPath<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let dir_name = match normalize_dir(&dir) {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid directory"}))),
    };

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join(dir_name);
    let _ = fs::create_dir_all(&base_dir);

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = match field.file_name() {
            Some(f) => f.to_string(),
            None => continue,
        };

        if !check_yaml_ext(&filename) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "only yaml files are allowed"})),
            );
        }

        let file_path = base_dir.join(&filename);
        if !is_path_safe(&file_path, &base_dir) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid filename"})),
            );
        }

        if file_path.exists() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "file already exists"})),
            );
        }

        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("failed to read uploaded file: {}", e)})),
                )
            }
        };

        if let Err(e) = fs::write(&file_path, &data) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to save file: {}", e)})),
            );
        }

        return (
            StatusCode::OK,
            Json(json!({
                "message": "File uploaded successfully",
                "filename": filename
            })),
        );
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "no file uploaded"})),
    )
}

pub async fn get_active_config_path(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config_path = { state.mihomo_service.config.read().await.mihomo.config_path.clone() };
    let rel_path = Path::new(&config_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    Json(json!({
        "success": true,
        "data": {
            "active_config": rel_path
        }
    }))
}

#[derive(Deserialize)]
pub struct SetActiveConfigRequest {
    pub filename: String,
}

pub async fn set_active_config_path(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetActiveConfigRequest>,
) -> impl IntoResponse {
    if !check_yaml_ext(&req.filename) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "only yaml files are allowed"})),
        );
    }

    let working_dir = { state.mihomo_service.config.read().await.mihomo.working_dir.clone() };
    let base_dir = Path::new(&working_dir).join("configs");
    let new_path = base_dir.join(&req.filename);

    if !is_path_safe(&new_path, &base_dir) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid filename"})),
        );
    }

    if !new_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "file does not exist"})),
        );
    }

    let new_path_str = new_path.to_string_lossy().to_string();
    {
        let mut cfg = state.mihomo_service.config.write().await;
        cfg.mihomo.config_path = new_path_str;
        if let Err(e) = save(&cfg, &state.mihomo_service.config_path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to update app config: {}", e)})),
            );
        }
    }

    let cfg = state.mihomo_service.config.read().await.clone();
    let status = state.mihomo_service.get_status().await;

    if cfg.mihomo.auto_restart && status == "running" {
        if let Err(e) = state.mihomo_service.restart().await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("config updated but failed to restart mihomo: {}", e)})),
            );
        }
        return (
            StatusCode::OK,
            Json(json!({"message": "Active config updated and mihomo restarted successfully"})),
        );
    }

    (
        StatusCode::OK,
        Json(json!({"message": "Active config updated successfully"})),
    )
}
