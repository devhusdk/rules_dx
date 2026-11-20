use crate::procs::*;
use std::process::Command;

#[test]
fn this_process_is_alive_and_a_reaped_child_is_not() {
    let _fork = dx_testing::forking();
    assert!(pid_alive(std::process::id()));
    let mut child = Command::new(dx_testing::process_probe())
        .spawn()
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
    let _fork = dx_testing::forking();
    let mut child = Command::new("cmd")
        .args(["/C", "exit 0"])
        .spawn()
        .expect("spawn");
    let pid = child.id();
    while child.try_wait().expect("poll").is_none() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!pid_alive(pid));
}

#[cfg(windows)]
#[test]
fn a_child_that_exited_with_259_is_not_alive() {
    let _fork = dx_testing::forking();
    let mut child = Command::new("cmd")
        .args(["/C", "exit 259"])
        .spawn()
        .expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    assert!(!pid_alive(pid));
}

#[cfg(windows)]
#[test]
fn the_live_table_has_this_process_and_its_parent() {
    assert_eq!(boot_id(), None);
    let own = std::process::id();
    let table = process_table();
    let (_, parent, _) = table
        .iter()
        .find(|(pid, _, _)| *pid == own)
        .expect("own pid");
    assert!(table.iter().any(|(pid, _, _)| pid == parent));
}

#[cfg(windows)]
#[test]
fn command_lines_see_a_child_with_a_marker_and_memory() {
    let _fork = dx_testing::forking();
    let mut child = Command::new("cmd")
        .args(["/C", "ping -n 30 127.0.0.1 >nul & rem dx-procs-marker"])
        .spawn()
        .expect("spawn");
    let pid = child.id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let found = loop {
        let found = command_lines()
            .into_iter()
            .find(|info| info.pid == pid && info.rss_bytes > 0);
        if found.is_some() || std::time::Instant::now() >= deadline {
            break found;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    child.kill().expect("kill");
    child.wait().expect("wait");
    let info = found.expect("child listed");
    assert!(info.command_line.contains("dx-procs-marker"));
    assert!(info.rss_bytes > 0);
}

#[test]
fn this_process_has_a_stable_start_time() {
    let first = start_time(std::process::id());
    assert!(first.is_some());
    assert_eq!(first, start_time(std::process::id()));
}

#[cfg(target_os = "macos")]
#[test]
fn start_time_has_microseconds_after_the_seconds() {
    let start = start_time(std::process::id()).expect("start");
    let (secs, micros) = start.split_once('.').expect("seconds and microseconds");
    assert!(secs.parse::<u64>().expect("seconds") > 0);
    assert_eq!(micros.len(), 6);
    assert_eq!(start_time(u32::MAX), None);
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
#[test]
fn start_time_runs_ps_in_the_c_locale() {
    let command = start_time_command(std::process::id());
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == "LC_ALL" && value == Some(std::ffi::OsStr::new("C")))
    );
}

#[cfg(unix)]
#[test]
fn procargs_take_argc_arguments_after_the_padded_path() {
    let mut buffer = 2i32.to_ne_bytes().to_vec();
    buffer.extend_from_slice(b"/bin/java\0\0\0java\0--workspace_directory=/w a\0HOME=/h\0");
    assert_eq!(
        parse_procargs(&buffer).as_deref(),
        Some("java\0--workspace_directory=/w a")
    );
    assert_eq!(parse_procargs(buffer.get(..6).expect("prefix")), None);
}

#[cfg(unix)]
#[test]
fn the_command_line_table_has_this_process() {
    let own = std::process::id();
    let table = command_line_table();
    let (_, line) = table.iter().find(|(pid, _)| *pid == own).expect("own pid");
    assert!(!line.is_empty());
}

#[cfg(unix)]
#[test]
fn an_unreaped_child_is_a_zombie() {
    let _fork = dx_testing::forking();
    let mut child = Command::new(dx_testing::process_probe())
        .spawn()
        .expect("spawn");
    let pid = child.id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !zombie(pid) {
        assert!(std::time::Instant::now() < deadline, "no zombie");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!zombie(std::process::id()));
    child.wait().expect("wait");
}
