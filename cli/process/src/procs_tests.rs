use super::*;
use std::process::Command;

#[test]
fn this_process_is_alive_and_a_reaped_child_is_not() {
    assert!(pid_alive(std::process::id()));
    let mut child = if cfg!(windows) {
        Command::new("cmd").args(["/C", "exit 0"]).spawn()
    } else {
        Command::new("true").spawn()
    }
    .expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    assert!(!pid_alive(pid));
}

#[test]
fn a_pid_that_does_not_exist_is_not_alive() {
    assert!(!pid_alive(u32::MAX - 7));
}

#[cfg(windows)]
#[test]
fn an_exited_child_with_an_open_handle_is_not_alive() {
    let mut child = Command::new("cmd").args(["/C", "exit 0"]).spawn().expect("spawn");
    let pid = child.id();
    while child.try_wait().expect("poll").is_none() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!pid_alive(pid));
}

#[cfg(windows)]
#[test]
fn a_child_that_exited_with_259_is_not_alive() {
    let mut child = Command::new("cmd").args(["/C", "exit 259"]).spawn().expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    assert!(!pid_alive(pid));
}

#[cfg(windows)]
#[test]
fn the_live_table_has_this_process_and_its_parent() {
    let own = std::process::id();
    let table = process_table();
    let (_, parent, _) = table.iter().find(|(pid, _, _)| *pid == own).expect("own pid");
    assert!(table.iter().any(|(pid, _, _)| pid == parent));
}

#[cfg(windows)]
#[test]
fn command_lines_see_a_child_with_a_marker_and_memory() {
    let mut child = Command::new("cmd")
        .args(["/C", "ping -n 30 127.0.0.1 >nul & rem dx-procs-marker"])
        .spawn()
        .expect("spawn");
    let pid = child.id();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let found = command_lines().into_iter().find(|info| info.pid == pid);
    child.kill().expect("kill");
    child.wait().expect("wait");
    let info = found.expect("child listed");
    assert!(info.command_line.contains("dx-procs-marker"));
    assert!(info.rss_bytes > 0);
}
