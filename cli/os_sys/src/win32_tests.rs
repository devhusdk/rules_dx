#![cfg(windows)]

use crate::win32::{
    Access, creation_time, disinherit_std_handles, exit_code, is_running, open, processes, resume,
    resume_thread, resume_threads, suspend, system_dirs, terminate, total_physical_memory, wait,
    working_set_bytes,
};
use std::os::windows::io::AsHandle;
use std::process::{Command, Stdio};
use std::time::Duration;

const CHILD: &str = "DX_OS_SYS_TEST_CHILD";

pub(crate) fn child(code: Option<i32>) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("exe"));
    command
        .args(["--exact", "win32_tests::child_main"])
        .env(
            CHILD,
            code.map_or_else(|| "sleep".to_owned(), |code| code.to_string()),
        )
        .stdin(Stdio::null());
    command
}

#[test]
fn child_main() {
    match std::env::var(CHILD).as_deref() {
        Ok("sleep") => std::thread::sleep(Duration::from_secs(30)),
        Ok(code) => std::process::exit(code.parse().expect("exit code")),
        Err(_) => {}
    }
}

#[test]
fn this_process_is_running_and_has_a_start_and_a_working_set() {
    let me = open(std::process::id(), Access::Wait).expect("open self");
    assert!(is_running(me.as_handle()).expect("wait"));
    assert!(creation_time(me.as_handle()).expect("times") > 0);
    assert!(working_set_bytes(me.as_handle()).expect("memory") > 0);
}

#[test]
fn an_exited_child_is_not_running() {
    let mut child = child(Some(0)).spawn().expect("spawn");
    child.wait().expect("wait");
    assert!(!is_running(child.as_handle()).expect("wait"));
}

#[test]
fn a_query_only_handle_cannot_be_waited_on() {
    let me = open(std::process::id(), Access::Query).expect("open self");
    assert!(is_running(me.as_handle()).is_err());
}

#[test]
fn a_file_handle_has_no_process_times_or_memory() {
    let file = std::fs::File::open(std::env::current_exe().expect("exe")).expect("open");
    assert!(creation_time(file.as_handle()).is_err());
    assert!(working_set_bytes(file.as_handle()).is_err());
}

#[test]
fn a_terminate_handle_opens_this_process() {
    assert!(open(std::process::id(), Access::Terminate).is_ok());
}

#[test]
fn an_unknown_pid_does_not_open() {
    assert!(open(u32::MAX - 2, Access::Query).is_err());
}

#[test]
fn the_process_table_lists_this_process_and_its_parent() {
    let me = std::process::id();
    let table = processes().expect("snapshot");
    let entry = table.iter().find(|entry| entry.pid == me).expect("self");
    assert!(!entry.exe.is_empty());
}

#[test]
fn a_suspended_child_resumes() {
    use std::os::windows::process::CommandExt;
    let mut child = child(Some(3))
        .creation_flags(crate::win32::CREATE_SUSPENDED)
        .spawn()
        .expect("spawn");
    resume_threads(child.id()).expect("resume");
    assert_eq!(child.wait().expect("wait").code(), Some(3));
}

#[test]
fn a_pid_without_threads_is_not_found() {
    let err = resume_threads(u32::MAX - 2).expect_err("no threads");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn the_machine_has_physical_memory() {
    assert!(total_physical_memory().expect("memory") > 0);
}

#[test]
fn a_suspended_child_resumes_and_terminates_with_the_given_code() {
    let mut child = child(None).spawn().expect("spawn");
    suspend(child.id()).expect("suspend");
    resume(child.id()).expect("resume");
    terminate(child.id(), 7).expect("terminate");
    assert_eq!(child.wait().expect("wait").code(), Some(7));
}

#[test]
fn an_exited_child_cannot_be_suspended_resumed_or_terminated() {
    let mut child = child(Some(0)).spawn().expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    drop(child);
    assert!(suspend(pid).is_err());
    assert!(resume(pid).is_err());
    assert!(terminate(pid, 1).is_err());
}

#[test]
fn std_handles_can_be_made_uninheritable() {
    disinherit_std_handles().expect("disinherit");
}

#[test]
fn a_waited_child_reports_its_exit_code() {
    let child = child(Some(7)).spawn().expect("spawn");
    assert!(wait(child.as_handle(), None).expect("wait"));
    assert_eq!(exit_code(child.as_handle()).expect("exit code"), 7);
}

#[test]
fn a_running_process_times_out_and_is_still_active() {
    let me = open(std::process::id(), Access::Wait).expect("open self");
    assert!(!wait(me.as_handle(), Some(Duration::from_millis(1))).expect("wait"));
    assert_eq!(exit_code(me.as_handle()).expect("exit code"), 259);
}

#[test]
fn a_duplicate_handle_opens_this_process() {
    assert!(open(std::process::id(), Access::Duplicate).is_ok());
}

#[test]
fn a_file_handle_is_not_a_process_or_thread() {
    let file = std::fs::File::open(std::env::current_exe().expect("exe")).expect("open");
    assert!(exit_code(file.as_handle()).is_err());
    assert!(resume_thread(file.as_handle()).is_err());
}

#[test]
fn a_query_only_handle_cannot_be_waited_on_forever() {
    let me = open(std::process::id(), Access::Query).expect("open self");
    assert!(wait(me.as_handle(), None).is_err());
}

#[test]
fn the_system_directory_is_inside_the_windows_directory() {
    let [system, windows] = system_dirs().expect("system dirs");
    assert!(system.is_dir(), "{}", system.display());
    assert_eq!(system.parent(), Some(windows.as_path()));
}
