use std::path::PathBuf;

use dx_testing::{Run, mkscratch, resolve_runfiles, run};

const FAKE_XCRUN: &str = r#"#!/bin/sh
printf '%s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    lipo)
        printf '%s\n' "$FAKE_ARCHS"
        ;;
    otool)
        cat "$FAKE_LOADS"
        ;;
esac
"#;

const SIM_LOADS: &str = "libledger.a(ledger.o):
Load command 1
      cmd LC_BUILD_VERSION
  cmdsize 24
 platform 7
    minos 14.0
      sdk 18.5
libledger.a(core.o):
Load command 1
      cmd LC_BUILD_VERSION
 platform 7
    minos 17.0
";

struct Fake {
    root: PathBuf,
    xcrun: String,
}

impl Fake {
    fn new(prefix: &str) -> Fake {
        let root = mkscratch(prefix).unwrap_or_else(|error| panic!("scratch: {error}"));
        let xcrun = root.join("xcrun");
        dx_testing::executable_script(&xcrun, FAKE_XCRUN);
        Fake {
            xcrun: xcrun.to_string_lossy().into_owned(),
            root,
        }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.root.join("log.txt")).unwrap_or_default()
    }
}

fn script(var: &str) -> PathBuf {
    resolve_runfiles(&std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a script")))
}

fn fails(ran: &Run, code: i32, stderr: &str) {
    assert_eq!(ran.status.code(), Some(code), "{}", ran.combined());
    assert_eq!(ran.stderr, format!("{stderr}\n"), "{}", ran.combined());
}

fn check(fake: &Fake, archs: &str, loads: &str) -> (Run, PathBuf) {
    check_inputs(fake, archs, loads, &["/out/libledger.a"])
}

fn check_inputs(fake: &Fake, archs: &str, loads: &str, inputs: &[&str]) -> (Run, PathBuf) {
    check_cell(
        fake,
        ["simulator/arm64", "iphonesimulator", "7", "arm64"],
        archs,
        loads,
        inputs,
    )
}

fn check_cell(
    fake: &Fake,
    cell: [&str; 4],
    archs: &str,
    loads: &str,
    inputs: &[&str],
) -> (Run, PathBuf) {
    check_kind(fake, ["apple_ios_app", "17.0"], cell, archs, loads, inputs)
}

fn check_kind(
    fake: &Fake,
    kind: [&str; 2],
    cell: [&str; 4],
    archs: &str,
    loads: &str,
    inputs: &[&str],
) -> (Run, PathBuf) {
    let loads_file = fake.root.join("loads.txt");
    std::fs::write(&loads_file, loads).unwrap_or_else(|error| panic!("write: {error}"));
    let bin = fake.root.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let linked = bin.join("xcrun");
    if !linked.exists() {
        std::os::unix::fs::symlink(&fake.xcrun, &linked)
            .unwrap_or_else(|error| panic!("symlink: {error}"));
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let report = fake.root.join("cell.txt");
    let log = fake.path("log.txt");
    let loads_path = loads_file.to_string_lossy().into_owned();
    let report_path = report.to_string_lossy().into_owned();
    let mut args = vec![kind[0], report_path.as_str()];
    args.extend_from_slice(&cell);
    args.extend_from_slice(&[kind[1], "16.4.0.16F6", "18.5"]);
    args.extend_from_slice(inputs);
    let ran = run(
        &script("DX_LIBRARY_CHECK"),
        &args,
        &[
            ("DX_APPLE_TOOL_PATH", path.as_str()),
            ("FAKE_LOG", log.as_str()),
            ("FAKE_ARCHS", archs),
            ("FAKE_LOADS", loads_path.as_str()),
        ],
    )
    .unwrap_or_else(|error| panic!("run: {error}"));
    (ran, report)
}

#[test]
fn a_simulator_archive_records_its_cell() {
    let fake = Fake::new("apple-cell-ok");
    let (ran, report) = check(&fake, "arm64", SIM_LOADS);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        std::fs::read_to_string(&report).unwrap_or_default(),
        "cell=simulator/arm64\nsdk=iphonesimulator\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libledger.a arch=arm64 platform=iphonesimulator minos=17.0\n"
    );
    assert_eq!(
        fake.log(),
        "lipo -archs /out/libledger.a\notool -arch arm64 -l /out/libledger.a\n"
    );
}

#[test]
fn wrong_library_slices_fail_without_a_report() {
    let fake = Fake::new("apple-cell-bad");
    let cases = [
        (
            "x86_64",
            SIM_LOADS.to_owned(),
            "apple_ios_app: library archive libledger.a carries x86_64, want arm64 for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("platform 7\n    minos 17.0", "platform 2\n    minos 17.0"),
            "apple_ios_app: library archive libledger.a is built for iphoneos, want iphonesimulator for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("platform 7", "platform 1"),
            "apple_ios_app: library archive libledger.a is built for macosx, want iphonesimulator for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("minos 17.0", "minos 18.2"),
            "apple_ios_app: library archive libledger.a needs iOS 18.2, above the app deployment 17.0",
        ),
        (
            "arm64",
            "libledger.a(ledger.o):\n".to_owned(),
            "apple_ios_app: library archive libledger.a declares no build platform; rebuild it for iphonesimulator",
        ),
    ];
    for (archs, loads, want) in cases {
        let (ran, report) = check(&fake, archs, &loads);
        fails(&ran, 1, want);
        assert!(!report.exists(), "{want}: a rejected cell leaves no report");
    }
}

#[test]
fn an_imported_framework_binary_records_its_cell() {
    let fake = Fake::new("apple-cell-framework");
    let (ran, report) = check_inputs(
        &fake,
        "x86_64 arm64",
        SIM_LOADS,
        &["/out/libledger.a", "/vendor/Share.framework/Share"],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        std::fs::read_to_string(&report).unwrap_or_default(),
        "cell=simulator/arm64\nsdk=iphonesimulator\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libledger.a arch=arm64 platform=iphonesimulator minos=17.0\nframework=Share arch=arm64 platform=iphonesimulator minos=17.0\n"
    );
    assert_eq!(
        fake.log(),
        "lipo -archs /out/libledger.a\notool -arch arm64 -l /out/libledger.a\nlipo -archs /vendor/Share.framework/Share\notool -arch arm64 -l /vendor/Share.framework/Share\n"
    );
}

#[test]
fn a_newer_framework_binary_fails_the_app_deployment() {
    let fake = Fake::new("apple-cell-framework-new");
    let (ran, report) = check_inputs(
        &fake,
        "arm64",
        &SIM_LOADS.replace("minos 17.0", "minos 18.0"),
        &["/vendor/Share.framework/Share"],
    );
    fails(
        &ran,
        1,
        "apple_ios_app: framework binary Share needs iOS 18.0, above the app deployment 17.0",
    );
    assert!(!report.exists(), "a rejected framework leaves no report");
}

#[test]
fn a_macos_archive_records_its_cell_and_names_macos() {
    let fake = Fake::new("apple-cell-macos");
    let loads = SIM_LOADS
        .replace("platform 7", "platform 1")
        .replace("minos 17.0", "minos 14.0");
    let cell = ["macos/arm64", "macosx", "1", "arm64"];
    let (ran, report) = check_kind(
        &fake,
        ["apple_macos_app", "14.0"],
        cell,
        "arm64",
        &loads,
        &["/out/libledger.a"],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(
        std::fs::read_to_string(&report)
            .unwrap_or_default()
            .ends_with("archive=libledger.a arch=arm64 platform=macosx minos=14.0\n"),
        "{}",
        ran.combined()
    );

    std::fs::remove_file(&report).unwrap_or_else(|error| panic!("rm: {error}"));
    let (ran, report) = check_kind(
        &fake,
        ["apple_macos_app", "14.0"],
        cell,
        "arm64",
        &loads.replace("minos 14.0", "minos 15.0"),
        &["/out/libledger.a"],
    );
    fails(
        &ran,
        1,
        "apple_macos_app: library archive libledger.a needs macOS 15.0, above the app deployment 14.0",
    );
    assert!(!report.exists(), "a rejected cell leaves no report");
}

const LEGACY_LOADS: &str = "libledger.a(ledger.o):
Load command 1
      cmd LC_VERSION_MIN_IPHONEOS
  cmdsize 16
  version 10.0
      sdk 17.5
";

#[test]
fn legacy_version_min_archives_resolve_by_arch() {
    let fake = Fake::new("apple-cell-legacy");
    let (ran, report) = check_cell(
        &fake,
        ["device/arm64", "iphoneos", "2", "arm64"],
        "arm64",
        LEGACY_LOADS,
        &["/out/libledger.a"],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(
        std::fs::read_to_string(&report)
            .unwrap_or_default()
            .ends_with("archive=libledger.a arch=arm64 platform=iphoneos minos=10.0\n"),
        "{}",
        ran.combined()
    );

    let (ran, report) = check_cell(
        &fake,
        ["simulator/x86_64", "iphonesimulator", "7", "x86_64"],
        "x86_64",
        LEGACY_LOADS,
        &["/out/libledger.a"],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(
        std::fs::read_to_string(&report)
            .unwrap_or_default()
            .ends_with("archive=libledger.a arch=x86_64 platform=iphonesimulator minos=10.0\n"),
        "{}",
        ran.combined()
    );

    std::fs::remove_file(&report).unwrap_or_else(|error| panic!("rm: {error}"));
    let (ran, report) = check(&fake, "arm64", LEGACY_LOADS);
    fails(
        &ran,
        1,
        "apple_ios_app: library archive libledger.a is built for iphoneos, want iphonesimulator for simulator/arm64",
    );
    assert!(!report.exists(), "a rejected cell leaves no report");

    let (ran, report) = check(
        &fake,
        "arm64",
        &LEGACY_LOADS.replace("LC_VERSION_MIN_IPHONEOS", "LC_VERSION_MIN_MACOSX"),
    );
    fails(
        &ran,
        1,
        "apple_ios_app: library archive libledger.a is built for macosx, want iphonesimulator for simulator/arm64",
    );
    assert!(!report.exists(), "a rejected cell leaves no report");
}

#[test]
fn a_library_cell_records_its_cell_for_apple_ios_lib() {
    let fake = Fake::new("apple-cell-lib");
    let loads = SIM_LOADS.replace("platform 7", "platform 2");
    let cell = ["device/arm64", "iphoneos", "2", "arm64"];
    let (ran, report) = check_kind(
        &fake,
        ["apple_ios_lib", "17.0"],
        cell,
        "arm64",
        &loads,
        &["/out/libshare.a"],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        std::fs::read_to_string(&report).unwrap_or_default(),
        "cell=device/arm64\nsdk=iphoneos\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libshare.a arch=arm64 platform=iphoneos minos=17.0\n"
    );

    std::fs::remove_file(&report).unwrap_or_else(|error| panic!("rm: {error}"));
    let (ran, report) = check_kind(
        &fake,
        ["apple_ios_lib", "17.0"],
        cell,
        "arm64",
        &loads.replace("minos 17.0", "minos 18.0"),
        &["/out/libshare.a"],
    );
    fails(
        &ran,
        1,
        "apple_ios_lib: library archive libshare.a needs iOS 18.0, above the lib deployment 17.0",
    );
    assert!(!report.exists(), "a rejected cell leaves no report");
}
