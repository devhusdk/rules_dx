//! Tests the simctl operations.

mod fake;

use std::io::Write;
use std::path::PathBuf;

use dx_apple_ops::simctl::{device, run};
use dx_apple_ops::util::Cmd;
use dx_apple_ops::xcode::Tools;
use dx_test_scratch::{scratch, write};
use fake::{Fake, args, env, fail, ok};

const DEVICES: &str = "== Devices ==
-- iOS 18.5 --
    iPhone 16 (6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10) (Shutdown)
    iPhone 16 Pro (0F4B8C2D-1A3E-4F5B-8C7D-9E0F1A2B3C4D) (Booted)
-- iOS 18.6 --
    iPhone 16 Pro (7A8B9C0D-1E2F-4A5B-8C6D-7E8F9A0B1C2D) (Shutdown)
";

const UDID: &str = "6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10";

fn is_list(cmd: &Cmd) -> bool {
    cmd.args.starts_with(&args(&["simctl", "list"]))
}

fn tools(fake: &Fake) -> Tools<'_> {
    Tools {
        developer_dir: Some(PathBuf::from("/x/Developer")),
        runner: fake,
    }
}

fn simulator() -> Fake {
    Fake::new(|cmd| if is_list(cmd) { ok(DEVICES) } else { ok("") })
}

#[test]
fn devices_parse_by_name_and_udid() {
    assert_eq!(
        device("    iPhone 16 (6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10) (Shutdown)"),
        Some(("iPhone 16", UDID))
    );
    assert_eq!(device("-- iOS 18.5 --"), None);
    assert_eq!(device("== Devices =="), None);
    assert_eq!(device("    iPhone 16 (not-hex!) (Shutdown)"), None);
}

#[test]
fn boot_resolves_one_named_simulator_to_its_udid() {
    let fake = simulator();
    let outcome = run(
        &tools(&fake),
        &args(&["boot"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    );
    assert_eq!(outcome, Ok(0));
    assert_eq!(
        fake.log(),
        [
            "simctl list devices available".to_owned(),
            format!("simctl bootstatus {UDID} -b"),
        ]
    );
}

#[test]
fn a_udid_selects_its_simulator() {
    let fake = simulator();
    let outcome = run(
        &tools(&fake),
        &args(&["launch"]),
        &env(&[
            ("DX_SIMCTL_DEVICE", UDID),
            ("DX_SIMCTL_BUNDLE_ID", "com.example.ledger"),
        ]),
    );
    assert_eq!(outcome, Ok(0));
    assert_eq!(
        fake.log()[1],
        format!("simctl launch --terminate-running-process {UDID} com.example.ledger")
    );
}

#[test]
fn launch_needs_a_bundle_id() {
    let fake = simulator();
    let failure = run(
        &tools(&fake),
        &args(&["launch"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    )
    .unwrap_err();
    assert_eq!(failure.code, 2);
    assert_eq!(
        failure.message,
        "DX_SIMCTL_BUNDLE_ID names the installed app"
    );
}

#[test]
fn simulator_selection_never_falls_back() {
    let fake = simulator();
    let none = run(&tools(&fake), &args(&["boot"]), &env(&[])).unwrap_err();
    assert_eq!(
        none.message,
        "DX_SIMCTL_DEVICE names the simulator by name or UDID"
    );
    let unknown = run(
        &tools(&fake),
        &args(&["boot"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPad")]),
    )
    .unwrap_err();
    assert_eq!(unknown.code, 2);
    assert!(
        unknown
            .message
            .starts_with("no available simulator named 'iPad'")
    );
    let twice = run(
        &tools(&fake),
        &args(&["boot"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16 Pro")]),
    )
    .unwrap_err();
    assert_eq!(
        twice.message,
        "'iPhone 16 Pro' names 2 simulators; pass one UDID"
    );
    let op = run(
        &tools(&fake),
        &args(&["erase"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    )
    .unwrap_err();
    assert_eq!(op.message, "unknown op 'erase'");
}

#[test]
fn simctl_failures_keep_their_exit_code() {
    let listing = Fake::new(|_| fail(69, "no service"));
    let failure = run(
        &tools(&listing),
        &args(&["boot"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    )
    .unwrap_err();
    assert_eq!(failure.code, 69);
    assert!(
        failure
            .message
            .contains("cannot list simulators: no service")
    );
    let boot = Fake::new(|cmd| {
        if is_list(cmd) {
            ok(DEVICES)
        } else {
            fail(3, "")
        }
    });
    assert_eq!(
        run(
            &tools(&boot),
            &args(&["boot"]),
            &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")])
        ),
        Ok(3)
    );
}

#[test]
fn a_host_without_xcrun_fails_with_127() {
    let fake = Fake::new(|_| dx_apple_ops::util::Output {
        code: None,
        stdout: String::new(),
        stderr: "No such file or directory".to_owned(),
    });
    let failure = run(
        &tools(&fake),
        &args(&["boot"]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    )
    .unwrap_err();
    assert_eq!(failure.code, 127);
    assert!(failure.message.contains("need a macOS host with Xcode"));
}

#[test]
fn install_passes_an_app_bundle_through() {
    let root = scratch("simctl-app");
    write(root.path(), "Ledger.app/Info.plist", "x");
    let app = root
        .path()
        .join("Ledger.app")
        .to_string_lossy()
        .into_owned();
    let fake = simulator();
    let outcome = run(
        &tools(&fake),
        &args(&["install", &app]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]),
    );
    assert_eq!(outcome, Ok(0));
    assert_eq!(fake.log()[1], format!("simctl install {UDID} {app}"));
}

#[test]
fn install_unpacks_the_ipa_and_installs_its_app_bundle() {
    let root = scratch("simctl-ipa");
    let ipa = root.path().join("Ledger.ipa");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&ipa).unwrap());
    writer
        .start_file(
            "Payload/Ledger.app/Info.plist",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(b"plist").unwrap();
    writer.finish().unwrap();
    let tmp = root.path().join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let tmp = tmp.to_string_lossy().into_owned();
    let ipa = ipa.to_string_lossy().into_owned();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let record = seen.clone();
    let fake = Fake::new(move |cmd| {
        if is_list(cmd) {
            return ok(DEVICES);
        }
        let bundle = PathBuf::from(&cmd.args[3]);
        *record.borrow_mut() = std::fs::read_to_string(bundle.join("Info.plist")).unwrap();
        ok("")
    });
    let outcome = run(
        &tools(&fake),
        &args(&["install", &ipa]),
        &env(&[("DX_SIMCTL_DEVICE", "iPhone 16"), ("TMPDIR", &tmp)]),
    );
    assert_eq!(outcome, Ok(0));
    assert_eq!(*seen.borrow(), "plist");
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
}

#[test]
fn install_rejects_a_missing_or_foreign_app() {
    let root = scratch("simctl-bad");
    write(root.path(), "notes.txt", "x");
    let notes = root.path().join("notes.txt").to_string_lossy().into_owned();
    let fake = simulator();
    let device = env(&[("DX_SIMCTL_DEVICE", "iPhone 16")]);
    let missing = run(&tools(&fake), &args(&["install", "/nope.ipa"]), &device).unwrap_err();
    assert_eq!(missing.code, 2);
    let foreign = run(&tools(&fake), &args(&["install", &notes]), &device).unwrap_err();
    assert_eq!(
        foreign.message,
        format!("{notes} is neither an .ipa nor an .app")
    );
}
