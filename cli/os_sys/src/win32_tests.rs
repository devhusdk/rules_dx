use crate::win32::{
    Access, creation_time, disinherit_std_handles, is_running, open, processes, resume,
    resume_threads, suspend, terminate, total_physical_memory, working_set_bytes,
};
use std::os::windows::io::AsHandle;

#[test]
fn this_process_is_running_and_has_a_start_and_a_working_set() {
    let me = open(std::process::id(), Access::Wait).expect("open self");
    assert!(is_running(me.as_handle()).expect("wait"));
    assert!(creation_time(me.as_handle()).expect("times") > 0);
    assert!(working_set_bytes(me.as_handle()).expect("memory") > 0);
}

#[test]
fn an_exited_child_is_not_running() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit 0"])
        .spawn()
        .expect("spawn");
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
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit 3"])
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
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "ping -n 30 127.0.0.1 >NUL"])
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    suspend(child.id()).expect("suspend");
    resume(child.id()).expect("resume");
    terminate(child.id(), 7).expect("terminate");
    assert_eq!(child.wait().expect("wait").code(), Some(7));
}

#[test]
fn an_exited_child_cannot_be_suspended_resumed_or_terminated() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit 0"])
        .spawn()
        .expect("spawn");
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
