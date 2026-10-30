use axum::http::{HeaderMap, HeaderValue};
use c0re::keys::{Key, Reach};
use c0re::limits::*;
use c0re::policy::Entrance;
use std::net::IpAddr;
use std::time::{Duration, Instant};

fn key(id: &str, rate: u32, conc: u32, daily: u32) -> Key {
    Key {
        id: id.into(), project: "p".into(), reach: Reach::Private, models: vec![],
        rate_per_min: rate, daily_cap: daily, max_concurrent: conc, max_ctx: 8192, max_body_mb: 20,
        note: String::new(), created_at: 0, last_used_at: None,
    }
}

#[test]
fn rate_allows_burst_up_to_limit_then_429() {
    let l = Limiter::new();
    let k = key("a", 3, 1, 10);
    let t = Instant::now();
    for _ in 0..3 {
        assert!(l.check_rate(&k, t).is_ok());
    }
    let d = l.check_rate(&k, t).unwrap_err();
    assert_eq!((d.status, d.code), (429, "rate_limited"));
    assert!(d.retry_after.unwrap() >= 1);
}

#[test]
fn rate_refills_over_time() {
    let l = Limiter::new();
    let k = key("a", 60, 1, 10); // one per second
    let t = Instant::now();
    for _ in 0..60 {
        l.check_rate(&k, t).unwrap();
    }
    assert!(l.check_rate(&k, t).is_err());
    assert!(l.check_rate(&k, t + Duration::from_millis(1100)).is_ok());
}

#[test]
fn rate_is_per_key() {
    let l = Limiter::new();
    let t = Instant::now();
    l.check_rate(&key("a", 1, 1, 10), t).unwrap();
    assert!(l.check_rate(&key("a", 1, 1, 10), t).is_err());
    assert!(l.check_rate(&key("b", 1, 1, 10), t).is_ok());
}

#[test]
fn rate_follows_changed_limit() {
    let l = Limiter::new();
    let t = Instant::now();
    l.check_rate(&key("a", 1, 1, 10), t).unwrap();
    assert!(l.check_rate(&key("a", 1, 1, 10), t).is_err());
    // Admin raises the limit: the bucket grows to the new size.
    assert!(l.check_rate(&key("a", 100, 1, 10), t + Duration::from_millis(100)).is_ok());
}

#[test]
fn concurrency_slot_freed_on_drop() {
    let l = Limiter::new();
    let k = key("a", 60, 1, 10);
    let p = l.try_enter(&k).unwrap();
    let d = l.try_enter(&k).unwrap_err();
    assert_eq!((d.status, d.code), (429, "too_many_concurrent"));
    drop(p);
    assert!(l.try_enter(&k).is_ok());
}

#[test]
fn daily_cap_enforced_at_limit() {
    let k = key("a", 60, 1, 5);
    assert!(check_daily(4, &k).is_ok());
    let d = check_daily(5, &k).unwrap_err();
    assert_eq!((d.status, d.code), (429, "daily_cap"));
}

#[test]
fn failure_blocker_blocks_after_10_in_window() {
    let f = FailureBlocker::new(10, Duration::from_secs(60), Duration::from_secs(60));
    let t = Instant::now();
    for _ in 0..9 {
        f.record_failure("1.2.3.4", t);
    }
    assert!(!f.is_blocked("1.2.3.4", t));
    f.record_failure("1.2.3.4", t);
    assert!(f.is_blocked("1.2.3.4", t));
    assert!(!f.is_blocked("5.6.7.8", t));
}

#[test]
fn failures_outside_window_do_not_count() {
    let f = FailureBlocker::new(10, Duration::from_secs(60), Duration::from_secs(60));
    let t = Instant::now();
    for _ in 0..9 {
        f.record_failure("x", t);
    }
    f.record_failure("x", t + Duration::from_secs(61));
    assert!(!f.is_blocked("x", t + Duration::from_secs(61)));
}

#[test]
fn failure_blocker_unblocks_after_period() {
    let f = FailureBlocker::new(2, Duration::from_secs(60), Duration::from_secs(60));
    let t = Instant::now();
    f.record_failure("x", t);
    f.record_failure("x", t);
    assert!(f.is_blocked("x", t + Duration::from_secs(59)));
    assert!(!f.is_blocked("x", t + Duration::from_secs(61)));
}

#[test]
fn failure_blocker_keys_public_by_forwarded_for() {
    let local: IpAddr = "127.0.0.1".parse().unwrap();
    let mut a = HeaderMap::new();
    a.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.5, 100.100.100.100"));
    let mut b = HeaderMap::new();
    b.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.7"));
    let sa = FailureBlocker::source_for(Entrance::Public, local, &a);
    let sb = FailureBlocker::source_for(Entrance::Public, local, &b);
    assert_eq!(sa, "203.0.113.5");
    assert_eq!(sb, "198.51.100.7");
    assert_eq!(FailureBlocker::source_for(Entrance::Public, local, &HeaderMap::new()), "public-unknown");

    let f = FailureBlocker::new(2, Duration::from_secs(60), Duration::from_secs(60));
    let t = Instant::now();
    f.record_failure(&sa, t);
    f.record_failure(&sa, t);
    assert!(f.is_blocked(&sa, t));
    assert!(!f.is_blocked(&sb, t));
}

#[test]
fn private_source_is_peer_ip() {
    let peer: IpAddr = "100.73.112.68".parse().unwrap();
    let mut h = HeaderMap::new();
    h.insert("x-forwarded-for", HeaderValue::from_static("1.1.1.1")); // ignored on private
    assert_eq!(FailureBlocker::source_for(Entrance::Private, peer, &h), "100.73.112.68");
}
