//! Usage log: one row per request, metadata only.

use crate::policy::Entrance;
use anyhow::Result;
use rusqlite::{Connection, Row, params};
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsageRecord {
    pub ts: i64,
    pub key_id: Option<String>,
    pub project: Option<String>,
    pub entrance: Entrance,
    pub path: String,
    pub model: Option<String>,
    pub status: u16,
    pub block_reason: Option<String>,
    pub latency_ms: u32,
    pub ttfb_ms: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyUsage {
    pub key_id: String,
    pub project: String,
    pub requests: u32,
    pub blocked: u32,
    pub avg_latency_ms: u32,
    pub prompt_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HourUsage {
    /// Start of the hour, Unix seconds (UTC).
    pub hour: i64,
    pub requests: u32,
    pub blocked: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub requests: u32,
    pub blocked: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct EntranceCounts {
    pub private: Counts,
    pub public: Counts,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub total: u32,
    pub blocked: u32,
    pub by_reason: Vec<(String, u32)>,
    pub per_key: Vec<KeyUsage>,
    pub per_hour: Vec<HourUsage>,
}

#[derive(Clone)]
pub struct UsageStore {
    conn: Arc<Mutex<Connection>>,
}

const COLS: &str = "ts, key_id, project, entrance, path, model, status, block_reason, latency_ms, ttfb_ms, prompt_tokens, output_tokens";

fn from_row(r: &Row) -> rusqlite::Result<UsageRecord> {
    let entrance: String = r.get(3)?;
    Ok(UsageRecord {
        ts: r.get(0)?,
        key_id: r.get(1)?,
        project: r.get(2)?,
        entrance: if entrance == "public" { Entrance::Public } else { Entrance::Private },
        path: r.get(4)?,
        model: r.get(5)?,
        status: r.get(6)?,
        block_reason: r.get(7)?,
        latency_ms: r.get(8)?,
        ttfb_ms: r.get(9)?,
        prompt_tokens: r.get(10)?,
        output_tokens: r.get(11)?,
    })
}

fn used_bytes(c: &Connection) -> Result<u64> {
    let page_size: i64 = c.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: i64 = c.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let free: i64 = c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
    Ok(((pages - free).max(0) * page_size) as u64)
}

impl UsageStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        UsageStore { conn }
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, r: &UsageRecord) -> Result<()> {
        self.db().execute(
            &format!("INSERT INTO usage ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
            params![
                r.ts,
                r.key_id,
                r.project,
                r.entrance.as_str(),
                r.path,
                r.model,
                r.status,
                r.block_reason,
                r.latency_ms,
                r.ttfb_ms,
                r.prompt_tokens,
                r.output_tokens
            ],
        )?;
        Ok(())
    }

    /// Requests forwarded to the engine for this key since local midnight.
    pub fn count_today(&self, key_id: &str, now: i64, utc_offset_secs: i32) -> Result<u32> {
        let off = utc_offset_secs as i64;
        let midnight = (now + off).div_euclid(86_400) * 86_400 - off;
        Ok(self.db().query_row(
            "SELECT COUNT(*) FROM usage WHERE key_id = ?1 AND ts >= ?2 AND block_reason IS NULL",
            params![key_id, midnight],
            |r| r.get(0),
        )?)
    }

    /// Deletes rows past retention, then oldest rows until the database fits `max_bytes`.
    pub fn prune(&self, now: i64, retention_days: u32, max_bytes: u64) -> Result<u64> {
        let c = self.db();
        let mut deleted = c.execute("DELETE FROM usage WHERE ts < ?1", [now - retention_days as i64 * 86_400])? as u64;
        // Freed pages are reused by new rows, so routine retention needs no VACUUM (which would
        // hold the database lock). Only shrink the file when it is over its size cap.
        let mut over_cap = false;
        while used_bytes(&c)? > max_bytes {
            over_cap = true;
            let rows: i64 = c.query_row("SELECT COUNT(*) FROM usage", [], |r| r.get(0))?;
            if rows == 0 {
                break;
            }
            let batch = (rows / 10).max(100);
            deleted += c.execute(
                "DELETE FROM usage WHERE id IN (SELECT id FROM usage ORDER BY ts, id LIMIT ?1)",
                [batch],
            )? as u64;
        }
        if over_cap {
            c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        }
        Ok(deleted)
    }

    pub fn recent(&self, limit: u32) -> Result<Vec<UsageRecord>> {
        let c = self.db();
        let mut st = c.prepare(&format!("SELECT {COLS} FROM usage ORDER BY ts DESC, id DESC LIMIT ?1"))?;
        let rows = st.query_map([limit], from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn by_entrance(&self, since: i64) -> Result<EntranceCounts> {
        let c = self.db();
        let mut st = c.prepare(
            "SELECT entrance, COUNT(*), COALESCE(SUM(block_reason IS NOT NULL), 0) FROM usage WHERE ts >= ?1 GROUP BY entrance",
        )?;
        let mut out = EntranceCounts::default();
        for row in st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?)))? {
            let (e, requests, blocked) = row?;
            let slot = if e == "public" { &mut out.public } else { &mut out.private };
            *slot = Counts { requests, blocked };
        }
        Ok(out)
    }

    pub fn summary(&self, since: i64) -> Result<Summary> {
        let c = self.db();
        let (total, blocked): (u32, u32) = c.query_row(
            "SELECT COUNT(*), COALESCE(SUM(block_reason IS NOT NULL), 0) FROM usage WHERE ts >= ?1",
            [since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut st = c.prepare(
            "SELECT block_reason, COUNT(*) FROM usage WHERE ts >= ?1 AND block_reason IS NOT NULL
             GROUP BY block_reason ORDER BY COUNT(*) DESC, block_reason",
        )?;
        let by_reason = st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare(
            "SELECT key_id, COALESCE(MAX(project), ''), COUNT(*), COALESCE(SUM(block_reason IS NOT NULL), 0),
                    COALESCE(CAST(AVG(CASE WHEN block_reason IS NULL THEN latency_ms END) AS INTEGER), 0),
                    COALESCE(SUM(prompt_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM usage WHERE ts >= ?1 AND key_id IS NOT NULL GROUP BY key_id ORDER BY COUNT(*) DESC",
        )?;
        let per_key = st
            .query_map([since], |r| {
                Ok(KeyUsage {
                    key_id: r.get(0)?,
                    project: r.get(1)?,
                    requests: r.get(2)?,
                    blocked: r.get(3)?,
                    avg_latency_ms: r.get(4)?,
                    prompt_tokens: r.get::<_, i64>(5)? as u64,
                    output_tokens: r.get::<_, i64>(6)? as u64,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut st = c.prepare(
            "SELECT (ts / 3600) * 3600 AS h, COUNT(*), COALESCE(SUM(block_reason IS NOT NULL), 0)
             FROM usage WHERE ts >= ?1 GROUP BY h ORDER BY h",
        )?;
        let per_hour = st
            .query_map([since], |r| Ok(HourUsage { hour: r.get(0)?, requests: r.get(1)?, blocked: r.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Summary { total, blocked, by_reason, per_key, per_hour })
    }
}
