use super::*;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn sleeper() -> std::process::Child {
    Command::new(dx_testing::process_probe())
        .arg("--sleep-ms=30000")
        .process_group(0)
        .spawn()
        .expect("sleeper starts")
}

#[test]
fn a_lease_without_a_live_dx_stops_its_group() {
    let scratch = dx_test_scratch::scratch("dx-groups-dead-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    std::fs::write(dir.join("424242-0.lock"), format!("{}\n", lease_line(pgid))).expect("lease");
    let reaped = reap_in(&dir);
    let _ = child.wait();
    assert_eq!(reaped, vec![Reaped { pid: 424242, pgid }]);
    assert!(!group_alive(pgid));
    assert!(!dir.join("424242-0.lock").exists());
    assert!(reap_in(&dir).is_empty());
}

#[test]
fn a_lease_held_by_a_live_dx_is_left_alone() {
    let scratch = dx_test_scratch::scratch("dx-groups-live-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let path = dir.join("424243-0.lock");
    let held = dx_atomic_fs::lease::try_lock_file(&path, true)
        .expect("lock")
        .expect("free");
    std::fs::write(&path, format!("{}\n", lease_line(pgid))).expect("lease");
    assert!(reap_in(&dir).is_empty());
    assert!(group_alive(pgid));
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    drop(held);
}

#[test]
fn a_lease_for_a_finished_group_is_dropped_quietly() {
    let scratch = dx_test_scratch::scratch("dx-groups-gone-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = Command::new(dx_testing::process_probe())
        .process_group(0)
        .spawn()
        .expect("probe");
    let pgid = child.id() as i32;
    let _ = child.wait();
    std::fs::write(dir.join("424244-0.lock"), format!("{}\n", lease_line(pgid))).expect("lease");
    assert!(reap_in(&dir).is_empty());
    assert!(!dir.join("424244-0.lock").exists());
}

#[test]
fn unrelated_files_are_ignored() {
    let scratch = dx_test_scratch::scratch("dx-groups-other-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("notes.txt"), "1\n").expect("file");
    assert!(reap_in(&dir).is_empty());
    assert!(dir.join("notes.txt").exists());
}

#[test]
fn a_lease_naming_a_reused_group_number_is_dropped_without_a_signal() {
    let scratch = dx_test_scratch::scratch("dx-groups-reused-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let line = format!("{pgid} {} 1", sys::namespace());
    std::fs::write(dir.join("424245-0.lock"), format!("{line}\n")).expect("lease");
    let reaped = reap_in(&dir);
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(reaped.is_empty());
    assert!(alive, "an unrelated group must survive");
    assert!(!dir.join("424245-0.lock").exists());
}

#[test]
fn a_lease_from_another_pid_namespace_is_left_alone() {
    let scratch = dx_test_scratch::scratch("dx-groups-namespace-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let path = dir.join("424246-0.lock");
    std::fs::write(&path, format!("{pgid} pid:[1] -\n")).expect("lease");
    assert!(reap_in(&dir).is_empty());
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(alive);
    assert!(path.exists());
}

#[test]
fn a_lease_dropped_with_its_group_still_running_leaves_nothing_to_reap() {
    let scratch = dx_test_scratch::scratch("dx-groups-drop-");
    let dir = scratch.path().join("groups");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let mut lease = GroupLease::acquire_in(&dir).expect("lease");
    lease.record(pgid);
    drop(lease);
    assert!(reap_in(&dir).is_empty());
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(
        alive,
        "a clean exit must not hand the group to the next run"
    );
}

fn leases(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "lock"))
        .collect()
}

fn probe_spec(args: &[&str], group: crate::lifecycle::StreamGroup) -> crate::lifecycle::StreamSpec {
    use crate::lifecycle::{EnvPolicy, StreamOutput, StreamSpec, StreamStdin};
    let mut argv = vec![dx_testing::process_probe().into_os_string()];
    argv.extend(args.iter().map(std::ffi::OsString::from));
    StreamSpec {
        argv,
        cwd: std::env::temp_dir(),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        stdin: StreamStdin::Null,
        stdout: StreamOutput::Inherit,
        stderr: StreamOutput::Inherit,
        group,
        held: Vec::new(),
    }
}

const QUICK: crate::lifecycle::CleanupBudget = crate::lifecycle::CleanupBudget {
    graceful: Duration::from_millis(500),
    forced: Duration::from_millis(500),
    drain: Duration::from_millis(500),
};

#[test]
fn an_owned_run_holds_a_lease_naming_its_group_and_removes_it() {
    use std::cell::Cell;
    let scratch = dx_test_scratch::scratch("dx-groups-run-");
    let dir = scratch.path().join("groups");
    let pgid = Cell::new(0);
    let reaped_none = Cell::new(false);
    let alive = Cell::new(false);
    let started = std::time::Instant::now();
    let withdraw = || {
        if started.elapsed() >= Duration::from_secs(5) {
            return true;
        }
        let Some(path) = leases(&dir).into_iter().next() else {
            return false;
        };
        let Some(recorded) = std::fs::read_to_string(&path)
            .ok()
            .as_deref()
            .and_then(parse_lease)
        else {
            return false;
        };
        pgid.set(recorded.pgid);
        reaped_none.set(reap_in(&dir).is_empty());
        alive.set(group_alive(recorded.pgid));
        true
    };
    let outcome = crate::lifecycle::run_streamed_in(
        probe_spec(&["--sleep-ms=30000"], crate::lifecycle::StreamGroup::Owned),
        QUICK,
        &|| None,
        &withdraw,
        Some(&dir),
    )
    .expect("probe runs");
    let pgid = pgid.get();
    assert_eq!(outcome, crate::lifecycle::StreamOutcome::Withdrawn);
    assert!(pgid > 1, "the lease names the child group");
    assert_ne!(pgid, std::process::id() as i32);
    assert!(reaped_none.get(), "a held lease is not reaped");
    assert!(alive.get(), "the group runs while the lease is held");
    assert!(leases(&dir).is_empty());
    assert!(!group_alive(pgid));
}

#[test]
fn a_terminal_run_takes_no_lease() {
    let scratch = dx_test_scratch::scratch("dx-groups-terminal-");
    let dir = scratch.path().join("groups");
    let outcome = crate::lifecycle::run_streamed_in(
        probe_spec(&["--exit-code=0"], crate::lifecycle::StreamGroup::Terminal),
        QUICK,
        &|| None,
        &|| false,
        Some(&dir),
    )
    .expect("probe runs");
    assert!(matches!(
        outcome,
        crate::lifecycle::StreamOutcome::Exited(_)
    ));
    // `GroupLease::acquire_in` creates the dir (via `create_dir_all` in the lock
    // open) and its Drop removes only the file, so an empty dir would mean a
    // lease was taken and released. A Terminal run must never reach it.
    assert!(
        !dir.exists(),
        "a Terminal run must not call GroupLease::acquire_in, which creates the lease dir"
    );
}

#[test]
fn a_failed_owned_spawn_leaves_no_lease() {
    let scratch = dx_test_scratch::scratch("dx-groups-missing-");
    let dir = scratch.path().join("groups");
    let mut spec = probe_spec(&[], crate::lifecycle::StreamGroup::Owned);
    spec.argv = vec![scratch.path().join("missing-binary").into_os_string()];
    let result = crate::lifecycle::run_streamed_in(spec, QUICK, &|| None, &|| false, Some(&dir));
    assert!(result.is_err());
    assert!(leases(&dir).is_empty());
}

fn backdate(path: &Path) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open lease")
        .set_modified(std::time::SystemTime::now() - (STALE_EMPTY + Duration::from_secs(60)))
        .expect("backdate lease");
}

#[test]
fn an_old_empty_lease_is_removed() {
    let scratch = dx_test_scratch::scratch("dx-groups-old-empty-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("424247-0.lock");
    std::fs::write(&path, "").expect("lease");
    backdate(&path);
    assert!(reap_in(&dir).is_empty());
    assert!(!path.exists());
}

#[test]
fn a_fresh_empty_lease_is_kept() {
    let scratch = dx_test_scratch::scratch("dx-groups-fresh-empty-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("424248-0.lock");
    std::fs::write(&path, "").expect("lease");
    assert!(reap_in(&dir).is_empty());
    assert!(path.exists());
}

#[test]
fn an_old_empty_lease_held_by_a_live_dx_is_kept() {
    let scratch = dx_test_scratch::scratch("dx-groups-held-empty-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("424249-0.lock");
    let held = dx_atomic_fs::lease::try_lock_file(&path, true)
        .expect("lock")
        .expect("free");
    backdate(&path);
    assert!(reap_in(&dir).is_empty());
    assert!(path.exists());
    drop(held);
}
