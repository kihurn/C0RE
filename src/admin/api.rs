//! Admin JSON API under `/admin/api`, private entrance only.

use super::auth::{self, SessionCheck};
use crate::config::KeyDefaults;
use crate::keys::{KeyPatch, NewKey, Revealed};
use crate::limits::FailureBlocker;
use crate::policy::{Denied, Entrance};
use crate::proxy::denied_response;
use crate::tailscale::{FunnelState, funnel_off, funnel_on, funnel_status};
use crate::{AppState, local_utc_offset, now_secs};
use axum::extract::{ConnectInfo, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

type St = State<Arc<AppState>>;
const COOKIE: &str = "c0re_session";
const CSRF_HEADER: &str = "x-c0re-csrf";

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/overview", get(overview))
        .route("/keys", get(list_keys).post(create_key))
        .route("/keys/{id}", patch(update_key).delete(delete_key))
        .route("/keys/{id}/reveal", post(reveal_key))
        .route("/usage", get(usage))
        .route("/models", get(models))
        .route("/settings", get(get_settings).put(put_settings))
        .route("/public", get(get_public).post(set_public))
        .route("/password", post(change_password))
        .fallback(|| async { err(404, "not_found", "No such admin endpoint.") })
        .with_state(state)
}

fn err(status: u16, code: &'static str, msg: impl Into<String>) -> Response {
    denied_response(&Denied::new(status, code, msg))
}

fn internal(e: impl std::fmt::Display) -> Response {
    tracing::error!("admin error: {e}");
    err(500, "internal", "Something went wrong on the PC. See the C0RE log for details.")
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|c| {
        let (k, v) = c.trim().split_once('=')?;
        (k == COOKIE).then(|| v.to_string())
    })
}

/// Extractor: a logged-in admin. Non-GET requests also need the CSRF header.
pub struct Authed(String);

impl FromRequestParts<Arc<AppState>> for Authed {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let Some(id) = session_cookie(&parts.headers) else {
            return Err(err(401, "not_logged_in", "Please log in."));
        };
        let csrf = if parts.method == Method::GET {
            None
        } else {
            Some(parts.headers.get(CSRF_HEADER).and_then(|v| v.to_str().ok()).unwrap_or(""))
        };
        let generation = auth::password_generation(state).map_err(internal)?;
        match state.admin.check(&id, csrf, generation) {
            SessionCheck::Ok => Ok(Authed(id)),
            SessionCheck::None => Err(err(401, "not_logged_in", "Your session has ended. Please log in again.")),
            SessionCheck::BadCsrf => Err(err(403, "bad_csrf", "This request was refused. Reload the page and try again.")),
        }
    }
}

async fn status(State(s): St, headers: HeaderMap) -> Response {
    // Same-origin only (no CORS headers), so other sites cannot read the token.
    let generation = match auth::password_generation(&s) {
        Ok(g) => g,
        Err(e) => return internal(e),
    };
    let csrf = session_cookie(&headers).and_then(|id| s.admin.csrf_for(&id, generation));
    match auth::password_set(&s) {
        Ok(set) => Json(json!({ "password_set": set, "logged_in": csrf.is_some(), "csrf": csrf })).into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
}

async fn login(State(s): St, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(b): Json<LoginBody>) -> Response {
    let source = FailureBlocker::source_for(Entrance::Private, peer.ip(), &headers);
    if s.failures.is_blocked(&source, Instant::now()) {
        return denied_response(
            &Denied::new(429, "too_many_failures", "Too many wrong passwords. Try again in a minute.").with_retry_after(60),
        );
    }
    match auth::password_set(&s) {
        Ok(false) => {
            return err(
                409,
                "no_password",
                "No admin password yet. On the PC, run: c0re admin set-password",
            );
        }
        Err(e) => return internal(e),
        Ok(true) => {}
    }
    let state = s.clone();
    let ok = tokio::task::spawn_blocking(move || auth::verify_password(&state, &b.password)).await;
    match ok {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => {
            s.failures.record_failure(&source, Instant::now());
            return err(401, "wrong_password", "That password is not right.");
        }
        Ok(Err(e)) => return internal(e),
        Err(e) => return internal(e),
    }
    let generation = match auth::password_generation(&s) {
        Ok(g) => g,
        Err(e) => return internal(e),
    };
    let (id, csrf) = match s.admin.create_session(generation) {
        Ok(v) => v,
        Err(e) => return internal(e),
    };
    let cookie = format!("{COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/admin; Max-Age=43200");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "csrf": csrf }))).into_response()
}

async fn logout(State(s): St, Authed(id): Authed) -> Response {
    s.admin.end_session(&id);
    let cookie = format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/admin; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

async fn engine_get(s: &AppState, path: &str) -> Option<Value> {
    let base = s.config().ollama_url.trim_end_matches('/').to_string();
    let r = s.http.get(format!("{base}{path}")).timeout(Duration::from_secs(3)).send().await.ok()?;
    r.json().await.ok()
}

fn today_start() -> i64 {
    let off = local_utc_offset() as i64;
    (now_secs() + off).div_euclid(86_400) * 86_400 - off
}

async fn overview(State(s): St, _a: Authed) -> Response {
    let version = engine_get(&s, "/api/version").await;
    let ps = engine_get(&s, "/api/ps").await;
    let loaded: Vec<Value> = ps
        .and_then(|v| v.get("models").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let size = m.get("size").and_then(Value::as_u64).unwrap_or(0);
            let vram = m.get("size_vram").and_then(Value::as_u64).unwrap_or(0);
            json!({
                "name": m.get("name").cloned().unwrap_or(Value::Null),
                "size": size,
                "size_vram": vram,
                "gpu_percent": if size > 0 { (vram * 100 / size).min(100) } else { 0 },
                "context": m.get("context_length").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    let (today, last24, recent) = (
        s.usage.summary(today_start()),
        s.usage.summary(now_secs() - 24 * 3600),
        s.usage.recent(20),
    );
    match (today, last24, recent) {
        (Ok(today), Ok(last24), Ok(recent)) => Json(json!({
            "engine": {
                "online": version.is_some(),
                "version": version.and_then(|v| v.get("version").cloned()),
                "loaded": loaded,
            },
            "today": { "requests": today.total, "blocked": today.blocked, "projects": today.per_key.len() },
            "per_hour": last24.per_hour,
            "entrances": s.usage.by_entrance(today_start()).unwrap_or_default(),
            "recent": recent,
            "keys": s.keys.list().map(|k| k.len()).unwrap_or(0),
        }))
        .into_response(),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => internal(e),
    }
}

async fn list_keys(State(s): St, _a: Authed) -> Response {
    match s.keys.list() {
        Ok(k) => Json(k).into_response(),
        Err(e) => internal(e),
    }
}

async fn create_key(State(s): St, _a: Authed, Json(b): Json<NewKey>) -> Response {
    match s.keys.create(b) {
        Ok((key, secret)) => Json(json!({ "key": key, "secret": secret })).into_response(),
        Err(e) => err(400, "bad_request", e.to_string()),
    }
}

async fn update_key(State(s): St, _a: Authed, Path(id): Path<String>, Json(p): Json<KeyPatch>) -> Response {
    match s.keys.update(&id, p) {
        Ok(Some(k)) => Json(k).into_response(),
        Ok(None) => err(404, "not_found", "No such key."),
        Err(e) => err(400, "bad_request", e.to_string()),
    }
}

async fn reveal_key(State(s): St, _a: Authed, Path(id): Path<String>) -> Response {
    match s.keys.reveal(&id) {
        Ok(Revealed::Secret(secret)) => {
            ([(header::CACHE_CONTROL, "no-store")], Json(json!({ "secret": secret }))).into_response()
        }
        Ok(Revealed::NotSaved) => err(
            409,
            "not_saved",
            "This key can't be shown again (it was made before C0RE 0.2, or on another Windows account). Delete it and make a new one.",
        ),
        Ok(Revealed::NotFound) => err(404, "not_found", "No such key."),
        Err(e) => internal(e),
    }
}

async fn delete_key(State(s): St, _a: Authed, Path(id): Path<String>) -> Response {
    match s.keys.delete(&id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(404, "not_found", "No such key."),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct UsageQuery {
    days: Option<u32>,
}

async fn usage(State(s): St, _a: Authed, Query(q): Query<UsageQuery>) -> Response {
    let days = q.days.unwrap_or(7).clamp(1, 90) as i64;
    match s.usage.summary(now_secs() - days * 86_400) {
        Ok(sum) => Json(json!({ "days": days, "summary": sum })).into_response(),
        Err(e) => internal(e),
    }
}

async fn models(State(s): St, _a: Authed) -> Response {
    let Some(tags) = engine_get(&s, "/api/tags").await else {
        return err(503, "engine_offline", "The AI engine is not running on the PC.");
    };
    let loaded: Vec<String> = engine_get(&s, "/api/ps")
        .await
        .and_then(|v| v.get("models").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    let base = s.config().ollama_url.trim_end_matches('/').to_string();
    let mut out = vec![];
    for m in tags.get("models").and_then(Value::as_array).cloned().unwrap_or_default() {
        let name = m.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let caps: Vec<String> = match s
            .http
            .post(format!("{base}/api/show"))
            .json(&json!({ "model": name }))
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(r) => r
                .json::<Value>()
                .await
                .ok()
                .and_then(|v| v.get("capabilities").and_then(Value::as_array).cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect(),
            Err(_) => vec![],
        };
        out.push(json!({
            "name": name,
            "size": m.get("size").cloned().unwrap_or(Value::Null),
            "modified_at": m.get("modified_at").cloned().unwrap_or(Value::Null),
            "loaded": loaded.contains(&name),
            "vision": caps.iter().any(|c| c == "vision"),
            "thinking": caps.iter().any(|c| c == "thinking"),
            "capabilities": caps,
        }));
    }
    Json(out).into_response()
}

#[derive(serde::Serialize, Deserialize)]
struct SettingsBody {
    private_defaults: KeyDefaults,
    public_defaults: KeyDefaults,
    retention_days: u32,
    max_db_mb: u64,
}

async fn get_settings(State(s): St, _a: Authed) -> Response {
    let c = s.config();
    Json(SettingsBody {
        private_defaults: c.private_defaults,
        public_defaults: c.public_defaults,
        retention_days: c.retention_days,
        max_db_mb: c.max_db_mb,
    })
    .into_response()
}

async fn put_settings(State(s): St, _a: Authed, Json(b): Json<SettingsBody>) -> Response {
    if !(1..=365).contains(&b.retention_days) || !(10..=10_000).contains(&b.max_db_mb) {
        return err(400, "bad_request", "Keep history between 1 and 365 days, and the database between 10 and 10000 MB.");
    }
    let mut c = s.config();
    c.private_defaults = b.private_defaults;
    c.public_defaults = b.public_defaults;
    c.retention_days = b.retention_days;
    c.max_db_mb = b.max_db_mb;
    if let Err(e) = c.save(&s.dir) {
        return internal(e);
    }
    *s.cfg.write().unwrap_or_else(|e| e.into_inner()) = c;
    Json(b).into_response()
}

fn funnel_json(f: FunnelState) -> Value {
    match f {
        FunnelState::On { url } => json!({ "state": "on", "url": url }),
        FunnelState::Off => json!({ "state": "off" }),
        FunnelState::Unavailable(reason) => json!({ "state": "unavailable", "reason": reason }),
    }
}

async fn get_public(State(s): St, _a: Authed) -> Response {
    let c = s.config();
    let r = s.tailscale.clone();
    match tokio::task::spawn_blocking(move || funnel_status(r.as_ref(), c.public_port, c.funnel_port)).await {
        Ok(f) => Json(funnel_json(f)).into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct PublicBody {
    on: bool,
}

async fn set_public(State(s): St, _a: Authed, Json(b): Json<PublicBody>) -> Response {
    let c = s.config();
    let r = s.tailscale.clone();
    let res = tokio::task::spawn_blocking(move || {
        if b.on {
            funnel_on(r.as_ref(), c.public_port, c.funnel_port).map(|url| FunnelState::On { url })
        } else {
            funnel_off(r.as_ref(), c.funnel_port).map(|_| FunnelState::Off)
        }
    })
    .await;
    match res {
        Ok(Ok(f)) => Json(funnel_json(f)).into_response(),
        Ok(Err(e)) => err(409, "public_unavailable", e.to_string()),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct PasswordBody {
    current: String,
    new: String,
}

async fn change_password(State(s): St, _a: Authed, Json(b): Json<PasswordBody>) -> Response {
    let state = s.clone();
    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<()>> {
        if !auth::verify_password(&state, &b.current)? {
            return Ok(None);
        }
        auth::set_password(&state, &b.new)?;
        Ok(Some(()))
    })
    .await;
    match res {
        Ok(Ok(Some(()))) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(None)) => err(401, "wrong_password", "The current password is not right."),
        Ok(Err(e)) => err(400, "bad_request", e.to_string()),
        Err(e) => internal(e),
    }
}
