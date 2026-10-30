use c0re::config::{Config, KeyDefaults};
use c0re::db;
use c0re::keys::{KeyPatch, KeyStore, NewKey, Reach};
use std::sync::{Arc, Mutex};

fn store() -> (KeyStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("c0re.db")).unwrap();
    (KeyStore::new(Arc::new(Mutex::new(conn)), &Config::default()), dir)
}

fn new_key(project: &str, reach: Reach) -> NewKey {
    NewKey { project: project.into(), reach, models: vec![], limits: None, note: String::new() }
}

#[test]
fn secret_has_expected_shape() {
    let (s, _d) = store();
    let (key, secret) = s.create(new_key("lenz", Reach::Private)).unwrap();
    assert!(secret.starts_with("c0re_"));
    assert_eq!(secret.len(), 5 + 43);
    assert!(secret[5..].chars().all(|c| c.is_ascii_alphanumeric()));
    assert!(key.id.starts_with("k_") && key.id.len() == 8);
}

#[test]
fn verify_accepts_secret_once_created() {
    let (s, _d) = store();
    let (key, secret) = s.create(new_key("lenz", Reach::Private)).unwrap();
    assert_eq!(s.verify(&secret).unwrap().unwrap().id, key.id);
}

#[test]
fn verify_rejects_wrong_secret_with_same_prefix() {
    let (s, _d) = store();
    let (_k, secret) = s.create(new_key("lenz", Reach::Private)).unwrap();
    let mut forged = secret.clone();
    let last = forged.pop().unwrap();
    forged.push(if last == 'a' { 'b' } else { 'a' });
    assert!(s.verify(&forged).unwrap().is_none());
}

#[test]
fn verify_rejects_garbage_without_panic() {
    let (s, _d) = store();
    s.create(new_key("lenz", Reach::Private)).unwrap();
    for g in ["", "c0re_", "c0re_short", "Bearer x", &"x".repeat(10_000), "c0re_ñññññññññññññ"] {
        assert!(s.verify(g).unwrap().is_none(), "accepted {g:?}");
    }
}

#[test]
fn secret_not_stored_in_plain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c0re.db");
    let s = KeyStore::new(Arc::new(Mutex::new(db::open(&path).unwrap())), &Config::default());
    let (_k, secret) = s.create(new_key("lenz", Reach::Private)).unwrap();
    drop(s);
    let bytes = std::fs::read(&path).unwrap();
    let wal = std::fs::read(dir.path().join("c0re.db-wal")).unwrap_or_default();
    let needle = secret.as_bytes();
    let found = |b: &[u8]| b.windows(needle.len()).any(|w| w == needle);
    assert!(!found(&bytes) && !found(&wal), "secret stored in plain text");
}

#[test]
fn private_and_public_keys_get_their_defaults() {
    let (s, _d) = store();
    let (p, _) = s.create(new_key("a", Reach::Private)).unwrap();
    let (q, _) = s.create(new_key("b", Reach::Public)).unwrap();
    assert_eq!((p.rate_per_min, p.daily_cap, p.max_concurrent, p.max_ctx, p.max_body_mb), (60, 2000, 2, 8192, 20));
    assert_eq!((q.rate_per_min, q.daily_cap, q.max_concurrent, q.max_ctx, q.max_body_mb), (20, 500, 1, 8192, 8));
}

#[test]
fn explicit_limits_override_defaults() {
    let (s, _d) = store();
    let lim = KeyDefaults { rate_per_min: 1, daily_cap: 2, max_concurrent: 3, max_ctx: 4096, max_body_mb: 5 };
    let (k, _) = s.create(NewKey { limits: Some(lim), ..new_key("a", Reach::Private) }).unwrap();
    assert_eq!((k.rate_per_min, k.daily_cap, k.max_concurrent, k.max_ctx, k.max_body_mb), (1, 2, 3, 4096, 5));
}

#[test]
fn update_changes_reach_and_limits() {
    let (s, _d) = store();
    let (k, _) = s.create(new_key("a", Reach::Private)).unwrap();
    let patch = KeyPatch { reach: Some(Reach::Off), rate_per_min: Some(5), models: Some(vec!["m".into()]), ..Default::default() };
    let u = s.update(&k.id, patch).unwrap().unwrap();
    assert_eq!(u.reach, Reach::Off);
    assert_eq!(u.rate_per_min, 5);
    assert_eq!(u.models, vec!["m".to_string()]);
    assert_eq!(s.get(&k.id).unwrap().unwrap(), u);
    assert!(s.update("k_nope00", KeyPatch::default()).unwrap().is_none());
}

#[test]
fn delete_removes_key() {
    let (s, _d) = store();
    let (k, secret) = s.create(new_key("a", Reach::Private)).unwrap();
    assert!(s.delete(&k.id).unwrap());
    assert!(!s.delete(&k.id).unwrap());
    assert!(s.verify(&secret).unwrap().is_none());
    assert!(s.list().unwrap().is_empty());
}

#[test]
fn invalid_project_name_rejected() {
    let (s, _d) = store();
    for bad in ["", &"x".repeat(41), "a/b", "<script>"] {
        assert!(s.create(new_key(bad, Reach::Private)).is_err(), "accepted {bad:?}");
    }
    assert!(s.create(new_key("my-site_2.0 beta", Reach::Private)).is_ok());
}

#[test]
fn touch_sets_last_used() {
    let (s, _d) = store();
    let (k, _) = s.create(new_key("a", Reach::Private)).unwrap();
    assert!(k.last_used_at.is_none());
    s.touch(&k.id, 1234).unwrap();
    assert_eq!(s.get(&k.id).unwrap().unwrap().last_used_at, Some(1234));
}

#[test]
fn migrations_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("c0re.db");
    drop(db::open(&p).unwrap());
    let conn = db::open(&p).unwrap();
    let v: i64 = conn.query_row("select max(version) from schema_version", [], |r| r.get(0)).unwrap();
    assert!(v >= 1);
}

#[test]
fn reach_parses_from_text() {
    assert_eq!("private".parse::<Reach>().unwrap(), Reach::Private);
    assert_eq!("PUBLIC".parse::<Reach>().unwrap(), Reach::Public);
    assert_eq!("off".parse::<Reach>().unwrap(), Reach::Off);
    assert!("open".parse::<Reach>().is_err());
}
