//! API keys: generation, hashing, verification and storage.
//!
//! Secrets are 256-bit random values, so a fast hash (SHA-256) is enough; a slow
//! password hash would only add latency to every request. Checks use only the hash; a
//! copy encrypted by the vault is kept so the dashboard can show the key again.

use crate::config::{Config, KeyDefaults};
use crate::vault::Vault;
use anyhow::{Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use subtle::ConstantTimeEq;

pub const SECRET_PREFIX: &str = "c0re_";
const SECRET_LEN: usize = 43;
const LOOKUP_LEN: usize = 12;
const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE32: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reach {
    Private,
    Public,
    Off,
}

impl Reach {
    pub fn as_str(self) -> &'static str {
        match self {
            Reach::Private => "private",
            Reach::Public => "public",
            Reach::Off => "off",
        }
    }
}

impl FromStr for Reach {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "private" => Ok(Reach::Private),
            "public" => Ok(Reach::Public),
            "off" => Ok(Reach::Off),
            other => bail!("reach must be private, public or off (got '{other}')"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub id: String,
    pub project: String,
    pub reach: Reach,
    pub models: Vec<String>,
    pub rate_per_min: u32,
    pub daily_cap: u32,
    pub max_concurrent: u32,
    pub max_ctx: u32,
    pub max_body_mb: u32,
    pub note: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    /// Whether the secret can be shown again (false for keys made before 0.2).
    #[serde(default)]
    pub revealable: bool,
}

/// What `KeyStore::reveal` found.
#[derive(Debug, PartialEq, Eq)]
pub enum Revealed {
    Secret(String),
    /// The key exists but its secret wasn't kept, or can't be decrypted here.
    NotSaved,
    NotFound,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewKey {
    pub project: String,
    pub reach: Reach,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub limits: Option<KeyDefaults>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct KeyPatch {
    pub project: Option<String>,
    pub reach: Option<Reach>,
    pub models: Option<Vec<String>>,
    pub rate_per_min: Option<u32>,
    pub daily_cap: Option<u32>,
    pub max_concurrent: Option<u32>,
    pub max_ctx: Option<u32>,
    pub max_body_mb: Option<u32>,
    pub note: Option<String>,
}

#[derive(Clone)]
pub struct KeyStore {
    conn: Arc<Mutex<Connection>>,
    vault: Arc<dyn Vault>,
    private_defaults: KeyDefaults,
    public_defaults: KeyDefaults,
}

fn random_string(alphabet: &[u8], len: usize) -> Result<String> {
    // Rejection sampling keeps every character uniformly likely.
    let limit = 256 - (256 % alphabet.len());
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    while out.len() < len {
        getrandom::fill(&mut buf).map_err(|e| anyhow!("no secure randomness: {e}"))?;
        for &b in &buf {
            if (b as usize) < limit && out.len() < len {
                out.push(alphabet[b as usize % alphabet.len()] as char);
            }
        }
    }
    Ok(out)
}

fn hash(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

fn validate_project(p: &str) -> Result<()> {
    let ok = (1..=40).contains(&p.chars().count())
        && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '.' | '-'));
    if !ok {
        bail!("project name must be 1–40 characters: letters, numbers, space, _ . -");
    }
    Ok(())
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const COLS: &str = "id, project, reach, models, rate_per_min, daily_cap, max_concurrent, max_ctx, max_body_mb, note, created_at, last_used_at";
const SELECT: &str = "SELECT id, project, reach, models, rate_per_min, daily_cap, max_concurrent, max_ctx, max_body_mb, note, created_at, last_used_at, secret_enc IS NOT NULL";

fn from_row(r: &Row) -> rusqlite::Result<Key> {
    let reach: String = r.get(2)?;
    let models: String = r.get(3)?;
    Ok(Key {
        id: r.get(0)?,
        project: r.get(1)?,
        reach: reach.parse().unwrap_or(Reach::Off), // unknown value: fail closed
        models: serde_json::from_str(&models).unwrap_or_default(),
        rate_per_min: r.get(4)?,
        daily_cap: r.get(5)?,
        max_concurrent: r.get(6)?,
        max_ctx: r.get(7)?,
        max_body_mb: r.get(8)?,
        note: r.get(9)?,
        created_at: r.get(10)?,
        last_used_at: r.get(11)?,
        revealable: r.get(12)?,
    })
}

impl KeyStore {
    pub fn new(conn: Arc<Mutex<Connection>>, cfg: &Config, vault: Arc<dyn Vault>) -> Self {
        KeyStore { conn, vault, private_defaults: cfg.private_defaults, public_defaults: cfg.public_defaults }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Creates a key and returns it with its secret. The secret itself is stored only
    /// encrypted by the vault; without one it is shown this once.
    pub fn create(&self, k: NewKey) -> Result<(Key, String)> {
        validate_project(&k.project)?;
        let lim = k.limits.unwrap_or(match k.reach {
            Reach::Public => self.public_defaults,
            _ => self.private_defaults,
        });
        let secret = format!("{SECRET_PREFIX}{}", random_string(BASE62, SECRET_LEN)?);
        let id = format!("k_{}", random_string(BASE32, 6)?);
        let sealed = match self.vault.seal(secret.as_bytes()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!("key {id} works but can't be shown again: {e:#}");
                None
            }
        };
        let key = Key {
            id,
            project: k.project,
            reach: k.reach,
            models: k.models,
            rate_per_min: lim.rate_per_min,
            daily_cap: lim.daily_cap,
            max_concurrent: lim.max_concurrent,
            max_ctx: lim.max_ctx,
            max_body_mb: lim.max_body_mb,
            note: k.note,
            created_at: now(),
            last_used_at: None,
            revealable: sealed.is_some(),
        };
        let prefix = &secret[SECRET_PREFIX.len()..SECRET_PREFIX.len() + LOOKUP_LEN];
        self.db().execute(
            &format!("INSERT INTO keys ({COLS}, prefix, hash, secret_enc) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)"),
            params![
                key.id,
                key.project,
                key.reach.as_str(),
                serde_json::to_string(&key.models)?,
                key.rate_per_min,
                key.daily_cap,
                key.max_concurrent,
                key.max_ctx,
                key.max_body_mb,
                key.note,
                key.created_at,
                key.last_used_at,
                prefix,
                hash(&secret),
                sealed
            ],
        )?;
        Ok((key, secret))
    }

    /// Returns the key a secret belongs to. Unknown prefixes are rejected without hashing.
    pub fn verify(&self, secret: &str) -> Result<Option<Key>> {
        let body = match secret.strip_prefix(SECRET_PREFIX) {
            Some(b) if b.len() == SECRET_LEN && b.bytes().all(|c| c.is_ascii_alphanumeric()) => b,
            _ => return Ok(None),
        };
        let db = self.db();
        let row: Option<(String, Key)> = db
            .query_row(&format!("{SELECT}, hash FROM keys WHERE prefix = ?1"), [&body[..LOOKUP_LEN]], |r| {
                Ok((r.get(13)?, from_row(r)?))
            })
            .optional()?;
        Ok(row.and_then(|(stored, key)| {
            bool::from(stored.as_bytes().ct_eq(hash(secret).as_bytes())).then_some(key)
        }))
    }

    pub fn list(&self) -> Result<Vec<Key>> {
        let db = self.db();
        let mut st = db.prepare(&format!("{SELECT} FROM keys ORDER BY created_at, id"))?;
        let rows = st.query_map([], from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<Option<Key>> {
        Ok(self.db().query_row(&format!("{SELECT} FROM keys WHERE id = ?1"), [id], from_row).optional()?)
    }

    pub fn update(&self, id: &str, p: KeyPatch) -> Result<Option<Key>> {
        let Some(mut k) = self.get(id)? else { return Ok(None) };
        if let Some(v) = p.project {
            validate_project(&v)?;
            k.project = v;
        }
        if let Some(v) = p.reach {
            k.reach = v;
        }
        if let Some(v) = p.models {
            k.models = v;
        }
        if let Some(v) = p.rate_per_min {
            k.rate_per_min = v;
        }
        if let Some(v) = p.daily_cap {
            k.daily_cap = v;
        }
        if let Some(v) = p.max_concurrent {
            k.max_concurrent = v;
        }
        if let Some(v) = p.max_ctx {
            k.max_ctx = v;
        }
        if let Some(v) = p.max_body_mb {
            k.max_body_mb = v;
        }
        if let Some(v) = p.note {
            k.note = v;
        }
        self.db().execute(
            "UPDATE keys SET project=?2, reach=?3, models=?4, rate_per_min=?5, daily_cap=?6, max_concurrent=?7,
             max_ctx=?8, max_body_mb=?9, note=?10 WHERE id=?1",
            params![
                k.id,
                k.project,
                k.reach.as_str(),
                serde_json::to_string(&k.models)?,
                k.rate_per_min,
                k.daily_cap,
                k.max_concurrent,
                k.max_ctx,
                k.max_body_mb,
                k.note
            ],
        )?;
        Ok(Some(k))
    }

    /// The key's secret, decrypted. Shown only if it still matches the key's hash, so a
    /// blob moved between rows can never reveal another key.
    pub fn reveal(&self, id: &str) -> Result<Revealed> {
        let row: Option<(Option<Vec<u8>>, String)> = self
            .db()
            .query_row("SELECT secret_enc, hash FROM keys WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((sealed, stored)) = row else { return Ok(Revealed::NotFound) };
        let Some(sealed) = sealed else { return Ok(Revealed::NotSaved) };
        let secret = match self.vault.open(&sealed).map(String::from_utf8) {
            Ok(Ok(s)) => s,
            Ok(Err(_)) => return Ok(Revealed::NotSaved),
            Err(e) => {
                tracing::warn!("key {id}: {e:#}");
                return Ok(Revealed::NotSaved);
            }
        };
        if bool::from(stored.as_bytes().ct_eq(hash(&secret).as_bytes())) {
            Ok(Revealed::Secret(secret))
        } else {
            Ok(Revealed::NotSaved)
        }
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.db().execute("DELETE FROM keys WHERE id = ?1", [id])? > 0)
    }

    pub fn touch(&self, id: &str, now: i64) -> Result<()> {
        self.db().execute("UPDATE keys SET last_used_at = ?2 WHERE id = ?1", params![id, now])?;
        Ok(())
    }
}
