use c0re::cli::run_in;
use serde_json::Value;

fn run(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    run_input(dir, args, "")
}

fn run_input(dir: &std::path::Path, args: &[&str], input: &str) -> (i32, String) {
    let mut out = Vec::new();
    let mut inp = std::io::Cursor::new(input.as_bytes().to_vec());
    let mut argv = vec!["c0re".to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    let code = run_in(dir, argv, &mut out, &mut inp).unwrap_or_else(|e| {
        out.extend_from_slice(format!("error: {e:#}").as_bytes());
        1
    });
    (code, String::from_utf8(out).unwrap())
}

fn secret_in(text: &str) -> String {
    text.split_whitespace().find(|w| w.starts_with("c0re_")).expect("secret printed").to_string()
}

#[test]
fn keys_create_prints_secret_once_and_id() {
    let d = tempfile::tempdir().unwrap();
    let (code, out) = run(d.path(), &["keys", "create", "--project", "lenz", "--reach", "private", "--models", "qwen3-vl:8b-instruct,qwen3-vl:4b-instruct"]);
    assert_eq!(code, 0, "{out}");
    let secret = secret_in(&out);
    assert_eq!(secret.len(), 48);
    assert!(out.contains("k_"));
    assert!(out.to_lowercase().contains("won't be shown again"));
    let (_, list) = run(d.path(), &["keys", "list", "--json"]);
    let v: Value = serde_json::from_str(&list).unwrap();
    assert_eq!(v[0]["models"].as_array().unwrap().len(), 2);
}

#[test]
fn keys_list_never_prints_secrets() {
    let d = tempfile::tempdir().unwrap();
    let (_, out) = run(d.path(), &["keys", "create", "--project", "site", "--reach", "public"]);
    let secret = secret_in(&out);
    for args in [vec!["keys", "list"], vec!["keys", "list", "--json"]] {
        let (code, list) = run(d.path(), &args);
        assert_eq!(code, 0);
        assert!(list.contains("site") && !list.contains(&secret));
    }
}

#[test]
fn keys_off_then_on() {
    let d = tempfile::tempdir().unwrap();
    run(d.path(), &["keys", "create", "--project", "site", "--reach", "public"]);
    let id = serde_json::from_str::<Value>(&run(d.path(), &["keys", "list", "--json"]).1).unwrap()[0]["id"].as_str().unwrap().to_string();
    assert_eq!(run(d.path(), &["keys", "off", &id]).0, 0);
    let v: Value = serde_json::from_str(&run(d.path(), &["keys", "list", "--json"]).1).unwrap();
    assert_eq!(v[0]["reach"], "off");
    assert_eq!(run(d.path(), &["keys", "on", &id, "--public"]).0, 0);
    let v: Value = serde_json::from_str(&run(d.path(), &["keys", "list", "--json"]).1).unwrap();
    assert_eq!(v[0]["reach"], "public");
    assert_eq!(run(d.path(), &["keys", "on", &id]).0, 0);
    let v: Value = serde_json::from_str(&run(d.path(), &["keys", "list", "--json"]).1).unwrap();
    assert_eq!(v[0]["reach"], "private");
}

#[test]
fn keys_edit_and_delete() {
    let d = tempfile::tempdir().unwrap();
    run(d.path(), &["keys", "create", "--project", "site", "--reach", "public"]);
    let id = serde_json::from_str::<Value>(&run(d.path(), &["keys", "list", "--json"]).1).unwrap()[0]["id"].as_str().unwrap().to_string();
    assert_eq!(run(d.path(), &["keys", "edit", &id, "--rate", "3", "--project", "site2"]).0, 0);
    let v: Value = serde_json::from_str(&run(d.path(), &["keys", "list", "--json"]).1).unwrap();
    assert_eq!((v[0]["rate_per_min"].as_u64(), v[0]["project"].as_str()), (Some(3), Some("site2")));
    assert_eq!(run(d.path(), &["keys", "delete", &id]).0, 0);
    assert_ne!(run(d.path(), &["keys", "delete", &id]).0, 0);
}

#[test]
fn keys_create_rejects_bad_reach() {
    let d = tempfile::tempdir().unwrap();
    let (code, _) = run(d.path(), &["keys", "create", "--project", "x", "--reach", "open"]);
    assert_ne!(code, 0);
}

#[test]
fn usage_json_shape() {
    let d = tempfile::tempdir().unwrap();
    let (code, out) = run(d.path(), &["usage", "--days", "7", "--json"]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["days"], 7);
    assert_eq!(v["summary"]["total"], 0);
}

#[test]
fn status_reports_engine_offline_when_no_ollama() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("c0re.toml"), "ollama_url = \"http://127.0.0.1:1\"\ntailscale_bin = \"definitely-not-installed-xyz\"\n").unwrap();
    let (code, out) = run(d.path(), &["status", "--json"]);
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["engine"]["online"], false);
    assert_eq!(v["private_port"], 7080);
    assert_eq!(v["public"]["state"], "unavailable");
}

#[test]
fn set_password_from_stdin() {
    let d = tempfile::tempdir().unwrap();
    let (code, out) = run_input(d.path(), &["admin", "set-password", "--stdin"], "a long enough password\n");
    assert_eq!(code, 0, "{out}");
    let state = c0re::AppState::open(d.path(), std::sync::Arc::new(c0re::tailscale::SystemRunner { bin: "x".into() })).unwrap();
    assert!(c0re::admin::auth::verify_password(&state, "a long enough password").unwrap());
    let (code, _) = run_input(d.path(), &["admin", "set-password", "--stdin"], "short\n");
    assert_ne!(code, 0);
}

#[test]
fn serve_records_why_it_could_not_start() {
    let d = tempfile::tempdir().unwrap();
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    std::fs::write(
        d.path().join("c0re.toml"),
        format!("private_port = {port}\npublic_port = {free}\ntailscale_bin = \"definitely-not-installed-xyz\"\n"),
    )
    .unwrap();
    let (code, _) = run(d.path(), &["serve"]);
    assert_ne!(code, 0);
    let note = std::fs::read_to_string(d.path().join("startup-error.txt")).expect("startup-error.txt written");
    assert!(note.contains(&port.to_string()), "{note}");
}

#[test]
fn keys_show_explains_when_a_key_cannot_be_shown() {
    // Off Windows there's no DPAPI, so no key is kept; on Windows this path is for 0.1 keys.
    let d = tempfile::tempdir().unwrap();
    let (_, out) = run(d.path(), &["keys", "create", "--project", "lenz"]);
    let id = out.split_whitespace().find(|w| w.starts_with("k_")).unwrap().to_string();
    let (code, out) = run(d.path(), &["keys", "show", &id]);
    assert_ne!(code, 0);
    assert!(out.contains("can't be shown again"), "{out}");
    let (code, out) = run(d.path(), &["keys", "show", "k_nope00"]);
    assert_ne!(code, 0);
    assert!(out.contains("No key with id k_nope00"), "{out}");
}
