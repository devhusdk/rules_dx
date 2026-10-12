use std::path::PathBuf;

fn staged(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name the staged binary"));
    dx_testing::resolve_runfiles(&rel)
}

fn binary_arch() -> &'static str {
    std::env::consts::ARCH
}

fn normalize_arch(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "amd64" | "x86_64" | "x86-64" => "x86_64".to_owned(),
        "arm64" | "aarch64" => "aarch64".to_owned(),
        "x86" | "i386" | "i686" => "x86".to_owned(),
        _ => lower,
    }
}

#[cfg(not(windows))]
fn host_arch() -> String {
    let output = std::process::Command::new("uname")
        .arg("-m")
        .output()
        .expect("uname -m must execute");
    assert!(
        output.status.success(),
        "uname -m failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    normalize_arch(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
fn host_arch() -> String {
    if let Ok(native) = std::env::var("PROCESSOR_ARCHITEW6432") {
        return normalize_arch(&native);
    }
    normalize_arch(
        &std::env::var("PROCESSOR_ARCHITECTURE")
            .unwrap_or_else(|_| panic!("PROCESSOR_ARCHITECTURE must be set")),
    )
}

fn report() -> String {
    format!(
        "preflight host={} host_arch={} binary_arch={}",
        std::env::consts::OS,
        host_arch(),
        binary_arch()
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
    let binary = binary_arch();
    assert!(
        matches!(binary, "x86_64" | "aarch64"),
        "unexpected binary arch {binary}"
    );
    let host = host_arch();
    assert!(
        matches!(host.as_str(), "x86_64" | "aarch64"),
        "unexpected host arch {host}"
    );
    assert_eq!(
        host.as_str(),
        binary,
        "emulated execution: host {host} differs from binary {binary}"
    );
    #[cfg(windows)]
    assert!(
        std::env::var("PROCESSOR_ARCHITEW6432").is_err(),
        "emulated execution under WOW64"
    );
    println!("{}", report());
}
