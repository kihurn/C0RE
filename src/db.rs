//! SQLite database: opening and schema migrations.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

/// Each entry is applied once, in order; append new migrations, never edit old ones.
const MIGRATIONS: &[&str] = &[
    // 1: keys
    "CREATE TABLE keys (
        id TEXT PRIMARY KEY,
        project TEXT NOT NULL,
        reach TEXT NOT NULL,
        models TEXT NOT NULL,
        rate_per_min INTEGER NOT NULL,
        daily_cap INTEGER NOT NULL,
        max_concurrent INTEGER NOT NULL,
        max_ctx INTEGER NOT NULL,
        max_body_mb INTEGER NOT NULL,
        note TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        last_used_at INTEGER,
        prefix TEXT NOT NULL UNIQUE,
        hash TEXT NOT NULL
    );",
    // 2: usage log — metadata only, never prompts, images or answers
    "CREATE TABLE usage (
        id INTEGER PRIMARY KEY,
        ts INTEGER NOT NULL,
        key_id TEXT,
        project TEXT,
        entrance TEXT NOT NULL,
        path TEXT NOT NULL,
        model TEXT,
        status INTEGER NOT NULL,
        block_reason TEXT,
        latency_ms INTEGER NOT NULL,
        ttfb_ms INTEGER,
        prompt_tokens INTEGER,
        output_tokens INTEGER
    );
    CREATE INDEX usage_ts ON usage (ts);
    CREATE INDEX usage_key_ts ON usage (key_id, ts);",
    // 3: small settings store (admin password hash)
    "CREATE TABLE settings (k TEXT PRIMARY KEY, v TEXT NOT NULL);",
];

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut conn = Connection::open(path).with_context(|| format!("opening database {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .with_context(|| format!("{} is not a usable C0RE database", path.display()))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql).with_context(|| format!("migration {}", i + 1))?;
        tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [i as i64 + 1])?;
        tx.commit()?;
    }
    Ok(())
}
