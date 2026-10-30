mod common;

use c0re::keys::{KeyPatch, Reach};
use common::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const M: &str = "qwen3-vl:8b-instruct";

async fn post(url: &str, path: &str, secret: Option<&str>, body: &Value) -> reqwest::Response {
    let mut r = client().post(format!("{url}{path}")).json(body);
    if let Some(s) = secret {
        r = r.bearer_auth(s);
    }
    r.send().await.unwrap()
}

async fn err_code(r: reqwest::Response) -> (u16, String) {
    let status = r.status().as_u16();
    let v: Value = r.json().await.unwrap();
    (status, v["error"]["code"].as_str().unwrap_or("").to_string())
}

fn usage_rows(g: &Gateway) -> Vec<c0re::usage::UsageRecord> {
    g.state.usage.recent(100).unwrap()
}

#[tokio::test]
async fn valid_key_streams_answer() {
    let g = start_gateway(MockOpts { chunks: 3, ..Default::default() }).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(r.status(), 200);
    let text = r.text().await.unwrap();
    assert_eq!(text.lines().count(), 4);
    assert!(text.contains("w2 "));
}

#[tokio::test]
async fn missing_key_401() {
    let g = start_gateway(MockOpts::default()).await;
    let r = post(&g.private, "/api/chat", None, &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (401, "missing_key".into()));
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn wrong_key_401() {
    let g = start_gateway(MockOpts::default()).await;
    let r = post(&g.private, "/api/chat", Some("c0re_notarealkeynotarealkeynotarealkeynotareal"), &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (401, "invalid_key".into()));
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn off_key_403() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Off, &[]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (403, "key_disabled".into()));
}

#[tokio::test]
async fn private_key_on_public_403_and_not_forwarded() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = post(&g.public, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (403, "key_not_public".into()));
    assert_eq!(g.mock.request_count(), 0);
    let row = &usage_rows(&g)[0];
    assert_eq!(row.block_reason.as_deref(), Some("key_not_public"));
}

#[tokio::test]
async fn public_key_works_on_public() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Public, &[]);
    let r = post(&g.public, "/api/chat", Some(&s), &chat_body(M, false)).await;
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn x_api_key_header_accepted() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = client().post(format!("{}/api/chat", g.private)).header("x-api-key", s).json(&chat_body(M, false)).send().await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn model_not_allowed_403() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &["qwen3-vl:4b-instruct"]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (403, "model_not_allowed".into()));
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn oversize_body_413() {
    let g = start_gateway(MockOpts::default()).await;
    let (k, s) = g.key(Reach::Private, &[]);
    g.state.keys.update(&k.id, KeyPatch { max_body_mb: Some(1), ..Default::default() }).unwrap();
    let big = json!({"model": M, "messages":[{"role":"user","content":"x".repeat(2 * 1024 * 1024)}]});
    let r = post(&g.private, "/api/chat", Some(&s), &big).await;
    assert_eq!(err_code(r).await, (413, "body_too_large".into()));
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn malformed_body_rejected_not_forwarded() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = client().post(format!("{}/api/chat", g.private)).bearer_auth(&s).body("{not json").send().await.unwrap();
    assert_eq!(err_code(r).await, (400, "bad_request".into()));
    let r = post(&g.private, "/api/chat", Some(&s), &json!({"messages":[]})).await;
    assert_eq!(err_code(r).await, (400, "bad_request".into()));
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn num_ctx_clamped_before_upstream() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let mut b = chat_body(M, false);
    b["options"] = json!({"num_ctx": 262144});
    b["keep_alive"] = json!(0);
    post(&g.private, "/api/chat", Some(&s), &b).await;
    let seen = g.mock.last_body().unwrap();
    assert_eq!(seen["options"]["num_ctx"], 8192);
    assert!(seen.get("keep_alive").is_none());
}

#[tokio::test]
async fn management_paths_404_with_valid_key() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Public, &[]);
    for path in ["/api/pull", "/api/delete", "/api/create", "/api/copy", "/api/push"] {
        for url in [&g.private, &g.public] {
            let r = post(url, path, Some(&s), &json!({"model": M})).await;
            assert_eq!(err_code(r).await, (404, "not_found".into()), "{path}");
        }
    }
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn admin_paths_404_on_public() {
    let g = start_gateway(MockOpts::default()).await;
    for path in ["/admin", "/admin/", "/admin/api/status", "/admin/index.html"] {
        let r = client().get(format!("{}{path}", g.public)).send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
    }
}

#[tokio::test]
async fn tags_filtered_to_key_models() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &["llama3"]);
    let v: Value = client().get(format!("{}/api/tags", g.private)).bearer_auth(&s).send().await.unwrap().json().await.unwrap();
    let names: Vec<&str> = v["models"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["llama3:latest"]);
    let v: Value = client().get(format!("{}/v1/models", g.private)).bearer_auth(&s).send().await.unwrap().json().await.unwrap();
    let ids: Vec<&str> = v["data"].as_array().unwrap().iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["llama3:latest"]);
}

#[tokio::test]
async fn rate_limit_429_with_retry_after() {
    let g = start_gateway(MockOpts::default()).await;
    let (k, s) = g.key(Reach::Private, &[]);
    g.state.keys.update(&k.id, KeyPatch { rate_per_min: Some(2), ..Default::default() }).unwrap();
    for _ in 0..2 {
        assert_eq!(post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await.status(), 200);
    }
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await;
    assert!(r.headers().get("retry-after").is_some());
    assert_eq!(err_code(r).await, (429, "rate_limited".into()));
}

#[tokio::test]
async fn daily_cap_429() {
    let g = start_gateway(MockOpts::default()).await;
    let (k, s) = g.key(Reach::Private, &[]);
    g.state.keys.update(&k.id, KeyPatch { daily_cap: Some(1), ..Default::default() }).unwrap();
    assert_eq!(post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await.status(), 200);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await;
    assert_eq!(err_code(r).await, (429, "daily_cap".into()));
}

#[tokio::test]
async fn disconnect_frees_concurrency_slot() {
    let g = start_gateway(MockOpts { chunks: 50, delay_ms: 50, ..Default::default() }).await;
    let (k, s) = g.key(Reach::Private, &[]);
    g.state.keys.update(&k.id, KeyPatch { max_concurrent: Some(1), ..Default::default() }).unwrap();
    let mut r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(r.status(), 200);
    r.chunk().await.unwrap(); // stream started
    let second = post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await;
    assert_eq!(err_code(second).await, (429, "too_many_concurrent".into()));
    drop(r); // client goes away mid-stream
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, false)).await;
        if r.status() == 200 {
            break;
        }
        assert!(Instant::now() < deadline, "slot never freed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(g.mock.state.cancelled.load(std::sync::atomic::Ordering::SeqCst), "upstream not cancelled");
}

#[tokio::test]
async fn engine_offline_503() {
    let g = start_gateway(MockOpts::default()).await;
    g.state.cfg.write().unwrap().ollama_url = "http://127.0.0.1:1".into();
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    assert_eq!(err_code(r).await, (503, "engine_offline".into()));
}

#[tokio::test]
async fn missing_model_404() {
    let g = start_gateway(MockOpts { missing_model: true, ..Default::default() }).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body("nope:1b", true)).await;
    assert_eq!(err_code(r).await, (404, "model_missing".into()));
}

#[tokio::test]
async fn upstream_drop_logged() {
    let g = start_gateway(MockOpts { chunks: 5, drop_after: Some(2), ..Default::default() }).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let r = post(&g.private, "/api/chat", Some(&s), &chat_body(M, true)).await;
    let _ = r.text().await; // body ends early / errors
    tokio::time::sleep(Duration::from_millis(200)).await;
    let row = &usage_rows(&g)[0];
    assert_eq!(row.block_reason.as_deref(), Some("upstream_dropped"));
}

#[tokio::test]
async fn usage_logged_without_content() {
    let g = start_gateway(MockOpts { chunks: 3, ..Default::default() }).await;
    let (k, s) = g.key(Reach::Private, &[]);
    let mut b = chat_body(M, true);
    b["messages"][0]["content"] = json!("SECRET-PROMPT-TEXT");
    let r = post(&g.private, "/api/chat", Some(&s), &b).await;
    let _ = r.text().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let row = &usage_rows(&g)[0];
    assert_eq!(row.key_id.as_deref(), Some(k.id.as_str()));
    assert_eq!(row.model.as_deref(), Some(M));
    assert_eq!((row.status, row.prompt_tokens, row.output_tokens), (200, Some(11), Some(3)));
    assert!(row.ttfb_ms.is_some());
    let db = std::fs::read(g._dir.path().join("c0re.db")).unwrap();
    let wal = std::fs::read(g._dir.path().join("c0re.db-wal")).unwrap_or_default();
    let needle = b"SECRET-PROMPT-TEXT";
    assert!(!db.windows(needle.len()).any(|w| w == needle) && !wal.windows(needle.len()).any(|w| w == needle));
    assert!(g.state.keys.get(&k.id).unwrap().unwrap().last_used_at.is_some());
}

#[tokio::test]
async fn public_failures_blocked_per_forwarded_client() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Public, &[]);
    let bad = "c0re_notarealkeynotarealkeynotarealkeynotareal";
    for _ in 0..10 {
        client().post(format!("{}/api/chat", g.public)).bearer_auth(bad).header("x-forwarded-for", "203.0.113.5")
            .json(&chat_body(M, false)).send().await.unwrap();
    }
    let r = client().post(format!("{}/api/chat", g.public)).bearer_auth(&s).header("x-forwarded-for", "203.0.113.5")
        .json(&chat_body(M, false)).send().await.unwrap();
    assert_eq!(err_code(r).await, (429, "too_many_failures".into()));
    let r = client().post(format!("{}/api/chat", g.public)).bearer_auth(&s).header("x-forwarded-for", "198.51.100.7")
        .json(&chat_body(M, false)).send().await.unwrap();
    assert_eq!(r.status(), 200, "other public client must not be blocked");
}

#[tokio::test]
async fn health_needs_no_key_and_leaks_nothing() {
    let g = start_gateway(MockOpts::default()).await;
    for url in [&g.private, &g.public] {
        let v: Value = client().get(format!("{url}/health")).send().await.unwrap().json().await.unwrap();
        assert_eq!(v, json!({"ok": true}));
    }
}

#[tokio::test]
async fn added_latency_under_50ms() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Private, &[]);
    let c = client();
    let mut direct = vec![];
    let mut via = vec![];
    for _ in 0..20 {
        let t = Instant::now();
        c.post(format!("{}/api/chat", g.mock.url)).json(&chat_body(M, false)).send().await.unwrap().bytes().await.unwrap();
        direct.push(t.elapsed());
        let t = Instant::now();
        c.post(format!("{}/api/chat", g.private)).bearer_auth(&s).json(&chat_body(M, false)).send().await.unwrap().bytes().await.unwrap();
        via.push(t.elapsed());
    }
    direct.sort();
    via.sort();
    let added = via[10].saturating_sub(direct[10]);
    assert!(added < Duration::from_millis(50), "added {added:?}");
}

#[tokio::test]
async fn case_variant_keys_rejected_not_forwarded() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Public, &[M]);
    let bodies = [
        json!({"model": M, "Model": "llama3:latest", "messages": []}),
        json!({"model": M, "Options": {"num_ctx": 262144}, "messages": []}),
        json!({"model": M, "KEEP_ALIVE": 0, "messages": []}),
        json!({"model": M, "ﬁeld": 1, "messages": []}),
    ];
    for b in bodies {
        let r = post(&g.public, "/api/chat", Some(&s), &b).await;
        assert_eq!(err_code(r).await, (400, "bad_request".into()), "{b}");
    }
    assert_eq!(g.mock.request_count(), 0);
}

#[tokio::test]
async fn load_time_options_stripped_before_upstream() {
    let g = start_gateway(MockOpts::default()).await;
    let (_k, s) = g.key(Reach::Public, &[]);
    let mut b = chat_body(M, false);
    b["options"] = json!({"num_gpu": 0, "main_gpu": 1, "num_batch": 1, "use_mmap": false, "use_mlock": true,
        "num_thread": 1, "low_vram": true, "temperature": 0.3, "top_p": 0.9, "seed": 4, "stop": ["x"], "num_predict": 64});
    assert_eq!(post(&g.public, "/api/chat", Some(&s), &b).await.status(), 200);
    let seen = g.mock.last_body().unwrap();
    assert_eq!(
        seen["options"],
        json!({"temperature": 0.3, "top_p": 0.9, "seed": 4, "stop": ["x"], "num_predict": 64})
    );
}
