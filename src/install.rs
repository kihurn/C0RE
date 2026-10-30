//! Windows setup: a logon task that runs C0RE in the background, and a firewall rule
//! letting only Tailscale devices reach the private entrance.

use crate::config::Config;
use crate::tailscale::CmdOutput;
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::Path;

pub const TASK_NAME: &str = "C0RE";
pub const FIREWALL_RULE: &str = "C0RE private (Tailscale)";
/// Tailscale's address range (CGNAT 100.64.0.0/10).
pub const TAILSCALE_RANGE: &str = "100.64.0.0/10";

/// Quotes a value for PowerShell (single quotes, embedded quotes doubled).
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn install_script(svc_exe: &Path, cfg: &Config) -> String {
    let exe = svc_exe.display().to_string();
    let workdir = svc_exe.parent().map(|p| p.display().to_string()).unwrap_or_default();
    format!(
        r#"$ErrorActionPreference = 'Stop'
try {{
    $user = "$env:USERDOMAIN\$env:USERNAME"
    $action = New-ScheduledTaskAction -Execute {exe} -Argument 'serve' -WorkingDirectory {workdir}
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
    $principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -MultipleInstances IgnoreNew
    Register-ScheduledTask -TaskName {task} -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'C0RE private AI gateway' -Force | Out-Null
    Write-Output 'Logon task installed: C0RE starts when you log in.'

    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {{
        Remove-NetFirewallRule -DisplayName {rule} -ErrorAction SilentlyContinue
        New-NetFirewallRule -DisplayName {rule} -Direction Inbound -Protocol TCP -LocalPort {port} -RemoteAddress '{range}' -Action Allow -Profile Any | Out-Null
        Write-Output 'Firewall rule added: only Tailscale devices can reach port {port}.'
    }} else {{
        Write-Output 'Firewall rule NOT added: this needs administrator rights.'
        Write-Output 'Right-click PowerShell, choose "Run as administrator", and run: c0re install'
    }}

    Start-ScheduledTask -TaskName {task}
    Write-Output 'C0RE is starting in the background.'
}} catch {{
    Write-Error $_
    exit 1
}}
"#,
        exe = ps_quote(&exe),
        workdir = ps_quote(&workdir),
        task = ps_quote(TASK_NAME),
        rule = ps_quote(FIREWALL_RULE),
        port = cfg.private_port,
        range = TAILSCALE_RANGE,
    )
}

pub fn uninstall_script() -> String {
    format!(
        r#"Stop-ScheduledTask -TaskName {task} -ErrorAction SilentlyContinue
Unregister-ScheduledTask -TaskName {task} -Confirm:$false -ErrorAction SilentlyContinue
Write-Output 'Logon task removed.'
Remove-NetFirewallRule -DisplayName {rule} -ErrorAction SilentlyContinue
Write-Output 'Firewall rule removed (if it existed and you have administrator rights).'
Write-Output 'Your keys and settings are kept in %LOCALAPPDATA%\C0RE.'
"#,
        task = ps_quote(TASK_NAME),
        rule = ps_quote(FIREWALL_RULE),
    )
}

/// PowerShell's `-EncodedCommand` format: base64 of UTF-16LE. Unlike piping the script on
/// stdin, this runs it as one script (so errors stop it) and keeps non-ASCII paths intact.
pub fn encoded_command(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn run_powershell(script: &str) -> Result<CmdOutput> {
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"])
        .arg(encoded_command(script))
        .output()
        .context("could not start PowerShell")?;
    Ok(CmdOutput {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

fn report(out: &mut dyn Write, r: CmdOutput) -> Result<i32> {
    write!(out, "{}", r.stdout)?;
    if r.status != 0 {
        writeln!(out, "{}", r.stderr.trim())?;
        bail!("PowerShell reported an error (exit {})", r.status);
    }
    Ok(0)
}

pub fn install(dir: &Path, out: &mut dyn Write) -> Result<i32> {
    let cfg = Config::load_or_create(dir)?;
    let svc = std::env::current_exe()?.with_file_name(if cfg!(windows) { "c0re-svc.exe" } else { "c0re-svc" });
    let script = install_script(&svc, &cfg);
    if !cfg!(windows) {
        writeln!(out, "`c0re install` sets up Windows. On this system, here is the script it would run:\n")?;
        write!(out, "{script}")?;
        return Ok(1);
    }
    if !svc.exists() {
        bail!("c0re-svc.exe must be in the same folder as c0re.exe ({}).", svc.display());
    }
    report(out, run_powershell(&script)?)
}

pub fn uninstall(out: &mut dyn Write) -> Result<i32> {
    if !cfg!(windows) {
        writeln!(out, "`c0re uninstall` is for Windows.")?;
        return Ok(1);
    }
    report(out, run_powershell(&uninstall_script())?)
}
