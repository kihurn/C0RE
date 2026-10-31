mod common;

use c0re::AppState;
use c0re::admin::set_password;
use common::*;
use reqwest::StatusCode;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

struct Admin {
    url: String,
    state: Arc<AppState>,
    runner: Arc<FakeRunner>,
    _mock: Mock,
    _dir: tempfile::TempDir,
}

const PASSWORD: &str = "correct horse battery";

async fn start(with_password: bool) -> Admin {
    let mock = mock_ollama(MockOpts::default()).await;
    let dir = tempfile::tempdir().unwrap();
    let runner = FakeRunner::new(|args| {
        if args.first() == Some(&"funnel") && args.get(1) == Some(&"--bg") {
            FakeRunner::ok("Available on the internet:\nhttps://kihurn-desktop.tail1234.ts.net:8443/\n")
        } else {
            FakeRunner::ok("{}")
        }
    });
    let state = AppState::open_with(dir.path(), runner.clone(), Arc::new(TestVault)).unwrap();
    state.cfg.write().unwrap().ollama_url = mock.url.clone();
    if with_password {
        set_password(&state, PASSWORD).unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = c0re::server::private_router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap()
    });
    Admin { url: format!("http://{addr}"), state, runner, _mock: mock, _dir: dir }
}

struct Session {
    cookie: String,
    csrf: String,
}

async fn login(a: &Admin, password: &str) -> reqwest::Response {
    client().post(format!("{}/admin/api/login", a.url)).json(&json!({ "password": password })).send().await.unwrap()
}

async fn session(a: &Admin) -> Session {
    let r = login(a, PASSWORD).await;
    assert_eq!(r.status(), 200);
    let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().split(';').next().unwrap().to_string();
    let v: Value = r.json().await.unwrap();
    Session { cookie, csrf: v["csrf"].as_str().unwrap().to_string() }
}

fn req(a: &Admin, s: &Session, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    client()
        .request(method, format!("{}/admin/api{path}", a.url))
        .header("cookie", &s.cookie)
        .header("x-c0re-csrf", &s.csrf)
}

#[tokio::test]
async fn no_password_means_login_refused_with_hint() {
    let a = start(false).await;
    let v: Value = client().get(format!("{}/admin/api/status", a.url)).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["password_set"], false);
    let r = login(&a, "anything at all").await;
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let v: Value = r.json().await.unwrap();
    assert!(v["error"]["message"].as_str().unwrap().contains("c0re admin set-password"));
}

#[tokio::test]
async fn short_password_rejected() {
    let a = start(false).await;
    assert!(set_password(&a.state, "short").is_err());
}

#[tokio::test]
async fn login_sets_cookie_and_returns_csrf() {
    let a = start(true).await;
    let r = login(&a, PASSWORD).await;
    assert_eq!(r.status(), 200);
    let cookie = r.headers().get("set-cookie").unwrap().to_str().unwrap().to_string();
    assert!(cookie.starts_with("c0re_session="));
    for attr in ["HttpOnly", "SameSite=Strict", "Path=/admin"] {
        assert!(cookie.contains(attr), "{cookie}");
    }
    let v: Value = r.json().await.unwrap();
    assert!(v["csrf"].as_str().unwrap().len() >= 32);
    let r = login(&a, "wrong password!").await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_requires_session() {
    let a = start(true).await;
    for path in ["/keys", "/overview", "/usage", "/models", "/settings", "/public"] {
        let r = client().get(format!("{}/admin/api{path}", a.url)).send().await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
}

#[tokio::test]
async fn post_without_csrf_403() {
    let a = start(true).await;
    let s = session(&a).await;
    let r = client()
        .post(format!("{}/admin/api/keys", a.url))
        .header("cookie", &s.cookie)
        .json(&json!({"project":"x","reach":"private"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert!(a.state.keys.list().unwrap().is_empty());
}

#[tokio::test]
async fn create_key_returns_secret_once() {
    let a = start(true).await;
    let s = session(&a).await;
    let r = req(&a, &s, reqwest::Method::POST, "/keys")
        .json(&json!({"project":"lenz","reach":"private","models":["qwen3-vl:8b-instruct"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    let secret = v["secret"].as_str().unwrap().to_string();
    assert!(secret.starts_with("c0re_"));
    assert_eq!(v["key"]["project"], "lenz");
    let list = req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().text().await.unwrap();
    assert!(list.contains("lenz") && !list.contains(&secret));
}

#[tokio::test]
async fn reveal_shows_the_secret_again() {
    let a = start(true).await;
    let s = session(&a).await;
    let v: Value = req(&a, &s, reqwest::Method::POST, "/keys")
        .json(&json!({"project":"lenz","reach":"private"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["key"]["revealable"], true);
    let (id, secret) = (v["key"]["id"].as_str().unwrap(), v["secret"].as_str().unwrap());
    let r = req(&a, &s, reqwest::Method::POST, &format!("/keys/{id}/reveal")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers().get("cache-control").unwrap(), "no-store");
    let got: Value = r.json().await.unwrap();
    assert_eq!(got["secret"], secret);
    // Needs the anti-forgery header like every other change.
    let r = client()
        .post(format!("{}/admin/api/keys/{id}/reveal", a.url))
        .header("cookie", &s.cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = req(&a, &s, reqwest::Method::POST, "/keys/k_nope00/reveal").send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reveal_of_an_old_key_explains_why_not() {
    let a = start(true).await;
    let s = session(&a).await;
    let (key, _) = a
        .state
        .keys
        .create(c0re::keys::NewKey {
            project: "old".into(),
            reach: c0re::keys::Reach::Private,
            models: vec![],
            limits: None,
            note: String::new(),
        })
        .unwrap();
    a.state.conn.lock().unwrap().execute("UPDATE keys SET secret_enc = NULL", []).unwrap();
    let r = req(&a, &s, reqwest::Method::POST, &format!("/keys/{}/reveal", key.id)).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["error"]["code"], "not_saved");
}

#[tokio::test]
async fn bad_key_input_is_400() {
    let a = start(true).await;
    let s = session(&a).await;
    let r = req(&a, &s, reqwest::Method::POST, "/keys").json(&json!({"project":"a/b","reach":"private"})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let r = req(&a, &s, reqwest::Method::POST, "/keys").json(&json!({"project":"ok","reach":"open"})).send().await.unwrap();
    assert!(r.status().is_client_error());
}

#[tokio::test]
async fn update_and_delete_key() {
    let a = start(true).await;
    let s = session(&a).await;
    let v: Value = req(&a, &s, reqwest::Method::POST, "/keys")
        .json(&json!({"project":"site","reach":"public"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = v["key"]["id"].as_str().unwrap();
    let u: Value = req(&a, &s, reqwest::Method::PATCH, &format!("/keys/{id}"))
        .json(&json!({"reach":"off","rate_per_min":5}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!((u["reach"].as_str(), u["rate_per_min"].as_u64()), (Some("off"), Some(5)));
    let r = req(&a, &s, reqwest::Method::DELETE, &format!("/keys/{id}")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    let r = req(&a, &s, reqwest::Method::DELETE, &format!("/keys/{id}")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn overview_and_models_read_the_engine() {
    let a = start(true).await;
    let s = session(&a).await;
    let v: Value = req(&a, &s, reqwest::Method::GET, "/overview").send().await.unwrap().json().await.unwrap();
    assert_eq!(v["engine"]["online"], true);
    assert_eq!(v["engine"]["version"], "0.40.13");
    assert_eq!(v["engine"]["loaded"][0]["name"], "qwen3-vl:8b-instruct");
    assert_eq!(v["entrances"]["private"]["requests"], 0);
    assert_eq!(v["entrances"]["public"]["blocked"], 0);
    let v: Value = req(&a, &s, reqwest::Method::GET, "/models").send().await.unwrap().json().await.unwrap();
    let m = v.as_array().unwrap().iter().find(|m| m["name"] == "qwen3-vl:8b-instruct").unwrap();
    assert_eq!(m["loaded"], true);
    assert_eq!(m["vision"], true);
}

#[tokio::test]
async fn public_toggle_calls_funnel() {
    let a = start(true).await;
    let s = session(&a).await;
    let v: Value = req(&a, &s, reqwest::Method::POST, "/public").json(&json!({"on": true})).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["state"], "on");
    assert_eq!(v["url"], "https://kihurn-desktop.tail1234.ts.net:8443");
    req(&a, &s, reqwest::Method::POST, "/public").json(&json!({"on": false})).send().await.unwrap();
    let calls = a.runner.calls.lock().unwrap().clone();
    assert!(calls.contains(&vec!["funnel".into(), "--bg".into(), "--https=8443".into(), "http://127.0.0.1:7071".into()]));
    assert!(calls.contains(&vec!["funnel".into(), "--https=8443".into(), "off".into()]));
}

#[tokio::test]
async fn settings_round_trip_and_persist() {
    let a = start(true).await;
    let s = session(&a).await;
    let mut v: Value = req(&a, &s, reqwest::Method::GET, "/settings").send().await.unwrap().json().await.unwrap();
    assert_eq!(v["retention_days"], 90);
    v["retention_days"] = json!(30);
    v["public_defaults"]["rate_per_min"] = json!(7);
    let r = req(&a, &s, reqwest::Method::PUT, "/settings").json(&v).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let saved = c0re::config::Config::load_or_create(a._dir.path()).unwrap();
    assert_eq!((saved.retention_days, saved.public_defaults.rate_per_min), (30, 7));
}

#[tokio::test]
async fn change_password_requires_current() {
    let a = start(true).await;
    let s = session(&a).await;
    let r = req(&a, &s, reqwest::Method::POST, "/password").json(&json!({"current":"nope nope nope","new":"another long one"})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = req(&a, &s, reqwest::Method::POST, "/password").json(&json!({"current":PASSWORD,"new":"another long one"})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert_eq!(login(&a, "another long one").await.status(), 200);
}

#[tokio::test]
async fn admin_static_has_csp_header() {
    let a = start(true).await;
    for path in ["/admin", "/admin/", "/admin/keys"] {
        let r = client().get(format!("{}{path}", a.url)).send().await.unwrap();
        assert_eq!(r.status(), 200, "{path}");
        let csp = r.headers().get("content-security-policy").unwrap().to_str().unwrap().to_string();
        assert!(csp.contains("default-src 'self'") && csp.contains("frame-ancestors 'none'"), "{csp}");
        assert!(r.headers().get("content-type").unwrap().to_str().unwrap().starts_with("text/html"));
    }
}

#[tokio::test]
async fn login_failures_blocked_after_10() {
    let a = start(true).await;
    for _ in 0..10 {
        login(&a, "wrong password!").await;
    }
    let r = login(&a, PASSWORD).await;
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn session_expires() {
    let a = start(true).await;
    a.state.admin.set_session_ttl(Duration::from_millis(200));
    let s = session(&a).await;
    assert_eq!(req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().status(), 200);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_ends_session() {
    let a = start(true).await;
    let s = session(&a).await;
    req(&a, &s, reqwest::Method::POST, "/logout").send().await.unwrap();
    assert_eq!(req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn status_returns_csrf_for_logged_in_session() {
    let a = start(true).await;
    let v: Value = client().get(format!("{}/admin/api/status", a.url)).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["logged_in"], false);
    assert!(v.get("csrf").is_none_or(Value::is_null));
    let s = session(&a).await;
    let v: Value = client().get(format!("{}/admin/api/status", a.url)).header("cookie", &s.cookie).send().await.unwrap().json().await.unwrap();
    assert_eq!(v["logged_in"], true);
    assert_eq!(v["csrf"], s.csrf.as_str());
}

#[tokio::test]
async fn cli_password_change_ends_server_sessions() {
    let a = start(true).await;
    let s = session(&a).await;
    assert_eq!(req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().status(), 200);
    // `c0re admin set-password` runs in a separate process with its own AppState.
    let cli_state = AppState::open(a._dir.path(), FakeRunner::new(|_| FakeRunner::ok(""))).unwrap();
    set_password(&cli_state, "a brand new password").unwrap();
    assert_eq!(req(&a, &s, reqwest::Method::GET, "/keys").send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
}
