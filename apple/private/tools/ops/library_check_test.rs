//! Tests the library cell check.

mod fake;

use dx_apple_ops::library_check::run;
use dx_apple_ops::xcode::{Tools, XCRUN};
use dx_test_scratch::scratch;
use fake::{Fake, args, fail, ok};
use std::path::PathBuf;

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

const LEGACY_LOADS: &str = "libledger.a(ledger.o):
Load command 1
      cmd LC_VERSION_MIN_IPHONEOS
  cmdsize 16
  version 10.0
      sdk 17.5
";

const SIM: [&str; 4] = ["simulator/arm64", "iphonesimulator", "7", "arm64"];

struct Checked {
    outcome: Result<u8, String>,
    report: PathBuf,
    log: Vec<String>,
    _root: dx_test_scratch::TempDir,
}

fn check(
    kind: &str,
    deployment: &str,
    cell: [&str; 4],
    archs: &str,
    loads: &str,
    inputs: &[&str],
) -> Checked {
    let archs = archs.to_owned();
    let loads = loads.to_owned();
    let fake = Fake::new(move |cmd| {
        assert_eq!(cmd.program, XCRUN);
        match cmd.args.first().map(String::as_str) {
            Some("lipo") if archs.is_empty() => fail(1, "not a Mach-O"),
            Some("lipo") => ok(&format!("{archs}\n")),
            Some("otool") => ok(&loads),
            other => panic!("unexpected {other:?}"),
        }
    });
    let root = scratch("library-check");
    let report = root.path().join("cell.txt");
    let text = report.to_string_lossy().into_owned();
    let mut argv = vec![kind, text.as_str()];
    argv.extend_from_slice(&cell);
    argv.extend_from_slice(&[deployment, "16.4.0.16F6", "18.5"]);
    argv.extend_from_slice(inputs);
    let tools = Tools {
        developer_dir: None,
        runner: &fake,
    };
    let outcome = run(&tools, &args(&argv)).map_err(|failure| failure.message);
    Checked {
        outcome,
        report,
        log: fake.log(),
        _root: root,
    }
}

fn rejects(checked: &Checked, want: &str) {
    assert_eq!(
        checked.outcome.as_ref().err().map(String::as_str),
        Some(want)
    );
    assert!(
        !checked.report.exists(),
        "{want}: a rejected cell leaves no report"
    );
}

fn written(checked: &Checked) -> String {
    assert_eq!(checked.outcome, Ok(0));
    std::fs::read_to_string(&checked.report).unwrap_or_default()
}

#[test]
fn a_simulator_archive_records_its_cell() {
    let checked = check(
        "apple_ios_app",
        "17.0",
        SIM,
        "arm64",
        SIM_LOADS,
        &["/out/libledger.a"],
    );
    assert_eq!(
        written(&checked),
        "cell=simulator/arm64\nsdk=iphonesimulator\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libledger.a arch=arm64 platform=iphonesimulator minos=17.0\n"
    );
    assert_eq!(
        checked.log,
        [
            "lipo -archs /out/libledger.a",
            "otool -arch arm64 -l /out/libledger.a"
        ]
    );
}

#[test]
fn wrong_library_slices_fail_without_a_report() {
    let cases = [
        (
            "x86_64",
            SIM_LOADS.to_owned(),
            "library archive libledger.a carries x86_64, want arm64 for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("platform 7\n    minos 17.0", "platform 2\n    minos 17.0"),
            "library archive libledger.a is built for iphoneos, want iphonesimulator for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("platform 7", "platform 1"),
            "library archive libledger.a is built for macosx, want iphonesimulator for simulator/arm64",
        ),
        (
            "arm64",
            SIM_LOADS.replace("minos 17.0", "minos 18.2"),
            "library archive libledger.a needs iOS 18.2, above the app deployment 17.0",
        ),
        (
            "arm64",
            "libledger.a(ledger.o):\n".to_owned(),
            "library archive libledger.a declares no build platform; rebuild it for iphonesimulator",
        ),
        (
            "",
            SIM_LOADS.to_owned(),
            "library archive libledger.a is not a Mach-O file",
        ),
    ];
    for (archs, loads, want) in cases {
        let checked = check(
            "apple_ios_app",
            "17.0",
            SIM,
            archs,
            &loads,
            &["/out/libledger.a"],
        );
        rejects(&checked, want);
    }
}

#[test]
fn an_imported_framework_binary_records_its_cell() {
    let checked = check(
        "apple_ios_app",
        "17.0",
        SIM,
        "x86_64 arm64",
        SIM_LOADS,
        &["/out/libledger.a", "/vendor/Share.framework/Share"],
    );
    assert_eq!(
        written(&checked),
        "cell=simulator/arm64\nsdk=iphonesimulator\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libledger.a arch=arm64 platform=iphonesimulator minos=17.0\nframework=Share arch=arm64 platform=iphonesimulator minos=17.0\n"
    );
    assert_eq!(
        checked.log,
        [
            "lipo -archs /out/libledger.a",
            "otool -arch arm64 -l /out/libledger.a",
            "lipo -archs /vendor/Share.framework/Share",
            "otool -arch arm64 -l /vendor/Share.framework/Share",
        ]
    );
}

#[test]
fn a_newer_framework_binary_fails_the_app_deployment() {
    let checked = check(
        "apple_ios_app",
        "17.0",
        SIM,
        "arm64",
        &SIM_LOADS.replace("minos 17.0", "minos 18.0"),
        &["/vendor/Share.framework/Share"],
    );
    rejects(
        &checked,
        "framework binary Share needs iOS 18.0, above the app deployment 17.0",
    );
}

#[test]
fn a_macos_archive_records_its_cell_and_names_macos() {
    let loads = SIM_LOADS
        .replace("platform 7", "platform 1")
        .replace("minos 17.0", "minos 14.0");
    let cell = ["macos/arm64", "macosx", "1", "arm64"];
    let checked = check(
        "apple_macos_app",
        "14.0",
        cell,
        "arm64",
        &loads,
        &["/out/libledger.a"],
    );
    assert!(
        written(&checked).ends_with("archive=libledger.a arch=arm64 platform=macosx minos=14.0\n")
    );

    let checked = check(
        "apple_macos_app",
        "14.0",
        cell,
        "arm64",
        &loads.replace("minos 14.0", "minos 15.0"),
        &["/out/libledger.a"],
    );
    rejects(
        &checked,
        "library archive libledger.a needs macOS 15.0, above the app deployment 14.0",
    );
}

#[test]
fn legacy_version_min_archives_resolve_by_arch() {
    let device = ["device/arm64", "iphoneos", "2", "arm64"];
    let checked = check(
        "apple_ios_app",
        "17.0",
        device,
        "arm64",
        LEGACY_LOADS,
        &["/out/libledger.a"],
    );
    assert!(
        written(&checked)
            .ends_with("archive=libledger.a arch=arm64 platform=iphoneos minos=10.0\n")
    );

    let x86 = ["simulator/x86_64", "iphonesimulator", "7", "x86_64"];
    let checked = check(
        "apple_ios_app",
        "17.0",
        x86,
        "x86_64",
        LEGACY_LOADS,
        &["/out/libledger.a"],
    );
    assert!(
        written(&checked)
            .ends_with("archive=libledger.a arch=x86_64 platform=iphonesimulator minos=10.0\n")
    );

    let checked = check(
        "apple_ios_app",
        "17.0",
        SIM,
        "arm64",
        LEGACY_LOADS,
        &["/out/libledger.a"],
    );
    rejects(
        &checked,
        "library archive libledger.a is built for iphoneos, want iphonesimulator for simulator/arm64",
    );

    let checked = check(
        "apple_ios_app",
        "17.0",
        SIM,
        "arm64",
        &LEGACY_LOADS.replace("LC_VERSION_MIN_IPHONEOS", "LC_VERSION_MIN_MACOSX"),
        &["/out/libledger.a"],
    );
    rejects(
        &checked,
        "library archive libledger.a is built for macosx, want iphonesimulator for simulator/arm64",
    );
}

#[test]
fn a_library_cell_records_its_cell_for_apple_ios_lib() {
    let loads = SIM_LOADS.replace("platform 7", "platform 2");
    let cell = ["device/arm64", "iphoneos", "2", "arm64"];
    let checked = check(
        "apple_ios_lib",
        "17.0",
        cell,
        "arm64",
        &loads,
        &["/out/libshare.a"],
    );
    assert_eq!(
        written(&checked),
        "cell=device/arm64\nsdk=iphoneos\nsdk_version=18.5\ndeployment=17.0\nxcode=16.4.0.16F6\narchive=libshare.a arch=arm64 platform=iphoneos minos=17.0\n"
    );

    let checked = check(
        "apple_ios_lib",
        "17.0",
        cell,
        "arm64",
        &loads.replace("minos 17.0", "minos 18.0"),
        &["/out/libshare.a"],
    );
    rejects(
        &checked,
        "library archive libshare.a needs iOS 18.0, above the lib deployment 17.0",
    );
}
