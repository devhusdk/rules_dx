use super::*;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
fn sleeper() -> std::process::Child {
    Command::new(dx_testing::process_probe())
        .arg("--sleep-ms=30000")
        .process_group(0)
        .spawn()
        .expect("sleeper starts")
}

#[cfg(unix)]
#[test]
fn a_lease_without_a_live_dx_stops_its_group() {
    let scratch = dx_test_scratch::scratch("dx-groups-dead-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    std::fs::write(
        dir.join("424242-0.lock"),
        format!("{}\n", lease_line(pgid).expect("line")),
    )
    .expect("lease");
    let reaped = reap_in(&dir);
    let _ = child.wait();
    assert_eq!(reaped, vec![Reaped { pid: 424242, pgid }]);
    assert!(!group_alive(pgid));
    assert!(!dir.join("424242-0.lock").exists());
    assert!(reap_in(&dir).is_empty());
}

#[cfg(unix)]
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
    std::fs::write(&path, format!("{}\n", lease_line(pgid).expect("line"))).expect("lease");
    assert!(reap_in(&dir).is_empty());
    assert!(group_alive(pgid));
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    drop(held);
}

#[cfg(unix)]
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
    let line = lease_line(pgid).expect("line");
    let _ = child.wait();
    std::fs::write(dir.join("424244-0.lock"), format!("{line}\n")).expect("lease");
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

#[cfg(unix)]
#[test]
fn a_lease_naming_a_reused_group_number_is_dropped_without_a_signal() {
    let scratch = dx_test_scratch::scratch("dx-groups-reused-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let line = format!("{pgid} {} 1 {}", sys::namespace(), sys::boot_id());
    std::fs::write(dir.join("424245-0.lock"), format!("{line}\n")).expect("lease");
    let reaped = reap_in(&dir);
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(reaped.is_empty());
    assert!(alive, "an unrelated group must survive");
    assert!(!dir.join("424245-0.lock").exists());
}

#[cfg(unix)]
#[test]
fn a_lease_from_another_pid_namespace_is_left_alone() {
    let scratch = dx_test_scratch::scratch("dx-groups-namespace-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let path = dir.join("424246-0.lock");
    std::fs::write(&path, format!("{pgid} pid:[1] 77 boot-a\n")).expect("lease");
    assert!(reap_in(&dir).is_empty());
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(alive);
    assert!(path.exists());
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
const QUICK: crate::lifecycle::CleanupBudget = crate::lifecycle::CleanupBudget {
    graceful: Duration::from_millis(500),
    forced: Duration::from_millis(500),
    drain: Duration::from_millis(500),
};

#[cfg(unix)]
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
        let recorded = leases(&dir)
            .first()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .as_deref()
            .and_then(parse_lease);
        if let Some(recorded) = &recorded {
            pgid.set(recorded.pgid);
            reaped_none.set(reap_in(&dir).is_empty());
            alive.set(group_alive(recorded.pgid));
        }
        recorded.is_some() || started.elapsed() >= Duration::from_secs(5)
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

#[cfg(unix)]
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

#[cfg(unix)]
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

fn recorded(start: &str) -> Recorded {
    Recorded {
        pgid: 4000,
        namespace: "pid:[1]".to_owned(),
        start: start.to_owned(),
        boot: "boot-a".to_owned(),
    }
}

fn observed() -> Observed {
    Observed {
        namespace: "pid:[1]".to_owned(),
        group_alive: true,
        leader: Leader::Present("77".to_owned()),
        boot: "boot-a".to_owned(),
        own: vec![10, 11, 12],
    }
}

#[test]
fn verdict_table() {
    let ok = recorded("77");
    assert_eq!(verdict(&ok, &observed()), Verdict::Stop);
    let mut now = observed();
    now.namespace = "pid:[2]".to_owned();
    assert_eq!(verdict(&ok, &now), Verdict::Keep);
    let mut now = observed();
    now.group_alive = false;
    assert_eq!(verdict(&ok, &now), Verdict::Drop);
    assert_eq!(verdict(&recorded("-"), &observed()), Verdict::Drop);
    let mut now = observed();
    now.boot = "boot-b".to_owned();
    assert_eq!(verdict(&ok, &now), Verdict::Drop);
    let mut now = observed();
    now.own = vec![4000];
    assert_eq!(verdict(&ok, &now), Verdict::Drop);
    let mut now = observed();
    now.leader = Leader::Present("78".to_owned());
    assert_eq!(verdict(&ok, &now), Verdict::Drop);
    now.leader = Leader::Unknown;
    assert_eq!(verdict(&ok, &now), Verdict::Keep);
    now.leader = Leader::Absent;
    assert_eq!(verdict(&ok, &now), Verdict::Stop);
}

#[test]
fn a_lease_written_in_one_locale_still_stops_its_group() {
    let start = "Di 4 Nov 10:00:00 2026";
    let mut now = observed();
    now.leader = Leader::Present(start.to_owned());
    assert_eq!(verdict(&recorded(start), &now), Verdict::Stop);
}

#[cfg(target_os = "linux")]
#[test]
fn verdict_drops_a_dash_namespace_on_linux() {
    let mut rec = recorded("77");
    rec.namespace = "-".to_owned();
    let mut now = observed();
    now.namespace = "-".to_owned();
    assert_eq!(verdict(&rec, &now), Verdict::Drop);
}

#[cfg(unix)]
#[test]
fn a_lease_with_unknown_start_is_dropped_without_a_signal() {
    let scratch = dx_test_scratch::scratch("dx-groups-unknown-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let path = dir.join("424250-0.lock");
    let line = format!("{pgid} {} - {}", sys::namespace(), sys::boot_id());
    std::fs::write(&path, format!("{line}\n")).expect("lease");
    let reaped = reap_in(&dir);
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(reaped.is_empty());
    assert!(alive);
    assert!(!path.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn a_lease_from_another_boot_is_dropped_without_a_signal() {
    let scratch = dx_test_scratch::scratch("dx-groups-boot-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    let line = lease_line(pgid).expect("line");
    let mut fields: Vec<&str> = line.split(' ').collect();
    fields[3] = "not-this-boot";
    let path = dir.join("424251-0.lock");
    std::fs::write(&path, format!("{}\n", fields.join(" "))).expect("lease");
    let reaped = reap_in(&dir);
    let alive = group_alive(pgid);
    sys::signal_group(pgid, 9);
    let _ = child.wait();
    assert!(reaped.is_empty());
    assert!(alive);
    assert!(!path.exists());
}

#[test]
fn a_three_field_lease_is_unparseable() {
    assert!(parse_lease("4242 pid:[1] 77").is_none());
    assert!(parse_lease("4242 pid:[1] 77 boot-a").is_some());
}

#[cfg(unix)]
#[test]
fn a_stop_is_written_to_the_reap_log() {
    let scratch = dx_test_scratch::scratch("dx-groups-log-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let mut child = sleeper();
    let pgid = child.id() as i32;
    std::fs::write(
        dir.join("424253-0.lock"),
        format!("{}\n", lease_line(pgid).expect("line")),
    )
    .expect("lease");
    reap_in(&dir);
    let _ = child.wait();
    let log = std::fs::read_to_string(dir.join("reaped.log")).expect("log");
    assert!(log.contains(&format!("stop reaper={}", std::process::id())));
    assert!(log.contains(&format!("pgid={pgid}")));
}

#[test]
fn record_writes_no_line_when_the_start_is_unknown() {
    assert!(lease_line(i32::MAX).is_none());
}

#[test]
fn the_reap_message_names_the_group_and_the_dead_run() {
    let message = reaped_message(&Reaped { pid: 12, pgid: 34 });
    assert_eq!(
        message,
        "stopped leftover process group 34 from dx run 12, which is gone"
    );
}

#[test]
fn the_lease_root_names_the_groups_directory() {
    assert!(reap_orphans().is_empty());
    let scratch = dx_test_scratch::scratch("dx-groups-root-");
    set_lease_root(scratch.path());
    let dir = lease_dir().expect("set");
    assert!(dir.ends_with(GROUPS_DIR_NAME));
    assert!(reap_orphans().is_empty());
}

#[test]
fn the_reap_log_rotates_once_it_passes_its_cap() {
    let scratch = dx_test_scratch::scratch("dx-groups-rotate-");
    let dir = scratch.path();
    std::fs::write(dir.join(LOG_NAME), vec![b'x'; LOG_CAP as usize + 1]).expect("big log");
    log_decision(dir, "stop", 1, &recorded("77"), &observed());
    assert!(dir.join(format!("{LOG_NAME}.1")).is_file());
    let fresh = std::fs::read_to_string(dir.join(LOG_NAME)).expect("fresh log");
    assert_eq!(fresh.lines().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_group_that_ignores_the_first_signal_is_killed() {
    let scratch = dx_test_scratch::scratch("dx-groups-kill-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let armed = scratch.path().join("armed");
    let script = format!(
        "trap '' TERM; : > '{}'; while :; do sleep 1; done",
        armed.display()
    );
    let mut child = Command::new("/bin/sh")
        .args(["-c", &script])
        .process_group(0)
        .spawn()
        .expect("shell starts");
    dx_testing::wait_armed(&armed);
    let pgid = child.id() as i32;
    std::fs::write(
        dir.join("424260-0.lock"),
        format!("{}\n", lease_line(pgid).expect("line")),
    )
    .expect("lease");
    let reaped = reap_in(&dir);
    let status = child.wait().expect("reaped");
    assert_eq!(reaped.len(), 1);
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(9)
    );
}

#[test]
fn recording_a_group_with_an_unknown_start_leaves_the_lease_empty() {
    let scratch = dx_test_scratch::scratch("dx-groups-empty-");
    let mut lease = GroupLease::acquire_in(scratch.path()).expect("lease");
    lease.record(i32::MAX);
    assert_eq!(std::fs::metadata(&lease.path).expect("lease file").len(), 0);
}

#[test]
fn a_leader_matches_only_when_present_with_the_recorded_start_or_gone() {
    assert!(leader_matches(&Leader::Present("7".to_owned()), "7"));
    assert!(!leader_matches(&Leader::Present("8".to_owned()), "7"));
    assert!(leader_matches(&Leader::Absent, "7"));
    assert!(!leader_matches(&Leader::Unknown, "7"));
}

#[cfg(unix)]
#[test]
fn a_gone_leader_still_counts_as_the_recorded_one() {
    let mut rec = recorded("77");
    rec.pgid = i32::MAX;
    assert!(still_recorded(&rec));
}

#[test]
fn signalling_a_reserved_group_number_does_nothing() {
    sys::signal_group(1, 9);
    sys::signal_group(0, 9);
}

#[cfg(target_os = "linux")]
#[test]
fn a_stat_read_is_classified_by_its_outcome() {
    use std::io::{Error, ErrorKind};
    let fields = (0..18).map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
    let stat = format!("1 (a b) S {fields} 4242 0");
    assert_eq!(
        sys::leader_from_stat(Ok(stat)),
        Leader::Present("4242".to_owned())
    );
    assert_eq!(
        sys::leader_from_stat(Ok("short".to_owned())),
        Leader::Unknown
    );
    assert_eq!(
        sys::leader_from_stat(Err(Error::from(ErrorKind::NotFound))),
        Leader::Absent
    );
    assert_eq!(
        sys::leader_from_stat(Err(Error::from(ErrorKind::PermissionDenied))),
        Leader::Unknown
    );
}

#[cfg(not(unix))]
#[test]
fn without_unix_groups_nothing_is_known_and_a_lease_is_dropped() {
    assert!(!group_alive(4000));
    sys::signal_group(4000, 9);
    assert_eq!(sys::cmdline(4000), "");
    assert!(sys::start_time(4000).is_none());
    assert!(sys::own_groups().is_empty());
    assert_eq!(sys::leader(4000), Leader::Unknown);
    assert!(lease_line(4000).is_none());
    let scratch = dx_test_scratch::scratch("dx-groups-portable-");
    let dir = scratch.path().join("groups");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("424260-0.lock");
    std::fs::write(&path, "4000 - 77 -\n").expect("lease");
    assert!(reap_in(&dir).is_empty());
    assert!(!path.exists());
}
