//! Re-pins the Maven lock shared by every nested consumer (manual, needs network).
//!
//! Writes the new lock to the undeclared outputs and fails while the committed lock is stale.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use dx_nested::{CONSUMER_MAVEN, CONSUMER_MAVEN_LOCK_ENV};

const PIN_DEADLINE: Duration = Duration::from_secs(1800);

fn tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(200)..].join("\n")
}

#[test]
fn committed_consumer_maven_lock_matches_a_fresh_pin() {
    let key = std::env::var(CONSUMER_MAVEN_LOCK_ENV).expect("lock env var");
    let committed = dx_testing::read_text(&dx_testing::rlocation(&key).expect("rlocation"));

    let case = dx_nested::Case::consumer("consumer_maven_pin", CONSUMER_MAVEN);
    case.write("BUILD.bazel", "");
    case.write("maven_install.json", &committed);
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
