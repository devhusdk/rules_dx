use std::path::{Path, PathBuf};

const DX_USAGE: &str = "dx <command> [flags] [scope ...] [-- bazel-options ...]";

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

fn parse_sysctl_flag(raw: Option<&str>) -> Option<bool> {
    match raw?.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn macos_host_arch(arm64: Option<bool>, uname: &str) -> String {
    if arm64 == Some(true) {
        "aarch64".to_owned()
    } else {
        normalize_arch(uname)
    }
}

#[cfg(target_os = "macos")]
fn sysctl(key: &str) -> Option<String> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", key])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(windows))]
fn uname_m() -> String {
    let output = std::process::Command::new("uname")
        .arg("-m")
        .output()
        .expect("uname -m must execute");
    assert!(
        output.status.success(),
        "uname -m failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(target_os = "macos")]
fn host_arch() -> String {
    macos_host_arch(
        parse_sysctl_flag(sysctl("hw.optional.arm64").as_deref()),
        &uname_m(),
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
fn host_arch() -> String {
    normalize_arch(&uname_m())
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

fn translated() -> bool {
    #[cfg(target_os = "macos")]
    {
        parse_sysctl_flag(sysctl("sysctl.proc_translated").as_deref()) == Some(true)
    }
    #[cfg(windows)]
    {
        std::env::var("PROCESSOR_ARCHITEW6432").is_ok()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}

fn staged_machine(path: &Path) -> String {
    use std::io::Read as _;
    let mut head = Vec::with_capacity(4096);
    std::fs::File::open(path)
        .and_then(|file| file.take(4096).read_to_end(&mut head))
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    dx_testing::image_machine(&head)
        .unwrap_or_else(|| panic!("{} is not an ELF, PE or thin Mach-O image", path.display()))
}

fn assert_native(name: &str, path: &Path) {
    let machine = staged_machine(path);
    assert!(
        machine == host_arch() && machine == binary_arch(),
        "{}: staged {name} at {} is {machine}",
        report(),
        path.display()
    );
}

fn report() -> String {
    format!(
        "preflight host={} translated={} host_arch={} binary_arch={}",
        std::env::consts::OS,
        translated(),
        host_arch(),
        binary_arch()
    )
}

#[test]
fn dx_builds_and_executes() {
    let dx = staged("DX_PREFLIGHT_DX");
    assert_native("dx", &dx);
    let run = dx_testing::run(&dx, &["--help"], &[]).expect("dx must execute");
    assert!(
        run.status.success(),
        "{}: dx --help failed: {}",
        report(),
        run.combined()
    );
    assert!(
        run.stdout
            .lines()
            .any(|line| line.trim_end_matches('\r') == DX_USAGE),
        "{}: dx --help printed no {DX_USAGE:?} line: {}",
        report(),
        run.combined()
    );
    println!("{}", report());
}

/// `--help` returns before the host check, so it cannot catch a released
/// binary that refuses its own host. `completion` needs no workspace and runs
/// after `dx_plan::platform::host_refusal`.
#[test]
fn dx_passes_host_check() {
    let dx = staged("DX_PREFLIGHT_DX");
    let scratch = dx_testing::mkscratch("preflight-host").expect("scratch dir");
    let output = dx_testing::command_for(&dx, &[])
        .expect("dx command")
        .args(["completion", "bash"])
        .current_dir(&scratch)
        .output()
        .expect("dx must execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("unsupported_platform") && !stderr.contains("unsupported_platform"),
        "{}: dx refused its own host: {stdout}{stderr}",
        report()
    );
    assert!(
        output.status.success(),
        "{}: dx completion bash failed: {stdout}{stderr}",
        report()
    );
    assert!(!stdout.trim().is_empty(), "{}: empty completion", report());
    println!("{}", report());
}

#[test]
fn native_fixture_builds_and_runs() {
    let hello = staged("DX_PREFLIGHT_HELLO");
    assert_native("hello", &hello);
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
    assert!(!translated(), "emulated execution: {}", report());
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert_eq!(
        parse_sysctl_flag(sysctl("hw.optional.arm64").as_deref()),
        Some(true),
        "hw.optional.arm64 must read 1 on Apple silicon"
    );
    println!("{}", report());
}

#[test]
fn sysctl_flags_parse() {
    assert_eq!(parse_sysctl_flag(Some("1\n")), Some(true));
    assert_eq!(parse_sysctl_flag(Some("0")), Some(false));
    assert_eq!(parse_sysctl_flag(Some("")), None);
    assert_eq!(parse_sysctl_flag(None), None);
    assert_eq!(parse_sysctl_flag(Some("garbage")), None);
}

#[test]
fn macos_host_arch_prefers_hw_optional_arm64() {
    assert_eq!(macos_host_arch(Some(true), "x86_64"), "aarch64");
    assert_eq!(macos_host_arch(None, "arm64"), "aarch64");
    assert_eq!(macos_host_arch(Some(false), "x86_64"), "x86_64");
}
