mod common;

use c0re::AppState;
use c0re::tailscale::*;
use common::FakeRunner;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const STATUS_ON: &str = r#"{"TCP":{"8443":{"HTTPS":true}},"Web":{"kihurn-desktop.tail1234.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:7071"}}}},"AllowFunnel":{"kihurn-desktop.tail1234.ts.net:8443":true}}"#;

fn err_out(stderr: &str) -> std::io::Result<CmdOutput> {
    Ok(CmdOutput { status: 1, stdout: String::new(), stderr: stderr.into() })
}

#[test]
fn parses_tailscale_ip() {
    let r = FakeRunner::new(|_| FakeRunner::ok("100.94.2.98\n"));
    assert_eq!(tailscale_ipv4(r.as_ref()), Some("100.94.2.98".parse().unwrap()));
    assert_eq!(r.calls.lock().unwrap()[0], vec!["ip", "-4"]);
}

#[test]
fn no_ip_when_tailscale_down() {
    let r = FakeRunner::new(|_| err_out("Tailscale is stopped."));
    assert_eq!(tailscale_ipv4(r.as_ref()), None);
    let r = FakeRunner::new(|_| FakeRunner::ok("not an ip"));
    assert_eq!(tailscale_ipv4(r.as_ref()), None);
}

#[test]
fn funnel_status_on_off_parsing() {
    let r = FakeRunner::new(|_| FakeRunner::ok(STATUS_ON));
    assert_eq!(
        funnel_status(r.as_ref(), 7071, 8443),
        FunnelState::On { url: "https://kihurn-desktop.tail1234.ts.net:8443".into() }
    );
    assert_eq!(r.calls.lock().unwrap()[0], vec!["funnel", "status", "--json"]);
    let r = FakeRunner::new(|_| FakeRunner::ok("{}"));
    assert_eq!(funnel_status(r.as_ref(), 7071, 8443), FunnelState::Off);
    // Funnel pointing somewhere else is not C0RE's public entrance.
    let other = STATUS_ON.replace("7071", "3000");
    let r = FakeRunner::new(move |_| FakeRunner::ok(&other));
    assert_eq!(funnel_status(r.as_ref(), 7071, 8443), FunnelState::Off);
}

#[test]
fn funnel_on_builds_exact_command() {
    let r = FakeRunner::new(|_| FakeRunner::ok("Available on the internet:\n\nhttps://kihurn-desktop.tail1234.ts.net:8443/\n|-- proxy http://127.0.0.1:7071\n"));
    let url = funnel_on(r.as_ref(), 7071, 8443).unwrap();
    assert_eq!(url, "https://kihurn-desktop.tail1234.ts.net:8443");
    assert_eq!(r.calls.lock().unwrap()[0], vec!["funnel", "--bg", "--https=8443", "http://127.0.0.1:7071"]);
}

#[test]
fn funnel_on_explains_https_not_enabled() {
    let r = FakeRunner::new(|_| err_out("error: HTTPS certificates are not enabled for this tailnet"));
    let e = funnel_on(r.as_ref(), 7071, 8443).unwrap_err().to_string();
    assert!(e.contains("HTTPS certificates"), "{e}");
    assert!(e.contains("DNS"), "{e}");
    let r = FakeRunner::new(|_| err_out("Funnel not available; \"funnel\" node attribute not set"));
    let e = funnel_on(r.as_ref(), 7071, 8443).unwrap_err().to_string();
    assert!(e.contains("Funnel permission"), "{e}");
}

#[test]
fn funnel_off_builds_exact_command() {
    let r = FakeRunner::new(|_| FakeRunner::ok(""));
    funnel_off(r.as_ref(), 8443).unwrap();
    assert_eq!(r.calls.lock().unwrap()[0], vec!["funnel", "--https=8443", "off"]);
}

#[test]
fn tailscale_cli_missing_makes_funnel_unavailable() {
    let r = FakeRunner::new(|_| Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found")));
    assert!(matches!(funnel_status(r.as_ref(), 7071, 8443), FunnelState::Unavailable(m) if m.contains("not installed")));
    assert!(funnel_on(r.as_ref(), 7071, 8443).is_err());
}

#[test]
fn corrupt_db_refuses_to_start() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("c0re.db"), vec![0x42u8; 8192]).unwrap();
    assert!(AppState::open(dir.path(), FakeRunner::new(|_| FakeRunner::ok(""))).is_err());
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn health(addr: &str) -> bool {
    reqwest::Client::new()
        .get(format!("http://{addr}/health"))
        .timeout(Duration::from_millis(300))
        .send()
        .await
        .is_ok_and(|r| r.status() == 200)
}

#[tokio::test]
async fn serves_both_entrances_and_stops_on_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path(), FakeRunner::new(|_| err_out("stopped"))).unwrap();
    let (pp, up) = (free_port(), free_port());
    {
        let mut c = state.cfg.write().unwrap();
        c.private_port = pp;
        c.public_port = up;
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(c0re::server::serve(state, async { let _ = rx.await; }));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(health(&format!("127.0.0.1:{pp}")).await);
    assert!(health(&format!("127.0.0.1:{up}")).await);
    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task).await.unwrap().unwrap().unwrap();
}

#[tokio::test]
async fn binds_tailscale_ip_when_it_appears() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    let runner = FakeRunner::new(move |args| {
        if args == ["ip", "-4"] && c2.fetch_add(1, Ordering::SeqCst) == 0 {
            return err_out("Tailscale is stopped.");
        }
        FakeRunner::ok("127.0.0.2\n")
    });
    let state = AppState::open(dir.path(), runner).unwrap();
    let pp = free_port();
    {
        let mut c = state.cfg.write().unwrap();
        c.private_port = pp;
        c.public_port = free_port();
        c.tailscale_retry_secs = 1;
    }
    let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(c0re::server::serve(state, async { let _ = rx.await; }));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!health(&format!("127.0.0.2:{pp}")).await, "bound before Tailscale was up");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(health(&format!("127.0.0.2:{pp}")).await, "Tailscale address never bound");
}

#[tokio::test]
async fn port_in_use_fails_with_port_in_message() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::open(dir.path(), FakeRunner::new(|_| err_out("stopped"))).unwrap();
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    {
        let mut c = state.cfg.write().unwrap();
        c.private_port = port;
        c.public_port = free_port();
    }
    let err = c0re::server::serve(state, std::future::pending()).await.unwrap_err().to_string();
    assert!(err.contains(&port.to_string()), "{err}");
    // Says how to find the program holding the port and how to move C0RE off it.
    assert!(err.contains(&format!("Get-NetTCPConnection -LocalPort {port}")), "{err}");
    assert!(err.contains("private_port"), "{err}");
}
