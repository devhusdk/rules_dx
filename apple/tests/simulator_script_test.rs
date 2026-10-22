use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const FAKE_XCRUN: &str = r#"#!/bin/sh
printf '%s\n' "$*" >>"$FAKE_LOG"
case "$1" in
    simctl)
        if [ "$2" = "list" ] && [ "$3" = "runtimes" ]; then
            cat "$FAKE_RUNTIMES"
            exit 0
        fi
        if [ "$2" = "list" ]; then
            cat "$FAKE_DEVICES"
            exit 0
        fi
        if [ "$2" = "create" ]; then
            printf '%s\n' 3C9D2E71-5B8A-4F06-9D1C-2A7E4B6F8031
            exit 0
        fi
        if [ "$2" = "bootstatus" ]; then
            exit "${FAKE_BOOT_EXIT:-0}"
        fi
        if [ "$2" = "install" ]; then
            [ -f "$4/Info.plist" ] || exit 9
        fi
        exit "${FAKE_EXIT:-0}"
        ;;
    lipo)
        printf '%s\n' "$FAKE_ARCHS"
        ;;
    otool)
        cat "$FAKE_LOADS"
        ;;
esac
"#;

const DEVICES: &str = "== Devices ==
-- iOS 18.5 --
    iPhone 16 (6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10) (Shutdown)
    iPhone 16 Pro (0F4B8C2D-1A3E-4F5B-8C7D-9E0F1A2B3C4D) (Booted)
-- iOS 18.6 --
    iPhone 16 Pro (7A8B9C0D-1E2F-4A5B-8C6D-7E8F9A0B1C2D) (Shutdown)
";

const RUNTIMES: &str = "== Runtimes ==
iOS 17.5 (17.5 - 21F79) - com.apple.CoreSimulator.SimRuntime.iOS-17-5
iOS 18.5 (18.5 - 22F77) - com.apple.CoreSimulator.SimRuntime.iOS-18-5
watchOS 11.5 (11.5 - 22T572) - com.apple.CoreSimulator.SimRuntime.watchOS-11-5
";

const CREATED: &str = "3C9D2E71-5B8A-4F06-9D1C-2A7E4B6F8031";

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
        std::fs::write(&xcrun, FAKE_XCRUN).unwrap_or_else(|error| panic!("write: {error}"));
        std::fs::set_permissions(&xcrun, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("chmod: {error}"));
        std::fs::write(root.join("devices.txt"), DEVICES)
            .unwrap_or_else(|error| panic!("write: {error}"));
        std::fs::write(root.join("runtimes.txt"), RUNTIMES)
            .unwrap_or_else(|error| panic!("write: {error}"));
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

fn fake_run(fake: &Fake, var: &str, envs: &[(&str, &str)]) -> Run {
    let log = fake.path("log.txt");
    let devices = fake.path("devices.txt");
    let runtimes = fake.path("runtimes.txt");
    let mut all = vec![
        ("XCRUN", fake.xcrun.as_str()),
        ("FAKE_LOG", log.as_str()),
        ("FAKE_DEVICES", devices.as_str()),
        ("FAKE_RUNTIMES", runtimes.as_str()),
    ];
    all.extend_from_slice(envs);
    run(&script(var), &[], &all).unwrap_or_else(|error| panic!("run: {error}"))
}

fn simctl(fake: &Fake, envs: &[(&str, &str)]) -> Run {
    fake_run(fake, "DX_SIMCTL_OPS", envs)
}

fn create(fake: &Fake, envs: &[(&str, &str)]) -> Run {
    fake_run(fake, "DX_CREATE_TEST_SIMULATOR", envs)
}

fn fails(ran: &Run, code: i32, stderr: &str) {
    assert_eq!(ran.status.code(), Some(code), "{}", ran.combined());
    assert_eq!(ran.stderr, format!("{stderr}\n"), "{}", ran.combined());
}

#[test]
fn boot_resolves_one_named_simulator_to_its_udid() {
    let fake = Fake::new("apple-simctl-boot");
    let ran = simctl(
        &fake,
        &[("DX_SIMCTL_OP", "boot"), ("DX_SIMCTL_DEVICE", "iPhone 16")],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        fake.log(),
        "simctl list devices available\nsimctl bootstatus 6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10 -b\n"
    );
}

#[test]
fn install_unpacks_the_ipa_and_installs_its_app_bundle() {
    let fake = Fake::new("apple-simctl-install");
    let payload = fake.root.join("Payload/Ledger.app");
    std::fs::create_dir_all(&payload).unwrap_or_else(|error| panic!("mkdir: {error}"));
    std::fs::write(payload.join("Info.plist"), "plist").unwrap_or_else(|error| panic!("{error}"));
    let ipa = fake.path("Ledger.ipa");
    let zipped = std::process::Command::new("zip")
        .current_dir(&fake.root)
        .args(["-qr", ipa.as_str(), "Payload"])
        .status()
        .unwrap_or_else(|error| panic!("zip: {error}"));
    assert!(zipped.success());
    let ran = simctl(
        &fake,
        &[
            ("DX_SIMCTL_OP", "install"),
            ("DX_SIMCTL_DEVICE", "0F4B8C2D-1A3E-4F5B-8C7D-9E0F1A2B3C4D"),
            ("DX_SIMCTL_APP", ipa.as_str()),
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let log = fake.log();
    let install = log.lines().nth(1).unwrap_or_default();
    assert!(
        install.starts_with("simctl install 0F4B8C2D-1A3E-4F5B-8C7D-9E0F1A2B3C4D /")
            && install.ends_with("/Payload/Ledger.app"),
        "{log}"
    );
    let unpacked = install
        .rsplit_once(' ')
        .map(|(_, path)| path)
        .unwrap_or_default();
    assert!(
        !Path::new(unpacked).exists(),
        "the unpacked bundle is removed"
    );
}

#[test]
fn launch_passes_the_bundle_id_to_the_selected_simulator() {
    let fake = Fake::new("apple-simctl-launch");
    let ran = simctl(
        &fake,
        &[
            ("DX_SIMCTL_OP", "launch"),
            ("DX_SIMCTL_DEVICE", "7A8B9C0D-1E2F-4A5B-8C6D-7E8F9A0B1C2D"),
            ("DX_SIMCTL_BUNDLE_ID", "com.example.ledger"),
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        fake.log(),
        "simctl list devices available\nsimctl launch --terminate-running-process 7A8B9C0D-1E2F-4A5B-8C6D-7E8F9A0B1C2D com.example.ledger\n"
    );
}

#[test]
fn simulator_selection_never_falls_back() {
    let fake = Fake::new("apple-simctl-select");
    fails(
        &simctl(&fake, &[("DX_SIMCTL_OP", "boot")]),
        2,
        "dx-apple-simctl: DX_SIMCTL_DEVICE names the simulator by name or UDID",
    );
    fails(
        &simctl(&fake, &[("DX_SIMCTL_OP", "boot"), ("DX_SIMCTL_DEVICE", "iPhone 15")]),
        2,
        "dx-apple-simctl: no available simulator named 'iPhone 15'; create one with xcrun simctl create",
    );
    fails(
        &simctl(
            &fake,
            &[
                ("DX_SIMCTL_OP", "boot"),
                ("DX_SIMCTL_DEVICE", "iPhone 16 Pro"),
            ],
        ),
        2,
        "dx-apple-simctl: 'iPhone 16 Pro' names 2 simulators; pass one UDID",
    );
    fails(
        &simctl(
            &fake,
            &[
                ("DX_SIMCTL_OP", "install"),
                ("DX_SIMCTL_DEVICE", "iPhone 16"),
            ],
        ),
        2,
        "dx-apple-simctl: DX_SIMCTL_APP names the built .ipa or .app",
    );
    fails(
        &simctl(
            &fake,
            &[("DX_SIMCTL_OP", "erase"), ("DX_SIMCTL_DEVICE", "iPhone 16")],
        ),
        2,
        "dx-apple-simctl: unknown op 'erase'",
    );
    assert!(
        !fake.log().contains("bootstatus"),
        "no rejected selection reaches simctl"
    );
}

#[test]
fn simctl_failures_keep_their_exit_code() {
    let fake = Fake::new("apple-simctl-exit");
    let ran = simctl(
        &fake,
        &[
            ("DX_SIMCTL_OP", "launch"),
            ("DX_SIMCTL_DEVICE", "iPhone 16"),
            ("DX_SIMCTL_BUNDLE_ID", "com.example.ledger"),
            ("FAKE_EXIT", "149"),
        ],
    );
    assert_eq!(ran.status.code(), Some(149), "{}", ran.combined());
}

#[test]
fn a_host_without_xcrun_fails_instead_of_running_anything() {
    let fake = Fake::new("apple-simctl-missing");
    let ran = simctl(
        &fake,
        &[
            ("XCRUN", "/nonexistent/xcrun"),
            ("DX_SIMCTL_OP", "boot"),
            ("DX_SIMCTL_DEVICE", "iPhone 16"),
        ],
    );
    fails(
        &ran,
        127,
        "dx-apple-simctl: no xcrun executable /nonexistent/xcrun on PATH; simulator operations need a macOS host with Xcode",
    );
}

#[test]
fn a_test_simulator_is_created_and_booted_on_the_exact_runtime() {
    let fake = Fake::new("apple-test-sim-create");
    let ran = create(
        &fake,
        &[
            ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
            ("SIMULATOR_OS_VERSION", "18.5"),
            ("SIMULATOR_SDK_VERSION", "18.5"),
            ("XCTESTRUN_RUNNER_PID", "4242"),
        ],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(ran.stdout, format!("{CREATED}\n"), "{}", ran.combined());
    assert_eq!(
        fake.log(),
        format!(
            "simctl list runtimes available\nsimctl create dx iPhone 16 iOS 18.5 4242 iPhone 16 com.apple.CoreSimulator.SimRuntime.iOS-18-5\nsimctl bootstatus {CREATED} -b\n"
        )
    );
}

#[test]
fn a_missing_test_runtime_fails_instead_of_using_the_sdk_runtime() {
    let fake = Fake::new("apple-test-sim-runtime");
    for os in ["17.9", "18"] {
        let runtime = format!(
            "com.apple.CoreSimulator.SimRuntime.iOS-{}",
            os.replace('.', "-")
        );
        let ran = create(
            &fake,
            &[
                ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
                ("SIMULATOR_OS_VERSION", os),
                ("SIMULATOR_SDK_VERSION", "18.5"),
            ],
        );
        fails(
            &ran,
            2,
            &format!("dx-apple-test-simulator: no available iOS {os} simulator runtime {runtime}; install it with xcodebuild -downloadPlatform iOS -buildVersion {os}"),
        );
        assert_eq!(ran.stdout, "", "{}", ran.combined());
    }
    fails(
        &create(&fake, &[("SIMULATOR_DEVICE_TYPE", "iPhone 16")]),
        2,
        "dx-apple-test-simulator: SIMULATOR_DEVICE_TYPE and SIMULATOR_OS_VERSION name the simulator to create",
    );
    assert_eq!(
        fake.log(),
        "simctl list runtimes available\nsimctl list runtimes available\n",
        "no simulator is created for a missing runtime"
    );
}

#[test]
fn a_test_simulator_that_cannot_boot_is_deleted() {
    let fake = Fake::new("apple-test-sim-boot");
    let ran = create(
        &fake,
        &[
            ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
            ("SIMULATOR_OS_VERSION", "17.5"),
            ("XCTESTRUN_RUNNER_PID", "7"),
            ("FAKE_BOOT_EXIT", "164"),
        ],
    );
    assert_eq!(ran.status.code(), Some(164), "{}", ran.combined());
    assert_eq!(ran.stdout, "", "{}", ran.combined());
    assert_eq!(
        fake.log(),
        format!(
            "simctl list runtimes available\nsimctl create dx iPhone 16 iOS 17.5 7 iPhone 16 com.apple.CoreSimulator.SimRuntime.iOS-17-5\nsimctl bootstatus {CREATED} -b\nsimctl delete {CREATED}\n"
        )
    );
}

#[test]
fn a_test_simulator_needs_xcrun() {
    let fake = Fake::new("apple-test-sim-xcrun");
    fails(
        &create(
            &fake,
            &[
                ("XCRUN", "/nonexistent/xcrun"),
                ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
                ("SIMULATOR_OS_VERSION", "18.5"),
            ],
        ),
        127,
        "dx-apple-test-simulator: no xcrun executable /nonexistent/xcrun on PATH; simulator tests need a macOS host with Xcode",
    );
    assert_eq!(fake.log(), "");
}

#[test]
fn a_test_simulator_is_shut_down_and_deleted_after_the_run() {
    let fake = Fake::new("apple-test-sim-delete");
    let ran = fake_run(
        &fake,
        "DX_DELETE_TEST_SIMULATOR",
        &[("SIMULATOR_UDID", CREATED)],
    );
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        fake.log(),
        format!("simctl shutdown {CREATED}\nsimctl delete {CREATED}\n")
    );
    fails(
        &fake_run(&fake, "DX_DELETE_TEST_SIMULATOR", &[]),
        2,
        "dx-apple-test-simulator: SIMULATOR_UDID names the simulator to delete",
    );
}

fn check(fake: &Fake, archs: &str, loads: &str) -> (Run, PathBuf) {
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
    let ran = run(
        &script("DX_LIBRARY_CHECK"),
        &[
            report_path.as_str(),
            "simulator/arm64",
            "iphonesimulator",
            "7",
            "arm64",
            "17.0",
            "16.4.0.16F6",
            "18.5",
            "/out/libledger.a",
        ],
        &[
            ("PATH", path.as_str()),
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
