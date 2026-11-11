//! Stands in for `bazel`, `uv` and `python` in `dx generate` tests; the name it
//! runs as picks the behaviour.

use std::path::Path;

fn write(path: &str, text: &str) {
    std::fs::write(path, text).expect("write file");
}

fn bazel(args: &[String]) {
    if let Ok(intended) = std::env::var("DX_GENERATE_INTENDED") {
        if !intended.is_empty() {
            let mode = std::env::var("DX_GENERATE_MODE").unwrap_or_else(|_| "default".into());
            write(
                &intended,
                &format!(
                    "{{\"mode\":\"{mode}\",\"result\":\"clean\",\"scopes\":[{{\"value\":\"//...\",\"results_complete\":true}}],\"files\":[],\"ignored_imports\":[]}}"
                ),
            );
        }
    }
    if std::env::var("CARGO_BAZEL_REPIN").is_ok_and(|v| !v.is_empty()) {
        write("Cargo.lock", "# lock\n");
    }
    let joined = format!(" {} ", args.join(" "));
    if joined.contains("pnpm") {
        write("pnpm-lock.yaml", "lockfileVersion: 9\n");
    } else if joined.contains(" uv ") || joined.contains("uv_lock") || joined.contains("/uv") {
        write("uv.lock", "version = 1\n");
    }
}

fn uv(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("--version") => println!("uv 0.13.7"),
        Some("lock") => {
            let empty = std::fs::metadata("uv.lock").map_or(true, |m| m.len() == 0);
            if empty {
                write("uv.lock", "version = 1\n");
            }
        }
        _ => {}
    }
}

fn main() {
    let mut argv = std::env::args();
    let program = argv.next().unwrap_or_default();
    let args: Vec<String> = argv.collect();
    let name = Path::new(&program)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match name {
        "bazel" => bazel(&args),
        "uv" => uv(&args),
        _ => {}
    }
}
