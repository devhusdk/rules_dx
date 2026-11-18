use super::*;
use crate::process::{SIGHUP, SIGKILL, group_of, signal};
use crate::tty::{foreground_group, open_pty};

#[test]
fn a_child_in_a_terminal_leads_its_foreground_group() {
    let (master, slave) = open_pty().expect("pty");
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    in_terminal(&mut command, slave);
    let mut child = command.spawn().expect("sleep");
    let pid = i32::try_from(child.id()).expect("pid");
    let held = foreground_group(&master);
    let group = group_of(pid);
    signal(pid, SIGKILL).expect("kill");
    child.wait().expect("wait");
    assert_eq!(held.expect("tcgetpgrp"), pid);
    assert_eq!(group.expect("getpgid"), pid);
}

#[test]
fn a_child_can_start_with_a_signal_ignored() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "kill -HUP $$; echo alive"]);
    ignore_signal(&mut command, SIGHUP);
    let output = command.output().expect("sh");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"alive\n");
}
