//! Runs C0RE: binds both entrances, waits for Tailscale, prunes the usage log.

use crate::policy::Entrance;
use crate::tailscale::tailscale_ipv4;
use crate::{AppState, now_secs, proxy};
use anyhow::{Context, Result};
use axum::Router;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Private entrance: the key-protected API plus the admin area.
pub fn private_router(state: Arc<AppState>) -> Router {
    crate::admin::admin_router(state.clone()).merge(proxy::api_router(state, Entrance::Private))
}

pub fn public_router(state: Arc<AppState>) -> Router {
    proxy::api_router(state, Entrance::Public)
}

async fn bind(ip: IpAddr, port: u16, what: &str, setting: &str) -> Result<TcpListener> {
    TcpListener::bind(SocketAddr::new(ip, port)).await.with_context(|| {
        format!(
            "Cannot listen on {ip}:{port} ({what}): another program is probably using port {port}. \
             See which one in PowerShell: Get-Process -Id (Get-NetTCPConnection -LocalPort {port}).OwningProcess \
             — then close it, or set {setting} to a free port in c0re.toml"
        )
    })
}

fn spawn_serve(listener: TcpListener, router: Router, mut stop: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        let shutdown = async move {
            let _ = stop.wait_for(|s| *s).await;
        };
        if let Err(e) = axum::serve(listener, svc).with_graceful_shutdown(shutdown).await {
            tracing::error!("listener stopped: {e}");
        }
    })
}

/// Serves until `shutdown` completes. Fails fast if a port is taken.
pub async fn serve(state: Arc<AppState>, shutdown: impl Future<Output = ()>) -> Result<()> {
    let cfg = state.config();
    let local = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let private_local = bind(local, cfg.private_port, "private entrance", "private_port").await?;
    let public_local = bind(local, cfg.public_port, "public entrance", "public_port").await?;
    let (stop_tx, stop_rx) = watch::channel(false);

    let mut tasks = vec![
        spawn_serve(private_local, private_router(state.clone()), stop_rx.clone()),
        spawn_serve(public_local, public_router(state.clone()), stop_rx.clone()),
    ];
    tracing::info!("C0RE listening: private 127.0.0.1:{}, public 127.0.0.1:{}", cfg.private_port, cfg.public_port);

    // Private entrance on the Tailscale address, as soon as Tailscale is up.
    {
        let state = state.clone();
        let mut stop = stop_rx.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                let cfg = state.config();
                let runner = state.tailscale.clone();
                let ip = tokio::task::spawn_blocking(move || tailscale_ipv4(runner.as_ref())).await.ok().flatten();
                if let Some(ip) = ip {
                    match bind(IpAddr::V4(ip), cfg.private_port, "private entrance on Tailscale", "private_port").await {
                        Ok(l) => {
                            tracing::info!("private entrance on Tailscale: {ip}:{}", cfg.private_port);
                            let _ = spawn_serve(l, private_router(state.clone()), stop.clone()).await;
                            return;
                        }
                        Err(e) => tracing::warn!("{e:#}"),
                    }
                } else {
                    tracing::info!("Tailscale not up yet; private entrance only on 127.0.0.1 for now");
                }
                let wait = Duration::from_secs(cfg.tailscale_retry_secs.max(1));
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = stop.wait_for(|s| *s) => return,
                }
            }
        }));
    }

    // Usage log upkeep every hour.
    {
        let state = state.clone();
        let mut stop = stop_rx.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                let cfg = state.config();
                let usage = state.usage.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    usage.prune(now_secs(), cfg.retention_days, cfg.max_db_mb * 1024 * 1024)
                })
                .await;
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3600)) => {}
                    _ = stop.wait_for(|s| *s) => return,
                }
            }
        }));
    }

    shutdown.await;
    let _ = stop_tx.send(true);
    for t in tasks {
        let _ = tokio::time::timeout(Duration::from_secs(5), t).await;
    }
    Ok(())
}

/// Logs to `<dir>/logs/c0re.log`, rotated daily, keeping 7 files. Never logs secrets or bodies.
pub fn init_logging(dir: &Path) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("c0re")
        .filename_suffix("log")
        .max_log_files(7)
        .build(dir.join("logs"))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("C0RE_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(writer).with_ansi(false).try_init().ok();
    Ok(guard)
}
