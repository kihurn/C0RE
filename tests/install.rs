use c0re::config::Config;
use c0re::install::{encoded_command, install_script, uninstall_script};
use std::path::Path;

#[test]
fn install_registers_logon_task_with_restart() {
    let s = install_script(Path::new(r"C:\Tools\C0RE\c0re-svc.exe"), &Config::default());
    assert!(s.contains("Register-ScheduledTask"));
    assert!(s.contains("-TaskName 'C0RE'"));
    assert!(s.contains("New-ScheduledTaskTrigger -AtLogOn"));
    assert!(s.contains("-Argument 'serve'"));
    assert!(s.contains("-RestartCount 3"));
    assert!(s.contains("-RestartInterval (New-TimeSpan -Minutes 1)"));
    assert!(s.contains("-ExecutionTimeLimit ([TimeSpan]::Zero)"));
    assert!(s.contains("-AllowStartIfOnBatteries"));
    assert!(s.contains("Start-ScheduledTask -TaskName 'C0RE'"));
}

#[test]
fn firewall_rule_limited_to_tailscale_range_and_7080() {
    let s = install_script(Path::new(r"C:\c0re-svc.exe"), &Config::default());
    assert!(s.contains("New-NetFirewallRule -DisplayName 'C0RE private (Tailscale)'"));
    assert!(s.contains("-LocalPort 7080"));
    assert!(s.contains("-RemoteAddress '100.64.0.0/10'"));
    assert!(s.contains("-Direction Inbound") && s.contains("-Protocol TCP") && s.contains("-Action Allow"));
    assert!(!s.contains("7071"), "public entrance must never get a firewall opening");
    // Replaces an older rule rather than stacking duplicates.
    assert!(s.find("Remove-NetFirewallRule").unwrap() < s.find("New-NetFirewallRule").unwrap());
    // Without admin rights it explains instead of failing silently.
    assert!(s.contains("administrator"));
}

#[test]
fn uses_configured_private_port() {
    let mut c = Config::default();
    c.private_port = 9123;
    assert!(install_script(Path::new(r"C:\c0re-svc.exe"), &c).contains("-LocalPort 9123"));
}

#[test]
fn paths_with_spaces_and_quotes_are_quoted() {
    let s = install_script(Path::new(r"C:\Program Files\C0RE's\c0re-svc.exe"), &Config::default());
    assert!(s.contains(r"-Execute 'C:\Program Files\C0RE''s\c0re-svc.exe'"), "{s}");
}

#[test]
fn uninstall_removes_task_and_rule() {
    let s = uninstall_script();
    assert!(s.contains("Unregister-ScheduledTask -TaskName 'C0RE' -Confirm:$false"));
    assert!(s.contains("Remove-NetFirewallRule -DisplayName 'C0RE private (Tailscale)'"));
    assert!(s.contains("Stop-ScheduledTask"));
}

#[test]
fn install_script_fails_loudly() {
    let s = install_script(Path::new(r"C:\c0re-svc.exe"), &Config::default());
    assert!(s.trim_start().starts_with("$ErrorActionPreference = 'Stop'"));
    assert!(s.contains("try {"), "{s}");
    assert!(s.contains("catch {"));
    assert!(s.contains("exit 1"));
}

#[test]
fn encoded_command_is_base64_utf16le() {
    assert_eq!(encoded_command("a"), "YQA=");
    // Non-ASCII paths survive (no console code page involved).
    assert_eq!(encoded_command("é"), "6QA=");
}
