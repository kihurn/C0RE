//! Pure request checks: routes, keys, reach, models and option rewriting.

use crate::keys::{Key, Reach};
use axum::http::{HeaderMap, Method};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Entrance {
    Private,
    Public,
}

impl Entrance {
    pub fn as_str(self) -> &'static str {
        match self {
            Entrance::Private => "private",
            Entrance::Public => "public",
        }
    }
}

/// A rejected request: HTTP status, stable error code and a message for people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denied {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    pub retry_after: Option<u64>,
}

impl Denied {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Denied { status, code, message: message.into(), retry_after: None }
    }
    pub fn with_retry_after(mut self, secs: u64) -> Self {
        self.retry_after = Some(secs);
        self
    }
    pub fn body(&self) -> Value {
        json!({ "error": { "code": self.code, "message": self.message } })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Chat,
    Generate,
    Embed,
    Tags,
    Show,
    Ps,
    Version,
    OpenAiChat,
    OpenAiModels,
    Health,
}

impl Route {
    /// Routes whose JSON body names a model that must be checked.
    pub fn has_model(self) -> bool {
        matches!(self, Route::Chat | Route::Generate | Route::Embed | Route::Show | Route::OpenAiChat)
    }
}

/// The only paths a key can use. Anything else (model management, admin) has no route.
pub fn route_for(method: &Method, path: &str) -> Option<Route> {
    let r = match (method.as_str(), path) {
        ("POST", "/api/chat") => Route::Chat,
        ("POST", "/api/generate") => Route::Generate,
        ("POST", "/api/embed") => Route::Embed,
        ("GET", "/api/tags") => Route::Tags,
        ("POST", "/api/show") => Route::Show,
        ("GET", "/api/ps") => Route::Ps,
        ("GET", "/api/version") => Route::Version,
        ("POST", "/v1/chat/completions") => Route::OpenAiChat,
        ("GET", "/v1/models") => Route::OpenAiModels,
        ("GET", "/health") => Route::Health,
        _ => return None,
    };
    Some(r)
}

pub fn bearer_secret(headers: &HeaderMap) -> Option<&str> {
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let (scheme, rest) = v.split_once(' ')?;
        return scheme.eq_ignore_ascii_case("bearer").then(|| rest.trim()).filter(|s| !s.is_empty());
    }
    headers.get("x-api-key").and_then(|v| v.to_str().ok()).map(str::trim).filter(|s| !s.is_empty())
}

pub fn check_reach(key: &Key, e: Entrance) -> Result<(), Denied> {
    match (key.reach, e) {
        (Reach::Off, _) => Err(Denied::new(403, "key_disabled", "This key is switched off.")),
        (Reach::Private, Entrance::Public) => {
            Err(Denied::new(403, "key_not_public", "This key can only be used on the private network."))
        }
        _ => Ok(()),
    }
}

fn normalise(model: &str) -> String {
    if model.contains(':') { model.to_string() } else { format!("{model}:latest") }
}

pub fn check_model(key: &Key, model: &str) -> Result<(), Denied> {
    if key.models.is_empty() || key.models.iter().any(|m| normalise(m) == normalise(model)) {
        Ok(())
    } else {
        Err(Denied::new(403, "model_not_allowed", format!("This key may not use the model '{model}'.")))
    }
}

/// Ollama (Go) matches JSON fields case-insensitively, so `"Model"` or `"KEEP_ALIVE"` would
/// reach it while our checks only look at the lowercase names. Every real field of the
/// Ollama and OpenAI APIs is lowercase snake_case, so anything else is refused.
pub fn check_keys(body: &Value) -> Result<(), Denied> {
    let Some(obj) = body.as_object() else {
        return Err(Denied::new(400, "bad_request", "The request body must be a JSON object."));
    };
    for k in obj.keys() {
        if k.is_empty() || !k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return Err(Denied::new(400, "bad_request", format!("Unknown field '{k}': field names are lowercase.")));
        }
    }
    Ok(())
}

/// Options that only shape one answer. Load-time options (num_gpu, main_gpu, num_batch,
/// use_mmap, num_thread, …) would make Ollama reload the model for everyone, so they are
/// dropped; num_ctx is kept but capped.
const ANSWER_OPTIONS: &[&str] = &[
    "temperature", "top_k", "top_p", "min_p", "typical_p", "tfs_z", "repeat_penalty", "repeat_last_n",
    "presence_penalty", "frequency_penalty", "penalize_newline", "seed", "stop", "num_predict", "num_keep",
    "mirostat", "mirostat_tau", "mirostat_eta", "num_ctx",
];

pub fn model_of(body: &Value) -> Result<String, Denied> {
    match body.get("model").and_then(Value::as_str) {
        Some(m) if !m.trim().is_empty() => Ok(m.to_string()),
        _ => Err(Denied::new(400, "bad_request", "The request body must be JSON with a \"model\" field.")),
    }
}

/// Makes a request safe to forward: caps the context size and stops clients from
/// unloading models. Everything else passes through unchanged.
pub fn rewrite_body(body: &mut Value, key: &Key) {
    if let Some(obj) = body.as_object_mut() {
        obj.remove("keep_alive");
        if let Some(opts) = obj.get_mut("options").and_then(Value::as_object_mut) {
            opts.retain(|k, _| ANSWER_OPTIONS.contains(&k.as_str()));
            if let Some(v) = opts.get("num_ctx") {
                match v.as_u64() {
                    Some(n) if n > 0 => {
                        opts.insert("num_ctx".into(), json!(n.min(key.max_ctx as u64)));
                    }
                    _ => {
                        opts.remove("num_ctx");
                    }
                }
            }
        }
    }
}
