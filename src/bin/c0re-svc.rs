// Windowless C0RE for the logon task: same as `c0re serve`, without a console window.
#![windows_subsystem = "windows"]

fn main() {
    let code = c0re::cli::run(vec!["c0re-svc".into(), "serve".into()]).unwrap_or(1);
    std::process::exit(code);
}
