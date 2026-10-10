//! Tests for the hermetic Go SDK launch plan.

use std::ffi::OsString;
use std::path::PathBuf;

use dx_go_launcher::{plan_in, GOROOT_REL};

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn stage(scratch: &std::path::Path) {
    let goroot_bin = scratch.join(GOROOT_REL).join("bin");
    std::fs::create_dir_all(&goroot_bin).expect("stage goroot bin");
    for name in [dx_go_launcher::go_file_name(), "go.env"] {
        std::fs::write(goroot_bin.join(name), b"x").expect("stage sdk file");
    }
    let tool_dir = scratch.join("dx-staticcheck");
    std::fs::create_dir_all(&tool_dir).expect("stage tool dir");
    std::fs::write(tool_dir.join(dx_go_launcher::tool_file_name()), b"x").expect("stage tool");
}

#[test]
fn plan_resolves_hermetic_go_env_from_scratch() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-go-launcher-")
        .tempdir_in(std::env::temp_dir())
        .expect("claim scratch");
    stage(scratch.path());
    let plan = plan_in(scratch.path(), &args(&["-f", "json", "a.go"])).expect("plan");
    let scratch = scratch.path();
    assert_eq!(
        plan.tool,
        scratch
            .join("dx-staticcheck")
            .join(dx_go_launcher::tool_file_name())
    );
    assert_eq!(
        plan.args,
        args(&["-f", "json", "a.go"]),
        "tool args pass through untouched"
    );
    let env: std::collections::BTreeMap<_, _> = plan.env_set.into_iter().collect();
    let goroot = scratch.join(GOROOT_REL);
    assert_eq!(
        env.get(&OsString::from("GOROOT")).map(PathBuf::from),
        Some(goroot.clone())
    );
    assert_eq!(
        env.get(&OsString::from("PATH")).map(PathBuf::from),
        Some(goroot.join("bin")),
        "PATH names only the SDK bin directory"
    );
    assert_eq!(
        env.get(&OsString::from("GOPROXY"))
            .map(|value| value.to_string_lossy().into_owned()),
        Some("off".to_owned())
    );
    assert_eq!(
        env.get(&OsString::from("GOTOOLCHAIN"))
            .map(|value| value.to_string_lossy().into_owned()),
        Some("local".to_owned())
    );
    for key in ["GOCACHE", "HOME", "GOPATH"] {
        let dir = PathBuf::from(
            env.get(&OsString::from(key))
                .unwrap_or_else(|| panic!("{key} is set")),
        );
        assert!(
            dir.starts_with(scratch),
            "{key} stays inside the scratch tree"
        );
        assert!(dir.is_dir(), "{key} directory is created");
    }
}

#[test]
fn plan_fails_without_an_sdk_tree() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-go-launcher-bare-")
        .tempdir_in(std::env::temp_dir())
        .expect("claim scratch");
    let error = plan_in(scratch.path(), &args(&[])).expect_err("no sdk fails");
    assert!(error.to_string().contains("Go SDK tree is missing"));
}

#[test]
fn plan_fails_without_a_go_binary() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-go-launcher-nogo-")
        .tempdir_in(std::env::temp_dir())
        .expect("claim scratch");
    std::fs::create_dir_all(scratch.path().join(GOROOT_REL).join("bin")).expect("stage bin");
    let error = plan_in(scratch.path(), &args(&[])).expect_err("no go fails");
    assert!(error.to_string().contains("Go binary is missing"));
}

#[test]
fn plan_fails_without_a_tool_binary() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-go-launcher-notool-")
        .tempdir_in(std::env::temp_dir())
        .expect("claim scratch");
    let goroot_bin = scratch.path().join(GOROOT_REL).join("bin");
    std::fs::create_dir_all(&goroot_bin).expect("stage bin");
    std::fs::write(goroot_bin.join(dx_go_launcher::go_file_name()), b"x").expect("stage go");
    let error = plan_in(scratch.path(), &args(&[])).expect_err("no tool fails");
    assert!(error.to_string().contains("staticcheck binary is missing"));
}
