#![allow(dead_code)]
//! Test harness: a mock Ollama and a C0RE gateway on ephemeral ports.

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use bytes::Bytes;
use c0re::keys::{Key, NewKey, Reach};
use c0re::policy::Entrance;
use c0re::tailscale::{CmdOutput, Runner};
use c0re::{AppState, proxy};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct MockOpts {
    pub chunks: usize,
    pub delay_ms: u64,
    /// Close the connection after this many chunks.
    pub drop_after: Option<usize>,
    pub missing_model: bool,
}

#[derive(Default)]
pub struct MockState {
    pub opts: Mutex<MockOpts>,
    pub bodies: Mutex<Vec<Value>>,
    pub requests: AtomicUsize,
    pub cancelled: AtomicBool,
}

pub struct Mock {
    pub url: String,
    pub state: Arc<MockState>,
}

impl Mock {
    pub fn last_body(&self) -> Option<Value> {
        self.state.bodies.lock().unwrap().last().cloned()
    }
    pub fn request_count(&self) -> usize {
        self.state.requests.load(Ordering::SeqCst)
    }
}

/// Sets `cancelled` if the stream is dropped before it finished.
struct CancelFlag(Arc<MockState>, bool);
impl Drop for CancelFlag {
    fn drop(&mut self) {
        if !self.1 {
            self.0.cancelled.store(true, Ordering::SeqCst);
        }
    }
}

async fn chat(State(s): State<Arc<MockState>>, body: Bytes) -> Response {
    s.requests.fetch_add(1, Ordering::SeqCst);
    let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    s.bodies.lock().unwrap().push(v.clone());
    let opts = s.opts.lock().unwrap().clone();
    let model = v["model"].as_str().unwrap_or("").to_string();
    if opts.missing_model {
        return (StatusCode::NOT_FOUND, axum::Json(json!({"error": format!("model '{model}' not found")}))).into_response();
    }
    let streaming = v.get("stream").and_then(Value::as_bool).unwrap_or(true);
    let n = opts.chunks.max(1);
    if !streaming {
        let text: String = (0..n).map(|i| format!("w{i} ")).collect();
        return axum::Json(json!({"model": model, "message": {"role":"assistant","content": text}, "done": true,
            "done_reason":"stop", "prompt_eval_count": 11, "eval_count": n})).into_response();
    }
    let st = s.clone();
    let stream = async_stream(move |tx| async move {
        let mut flag = CancelFlag(st.clone(), false);
        for i in 0..n {
            if opts.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(opts.delay_ms)).await;
            }
            if opts.drop_after == Some(i) {
                flag.1 = true;
                let _ = tx.send(Err(std::io::Error::other("drop"))).await;
                return;
            }
            let line = json!({"model": model, "message": {"role":"assistant","content": format!("w{i} ")}, "done": false});
            if tx.send(Ok(Bytes::from(format!("{line}\n")))).await.is_err() {
                return; // client gone: flag drop marks cancelled
            }
        }
        let last = json!({"model": model, "message": {"role":"assistant","content": ""}, "done": true,
            "done_reason":"stop", "prompt_eval_count": 11, "eval_count": n});
        let _ = tx.send(Ok(Bytes::from(format!("{last}\n")))).await;
        flag.1 = true;
    });
    Response::builder().header("content-type", "application/x-ndjson").body(Body::from_stream(stream)).unwrap()
}

/// Minimal channel-backed stream so the mock can sleep between chunks.
fn async_stream<F, Fut>(f: F) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(f(tx));
    futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
}

pub async fn mock_ollama(opts: MockOpts) -> Mock {
    let state = Arc::new(MockState { opts: Mutex::new(opts), ..Default::default() });
    let app = Router::new()
        .route("/api/chat", post(chat))
        .route("/api/generate", post(chat))
        .route("/v1/chat/completions", post(chat))
        .route("/api/version", get(|| async { axum::Json(json!({"version":"0.40.13"})) }))
        .route(
            "/api/tags",
            get(|| async {
                axum::Json(json!({"models":[
                    {"name":"qwen3-vl:8b-instruct","model":"qwen3-vl:8b-instruct","size":6100000000u64},
                    {"name":"qwen3-vl:4b-instruct","model":"qwen3-vl:4b-instruct","size":3300000000u64},
                    {"name":"llama3:latest","model":"llama3:latest","size":4700000000u64}]}))
            }),
        )
        .route(
            "/v1/models",
            get(|| async {
                axum::Json(json!({"object":"list","data":[
                    {"id":"qwen3-vl:8b-instruct","object":"model"},{"id":"llama3:latest","object":"model"}]}))
            }),
        )
        .route(
            "/api/ps",
            get(|| async {
                axum::Json(json!({"models":[{"name":"qwen3-vl:8b-instruct","model":"qwen3-vl:8b-instruct","size":6100000000u64,"size_vram":6100000000u64}]}))
            }),
        )
        .route("/api/show", post(|| async { axum::Json(json!({"capabilities":["completion","vision"]})) }))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Mock { url: format!("http://{addr}"), state }
}

pub struct FakeRunner {
    pub calls: Mutex<Vec<Vec<String>>>,
    pub reply: Mutex<Box<dyn Fn(&[&str]) -> std::io::Result<CmdOutput> + Send>>,
}

impl FakeRunner {
    pub fn new(reply: impl Fn(&[&str]) -> std::io::Result<CmdOutput> + Send + 'static) -> Arc<FakeRunner> {
        Arc::new(FakeRunner { calls: Mutex::new(vec![]), reply: Mutex::new(Box::new(reply)) })
    }
    pub fn ok(stdout: &str) -> std::io::Result<CmdOutput> {
        Ok(CmdOutput { status: 0, stdout: stdout.into(), stderr: String::new() })
    }
}

impl Runner for FakeRunner {
    fn run(&self, args: &[&str]) -> std::io::Result<CmdOutput> {
        self.calls.lock().unwrap().push(args.iter().map(|s| s.to_string()).collect());
        (self.reply.lock().unwrap())(args)
    }
}

pub struct Gateway {
    pub private: String,
    pub public: String,
    pub state: Arc<AppState>,
    pub mock: Mock,
    pub _dir: tempfile::TempDir,
}

impl Gateway {
    pub fn key(&self, reach: Reach, models: &[&str]) -> (Key, String) {
        self.state
            .keys
            .create(NewKey {
                project: "test".into(),
                reach,
                models: models.iter().map(|s| s.to_string()).collect(),
                limits: None,
                note: String::new(),
            })
            .unwrap()
    }
}

async fn serve_router(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap()
    });
    format!("http://{addr}")
}

pub async fn start_gateway(opts: MockOpts) -> Gateway {
    let mock = mock_ollama(opts).await;
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path(), FakeRunner::new(|_| FakeRunner::ok(""))).unwrap();
    state.cfg.write().unwrap().ollama_url = mock.url.clone();
    let private = serve_router(proxy::api_router(state.clone(), Entrance::Private)).await;
    let public = serve_router(proxy::api_router(state.clone(), Entrance::Public)).await;
    Gateway { private, public, state, mock, _dir: dir }
}

pub fn client() -> reqwest::Client {
    reqwest::Client::new()
}

pub fn chat_body(model: &str, stream: bool) -> Value {
    json!({"model": model, "stream": stream, "messages":[{"role":"user","content":"hi"}]})
}

/// Stand-in for Windows DPAPI in tests: reversible, and never stores the plain bytes.
pub struct TestVault;

impl c0re::vault::Vault for TestVault {
    fn seal(&self, plain: &[u8]) -> anyhow::Result<Vec<u8>> {
        Ok(b"TV1".iter().copied().chain(plain.iter().map(|b| b ^ 0x5a)).collect())
    }
    fn open(&self, sealed: &[u8]) -> anyhow::Result<Vec<u8>> {
        let body = sealed.strip_prefix(b"TV1").ok_or_else(|| anyhow::anyhow!("not sealed by TestVault"))?;
        Ok(body.iter().map(|b| b ^ 0x5a).collect())
    }
}
