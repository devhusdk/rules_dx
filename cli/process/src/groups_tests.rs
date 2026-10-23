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
