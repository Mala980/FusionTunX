use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::http::{Response, StatusCode, Uri};
use axum::response::IntoResponse;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "dash/dist/"]
pub struct DashboardAssets;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() {
        path = "index.html".to_string();
    }

    if let Some(content) = DashboardAssets::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())
            .body(Body::from(content.data))
            .unwrap();
    }

    // SPA fallback: if not an /api/ or /docs/ path, return index.html
    if !uri.path().starts_with("/api/") && !uri.path().starts_with("/docs") {
        if let Some(content) = DashboardAssets::get("index.html") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
                .body(Body::from(content.data))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(r#"{"error":"Not found"}"#))
        .unwrap()
}

const SWAGGER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>FusionTunX API Docs</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" crossorigin></script>
<script>
  window.onload = () => {
    window.ui = SwaggerUIBundle({
      url: '/docs/swagger.json',
      dom_id: '#swagger-ui',
    });
  };
</script>
</body>
</html>"#;

pub async fn swagger_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    if path == "/docs/swagger.json" || path == "/docs/swagger.json/" {
        if let Ok(content) = std::fs::read_to_string("docs/swagger.json") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(Body::from(content))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
        .body(Body::from(SWAGGER_HTML))
        .unwrap()
}
