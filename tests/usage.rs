use c0re::db;
use c0re::policy::Entrance;
use c0re::usage::{UsageRecord, UsageStore};
use std::sync::{Arc, Mutex};

fn store() -> (UsageStore, Arc<Mutex<rusqlite::Connection>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let conn = Arc::new(Mutex::new(db::open(&dir.path().join("c0re.db")).unwrap()));
    (UsageStore::new(conn.clone()), conn, dir)
}

fn rec(ts: i64, key: &str, status: u16, block: Option<&str>) -> UsageRecord {
    UsageRecord {
        ts,
        key_id: Some(key.into()),
        project: Some(format!("proj-{key}")),
        entrance: Entrance::Private,
        path: "/api/chat".into(),
        model: Some("qwen3-vl:8b-instruct".into()),
        status,
        block_reason: block.map(Into::into),
        latency_ms: 120,
        ttfb_ms: Some(40),
        prompt_tokens: Some(30),
        output_tokens: Some(12),
    }
}

#[test]
fn insert_and_recent_round_trip() {
    let (s, _c, _d) = store();
    s.insert(&rec(100, "k_a", 200, None)).unwrap();
    s.insert(&rec(200, "k_b", 429, Some("rate_limited"))).unwrap();
    let r = s.recent(10).unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0], rec(200, "k_b", 429, Some("rate_limited")), "newest first");
    assert_eq!(r[1], rec(100, "k_a", 200, None));
}

#[test]
fn count_today_respects_local_midnight() {
    let (s, _c, _d) = store();
    let offset = 8 * 3600; // UTC+8
    let day = 20_000 * 86_400_i64; // a UTC midnight
    let local_midnight = day - offset as i64; // 00:00 local, as a UTC timestamp
    s.insert(&rec(local_midnight - 10, "k_a", 200, None)).unwrap(); // yesterday locally
    s.insert(&rec(local_midnight + 10, "k_a", 200, None)).unwrap();
    s.insert(&rec(local_midnight + 20, "k_a", 200, None)).unwrap();
    s.insert(&rec(local_midnight + 30, "k_a", 429, Some("rate_limited"))).unwrap(); // not forwarded
    s.insert(&rec(local_midnight + 40, "k_b", 200, None)).unwrap(); // other key
    assert_eq!(s.count_today("k_a", local_midnight + 3600, offset).unwrap(), 2);
}

#[test]
fn prune_drops_rows_older_than_retention() {
    let (s, _c, _d) = store();
    let now = 100 * 86_400;
    s.insert(&rec(now - 91 * 86_400, "k_a", 200, None)).unwrap();
    s.insert(&rec(now - 89 * 86_400, "k_a", 200, None)).unwrap();
    let deleted = s.prune(now, 90, 100 * 1024 * 1024).unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(s.recent(10).unwrap().len(), 1);
}

#[test]
fn prune_shrinks_to_max_bytes_oldest_first() {
    let (s, c, _d) = store();
    for i in 0..20_000 {
        s.insert(&rec(1_000 + i, "k_a", 200, None)).unwrap();
    }
    let cap = 256 * 1024;
    s.prune(30_000, 90_000, cap).unwrap();
    let used: i64 = c
        .lock()
        .unwrap()
        .query_row(
            "SELECT (page_count - freelist_count) * page_size FROM pragma_page_count(), pragma_freelist_count(), pragma_page_size()",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(used as u64 <= cap, "used {used} > cap {cap}");
    let recent = s.recent(1).unwrap();
    assert_eq!(recent[0].ts, 1_000 + 19_999, "newest row kept");
}

#[test]
fn summary_groups_by_key_and_hour() {
    let (s, _c, _d) = store();
    s.insert(&rec(3600, "k_a", 200, None)).unwrap();
    s.insert(&rec(3700, "k_a", 200, None)).unwrap();
    s.insert(&rec(7300, "k_b", 403, Some("key_not_public"))).unwrap();
    let sum = s.summary(0).unwrap();
    assert_eq!(sum.total, 3);
    assert_eq!(sum.blocked, 1);
    assert_eq!(sum.by_reason, vec![("key_not_public".to_string(), 1)]);
    let a = sum.per_key.iter().find(|k| k.key_id == "k_a").unwrap();
    assert_eq!((a.requests, a.blocked, a.prompt_tokens, a.output_tokens), (2, 0, 60, 24));
    assert_eq!(sum.per_hour.iter().map(|h| (h.hour, h.requests, h.blocked)).collect::<Vec<_>>(), vec![(3600, 2, 0), (7200, 1, 1)]);
}

#[test]
fn by_entrance_counts_requests_and_blocks() {
    let (s, _c, _d) = store();
    s.insert(&rec(100, "k_a", 200, None)).unwrap();
    s.insert(&rec(101, "k_a", 403, Some("key_not_public"))).unwrap();
    let mut public = rec(102, "k_b", 200, None);
    public.entrance = Entrance::Public;
    s.insert(&public).unwrap();
    s.insert(&rec(5, "k_a", 200, None)).unwrap(); // before `since`
    let e = s.by_entrance(50).unwrap();
    assert_eq!((e.private.requests, e.private.blocked), (2, 1));
    assert_eq!((e.public.requests, e.public.blocked), (1, 0));
}

#[test]
fn schema_has_no_content_columns() {
    let (_s, c, _d) = store();
    let c = c.lock().unwrap();
    let mut st = c.prepare("SELECT name FROM pragma_table_info('usage')").unwrap();
    let cols: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    let allowed = [
        "id", "ts", "key_id", "project", "entrance", "path", "model", "status", "block_reason",
        "latency_ms", "ttfb_ms", "prompt_tokens", "output_tokens",
    ];
    for col in &cols {
        assert!(allowed.contains(&col.as_str()), "unexpected column {col}");
    }
}

#[test]
fn retention_prune_does_not_vacuum() {
    let (s, c, _d) = store();
    for i in 0..5_000 {
        s.insert(&rec(i, "k_a", 200, None)).unwrap();
    }
    s.insert(&rec(200 * 86_400, "k_a", 200, None)).unwrap();
    let deleted = s.prune(200 * 86_400, 90, 100 * 1024 * 1024).unwrap();
    assert_eq!(deleted, 5_000);
    let free: i64 = c.lock().unwrap().query_row("PRAGMA freelist_count", [], |r| r.get(0)).unwrap();
    assert!(free > 0, "VACUUM ran on a routine retention prune (it locks the database)");
}
