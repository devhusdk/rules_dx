//! Probes the Windows user backend: network denial, loopback, explicit file denies and the MSVC
//! denies. Like the Linux and macOS masks, the user backend hides only what `denies` lists.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

use hermetic::{AclGroup, Isolation, Net, Outcome, WINDOWS_DENIED_MARKER, capture};

const OUTSIDE: &str = "1.1.1.1:443";

/// The MSVC deny test rewrites ACLs on host-wide trees, so it runs alone; the others share.
static HOST_ACLS: RwLock<()> = RwLock::new(());

fn shared() -> RwLockReadGuard<'static, ()> {
    HOST_ACLS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    let _shared = shared();
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
fn network_denied_cannot_resolve_names() {
    let _shared = shared();
    let scene = scene("resolve");
    let denied = scene.run(Net::Denied, &["resolve", "example.com"]);
    assert_ne!(denied.exit, 0, "{}", denied.stdout);
    let allowed = scene.run(Net::Allowed, &["resolve", "example.com"]);
    assert_eq!(allowed.exit, 0, "{}", allowed.stderr);
}

#[test]
fn network_denied_blocks_a_local_resolver_port() {
    let _shared = shared();
    let scene = scene("resolver");
    let denied = scene.run(Net::Denied, &["connect", "127.0.0.1:53"]);
    assert_ne!(denied.exit, 0, "{}", denied.stdout);
    assert!(
        denied.stderr.contains(WINDOWS_DENIED_MARKER),
        "{}",
        denied.stderr
    );
}

#[test]
fn files_are_visible_only_where_granted() {
    let _shared = shared();
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

#[test]
fn a_group_set_up_once_denies_its_members_until_teardown() {
    let _shared = shared();
    let mut scene = scene("group");
    let secret = scene.ungranted.join("secret.txt");
    std::fs::write(&secret, "hidden").expect("secret");
    let open = scene.granted.join("open.txt");
    std::fs::write(&open, "visible").expect("open");
    let before = [icacls(&scene.granted), icacls(&scene.ungranted)];
    let name = format!("dxh-{}-test", std::process::id());
    let out = scene.root.join("out");
    let group = AclGroup::setup(&mut scene.isolation, &name, &out).expect("group setup");
    let timeline = std::fs::read_to_string(out.join("logs").join("timeline.log")).unwrap();
    assert!(timeline.contains("acl deny "), "{timeline}");
    assert!(icacls(&scene.ungranted).contains(&name));
    let denied = scene.run(Net::Denied, &["read", &text(&secret)]);
    assert_ne!(denied.exit, 0, "{}", denied.stdout);
    let allowed = scene.run(Net::Denied, &["read", &text(&open)]);
    assert_eq!(allowed.exit, 0, "{}", allowed.stderr);
    assert!(allowed.stdout.contains("visible"));
    drop(group);
    assert_eq!([icacls(&scene.granted), icacls(&scene.ungranted)], before);
}

#[test]
fn stale_groups_are_removed_when_a_new_group_is_set_up() {
    // Alone, so no concurrent setup removes the stale group over other paths.
    let _alone = HOST_ACLS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut scene = scene("stale group");
    let before = [icacls(&scene.granted), icacls(&scene.ungranted)];
    let mut child = std::process::Command::new("cmd")
        .args(["/c", "exit 0"])
        .spawn()
        .expect("cmd starts");
    let dead = child.id();
    child.wait().expect("cmd exits");
    let stale = format!("dxh-{dead}");
    std::mem::forget(AclGroup::setup(&mut scene.isolation, &stale).expect("stale group setup"));
    assert!(icacls(&scene.ungranted).contains(&stale));
    let fresh = format!("dxh-{}-fresh", std::process::id());
    drop(AclGroup::setup(&mut scene.isolation, &fresh).expect("fresh group setup"));
    assert_eq!([icacls(&scene.granted), icacls(&scene.ungranted)], before);
    let listed = std::process::Command::new("net")
        .args(["localgroup", stale.as_str()])
        .output()
        .expect("net runs");
    assert!(!listed.status.success(), "{stale} is still a local group");
}

fn icacls(path: &Path) -> String {
    let out = std::process::Command::new("icacls")
        .arg(path)
        .output()
        .expect("icacls runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn directories_above_a_grant_keep_their_acls() {
    let _shared = shared();
    let scene = scene("above");
    let before = icacls(&scene.root);
    let canon = scene.run(Net::Denied, &["canon", &text(&scene.granted)]);
    assert_eq!(canon.exit, 0, "{}", canon.stderr);
    // The run user can read what `denies` leaves out, so nothing above the grant is hidden (#8699).
    assert_eq!(icacls(&scene.root), before);
}

#[test]
fn a_granted_directory_can_be_canonicalized() {
    let _shared = shared();
    let scene = scene("canon");
    let outcome = scene.run(Net::Denied, &["canon", &text(&scene.granted)]);
    assert_eq!(outcome.exit, 0, "{}", outcome.stderr);
}

#[test]
fn network_allowed_reaches_the_outside() {
    let _shared = shared();
    let outcome = scene("allowed").run(Net::Allowed, &["connect", OUTSIDE]);
    assert_eq!(outcome.exit, 0, "{}", outcome.stderr);
}

#[test]
fn the_msvc_denies_hide_visual_studio_and_restore_its_acls() {
    let _alone = HOST_ACLS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let scene = scene("msvc");
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let denies =
        hermetic::windows_msvc_denies(program_files.as_deref(), program_files_x86.as_deref());
    if denies.is_empty() {
        eprintln!("no Visual Studio or Windows SDK on this host; nothing to deny");
        return;
    }
    let above = scene.root.join("above.txt");
    std::fs::write(&above, "hidden").expect("above");
    let read = scene.run(Net::Denied, &["read", &text(&above)]);
    assert_ne!(
        read.exit,
        0,
        "readable before the deny: {}{}",
        read.stdout,
        icacls(&scene.root)
    );
    let before: Vec<String> = denies.iter().map(|dir| icacls(dir)).collect();
    let isolation = Isolation {
        denies: denies.clone(),
        ..scene.isolation.clone()
    };
    let started = std::time::Instant::now();
    hermetic::isolation_check(&isolation, None, &scene.granted.join("check.cmd"))
        .expect("every denied tree is unreadable");
    eprintln!(
        "denied {} trees in {:?}: {denies:?}",
        denies.len(),
        started.elapsed()
    );
    let after: Vec<String> = denies.iter().map(|dir| icacls(dir)).collect();
    assert_eq!(after, before);
    let read = scene.run(Net::Denied, &["read", &text(&above)]);
    assert_ne!(
        read.exit,
        0,
        "readable after the deny: {}{}",
        read.stdout,
        icacls(&scene.root)
    );
}

#[test]
fn a_granted_path_listed_as_a_deny_is_reported_readable() {
    let _alone = HOST_ACLS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut scene = scene("readable");
    let name = format!("dxh-{}-readable", std::process::id());
    let _group = AclGroup::setup(&mut scene.isolation, &name).expect("group setup");
    let isolation = Isolation {
        denies: vec![scene.granted.clone()],
        ..scene.isolation.clone()
    };
    let err = hermetic::isolation_check(&isolation, None, &scene.granted.join("check.cmd"))
        .expect_err("a granted path stays readable in group mode");
    assert!(err.contains("is still readable"), "{err}");
}
