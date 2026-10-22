use dx_testing::{mkscratch, resolve_runfiles, run};

#[test]
fn a_linux_executor_never_stands_in_for_macos() {
    let probe = resolve_runfiles(&std::env::var("DX_APPLE_PROBE").unwrap_or_default());
    let out = mkscratch("apple-probe")
        .unwrap_or_else(|error| panic!("scratch: {error}"))
        .join("report.txt");
    let report = out.to_string_lossy().into_owned();
    let ran = run(
        &probe,
        &[
            report.as_str(),
            "managed",
            "macos_arm64",
            "16.4.0.16F6",
            "iphonesimulator",
            "18.5",
            "17.0",
            "14.5",
            "/usr/bin/cc",
            "grpcs://mac.example.com",
        ],
        &[],
    )
    .unwrap_or_else(|error| panic!("run probe: {error}"));
    assert_eq!(ran.status.code(), Some(1), "{}", ran.combined());
    let arch = std::env::consts::ARCH;
    assert_eq!(
        ran.stderr,
        format!("apple: probe ran on Linux {arch}, not a macOS executor; the managed route needs macos_arm64 with Xcode 16.4.0.16F6\n")
    );
    assert!(!out.exists(), "a failed probe leaves no report");
}
