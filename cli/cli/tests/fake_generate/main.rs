//! Stands in for `bazel`, `uv` and `python` in `dx generate` tests; the name it
//! runs as picks the behaviour.

use std::path::Path;

fn write(path: &str, text: &str) {
    std::fs::write(path, text).expect("write file");
}

/// Runs the gazelle binary named by `DX_FAKE_GAZELLE` for a `dx:generate` or `dx:generate_check` argument; false when it does not apply.
fn gazelle(args: &[String]) -> bool {
    let Some(bin) = std::env::var("DX_FAKE_GAZELLE")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return false;
    };
    let Some(target) = args
        .iter()
        .find(|a| a.ends_with("dx:generate") || a.ends_with("dx:generate_check"))
    else {
        return false;
    };
    let cwd = std::env::current_dir().expect("current dir");
    let mut command = std::process::Command::new(bin);
    command.arg(format!("-repo_root={}", cwd.display()));
    let excludes = std::env::var("DX_FAKE_GAZELLE_ARGS").unwrap_or_default();
    command.args(excludes.split(' ').filter(|a| !a.is_empty()));
    if target.ends_with("dx:generate_check") {
        command.arg("-mode=diff");
    }
    if let Some(split) = args.iter().position(|a| a == "--") {
        command.args(&args[split + 1..]);
    }
    let status = command.status().expect("run gazelle");
    std::process::exit(status.code().unwrap_or(1));
}

fn bazel(args: &[String]) {
    gazelle(args);
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
