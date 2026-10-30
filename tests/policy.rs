use axum::http::{HeaderMap, HeaderValue, Method};
use c0re::keys::{Key, Reach};
use c0re::policy::*;
use serde_json::json;

fn key(reach: Reach, models: &[&str]) -> Key {
    Key {
        id: "k_test01".into(),
        project: "t".into(),
        reach,
        models: models.iter().map(|s| s.to_string()).collect(),
        rate_per_min: 60,
        daily_cap: 2000,
        max_concurrent: 2,
        max_ctx: 8192,
        max_body_mb: 20,
        note: String::new(),
        created_at: 0,
        last_used_at: None,
    }
}

#[test]
fn routes_api_paths() {
    let cases = [
        (Method::POST, "/api/chat", Route::Chat),
        (Method::POST, "/api/generate", Route::Generate),
        (Method::POST, "/api/embed", Route::Embed),
        (Method::GET, "/api/tags", Route::Tags),
        (Method::POST, "/api/show", Route::Show),
        (Method::GET, "/api/ps", Route::Ps),
        (Method::GET, "/api/version", Route::Version),
        (Method::POST, "/v1/chat/completions", Route::OpenAiChat),
        (Method::GET, "/v1/models", Route::OpenAiModels),
        (Method::GET, "/health", Route::Health),
    ];
    for (m, p, r) in cases {
        assert_eq!(route_for(&m, p), Some(r), "{m} {p}");
    }
    assert_eq!(route_for(&Method::GET, "/api/chat"), None, "wrong method");
}

#[test]
fn management_paths_have_no_route() {
    for p in ["/api/pull", "/api/push", "/api/create", "/api/copy", "/api/delete", "/api/blobs/x", "/admin", "/admin/api/keys", "/"] {
        assert_eq!(route_for(&Method::POST, p), None, "{p}");
        assert_eq!(route_for(&Method::GET, p), None, "{p}");
        assert_eq!(route_for(&Method::DELETE, p), None, "{p}");
    }
}

#[test]
fn bearer_and_x_api_key_both_work() {
    let mut h = HeaderMap::new();
    h.insert("authorization", HeaderValue::from_static("Bearer c0re_abc"));
    assert_eq!(bearer_secret(&h), Some("c0re_abc"));
    let mut h = HeaderMap::new();
    h.insert("x-api-key", HeaderValue::from_static("c0re_xyz"));
    assert_eq!(bearer_secret(&h), Some("c0re_xyz"));
    let mut h = HeaderMap::new();
    h.insert("authorization", HeaderValue::from_static("Basic Zm9v"));
    assert_eq!(bearer_secret(&h), None);
    assert_eq!(bearer_secret(&HeaderMap::new()), None);
}

#[test]
fn off_key_denied_everywhere() {
    for e in [Entrance::Private, Entrance::Public] {
        let d = check_reach(&key(Reach::Off, &[]), e).unwrap_err();
        assert_eq!((d.status, d.code), (403, "key_disabled"));
    }
}

#[test]
fn private_key_denied_on_public() {
    let d = check_reach(&key(Reach::Private, &[]), Entrance::Public).unwrap_err();
    assert_eq!((d.status, d.code), (403, "key_not_public"));
    assert!(check_reach(&key(Reach::Private, &[]), Entrance::Private).is_ok());
}

#[test]
fn public_key_allowed_on_both() {
    assert!(check_reach(&key(Reach::Public, &[]), Entrance::Public).is_ok());
    assert!(check_reach(&key(Reach::Public, &[]), Entrance::Private).is_ok());
}

#[test]
fn empty_models_list_allows_any() {
    assert!(check_model(&key(Reach::Private, &[]), "anything:7b").is_ok());
}

#[test]
fn model_not_in_list_denied() {
    let k = key(Reach::Private, &["qwen3-vl:8b-instruct"]);
    assert!(check_model(&k, "qwen3-vl:8b-instruct").is_ok());
    let d = check_model(&k, "gemma4:latest").unwrap_err();
    assert_eq!((d.status, d.code), (403, "model_not_allowed"));
}

#[test]
fn model_without_tag_matches_latest() {
    let k = key(Reach::Private, &["llama3:latest"]);
    assert!(check_model(&k, "llama3").is_ok());
    let k = key(Reach::Private, &["llama3"]);
    assert!(check_model(&k, "llama3:latest").is_ok());
}

#[test]
fn clamps_num_ctx() {
    let mut b = json!({"model":"m","options":{"num_ctx":262144,"temperature":0.3}});
    rewrite_body(&mut b, &key(Reach::Private, &[]));
    assert_eq!(b["options"]["num_ctx"], 8192);
    assert_eq!(b["options"]["temperature"], 0.3);
}

#[test]
fn keeps_smaller_num_ctx() {
    let mut b = json!({"model":"m","options":{"num_ctx":4096}});
    rewrite_body(&mut b, &key(Reach::Private, &[]));
    assert_eq!(b["options"]["num_ctx"], 4096);
}

#[test]
fn removes_non_numeric_num_ctx() {
    for bad in [json!("big"), json!(-5), json!(1.5e12), json!(null)] {
        let mut b = json!({"model":"m","options":{"num_ctx": bad}});
        rewrite_body(&mut b, &key(Reach::Private, &[]));
        assert!(b["options"].get("num_ctx").is_none() || b["options"]["num_ctx"] == 8192, "{b}");
    }
}

#[test]
fn drops_keep_alive() {
    let mut b = json!({"model":"m","keep_alive":0});
    rewrite_body(&mut b, &key(Reach::Private, &[]));
    assert!(b.get("keep_alive").is_none());
}

#[test]
fn leaves_images_think_stream_untouched() {
    let orig = json!({"model":"m","stream":true,"think":true,"messages":[{"role":"user","content":"hi","images":["QUJD"]}]});
    let mut b = orig.clone();
    rewrite_body(&mut b, &key(Reach::Private, &[]));
    assert_eq!(b, orig);
}

#[test]
fn missing_model_is_bad_request() {
    for b in [json!({}), json!({"model": 5}), json!({"model": ""}), json!([1, 2])] {
        let d = model_of(&b).unwrap_err();
        assert_eq!((d.status, d.code), (400, "bad_request"));
    }
    assert_eq!(model_of(&json!({"model":"m:1"})).unwrap(), "m:1");
}

#[test]
fn denied_body_shape() {
    let d = Denied::new(429, "rate_limited", "Slow down.").with_retry_after(7);
    assert_eq!(d.body(), json!({"error":{"code":"rate_limited","message":"Slow down."}}));
    assert_eq!(d.retry_after, Some(7));
}

#[test]
fn top_level_keys_must_be_lowercase_ascii() {
    assert!(check_keys(&json!({"model":"m","messages":[],"stream":true,"keep_alive":5,"response_format":{}})).is_ok());
    for bad in [json!({"Model":"m"}), json!({"OPTIONS":{}}), json!({"keep_Alive":1}), json!({"ﬁeld":1})] {
        let d = check_keys(&bad).unwrap_err();
        assert_eq!((d.status, d.code), (400, "bad_request"), "{bad}");
    }
}
