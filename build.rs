// Make sure the embedded dashboard folder exists so the crate always compiles,
// even before `npm run build` has been run in web/.
fn main() {
    let dist = std::path::Path::new("web/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        std::fs::create_dir_all(dist).expect("create web/dist");
        std::fs::write(
            &index,
            "<!doctype html><title>C0RE</title><p>Dashboard not built. Run <code>npm run build</code> in web/.</p>",
        )
        .expect("write placeholder");
    }
    println!("cargo:rerun-if-changed=web/dist");
}
