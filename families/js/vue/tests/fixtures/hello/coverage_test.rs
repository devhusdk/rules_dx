use std::path::PathBuf;
use std::process::Command;

fn lcov() -> String {
    let test = dx_testing::resolve_runfiles(&std::env::var("HELLO_TEST").expect("path env is set"));
    let root = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join("coverage_hello");
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("raw");
    std::fs::create_dir_all(&dir).expect("create coverage directory");
    let output = root.join("coverage.dat");
    let run = Command::new(test)
        .env("COVERAGE", "1")
        .env("COVERAGE_DIR", &dir)
        .env("COVERAGE_OUTPUT_FILE", &output)
        .output()
        .expect("test runs");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let report = std::fs::read_to_string(&output).unwrap_or_else(|err| panic!("no LCOV: {err}"));
    report.replace('\\', "/")
}

fn sources(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter_map(|line| line.strip_prefix("SF:"))
        .collect()
}

fn hits(report: &str, source: &str) -> u64 {
    let mut inside = false;
    let mut total = 0;
    for line in report.lines() {
        if let Some(name) = line.strip_prefix("SF:") {
            inside = name.ends_with(source);
        } else if inside {
            if let Some(count) = line.strip_prefix("DA:") {
                total += count
                    .split(',')
                    .nth(1)
                    .and_then(|hit| hit.parse::<u64>().ok())
                    .unwrap_or(0);
            }
        }
    }
    total
}

#[test]
fn hello_test_covers_the_component_and_its_helper() {
    let report = lcov();
    for source in ["hello/Hello.vue", "hello/helper.js"] {
        assert!(
            sources(&report).iter().any(|name| name.ends_with(source)),
            "{source} missing from {:?}",
            sources(&report)
        );
        assert!(hits(&report, source) > 0, "{source} has no executed lines");
    }
}
