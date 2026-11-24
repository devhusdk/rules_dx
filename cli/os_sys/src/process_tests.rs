use super::*;
use std::os::unix::process::CommandExt;

#[test]
fn this_process_and_its_group_are_reachable() {
    let me = i32::try_from(std::process::id()).expect("pid");
    assert_eq!(probe(me), Presence::Reachable);
    if own_group() > 0 {
        assert_eq!(probe(-own_group()), Presence::Reachable);
    }
    assert_eq!(group_of(me).expect("getpgid"), own_group());
    assert!(own_session().expect("getsid") >= 0);
}

#[test]
fn a_missing_pid_is_missing() {
    assert_eq!(probe(i32::MAX - 1), Presence::Missing);
    assert_eq!(
        signal(i32::MAX - 1, SIGTERM)
            .expect_err("missing")
            .raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
fn pid_zero_and_unknown_signals_are_rejected() {
    assert_eq!(probe(0), Presence::Unknown);
    assert_eq!(
        signal(0, SIGTERM).expect_err("zero").kind(),
        std::io::ErrorKind::InvalidInput
    );
    let me = i32::try_from(std::process::id()).expect("pid");
    assert_eq!(
        signal(me, 0).expect_err("signal 0").kind(),
        std::io::ErrorKind::InvalidInput
    );
}

#[test]
fn a_child_can_be_signalled_and_reaped() {
    let mut child = std::process::Command::new("/bin/cat")
        .stdin(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("cat");
    let pid = i32::try_from(child.id()).expect("pid");
    assert_eq!(group_of(pid).expect("getpgid"), pid);
    assert_eq!(stopped_by(pid).expect("waitid"), None);
    signal(-pid, SIGKILL).expect("kill");
    reap(pid).expect("waitpid");
    assert_eq!(probe(pid), Presence::Missing);
    assert!(child.try_wait().is_err(), "already reaped");
}

#[cfg(target_os = "linux")]
#[test]
fn an_unattributed_signal_reaches_the_child() {
    let mut child = std::process::Command::new("/bin/cat")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("cat");
    let pid = i32::try_from(child.id()).expect("pid");
    queue_unattributed(pid, SIGTERM).expect("rt_sigqueueinfo");
    let status = child.wait().expect("wait");
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(SIGTERM)
    );
    assert_eq!(
        queue_unattributed(i32::MAX - 1, SIGTERM)
            .expect_err("missing")
            .raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn stat_start_reads_the_twenty_second_field_after_the_name() {
    let stat = b"42 (a) b) c) S 1 42 42 0 -1 4194560 1 0 0 0 0 0 0 0 20 0 1 0 98765 1 2\n";
    assert_eq!(stat_start(stat), Some(&b"98765"[..]));
    assert_eq!(stat_start(b"42 (short) S 1"), None);
    assert_eq!(stat_start(b"no name"), None);
    assert!(start_time(std::process::id()).is_some());
}
