#[test]
fn stop_of_reports_the_holder_once_then_the_signal() {
    let (mut err, mut events) = (Vec::new(), Vec::new());
    let mut gate = super::SlotGate {
        err: &mut err,
        events: &mut events,
        phase: dx_plan::slots::Phase::new(dx_plan::args::Command::Update),
        busy: Some("pid 7".to_owned()),
    };
    assert_eq!(
        super::stop_of(&mut gate, 1),
        dx_plan::slots::Stop::Busy("pid 7".to_owned())
    );
    assert_eq!(
        super::stop_of(&mut gate, 130),
        dx_plan::slots::Stop::Signal(130)
    );
}

fn cargo_env(repin: &str, repository: &str) -> Vec<(String, String)> {
    dx_update::backend::cargo_env(repin, repository)
}

#[test]
fn consumer_cargo_env_forces_both_repin_triggers() {
    assert_eq!(
        cargo_env("false", "crates"),
        vec![
            ("CARGO_BAZEL_ISOLATED".to_owned(), "true".to_owned()),
            ("CARGO_BAZEL_REPIN".to_owned(), "false".to_owned()),
            ("CARGO_BAZEL_REPIN_ONLY".to_owned(), "crates".to_owned()),
            ("REPIN".to_owned(), "false".to_owned()),
            ("CARGO_BAZEL_DIGEST_CHECK".to_owned(), "1".to_owned()),
        ]
    );
}

#[test]
fn consumer_cargo_check_env_checks_the_lock_digest_but_apply_does_not() {
    let has = |env: Vec<(String, String)>| env.iter().any(|(k, _)| k == "CARGO_BAZEL_DIGEST_CHECK");
    assert!(has(cargo_env("false", "crates")));
    assert!(!has(cargo_env("workspace", "crates")));
    assert!(!has(cargo_env("serde", "crates")));
}

#[cfg(unix)]
#[test]
fn scratch_removal_survives_a_read_only_module_cache() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = crate::staged::Scratch::create("readonly").expect("scratch");
    let dir = scratch.0.join("gomodcache/pkg/mod");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("f.go"), "package f\n").expect("file");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    let path = scratch.0.clone();
    drop(scratch);
    assert!(!path.exists(), "{path:?}");
}
