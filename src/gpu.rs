//! Live GPU readings for the dashboard, from `nvidia-smi` (installed with the NVIDIA
//! driver). One short query at most every two seconds, and only while the dashboard asks;
//! nothing runs in the background.

use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GpuStats {
    pub name: String,
    pub util_percent: Option<u32>,
    pub mem_used_mb: Option<u32>,
    pub mem_total_mb: Option<u32>,
    pub temp_c: Option<u32>,
    pub power_w: Option<u32>,
}

const QUERY: &str = "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw";
const CACHE_FOR: Duration = Duration::from_secs(2);

/// Reads the first GPU's line of `nvidia-smi --format=csv,noheader,nounits` output.
/// Fields the card doesn't report ("[N/A]") come back as None.
pub fn parse_nvidia_smi(out: &str) -> Option<GpuStats> {
    let line = out.lines().next()?;
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    if f.len() < 6 || f[0].is_empty() {
        return None;
    }
    let n = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| v.round() as u32);
    let g = GpuStats {
        name: f[0].to_string(),
        util_percent: n(f[1]),
        mem_used_mb: n(f[2]),
        mem_total_mb: n(f[3]),
        temp_c: n(f[4]),
        power_w: n(f[5]),
    };
    // A line that isn't a reading (an error message) has no numbers at all.
    (g.util_percent.is_some() || g.mem_total_mb.is_some()).then_some(g)
}

fn run_nvidia_smi() -> Option<String> {
    let mut cmd = std::process::Command::new("nvidia-smi");
    cmd.args([QUERY, "--format=csv,noheader,nounits"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Default)]
pub struct GpuMonitor {
    last: Mutex<Option<(Instant, Option<GpuStats>)>>,
}

impl GpuMonitor {
    /// Current readings, or None when there's no NVIDIA GPU or driver tool. Gives up after
    /// three seconds so a stuck driver can't hold up the dashboard.
    pub async fn read(&self) -> Option<GpuStats> {
        if let Some((at, v)) = self.last.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
            && at.elapsed() < CACHE_FOR
        {
            return v;
        }
        let read = tokio::task::spawn_blocking(|| run_nvidia_smi().and_then(|o| parse_nvidia_smi(&o)));
        let v = tokio::time::timeout(Duration::from_secs(3), read).await.ok().and_then(Result::ok).flatten();
        *self.last.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((Instant::now(), v.clone()));
        v
    }
}
