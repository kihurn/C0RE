//! Admin password (Argon2id) and in-memory sessions with a CSRF token each.

use crate::AppState;
use anyhow::{Result, anyhow, bail};
use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;

pub const MIN_PASSWORD_LEN: usize = 10;
const PASSWORD_KEY: &str = "admin_password";
const GENERATION_KEY: &str = "admin_password_generation";

struct Session {
    csrf: String,
    expires: Instant,
    /// Password generation the session was created under; a newer password ends it.
    generation: i64,
}

pub struct AdminState {
    sessions: Mutex<HashMap<String, Session>>,
    ttl: RwLock<Duration>,
}

impl Default for AdminState {
    fn default() -> Self {
        AdminState { sessions: Mutex::new(HashMap::new()), ttl: RwLock::new(Duration::from_secs(12 * 3600)) }
    }
}

fn token() -> Result<String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| anyhow!("no secure randomness: {e}"))?;
    Ok(hex::encode(b))
}

impl AdminState {
    pub fn set_session_ttl(&self, ttl: Duration) {
        *self.ttl.write().unwrap_or_else(std::sync::PoisonError::into_inner) = ttl;
    }

    /// Returns (session id, csrf token).
    pub fn create_session(&self, generation: i64) -> Result<(String, String)> {
        let (id, csrf) = (token()?, token()?);
        let ttl = *self.ttl.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut s = self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        s.retain(|_, v| v.expires > now);
        s.insert(id.clone(), Session { csrf: csrf.clone(), expires: now + ttl, generation });
        Ok((id, csrf))
    }

    /// Checks the session and, for state-changing requests, its CSRF token.
    pub fn check(&self, id: &str, csrf: Option<&str>, generation: i64) -> SessionCheck {
        let mut s = self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match s.get(id) {
            Some(sess) if sess.expires > Instant::now() && sess.generation == generation => match csrf {
                None => SessionCheck::Ok,
                Some(c) if bool::from(c.as_bytes().ct_eq(sess.csrf.as_bytes())) => SessionCheck::Ok,
                Some(_) => SessionCheck::BadCsrf,
            },
            Some(_) => {
                s.remove(id);
                SessionCheck::None
            }
            None => SessionCheck::None,
        }
    }

    /// The CSRF token of a live session (lets the dashboard recover it after a reload).
    pub fn csrf_for(&self, id: &str, generation: i64) -> Option<String> {
        let s = self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        s.get(id).filter(|v| v.expires > Instant::now() && v.generation == generation).map(|v| v.csrf.clone())
    }

    pub fn end_session(&self, id: &str) {
        self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(id);
    }

    pub fn end_all_sessions(&self) {
        self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SessionCheck {
    Ok,
    None,
    BadCsrf,
}

pub fn set_password(state: &AppState, password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        bail!("The admin password must be at least {MIN_PASSWORD_LEN} characters.");
    }
    let hash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| anyhow!("could not hash password: {e}"))?
        .to_string();
    let mut c = state.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let tx = c.transaction()?;
    tx.execute(
        "INSERT INTO settings (k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![PASSWORD_KEY, hash],
    )?;
    // Bumping the generation ends every session, including those held by a running server
    // when the password is changed from the CLI.
    tx.execute(
        "INSERT INTO settings (k, v) VALUES (?1, '1') ON CONFLICT(k) DO UPDATE SET v = CAST(v AS INTEGER) + 1",
        params![GENERATION_KEY],
    )?;
    tx.commit()?;
    drop(c);
    state.admin.end_all_sessions();
    Ok(())
}

/// Current password generation (0 before any password is set).
pub fn password_generation(state: &AppState) -> Result<i64> {
    let v: Option<String> = state
        .conn
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .query_row("SELECT v FROM settings WHERE k = ?1", [GENERATION_KEY], |r| r.get(0))
        .optional()?;
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

pub fn password_set(state: &AppState) -> Result<bool> {
    Ok(stored_hash(state)?.is_some())
}

fn stored_hash(state: &AppState) -> Result<Option<String>> {
    Ok(state
        .conn
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .query_row("SELECT v FROM settings WHERE k = ?1", [PASSWORD_KEY], |r| r.get(0))
        .optional()?)
}

pub fn verify_password(state: &AppState, password: &str) -> Result<bool> {
    let Some(stored) = stored_hash(state)? else { return Ok(false) };
    let parsed = PasswordHash::new(&stored).map_err(|e| anyhow!("stored password hash is invalid: {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}
