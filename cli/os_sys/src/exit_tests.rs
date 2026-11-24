use crate::exit::wait_gone;
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::Duration;

const CHILD: &str = "DX_OS_SYS_EXIT_CHILD";

#[test]
fn holds_until_stdin_closes() {
    if std::env::var_os(CHILD).is_some() {
        drop(std::io::stdin().read_to_end(&mut Vec::new()));
    }
}

#[test]
fn returns_once_the_process_exits() {
    let mut child = Command::new(std::env::current_exe().expect("exe"))
        .args(["--exact", "exit_tests::holds_until_stdin_closes"])
        .env(CHILD, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    let pid = child.id();
    assert!(!wait_gone(pid, Duration::ZERO).expect("wait while running"));
    drop(child.stdin.take());
    assert!(wait_gone(pid, Duration::from_secs(60)).expect("wait for exit"));
    assert!(child.wait().expect("reap").success());
}

#[test]
fn a_missing_pid_is_already_gone() {
    assert!(wait_gone(0x7fff_fff0, Duration::ZERO).expect("wait"));
}
