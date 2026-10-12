use std::path::PathBuf;

fn staged(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name the staged binary"));
    dx_testing::resolve_runfiles(&rel)
}

fn report() -> String {
    format!(
        "preflight host={} arch={}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[test]
fn dx_builds_and_executes() {
    let dx = staged("DX_PREFLIGHT_DX");
    let run = dx_testing::run(&dx, &["--help"], &[]).expect("dx must execute");
    assert!(
        run.status.success(),
        "{}: dx --help failed: {}",
        report(),
        run.combined()
    );
    assert!(
        run.stdout.contains("dx"),
        "{}: dx --help printed no dx usage: {}",
        report(),
        run.combined()
    );
    println!("{}", report());
}

#[test]
fn native_fixture_builds_and_runs() {
    let hello = staged("DX_PREFLIGHT_HELLO");
    let run = dx_testing::run(&hello, &[], &[]).expect("hello must execute");
    assert!(
        run.status.success(),
        "{}: hello failed: {}",
        report(),
        run.combined()
    );
    assert_eq!(
        run.stdout.trim_end_matches(['\r', '\n']),
        "hello 42",
        "{}: unexpected hello output",
        report()
    );
    println!("{}", report());
}

#[test]
fn reports_host_and_binary_architecture() {
    assert!(
        matches!(std::env::consts::ARCH, "x86_64" | "aarch64"),
        "unexpected native arch {}",
        std::env::consts::ARCH
    );
    println!("{}", report());
}
