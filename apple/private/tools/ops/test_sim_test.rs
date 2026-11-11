//! Tests the test simulator programs.

mod fake;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use dx_apple_ops::test_sim::{create, delete, owner};
use dx_apple_ops::xcode::Tools;
use fake::{Fake, env, fail, ok};

const RUNTIMES: &str = "== Runtimes ==
iOS 17.5 (17.5 - 21F79) - com.apple.CoreSimulator.SimRuntime.iOS-17-5
iOS 18.5 (18.5 - 22F77) - com.apple.CoreSimulator.SimRuntime.iOS-18-5
watchOS 11.5 (11.5 - 22T572) - com.apple.CoreSimulator.SimRuntime.watchOS-11-5
";

const CREATED: &str = "3C9D2E71-5B8A-4F06-9D1C-2A7E4B6F8031";

const OLD: &str = "0F4B8C2D-1A3E-4F5B-8C7D-9E0F1A2B3C4D";

const KEEP: &str = "7A8B9C0D-1E2F-4A5B-8C6D-7E8F9A0B1C2D";

fn tools(fake: &Fake) -> Tools<'_> {
    Tools {
        developer_dir: Some(PathBuf::from("/x/Developer")),
        runner: fake,
    }
}

fn request() -> impl Fn(&str) -> Option<String> {
    env(&[
        ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
        ("SIMULATOR_OS_VERSION", "18.5"),
    ])
}

fn devices() -> String {
    format!(
        "== Devices ==\n-- iOS 18.5 --\n    dx iPhone 16 iOS 18.5 pgid 4242 ({OLD}) (Shutdown)\n    dx iPhone 16 iOS 18.5 pgid 77 ({KEEP}) (Booted)\n    iPhone 16 (6B1E0A62-37C2-4E4D-9E27-0A4C1E2F3B10) (Shutdown)\n"
    )
}

fn simulator() -> Fake {
    Fake::new(|cmd| {
        let line = cmd.args.join(" ");
        if line == "simctl list runtimes available" {
            ok(RUNTIMES)
        } else if line == "simctl list devices available" {
            ok(&devices())
        } else if line.starts_with("simctl create") {
            ok(&format!("{CREATED}\n"))
        } else {
            ok("")
        }
    })
}

#[test]
fn owners_are_read_from_dx_names_only() {
    assert_eq!(owner("dx iPhone 16 iOS 18.5 pgid 4242"), Some(4242));
    assert_eq!(owner("iPhone 16"), None);
    assert_eq!(owner("dx iPhone iOS 18.5 pgid x"), None);
    assert_eq!(owner("dx iPhone iOS abc pgid 1"), None);
}

#[test]
fn a_test_simulator_is_created_and_booted_on_the_exact_runtime() {
    let fake = simulator();
    let udid = create(&tools(&fake), &request(), 9, &|_| true).unwrap();
    assert_eq!(udid, CREATED);
    assert_eq!(
        fake.log(),
        [
            "simctl list runtimes available".to_owned(),
            "simctl list devices available".to_owned(),
            "simctl create dx iPhone 16 iOS 18.5 pgid 9 iPhone 16 com.apple.CoreSimulator.SimRuntime.iOS-18-5".to_owned(),
            format!("simctl bootstatus {CREATED} -b"),
        ]
    );
}

#[test]
fn a_missing_test_runtime_fails_instead_of_using_the_sdk_runtime() {
    let fake = simulator();
    let wanted = env(&[
        ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
        ("SIMULATOR_OS_VERSION", "19.0"),
    ]);
    let failure = create(&tools(&fake), &wanted, 9, &|_| true).unwrap_err();
    assert_eq!(failure.code, 2);
    assert!(failure.message.starts_with(
        "no available iOS 19.0 simulator runtime com.apple.CoreSimulator.SimRuntime.iOS-19-0;"
    ));
    assert_eq!(fake.log(), ["simctl list runtimes available"]);
}

#[test]
fn a_request_needs_both_names_and_never_reuses() {
    let fake = simulator();
    let partial = env(&[("SIMULATOR_DEVICE_TYPE", "iPhone 16")]);
    assert_eq!(
        create(&tools(&fake), &partial, 9, &|_| true)
            .unwrap_err()
            .code,
        2
    );
    let reuse = env(&[
        ("SIMULATOR_DEVICE_TYPE", "iPhone 16"),
        ("SIMULATOR_OS_VERSION", "18.5"),
        ("SIMULATOR_REUSE_SIMULATOR", "1"),
    ]);
    let failure = create(&tools(&fake), &reuse, 9, &|_| true).unwrap_err();
    assert_eq!(
        failure.message,
        "test simulators are never reused; SIMULATOR_REUSE_SIMULATOR must be unset"
    );
    assert!(fake.log().is_empty());
}

#[test]
fn a_test_simulator_that_cannot_boot_is_deleted() {
    let fake = Fake::new(|cmd| {
        let line = cmd.args.join(" ");
        if line == "simctl list runtimes available" {
            ok(RUNTIMES)
        } else if line.starts_with("simctl create") {
            ok(CREATED)
        } else if line.starts_with("simctl bootstatus") {
            fail(5, "boot failed")
        } else {
            ok("")
        }
    });
    let failure = create(&tools(&fake), &request(), 9, &|_| true).unwrap_err();
    assert_eq!(failure.code, 5);
    assert_eq!(
        fake.log().last().unwrap(),
        &format!("simctl delete {CREATED}")
    );
}

#[test]
fn a_simulator_left_by_a_dead_run_is_swept() {
    let fake = simulator();
    let asked = Rc::new(Cell::new(0));
    let seen = asked.clone();
    let alive = move |group: i32| {
        seen.set(seen.get() + 1);
        group != 4242
    };
    create(&tools(&fake), &request(), 9, &alive).unwrap();
    let log = fake.log();
    assert!(log.contains(&format!("simctl shutdown {OLD}")));
    assert!(log.contains(&format!("simctl delete {OLD}")));
    assert!(!log.iter().any(|line| line.contains(KEEP)));
    assert_eq!(asked.get(), 2);
}

#[test]
fn a_failed_sweep_still_creates() {
    let fake = Fake::new(|cmd| {
        let line = cmd.args.join(" ");
        if line == "simctl list runtimes available" {
            ok(RUNTIMES)
        } else if line == "simctl list devices available" {
            fail(1, "boom")
        } else {
            ok(CREATED)
        }
    });
    assert_eq!(
        create(&tools(&fake), &request(), 9, &|_| true).unwrap(),
        CREATED
    );
}

#[test]
fn list_failures_keep_the_simctl_exit_code() {
    let fake = Fake::new(|_| fail(69, "no service"));
    let failure = create(&tools(&fake), &request(), 9, &|_| true).unwrap_err();
    assert_eq!(failure.code, 69);
    assert!(failure.message.contains("cannot list runtimes: no service"));
}

#[test]
fn a_test_simulator_is_shut_down_and_deleted_after_the_run() {
    let fake = simulator();
    assert_eq!(
        delete(&tools(&fake), &env(&[("SIMULATOR_UDID", CREATED)])),
        Ok(0)
    );
    assert_eq!(
        fake.log(),
        [
            format!("simctl shutdown {CREATED}"),
            format!("simctl delete {CREATED}")
        ]
    );
    let failure = delete(&tools(&fake), &env(&[])).unwrap_err();
    assert_eq!(
        failure.message,
        "SIMULATOR_UDID names the simulator to delete"
    );
}

#[test]
fn a_failed_delete_does_not_fail_the_test() {
    let fake = Fake::new(|cmd| {
        if cmd.args[1] == "delete" {
            fail(1, "busy")
        } else {
            ok("")
        }
    });
    assert_eq!(
        delete(&tools(&fake), &env(&[("SIMULATOR_UDID", CREATED)])),
        Ok(0)
    );
}
