use c0re::config::{Config, KeyDefaults};
use std::fs;

#[test]
fn defaults_match_spec() {
    let c = Config::default();
    assert_eq!((c.private_port, c.public_port, c.funnel_port), (7070, 7071, 8443));
    assert_eq!(c.ollama_url, "http://127.0.0.1:11434");
    assert_eq!(c.tailscale_bin, "tailscale");
    assert_eq!(c.retention_days, 90);
    assert_eq!(c.max_db_mb, 100);
    assert_eq!(
        c.private_defaults,
        KeyDefaults { rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20 }
    );
    assert_eq!(
        c.public_defaults,
        KeyDefaults { rate_per_min: 20, daily_cap: 500, max_concurrent: 1, max_ctx: 8192, max_body_mb: 8 }
    );
}

#[test]
fn creates_file_on_first_load() {
    let dir = tempfile::tempdir().unwrap();
    let c = Config::load_or_create(dir.path()).unwrap();
    assert_eq!(c, Config::default());
    assert!(dir.path().join("c0re.toml").exists());
}

#[test]
fn partial_file_merges_defaults() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("c0re.toml"), "private_port = 9000\n[public_defaults]\nrate_per_min = 5\n").unwrap();
    let c = Config::load_or_create(dir.path()).unwrap();
    assert_eq!(c.private_port, 9000);
    assert_eq!(c.public_port, 7071);
    assert_eq!(c.public_defaults.rate_per_min, 5);
    assert_eq!(c.public_defaults.daily_cap, 500);
}

#[test]
fn invalid_toml_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("c0re.toml"), "private_port = \"abc\"").unwrap();
    assert!(Config::load_or_create(dir.path()).is_err());
}

#[test]
fn save_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Config::default();
    c.retention_days = 30;
    c.save(dir.path()).unwrap();
    assert_eq!(Config::load_or_create(dir.path()).unwrap().retention_days, 30);
}
