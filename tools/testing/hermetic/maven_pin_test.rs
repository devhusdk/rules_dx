//! Re-pins a consumer's Maven lock inside its materialized workspace (manual, needs network).
//!
//! Writes the new lock to the undeclared outputs and fails while the committed lock is stale.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

const PIN_DEADLINE: Duration = Duration::from_secs(1800);

fn tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(200)..].join("\n")
}

#[test]
fn committed_maven_lock_matches_a_fresh_pin() {
    let rel =
        std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST names the manifest");
    let text = std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("manifest reads");
    let base = std::env::current_dir().expect("runfiles cwd");
    let manifest = hermetic::parse_manifest(&text, &base).expect("manifest parses");
    let committed = manifest
        .files
        .iter()
        .find(|(_, dest)| dest == "maven_install.json")
        .map(|(src, _)| dx_testing::read_text(src))
        .expect("the manifest maps a maven_install.json");
    let source = hermetic::source_tree(&manifest).expect("source tree");

    let case = dx_nested::case("cpp_rust_pin");
    hermetic::materialize(&manifest, case.root(), &source, "cc").expect("materializes");
    let log_path = case.root().with_extension("pin.log");
    let log = File::create(&log_path).expect("pin log");
    let mut command = case.bazel_command(&["run", "--show_progress_rate_limit=5", "@maven//:pin"]);
    command
        .env("REPIN", "1")
        .env("RJE_VERBOSE", "1")
        .stdout(Stdio::from(log.try_clone().expect("clone log")))
        .stderr(Stdio::from(log));
    let mut child = command.spawn().expect("spawn pin");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait pin") {
            break status;
        }
        if started.elapsed() > PIN_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "pin exceeded {}s; last output:\n{}",
                PIN_DEADLINE.as_secs(),
                tail(&log_path)
            );
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    assert!(status.success(), "{}", tail(&log_path));

    let pinned = dx_testing::read_text(&case.root().join("maven_install.json"));
    if let Ok(out) = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).expect("outputs dir");
        std::fs::write(out.join("maven_install.json"), &pinned).expect("write pinned lock");
    }
    hermetic::lock_drift(&committed, &pinned).unwrap_or_else(|err| panic!("{err}"));
}
