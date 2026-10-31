//! Per-key rate, concurrency and daily limits, and per-source blocking of failed keys.

use crate::keys::Key;
use crate::policy::{Denied, Entrance};
use axum::http::HeaderMap;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct Bucket {
    capacity: f64,
    tokens: f64,
    last: Instant,
}

#[derive(Default)]
pub struct Limiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    slots: Mutex<HashMap<String, (u32, Arc<Semaphore>)>>,
}

impl Limiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Token bucket: `rate_per_min` requests may burst at once, refilling evenly over a minute.
    pub fn check_rate(&self, key: &Key, now: Instant) -> Result<(), Denied> {
        let cap = key.rate_per_min.max(1) as f64;
        let per_sec = cap / 60.0;
        let mut map = self.buckets.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let b = map.entry(key.id.clone()).or_insert(Bucket { capacity: cap, tokens: cap, last: now });
        if b.capacity != cap {
            b.tokens = (b.tokens + cap - b.capacity).clamp(0.0, cap);
            b.capacity = cap;
        }
        let elapsed = now.saturating_duration_since(b.last).as_secs_f64();
        b.tokens = (b.tokens + elapsed * per_sec).min(cap);
        b.last = now;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            Ok(())
        } else {
            let wait = ((1.0 - b.tokens) / per_sec).ceil().max(1.0) as u64;
            Err(Denied::new(429, "rate_limited", format!("Too many requests for this key. Try again in {wait} s."))
                .with_retry_after(wait))
        }
    }

    /// Takes one of the key's concurrent slots; the slot is released when the permit drops.
    pub fn try_enter(&self, key: &Key) -> Result<OwnedSemaphorePermit, Denied> {
        let limit = key.max_concurrent.max(1);
        let sem = {
            let mut map = self.slots.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let entry = map.entry(key.id.clone()).or_insert_with(|| (limit, Arc::new(Semaphore::new(limit as usize))));
            if entry.0 != limit {
                *entry = (limit, Arc::new(Semaphore::new(limit as usize)));
            }
            entry.1.clone()
        };
        sem.try_acquire_owned().map_err(|_| {
            Denied::new(429, "too_many_concurrent", "This key already has the maximum number of requests running.")
                .with_retry_after(1)
        })
    }
}

pub fn check_daily(used_today: u32, key: &Key) -> Result<(), Denied> {
    if used_today >= key.daily_cap {
        Err(Denied::new(429, "daily_cap", "This key has used its requests for today.").with_retry_after(3600))
    } else {
        Ok(())
    }
}

struct Failures {
    count: u32,
    window_start: Instant,
    blocked_until: Option<Instant>,
}

pub struct FailureBlocker {
    max_failures: u32,
    window: Duration,
    block_for: Duration,
    map: Mutex<HashMap<String, Failures>>,
}

const MAX_TRACKED_SOURCES: usize = 10_000;

impl FailureBlocker {
    pub fn new(max_failures: u32, window: Duration, block_for: Duration) -> Self {
        FailureBlocker { max_failures, window, block_for, map: Mutex::new(HashMap::new()) }
    }

    /// Who to blame for a failed key. On the public entrance every connection comes from
    /// Tailscale Funnel on 127.0.0.1, so the real client is the first `X-Forwarded-For` entry.
    pub fn source_for(e: Entrance, peer: IpAddr, headers: &HeaderMap) -> String {
        match e {
            Entrance::Private => peer.to_string(),
            Entrance::Public => headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "public-unknown".into()),
        }
    }

    pub fn is_blocked(&self, source: &str, now: Instant) -> bool {
        let map = self.map.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(source).and_then(|f| f.blocked_until).is_some_and(|until| now < until)
    }

    pub fn record_failure(&self, source: &str, now: Instant) {
        let mut map = self.map.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() >= MAX_TRACKED_SOURCES {
            let (window, block_for) = (self.window, self.block_for);
            map.retain(|_, f| {
                now.saturating_duration_since(f.window_start) < window.max(block_for)
                    || f.blocked_until.is_some_and(|u| now < u)
            });
        }
        let f = map.entry(source.to_string()).or_insert(Failures { count: 0, window_start: now, blocked_until: None });
        if now.saturating_duration_since(f.window_start) >= self.window {
            f.count = 0;
            f.window_start = now;
        }
        f.count += 1;
        if f.count >= self.max_failures {
            f.blocked_until = Some(now + self.block_for);
            f.count = 0;
            f.window_start = now;
        }
    }
}
