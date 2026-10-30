//! Admin area: dashboard files and the admin API. Mounted on the private entrance only.

pub mod api;
pub mod auth;

pub use auth::{AdminState, set_password};

use crate::AppState;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

pub const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; \
connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// `/admin` (dashboard) and `/admin/api/*`.
pub fn admin_router(state: Arc<AppState>) -> Router {
    Router::new().nest("/admin/api", api::router(state)).nest_service("/admin", axum::routing::get(static_file))
}

async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // Real files by name; anything else is a dashboard page, served by index.html.
    let (name, file) = match Assets::get(path) {
        Some(f) if !path.is_empty() => (path.to_string(), f),
        _ => match Assets::get("index.html") {
            Some(f) => ("index.html".to_string(), f),
            None => return (StatusCode::NOT_FOUND, "Dashboard not built").into_response(),
        },
    };
    let mime = mime_guess::from_path(&name).first_or_octet_stream();
    let mut resp = Response::new(Body::from(file.data.into_owned()));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    let cache = if name == "index.html" { "no-cache" } else { "public, max-age=31536000, immutable" };
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    resp
}
