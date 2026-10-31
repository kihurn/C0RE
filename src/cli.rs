//! The `c0re` command-line tool. Works directly on the PC's data folder; no login needed.

use crate::config::{Config, KeyDefaults, data_dir};
use crate::keys::{Key, KeyPatch, NewKey, Reach, Revealed};
use crate::tailscale::{FunnelState, SystemRunner, funnel_off, funnel_on, funnel_status, tailscale_ipv4};
use crate::{AppState, admin, now_secs};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "c0re", version, about = "C0RE: your private AI gateway")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the gateway in this window (Ctrl+C to stop)
    Serve,
    /// Start C0RE automatically when you log in, and allow Tailscale devices through the firewall
    Install,
    /// Undo `install`
    Uninstall,
    /// Admin dashboard password
    Admin {
        #[command(subcommand)]
        cmd: AdminCmd,
    },
    /// Create and manage project keys
    Keys {
        #[command(subcommand)]
        cmd: KeysCmd,
    },
    /// Turn the public internet address (Tailscale Funnel) on or off
    Public {
        #[command(subcommand)]
        cmd: PublicCmd,
    },
    /// Requests per key over the last days
    Usage {
        #[arg(long)]
        key: Option<String>,
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        json: bool,
    },
    /// Engine, entrances and public address at a glance
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AdminCmd {
    /// Set or change the dashboard password
    SetPassword {
        /// Read the password from standard input instead of prompting
        #[arg(long)]
        stdin: bool,
    },
}

#[derive(Args, Default)]
struct LimitArgs {
    /// Requests per minute
    #[arg(long)]
    rate: Option<u32>,
    /// Requests per day
    #[arg(long)]
    daily: Option<u32>,
    /// Requests running at the same time
    #[arg(long)]
    concurrent: Option<u32>,
    /// Largest context (num_ctx) the key may ask for
    #[arg(long)]
    max_ctx: Option<u32>,
    /// Largest request in MB
    #[arg(long)]
    max_body_mb: Option<u32>,
}

#[derive(Subcommand)]
enum KeysCmd {
    /// Create a key
    Create {
        #[arg(long)]
        project: String,
        /// private, public or off
        #[arg(long, default_value = "private")]
        reach: Reach,
        /// Allowed models, comma-separated (default: all)
        #[arg(long, value_delimiter = ',')]
        models: Vec<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[command(flatten)]
        limits: LimitArgs,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    /// Switch a key off
    Off { id: String },
    /// Switch a key back on (private network only, unless --public)
    On {
        id: String,
        #[arg(long)]
        public: bool,
    },
    /// Show a key's secret again
    Show { id: String },
    Delete { id: String },
    /// Change a key's project name, reach, models or limits
    Edit {
        id: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        reach: Option<Reach>,
        #[arg(long, value_delimiter = ',')]
        models: Option<Vec<String>>,
        #[arg(long)]
        note: Option<String>,
        #[command(flatten)]
        limits: LimitArgs,
    },
}

#[derive(Subcommand)]
enum PublicCmd {
    On,
    Off,
    Status,
}

/// Entry point used by the binaries.
pub fn run(args: Vec<String>) -> Result<i32> {
    let stdout = std::io::stdout();
    let stdin = std::io::stdin();
    run_in(&data_dir(), args, &mut stdout.lock(), &mut stdin.lock())
}

fn open(dir: &Path) -> Result<Arc<AppState>> {
    let cfg = Config::load_or_create(dir)?;
    AppState::open(dir, Arc::new(SystemRunner::new(cfg.tailscale_bin)))
}

/// Runs a command against the data folder `dir`, writing to `out` and reading from `input`.
pub fn run_in(dir: &Path, args: Vec<String>, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<i32> {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            write!(out, "{e}")?;
            return Ok(if e.use_stderr() { 2 } else { 0 });
        }
    };
    match cli.cmd {
        Cmd::Serve => serve(dir),
        Cmd::Install => crate::install::install(dir, out),
        Cmd::Uninstall => crate::install::uninstall(out),
        Cmd::Admin { cmd: AdminCmd::SetPassword { stdin } } => set_password(dir, stdin, out, input),
        Cmd::Keys { cmd } => keys(dir, cmd, out),
        Cmd::Public { cmd } => public(dir, cmd, out),
        Cmd::Usage { key, days, json } => usage(dir, key, days, json, out),
        Cmd::Status { json } => status(dir, json, out),
    }
}

pub const STARTUP_ERROR_FILE: &str = "startup-error.txt";

/// Runs the gateway. If it cannot start, the reason goes to the log and to
/// `startup-error.txt`, because the background service has no window to show it in.
fn serve(dir: &Path) -> Result<i32> {
    std::fs::create_dir_all(dir)?;
    let _guard = crate::server::init_logging(dir).ok();
    let note = dir.join(STARTUP_ERROR_FILE);
    let _ = std::fs::remove_file(&note);
    let result = (|| -> Result<()> {
        let state = open(dir)?;
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(crate::server::serve(state, async {
            if tokio::signal::ctrl_c().await.is_err() {
                std::future::pending::<()>().await; // no Ctrl+C handler: run until killed
            }
        }))
    })();
    if let Err(e) = &result {
        tracing::error!("C0RE could not start: {e:#}");
        let _ = std::fs::write(&note, format!("{} C0RE could not start: {e:#}\n", chrono::Local::now().to_rfc3339()));
    }
    result.map(|_| 0)
}

fn set_password(dir: &Path, from_stdin: bool, out: &mut dyn Write, input: &mut dyn BufRead) -> Result<i32> {
    let password = if from_stdin {
        let mut line = String::new();
        input.read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        let a = rpassword::prompt_password("New admin password (10+ characters): ")?;
        let b = rpassword::prompt_password("Type it again: ")?;
        if a != b {
            bail!("The two passwords did not match.");
        }
        a
    };
    let state = open(dir)?;
    admin::set_password(&state, &password)?;
    writeln!(out, "Admin password set. Any open dashboard sessions were logged out.")?;
    Ok(0)
}

fn limits(base: KeyDefaults, l: &LimitArgs) -> KeyDefaults {
    KeyDefaults {
        rate_per_min: l.rate.unwrap_or(base.rate_per_min),
        daily_cap: l.daily.unwrap_or(base.daily_cap),
        max_concurrent: l.concurrent.unwrap_or(base.max_concurrent),
        max_ctx: l.max_ctx.unwrap_or(base.max_ctx),
        max_body_mb: l.max_body_mb.unwrap_or(base.max_body_mb),
    }
}

fn print_keys(keys: &[Key], out: &mut dyn Write) -> Result<()> {
    if keys.is_empty() {
        writeln!(out, "No keys yet. Create one with: c0re keys create --project <name> --reach private")?;
        return Ok(());
    }
    writeln!(out, "{:<9} {:<20} {:<8} {:>6} {:>7} {:>4}  MODELS", "ID", "PROJECT", "REACH", "/MIN", "/DAY", "CONC")?;
    for k in keys {
        let models = if k.models.is_empty() { "all".to_string() } else { k.models.join(", ") };
        writeln!(
            out,
            "{:<9} {:<20} {:<8} {:>6} {:>7} {:>4}  {}",
            k.id,
            k.project,
            k.reach.as_str(),
            k.rate_per_min,
            k.daily_cap,
            k.max_concurrent,
            models
        )?;
    }
    Ok(())
}

fn keys(dir: &Path, cmd: KeysCmd, out: &mut dyn Write) -> Result<i32> {
    let state = open(dir)?;
    let cfg = state.config();
    match cmd {
        KeysCmd::Create { project, reach, models, note, limits: l } => {
            let base = if reach == Reach::Public { cfg.public_defaults } else { cfg.private_defaults };
            let (key, secret) = state.keys.create(NewKey { project, reach, models, limits: Some(limits(base, &l)), note })?;
            writeln!(out, "Created key {} for '{}' ({}).", key.id, key.project, key.reach.as_str())?;
            writeln!(out)?;
            writeln!(out, "    {secret}")?;
            writeln!(out)?;
            if key.revealable {
                writeln!(out, "Projects send it as 'Authorization: Bearer <key>'. See it again with: c0re keys show {}", key.id)?;
            } else {
                writeln!(out, "Copy it now: it won't be shown again. Projects send it as 'Authorization: Bearer <key>'.")?;
            }
        }
        KeysCmd::List { json } => {
            let keys = state.keys.list()?;
            if json {
                writeln!(out, "{}", serde_json::to_string_pretty(&keys)?)?;
            } else {
                print_keys(&keys, out)?;
            }
        }
        KeysCmd::Off { id } => edit(&state, &id, KeyPatch { reach: Some(Reach::Off), ..Default::default() }, out)?,
        KeysCmd::On { id, public } => {
            let reach = if public { Reach::Public } else { Reach::Private };
            edit(&state, &id, KeyPatch { reach: Some(reach), ..Default::default() }, out)?
        }
        KeysCmd::Show { id } => match state.keys.reveal(&id)? {
            Revealed::Secret(secret) => writeln!(out, "{secret}")?,
            Revealed::NotSaved => bail!(
                "Key {id} can't be shown again (it was made before C0RE 0.2, or on another Windows account). \
                 Delete it and make a new one."
            ),
            Revealed::NotFound => bail!("No key with id {id}."),
        },
        KeysCmd::Delete { id } => {
            if !state.keys.delete(&id)? {
                bail!("No key with id {id}.");
            }
            writeln!(out, "Deleted key {id}.")?;
        }
        KeysCmd::Edit { id, project, reach, models, note, limits: l } => {
            let patch = KeyPatch {
                project,
                reach,
                models,
                note,
                rate_per_min: l.rate,
                daily_cap: l.daily,
                max_concurrent: l.concurrent,
                max_ctx: l.max_ctx,
                max_body_mb: l.max_body_mb,
            };
            edit(&state, &id, patch, out)?
        }
    }
    Ok(0)
}

fn edit(state: &AppState, id: &str, patch: KeyPatch, out: &mut dyn Write) -> Result<()> {
    let k = state.keys.update(id, patch)?.with_context(|| format!("No key with id {id}."))?;
    writeln!(out, "Updated {}: '{}' is now {}.", k.id, k.project, k.reach.as_str())?;
    Ok(())
}

fn describe_funnel(f: &FunnelState) -> Value {
    match f {
        FunnelState::On { url } => json!({ "state": "on", "url": url }),
        FunnelState::Off => json!({ "state": "off" }),
        FunnelState::Unavailable(r) => json!({ "state": "unavailable", "reason": r }),
    }
}

fn public(dir: &Path, cmd: PublicCmd, out: &mut dyn Write) -> Result<i32> {
    let state = open(dir)?;
    let c = state.config();
    let r = state.tailscale.as_ref();
    match cmd {
        PublicCmd::On => {
            let url = funnel_on(r, c.public_port, c.funnel_port)?;
            writeln!(out, "Public address is ON: {url}")?;
            writeln!(out, "Only keys with reach 'public' work there.")?;
        }
        PublicCmd::Off => {
            funnel_off(r, c.funnel_port)?;
            writeln!(out, "Public address is OFF.")?;
        }
        PublicCmd::Status => match funnel_status(r, c.public_port, c.funnel_port) {
            FunnelState::On { url } => writeln!(out, "Public address is ON: {url}")?,
            FunnelState::Off => writeln!(out, "Public address is OFF.")?,
            FunnelState::Unavailable(why) => writeln!(out, "Public address unavailable: {why}")?,
        },
    }
    Ok(0)
}

fn usage(dir: &Path, key: Option<String>, days: u32, json_out: bool, out: &mut dyn Write) -> Result<i32> {
    let state = open(dir)?;
    let days = days.clamp(1, 90);
    let mut sum = state.usage.summary(now_secs() - days as i64 * 86_400)?;
    if let Some(k) = &key {
        sum.per_key.retain(|u| &u.key_id == k);
    }
    if json_out {
        writeln!(out, "{}", serde_json::to_string_pretty(&json!({ "days": days, "summary": sum }))?)?;
        return Ok(0);
    }
    writeln!(out, "Last {days} day(s): {} requests, {} blocked.", sum.total, sum.blocked)?;
    if !sum.per_key.is_empty() {
        writeln!(out, "\n{:<9} {:<20} {:>8} {:>8} {:>9} {:>10} {:>10}", "KEY", "PROJECT", "REQS", "BLOCKED", "AVG MS", "TOK IN", "TOK OUT")?;
        for u in &sum.per_key {
            writeln!(
                out,
                "{:<9} {:<20} {:>8} {:>8} {:>9} {:>10} {:>10}",
                u.key_id, u.project, u.requests, u.blocked, u.avg_latency_ms, u.prompt_tokens, u.output_tokens
            )?;
        }
    }
    if !sum.by_reason.is_empty() {
        writeln!(out, "\nBlocked by reason:")?;
        for (reason, n) in &sum.by_reason {
            writeln!(out, "  {reason:<22} {n}")?;
        }
    }
    Ok(0)
}

fn status(dir: &Path, json_out: bool, out: &mut dyn Write) -> Result<i32> {
    let state = open(dir)?;
    let c = state.config();
    let version = engine_version(&c.ollama_url);
    let ts_ip = tailscale_ipv4(state.tailscale.as_ref());
    let funnel = funnel_status(state.tailscale.as_ref(), c.public_port, c.funnel_port);
    let v = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": dir.display().to_string(),
        "engine": { "online": version.is_some(), "url": c.ollama_url, "version": version },
        "private_port": c.private_port,
        "public_port": c.public_port,
        "tailscale_ip": ts_ip.map(|ip| ip.to_string()),
        "public": describe_funnel(&funnel),
        "keys": state.keys.list()?.len(),
        "admin_password_set": admin::auth::password_set(&state)?,
    });
    if json_out {
        writeln!(out, "{}", serde_json::to_string_pretty(&v)?)?;
        return Ok(0);
    }
    writeln!(out, "C0RE {}", env!("CARGO_PKG_VERSION"))?;
    match &v["engine"]["version"] {
        Value::String(ver) => writeln!(out, "Engine:   online (Ollama {ver}) at {}", c.ollama_url)?,
        _ => writeln!(out, "Engine:   OFFLINE — open Ollama on this PC ({})", c.ollama_url)?,
    }
    match ts_ip {
        Some(ip) => writeln!(out, "Private:  http://{ip}:{p} (Tailscale) and http://127.0.0.1:{p}", p = c.private_port)?,
        None => writeln!(out, "Private:  http://127.0.0.1:{} only — Tailscale is not running", c.private_port)?,
    }
    match funnel {
        FunnelState::On { url } => writeln!(out, "Public:   ON at {url}")?,
        FunnelState::Off => writeln!(out, "Public:   off")?,
        FunnelState::Unavailable(why) => writeln!(out, "Public:   unavailable ({why})")?,
    }
    writeln!(out, "Keys:     {}", v["keys"])?;
    if v["admin_password_set"] == false {
        writeln!(out, "Admin:    no password yet — run: c0re admin set-password")?;
    }
    Ok(0)
}

fn engine_version(base: &str) -> Option<String> {
    let url = format!("{}/api/version", base.trim_end_matches('/'));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    rt.block_on(async {
        let r = reqwest::Client::new().get(url).timeout(Duration::from_secs(3)).send().await.ok()?;
        let v: Value = r.json().await.ok()?;
        v.get("version").and_then(Value::as_str).map(str::to_string)
    })
}
