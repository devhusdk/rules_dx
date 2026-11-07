use std::path::PathBuf;
use std::process::Command;

fn path(var: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&std::env::var(var).expect("path env is set"))
}

fn lcov(var: &str) -> String {
    let root = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set"))
        .join(format!("coverage_{var}"));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("raw");
    std::fs::create_dir_all(&dir).expect("create coverage directory");
    let output = root.join("coverage.dat");
    let run = Command::new(path(var))
        .env("COVERAGE", "1")
        .env("COVERAGE_DIR", &dir)
        .env("COVERAGE_OUTPUT_FILE", &output)
        .output()
        .expect("test runs");
    assert!(
        run.status.success(),
        "{var}: {}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let report =
        std::fs::read_to_string(&output).unwrap_or_else(|err| panic!("{var} wrote no LCOV: {err}"));
    assert!(
        report.contains("SF:"),
        "{var} wrote LCOV without sources: {}",
        String::from_utf8_lossy(&run.stderr)
    );
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
fn registry_test_covers_the_registry_and_the_generated_module() {
    let report = lcov("REGISTRY_TEST");
    for source in ["registry/registry.js", "registry/build_info.js"] {
        assert!(
            sources(&report).iter().any(|name| name.ends_with(source)),
            "{source} missing from {:?}",
            sources(&report)
        );
        assert!(hits(&report, source) > 0, "{source} has no executed lines");
    }
}

#[test]
fn cli_test_covers_the_cli() {
    let report = lcov("CLI_TEST");
    assert!(
        sources(&report)
            .iter()
            .any(|name| name.ends_with("cli/cli.js")),
        "cli.js missing from {:?}",
        sources(&report)
    );
    assert!(
        hits(&report, "cli/cli.js") > 0,
        "cli.js has no executed lines"
    );
}
