//! C0RE configuration (`c0re.toml` in the data directory).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "c0re.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyDefaults {
    pub rate_per_min: u32,
    pub daily_cap: u32,
    pub max_concurrent: u32,
    pub max_ctx: u32,
    pub max_body_mb: u32,
}

impl KeyDefaults {
    pub const PRIVATE: KeyDefaults =
        KeyDefaults { rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20 };
    pub const PUBLIC: KeyDefaults =
        KeyDefaults { rate_per_min: 20, daily_cap: 500, max_concurrent: 1, max_ctx: 8192, max_body_mb: 8 };
}

impl Default for KeyDefaults {
    fn default() -> Self {
        Self::PRIVATE
    }
}

fn public_defaults() -> KeyDefaults {
    KeyDefaults::PUBLIC
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub private_port: u16,
    pub public_port: u16,
    pub funnel_port: u16,
    pub ollama_url: String,
    pub tailscale_bin: String,
    pub retention_days: u32,
    pub max_db_mb: u64,
    /// How often to look for the Tailscale address while Tailscale is down.
    pub tailscale_retry_secs: u64,
    pub private_defaults: KeyDefaults,
    #[serde(default = "public_defaults")]
    pub public_defaults: KeyDefaults,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            private_port: 7080,
            public_port: 7071,
            funnel_port: 8443,
            ollama_url: "http://127.0.0.1:11434".into(),
            tailscale_bin: "tailscale".into(),
            retention_days: 90,
            max_db_mb: 100,
            tailscale_retry_secs: 30,
            private_defaults: KeyDefaults::PRIVATE,
            public_defaults: KeyDefaults::PUBLIC,
        }
    }
}

impl Config {
    /// Reads `c0re.toml` from `dir`, creating it with defaults when missing.
    /// A file that exists but does not parse is an error: config typos must be visible.
    pub fn load_or_create(dir: &Path) -> Result<Config> {
        let path = dir.join(CONFIG_FILE);
        if !path.exists() {
            let c = Config::default();
            c.save(dir)?;
            return Ok(c);
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let file: toml::Table = toml::from_str(&text).with_context(|| format!("{} is not valid TOML", path.display()))?;
        // Merge the file over the full defaults so a partial section keeps its own defaults.
        let mut merged = toml::Table::try_from(Config::default())?;
        merge(&mut merged, file);
        toml::Value::Table(merged)
            .try_into()
            .with_context(|| format!("{} has an invalid value", path.display()))
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{CONFIG_FILE}.tmp"));
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, dir.join(CONFIG_FILE))?;
        Ok(())
    }
}

fn merge(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

/// `C0RE_HOME`, else `%LOCALAPPDATA%\C0RE`, else `~/.local/share/c0re`.
pub fn data_dir() -> PathBuf {
    if let Some(h) = std::env::var_os("C0RE_HOME") {
        return PathBuf::from(h);
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(l).join("C0RE");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/c0re")
}
