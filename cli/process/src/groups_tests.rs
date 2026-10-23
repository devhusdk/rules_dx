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
    std::fs::write(dir.join("424242-0.lock"), format!("{pgid}\n")).expect("lease");
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
    let held = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .expect("lease file");
    held.try_lock().expect("lock");
    std::fs::write(&path, format!("{pgid}\n")).expect("lease");
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
    std::fs::write(dir.join("424244-0.lock"), format!("{pgid}\n")).expect("lease");
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
