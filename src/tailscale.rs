//! Wrapper around the `tailscale` command-line tool: the PC's Tailscale address and
//! turning the public address (Funnel) on and off.

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs the tailscale CLI; faked in tests.
pub trait Runner: Send + Sync {
    fn run(&self, args: &[&str]) -> std::io::Result<CmdOutput>;
}

pub struct SystemRunner {
    pub bin: String,
}

impl Runner for SystemRunner {
    fn run(&self, args: &[&str]) -> std::io::Result<CmdOutput> {
        let mut cmd = std::process::Command::new(&self.bin);
        cmd.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let out = cmd.output()?;
        Ok(CmdOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum FunnelState {
    On { url: String },
    Off,
    Unavailable(String),
}

fn not_installed(e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        "Tailscale is not installed (or not on PATH) on this PC.".into()
    } else {
        format!("Could not run tailscale: {e}")
    }
}

pub fn tailscale_ipv4(r: &dyn Runner) -> Option<Ipv4Addr> {
    let out = r.run(&["ip", "-4"]).ok()?;
    if out.status != 0 {
        return None;
    }
    out.stdout.lines().find_map(|l| l.trim().parse().ok())
}

pub fn funnel_status(r: &dyn Runner, public_port: u16, funnel_port: u16) -> FunnelState {
    let out = match r.run(&["funnel", "status", "--json"]) {
        Ok(o) => o,
        Err(e) => return FunnelState::Unavailable(not_installed(&e)),
    };
    if out.status != 0 {
        return FunnelState::Unavailable(out.stderr.trim().to_string());
    }
    let Ok(v) = serde_json::from_str::<Value>(&out.stdout) else { return FunnelState::Off };
    let target = format!("127.0.0.1:{public_port}");
    let suffix = format!(":{funnel_port}");
    let allowed = v.get("AllowFunnel").and_then(Value::as_object);
    let web = v.get("Web").and_then(Value::as_object);
    for (host_port, on) in allowed.into_iter().flatten() {
        if on.as_bool() != Some(true) || !host_port.ends_with(&suffix) {
            continue;
        }
        let proxies_to_us = web
            .and_then(|w| w.get(host_port))
            .and_then(|h| h.get("Handlers"))
            .and_then(Value::as_object)
            .is_some_and(|hs| {
                hs.values().any(|h| h.get("Proxy").and_then(Value::as_str).is_some_and(|p| p.contains(&target)))
            });
        if proxies_to_us {
            return FunnelState::On { url: format!("https://{host_port}") };
        }
    }
    FunnelState::Off
}

fn explain(stderr: &str) -> String {
    let s = stderr.to_lowercase();
    let hint = if s.contains("https") || s.contains("certificate") {
        "Enable HTTPS certificates for your tailnet: Tailscale admin console → DNS → HTTPS Certificates."
    } else if s.contains("funnel") && (s.contains("attribute") || s.contains("not available") || s.contains("policy") || s.contains("not enabled")) {
        "Give this PC the Funnel permission: Tailscale admin console → Access controls → add the \"funnel\" node attribute for this machine."
    } else {
        "Tailscale refused to turn on the public address."
    };
    format!("{hint} (tailscale said: {})", stderr.trim())
}

/// Turns the public address on and returns its URL.
pub fn funnel_on(r: &dyn Runner, public_port: u16, funnel_port: u16) -> Result<String> {
    let https = format!("--https={funnel_port}");
    let target = format!("http://127.0.0.1:{public_port}");
    let out = r.run(&["funnel", "--bg", &https, &target]).map_err(|e| anyhow!(not_installed(&e)))?;
    if out.status != 0 {
        bail!(explain(&out.stderr));
    }
    out.stdout
        .split_whitespace()
        .find(|w| w.starts_with("https://"))
        .map(|u| u.trim_end_matches('/').to_string())
        .ok_or_else(|| anyhow!("Public address turned on, but Tailscale did not report its URL."))
}

pub fn funnel_off(r: &dyn Runner, funnel_port: u16) -> Result<()> {
    let https = format!("--https={funnel_port}");
    let out = r.run(&["funnel", &https, "off"]).map_err(|e| anyhow!(not_installed(&e)))?;
    if out.status != 0 {
        bail!("Could not turn the public address off: {}", out.stderr.trim());
    }
    Ok(())
}
