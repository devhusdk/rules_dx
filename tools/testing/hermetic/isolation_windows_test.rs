//! Probes the Windows user backend: network denial, loopback and file grants.
#![cfg(windows)]

use std::path::{Path, PathBuf};

use hermetic::{capture, Isolation, Net, Outcome, WINDOWS_DENIED_MARKER};

const OUTSIDE: &str = "1.1.1.1:443";

struct Scene {
    isolation: Isolation,
    root: PathBuf,
    probe: PathBuf,
    granted: PathBuf,
    ungranted: PathBuf,
}

fn scene(name: &str) -> Scene {
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set under Bazel"));
    let root = tmp.join(format!("appcontainer probe {name}"));
    hermetic::remove_root(&root);
    let granted = root.join("granted");
    let ungranted = root.join("ungranted");
    std::fs::create_dir_all(&granted).expect("granted dir");
    std::fs::create_dir_all(&ungranted).expect("ungranted dir");
    let built = dx_testing::resolve_runfiles(&std::env::var("DX_HERMETIC_WINDOWS_PROBE").unwrap());
    let probe = granted.join("probe.exe");
    std::fs::copy(&built, &probe).expect("copy the probe");
    let launcher =
        dx_testing::resolve_runfiles(&std::env::var("DX_HERMETIC_APPCONTAINER").unwrap());
    let isolation = Isolation {
        launcher,
        grants: vec![granted.clone()],
        denies: vec![ungranted.clone()],
        ..Isolation::default()
    };
    Scene {
        isolation,
        root,
        probe,
        granted,
        ungranted,
    }
}

impl Scene {
    fn run(&self, net: Net, args: &[&str]) -> Outcome {
        let mut argv = vec![self.probe.to_string_lossy().into_owned()];
        argv.extend(args.iter().map(|arg| (*arg).to_owned()));
        capture(self.isolation.command(net, &argv)).expect("launcher starts")
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn network_denied_blocks_outbound_but_keeps_loopback() {
    let scene = scene("denied");
    let outbound = scene.run(Net::Denied, &["connect", OUTSIDE]);
    assert_ne!(outbound.exit, 0, "{}", outbound.stdout);
    assert!(
        outbound.stderr.contains(WINDOWS_DENIED_MARKER),
        "{}",
        outbound.stderr
    );
    let single = scene.run(Net::Denied, &["loopback"]);
    assert_eq!(single.exit, 0, "{}", single.stderr);
    let pair = scene.run(Net::Denied, &["parent"]);
    assert_eq!(pair.exit, 0, "{}", pair.stderr);
}

#[test]
fn files_are_visible_only_where_granted() {
    let scene = scene("files");
    let secret = scene.ungranted.join("secret.txt");
    std::fs::write(&secret, "hidden").expect("secret");
    let open = scene.granted.join("open.txt");
    std::fs::write(&open, "visible").expect("open");
    let denied = scene.run(Net::Denied, &["read", &text(&secret)]);
    assert_ne!(denied.exit, 0, "{}", denied.stdout);
    let allowed = scene.run(Net::Denied, &["read", &text(&open)]);
    assert_eq!(allowed.exit, 0, "{}", allowed.stderr);
    assert!(allowed.stdout.contains("visible"));
}

fn icacls(path: &Path) -> String {
    let out = std::process::Command::new("icacls")
        .arg(path)
        .output()
        .expect("icacls runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn directories_above_a_grant_are_listable_but_not_readable() {
    let scene = scene("above");
    let above = scene.root.join("above.txt");
    std::fs::write(&above, "hidden").expect("above");
    let before = icacls(&scene.root);
    let canon = scene.run(Net::Denied, &["canon", &text(&scene.granted)]);
    assert_eq!(canon.exit, 0, "{}", canon.stderr);
    let read = scene.run(Net::Denied, &["read", &text(&above)]);
    assert_ne!(read.exit, 0, "{}", read.stdout);
    assert_eq!(icacls(&scene.root), before);
}

#[test]
fn a_granted_directory_can_be_canonicalized() {
    let scene = scene("canon");
    let outcome = scene.run(Net::Denied, &["canon", &text(&scene.granted)]);
    assert_eq!(outcome.exit, 0, "{}", outcome.stderr);
}

#[test]
fn network_allowed_reaches_the_outside() {
    let outcome = scene("allowed").run(Net::Allowed, &["connect", OUTSIDE]);
    assert_eq!(outcome.exit, 0, "{}", outcome.stderr);
}
