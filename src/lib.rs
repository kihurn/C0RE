//! C0RE: a private gateway in front of a local Ollama.

pub mod admin;
pub mod cli;
pub mod config;
pub mod db;
pub mod gpu;
pub mod install;
pub mod keys;
pub mod limits;
pub mod policy;
pub mod proxy;
pub mod server;
pub mod tailscale;
pub mod usage;
pub mod vault;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// Everything the gateway, admin API and CLI share.
pub struct AppState {
    pub dir: PathBuf,
    pub cfg: RwLock<config::Config>,
    pub conn: Arc<Mutex<rusqlite::Connection>>,
    pub keys: keys::KeyStore,
    pub usage: usage::UsageStore,
    pub limiter: limits::Limiter,
    pub failures: limits::FailureBlocker,
    pub http: reqwest::Client,
    pub admin: admin::AdminState,
    pub gpu: gpu::GpuMonitor,
    pub tailscale: Arc<dyn tailscale::Runner>,
}

pub const DB_FILE: &str = "c0re.db";

impl AppState {
    /// Loads config and database from `dir`. Refuses to start if either is unusable:
    /// C0RE never runs without working key checks.
    pub fn open(dir: &Path, tailscale: Arc<dyn tailscale::Runner>) -> Result<Arc<AppState>> {
        Self::open_with(dir, tailscale, vault::system())
    }

    pub fn open_with(
        dir: &Path,
        tailscale: Arc<dyn tailscale::Runner>,
        vault: Arc<dyn vault::Vault>,
    ) -> Result<Arc<AppState>> {
        let cfg = config::Config::load_or_create(dir)?;
        let conn = Arc::new(Mutex::new(db::open(&dir.join(DB_FILE))?));
        Ok(Arc::new(AppState {
            dir: dir.to_path_buf(),
            keys: keys::KeyStore::new(conn.clone(), &cfg, vault),
            usage: usage::UsageStore::new(conn.clone()),
            cfg: RwLock::new(cfg),
            conn,
            limiter: limits::Limiter::new(),
            failures: limits::FailureBlocker::new(10, Duration::from_secs(60), Duration::from_secs(60)),
            http: reqwest::Client::builder().connect_timeout(Duration::from_secs(3)).build()?,
            admin: admin::AdminState::default(),
            gpu: gpu::GpuMonitor::default(),
            tailscale,
        }))
    }

    pub fn config(&self) -> config::Config {
        self.cfg.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The PC's current UTC offset (follows daylight saving), for "today" in daily caps.
pub fn local_utc_offset() -> i32 {
    chrono::Local::now().offset().local_minus_utc()
}
