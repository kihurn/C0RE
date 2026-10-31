//! The gateway: checks every request (spec §3.2), forwards it to Ollama, streams the
//! answer back and logs the request's metadata.

use crate::keys::Key;
use crate::limits::{FailureBlocker, check_daily};
use crate::policy::{self, Denied, Entrance, Route};
use crate::usage::UsageRecord;
use crate::{AppState, local_utc_offset, now_secs};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::OwnedSemaphorePermit;

#[derive(Clone)]
struct Ctx {
    state: Arc<AppState>,
    entrance: Entrance,
}

/// All key-protected API routes for one entrance. Unknown paths (including model
/// management and the admin area) get 404.
pub fn api_router(state: Arc<AppState>, entrance: Entrance) -> Router {
    Router::new().fallback(handle).with_state(Ctx { state, entrance })
}

/// Collects what we know about a request so every outcome is logged the same way.
struct Log {
    state: Arc<AppState>,
    entrance: Entrance,
    path: String,
    start: Instant,
    ts: i64,
    key: Option<(String, String)>,
    model: Option<String>,
}

impl Log {
    fn record(&self, status: u16, reason: Option<&str>, ttfb_ms: Option<u32>, tokens: (Option<u32>, Option<u32>)) {
        let rec = UsageRecord {
            ts: self.ts,
            key_id: self.key.as_ref().map(|k| k.0.clone()),
            project: self.key.as_ref().map(|k| k.1.clone()),
            entrance: self.entrance,
            path: self.path.clone(),
            model: self.model.clone(),
            status,
            block_reason: reason.map(str::to_string),
            latency_ms: self.start.elapsed().as_millis().min(u32::MAX as u128) as u32,
            ttfb_ms,
            prompt_tokens: tokens.0,
            output_tokens: tokens.1,
        };
        if let Err(e) = self.state.usage.insert(&rec) {
            tracing::warn!("could not write usage log: {e:#}");
        }
    }

    fn deny(&self, d: Denied) -> Response {
        self.record(d.status, Some(d.code), None, (None, None));
        denied_response(&d)
    }
}

pub fn denied_response(d: &Denied) -> Response {
    let status = StatusCode::from_u16(d.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut resp = (status, axum::Json(d.body())).into_response();
    if let Some(secs) = d.retry_after {
        resp.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs));
    }
    resp
}

fn internal(e: impl std::fmt::Display) -> Denied {
    tracing::error!("internal error while checking a request: {e}");
    Denied::new(500, "internal", "C0RE hit an internal error; the request was not forwarded.")
}

async fn handle(State(ctx): State<Ctx>, ConnectInfo(peer): ConnectInfo<SocketAddr>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();

    let Some(route) = policy::route_for(&method, &path) else {
        return denied_response(&Denied::new(404, "not_found", "No such endpoint."));
    };
    if route == Route::Health {
        return axum::Json(json!({ "ok": true })).into_response();
    }

    let state = ctx.state.clone();
    let mut log = Log {
        state: state.clone(),
        entrance: ctx.entrance,
        path: path.clone(),
        start: Instant::now(),
        ts: now_secs(),
        key: None,
        model: None,
    };

    // Who to blame for bad keys, and whether they're already blocked.
    let source = FailureBlocker::source_for(ctx.entrance, peer.ip(), req.headers());
    if state.failures.is_blocked(&source, Instant::now()) {
        return log.deny(
            Denied::new(429, "too_many_failures", "Too many failed key attempts. Try again in a minute.")
                .with_retry_after(60),
        );
    }

    // Key present and valid.
    let Some(secret) = policy::bearer_secret(req.headers()).map(str::to_string) else {
        state.failures.record_failure(&source, Instant::now());
        return log.deny(Denied::new(401, "missing_key", "Send your C0RE key as 'Authorization: Bearer <key>'."));
    };
    let key: Key = match state.keys.verify(&secret) {
        Ok(Some(k)) => k,
        Ok(None) => {
            state.failures.record_failure(&source, Instant::now());
            return log.deny(Denied::new(401, "invalid_key", "This key is not valid."));
        }
        Err(e) => return log.deny(internal(e)),
    };
    log.key = Some((key.id.clone(), key.project.clone()));

    // Reach for this entrance.
    if let Err(d) = policy::check_reach(&key, ctx.entrance) {
        return log.deny(d);
    }

    // Body size, JSON, model, rewrite.
    let limit = key.max_body_mb as usize * 1024 * 1024;
    let raw = match axum::body::to_bytes(req.into_body(), limit).await {
        Ok(b) => b,
        Err(_) => {
            return log.deny(Denied::new(
                413,
                "body_too_large",
                format!("Request is larger than this key's {} MB limit.", key.max_body_mb),
            ));
        }
    };
    let mut body: Option<Value> = None;
    if method == Method::POST {
        let mut v: Value = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => return log.deny(Denied::new(400, "bad_request", "The request body must be valid JSON.")),
        };
        if let Err(d) = policy::check_keys(&v) {
            return log.deny(d);
        }
        if route.has_model() {
            let model = match policy::model_of(&v) {
                Ok(m) => m,
                Err(d) => return log.deny(d),
            };
            log.model = Some(model.clone());
            if let Err(d) = policy::check_model(&key, &model) {
                return log.deny(d);
            }
        }
        policy::rewrite_body(&mut v, &key);
        body = Some(v);
    }

    // Limits.
    if let Err(d) = state.limiter.check_rate(&key, Instant::now()) {
        return log.deny(d);
    }
    match state.usage.count_today(&key.id, now_secs(), local_utc_offset()) {
        Ok(used) => {
            if let Err(d) = check_daily(used, &key) {
                return log.deny(d);
            }
        }
        Err(e) => return log.deny(internal(e)),
    }
    let permit = match state.limiter.try_enter(&key) {
        Ok(p) => p,
        Err(d) => return log.deny(d),
    };

    forward(state, log, key, route, method, format!("{path}{query}"), body, permit).await
}

#[allow(clippy::too_many_arguments)]
async fn forward(
    state: Arc<AppState>,
    log: Log,
    key: Key,
    route: Route,
    method: Method,
    path_and_query: String,
    body: Option<Value>,
    permit: OwnedSemaphorePermit,
) -> Response {
    let base = state.config().ollama_url.trim_end_matches('/').to_string();
    let mut rb = state.http.request(method, format!("{base}{path_and_query}"));
    if let Some(b) = &body {
        rb = rb.header(header::CONTENT_TYPE, "application/json").body(serde_json::to_vec(b).unwrap_or_default());
    }
    let upstream = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Ollama unreachable: {e}");
            return log.deny(Denied::new(503, "engine_offline", "The AI engine is not running on the PC."));
        }
    };
    let status = upstream.status();

    if status == reqwest::StatusCode::NOT_FOUND && route.has_model() {
        let text = upstream.text().await.unwrap_or_default();
        if text.to_lowercase().contains("not found") {
            let model = log.model.clone().unwrap_or_default();
            return log.deny(Denied::new(404, "model_missing", format!("The model '{model}' is not installed on the PC.")));
        }
        log.record(404, None, None, (None, None));
        return (StatusCode::NOT_FOUND, text).into_response();
    }

    // Model lists: only show what this key may use.
    if matches!(route, Route::Tags | Route::Ps | Route::OpenAiModels) && status.is_success() {
        let mut v: Value = match upstream.json().await {
            Ok(v) => v,
            Err(_) => return log.deny(Denied::new(502, "engine_offline", "The AI engine sent an unreadable reply.")),
        };
        let field = if route == Route::OpenAiModels { ("data", "id") } else { ("models", "name") };
        if let Some(list) = v.get_mut(field.0).and_then(Value::as_array_mut) {
            list.retain(|m| m.get(field.1).and_then(Value::as_str).is_some_and(|n| policy::check_model(&key, n).is_ok()));
        }
        log.record(status.as_u16(), None, None, (None, None));
        let _ = state.keys.touch(&key.id, now_secs());
        drop(permit);
        return axum::Json(v).into_response();
    }

    // Everything else streams straight through.
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let guard = StreamGuard {
        log,
        key_id: key.id.clone(),
        status: status.as_u16(),
        first_byte: None,
        tokens: TokenTail::default(),
        outcome: Outcome::ClientGone,
        _permit: permit,
    };
    let stream = futures_util::stream::unfold((upstream.bytes_stream(), guard), |(mut up, mut g)| async move {
        match up.next().await {
            Some(Ok(chunk)) => {
                g.first_byte.get_or_insert_with(|| g.log.start.elapsed().as_millis() as u32);
                g.tokens.push(&chunk);
                Some((Ok::<Bytes, std::io::Error>(chunk), (up, g)))
            }
            Some(Err(e)) => {
                g.outcome = Outcome::UpstreamDropped;
                Some((Err(std::io::Error::other(e)), (up, g)))
            }
            None => {
                if g.outcome == Outcome::ClientGone {
                    g.outcome = Outcome::Complete;
                }
                drop(g); // writes the log entry and frees the key's slot
                None
            }
        }
    });
    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    if let Some(ct) = content_type {
        resp.headers_mut().insert(header::CONTENT_TYPE, ct);
    }
    resp
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Complete,
    UpstreamDropped,
    /// The stream was dropped before it finished: the client disconnected.
    ClientGone,
}

/// Owns everything tied to one streamed answer. Dropping it — at the end of the stream
/// or when the client disconnects — frees the key's slot, cancels the upstream request
/// and writes the usage log.
struct StreamGuard {
    log: Log,
    key_id: String,
    status: u16,
    first_byte: Option<u32>,
    tokens: TokenTail,
    outcome: Outcome,
    _permit: OwnedSemaphorePermit,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        let reason = match self.outcome {
            Outcome::UpstreamDropped => Some("upstream_dropped"),
            Outcome::Complete | Outcome::ClientGone => None,
        };
        self.log.record(self.status, reason, self.first_byte, self.tokens.counts());
        if (200..300).contains(&self.status) {
            let _ = self.log.state.keys.touch(&self.key_id, now_secs());
        }
    }
}

/// Keeps only the last JSON line of a response, where Ollama reports token counts.
#[derive(Default)]
struct TokenTail {
    last_line: Vec<u8>,
    partial: Vec<u8>,
    overflow: bool,
}

const MAX_TAIL: usize = 1024 * 1024;

impl TokenTail {
    fn push(&mut self, chunk: &[u8]) {
        for piece in chunk.split_inclusive(|b| *b == b'\n') {
            if self.partial.len() + piece.len() > MAX_TAIL {
                self.overflow = true;
                self.partial.clear();
                continue;
            }
            self.partial.extend_from_slice(piece);
            if piece.ends_with(b"\n") {
                if self.partial.iter().any(|b| !b.is_ascii_whitespace()) && !self.overflow {
                    self.last_line = std::mem::take(&mut self.partial);
                } else {
                    self.partial.clear();
                }
                self.overflow = false;
            }
        }
    }

    fn counts(&self) -> (Option<u32>, Option<u32>) {
        let line = if self.partial.iter().any(|b| !b.is_ascii_whitespace()) { &self.partial } else { &self.last_line };
        let Ok(v) = serde_json::from_slice::<Value>(line) else { return (None, None) };
        let get = |k: &str| v.get(k).and_then(Value::as_u64).map(|n| n.min(u32::MAX as u64) as u32);
        (get("prompt_eval_count"), get("eval_count"))
    }
}
