mod common;

use c0re::config::Config;
use c0re::db;
use c0re::keys::{KeyStore, NewKey, Reach, Revealed};
use c0re::vault::{self, Vault};
use common::TestVault;
use std::sync::{Arc, Mutex};

fn new_key(project: &str) -> NewKey {
    NewKey { project: project.into(), reach: Reach::Private, models: vec![], limits: None, note: String::new() }
}

fn store_with(dir: &std::path::Path, v: Arc<dyn Vault>) -> KeyStore {
    let conn = db::open(&dir.join("c0re.db")).unwrap();
    KeyStore::new(Arc::new(Mutex::new(conn)), &Config::default(), v)
}

#[test]
fn reveal_returns_the_secret_it_created() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(TestVault));
    let (key, secret) = s.create(new_key("lenz")).unwrap();
    assert!(key.revealable);
    assert_eq!(s.reveal(&key.id).unwrap(), Revealed::Secret(secret));
    assert!(s.get(&key.id).unwrap().unwrap().revealable);
    assert!(s.list().unwrap()[0].revealable);
}

#[test]
fn reveal_unknown_key_is_not_found() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(TestVault));
    assert_eq!(s.reveal("k_nope00").unwrap(), Revealed::NotFound);
}

#[test]
fn without_a_vault_keys_still_work_but_cannot_be_revealed() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(vault::Unavailable));
    let (key, secret) = s.create(new_key("lenz")).unwrap();
    assert!(!key.revealable);
    assert_eq!(s.verify(&secret).unwrap().unwrap().id, key.id);
    assert_eq!(s.reveal(&key.id).unwrap(), Revealed::NotSaved);
}

#[test]
fn keys_from_before_this_version_are_not_saved() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(TestVault));
    let (key, _) = s.create(new_key("lenz")).unwrap();
    // A key created by 0.1 has no stored secret.
    let conn = rusqlite::Connection::open(d.path().join("c0re.db")).unwrap();
    conn.execute("UPDATE keys SET secret_enc = NULL", []).unwrap();
    assert_eq!(s.reveal(&key.id).unwrap(), Revealed::NotSaved);
    assert!(!s.get(&key.id).unwrap().unwrap().revealable);
}

#[test]
fn a_secret_that_no_longer_decrypts_is_not_saved_rather_than_an_error() {
    // e.g. c0re.db copied to another Windows account: DPAPI refuses to decrypt.
    let d = tempfile::tempdir().unwrap();
    let (key, _) = store_with(d.path(), Arc::new(TestVault)).create(new_key("lenz")).unwrap();
    let other = store_with(d.path(), Arc::new(RefusingVault));
    assert_eq!(other.reveal(&key.id).unwrap(), Revealed::NotSaved);
}

#[test]
fn a_tampered_secret_is_never_shown() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(TestVault));
    let (a, _) = s.create(new_key("a")).unwrap();
    let (b, _) = s.create(new_key("b")).unwrap();
    // Copy b's stored secret onto a: decrypts fine, but isn't a's key.
    let conn = rusqlite::Connection::open(d.path().join("c0re.db")).unwrap();
    conn.execute("UPDATE keys SET secret_enc = (SELECT secret_enc FROM keys WHERE id = ?2) WHERE id = ?1", [&a.id, &b.id])
        .unwrap();
    assert_eq!(s.reveal(&a.id).unwrap(), Revealed::NotSaved);
}

#[test]
fn stored_secret_is_encrypted_not_plain() {
    let d = tempfile::tempdir().unwrap();
    let s = store_with(d.path(), Arc::new(TestVault));
    let (key, secret) = s.create(new_key("lenz")).unwrap();
    let conn = rusqlite::Connection::open(d.path().join("c0re.db")).unwrap();
    let blob: Vec<u8> = conn.query_row("SELECT secret_enc FROM keys WHERE id = ?1", [&key.id], |r| r.get(0)).unwrap();
    assert!(!blob.windows(secret.len()).any(|w| w == secret.as_bytes()));
}

struct RefusingVault;
impl Vault for RefusingVault {
    fn seal(&self, _: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("no")
    }
    fn open(&self, _: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("not this account")
    }
}

#[cfg(windows)]
#[test]
fn dpapi_round_trip() {
    let v = vault::system();
    let sealed = v.seal(b"c0re_secret").unwrap();
    assert_ne!(sealed, b"c0re_secret");
    assert_eq!(v.open(&sealed).unwrap(), b"c0re_secret");
}
