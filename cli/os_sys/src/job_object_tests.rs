#![cfg(windows)]

use crate::job_object::{JobObject, Limit};
use crate::win32::{CREATE_SUSPENDED, resume_threads};
use crate::win32_tests::child;
use std::os::windows::io::AsHandle;
use std::os::windows::process::CommandExt;
use std::process::Child;

fn suspended_child() -> Child {
    child(None)
        .creation_flags(CREATE_SUSPENDED)
        .spawn()
        .expect("spawn")
}

#[test]
fn a_member_is_listed_counted_and_ended_with_the_job() {
    let job = JobObject::new(Limit::KillOnClose).expect("job");
    let mut child = suspended_child();
    job.assign(child.as_handle()).expect("assign");
    resume_threads(child.id()).expect("resume");
    assert!(job.process_ids().expect("ids").contains(&child.id()));
    assert!(job.active_processes().expect("count") >= 1);
    job.terminate(7).expect("terminate");
    assert_eq!(child.wait().expect("wait").code(), Some(7));
}

#[test]
fn terminate_member_ends_only_processes_in_the_job() {
    let job = JobObject::new(Limit::BreakawayOk).expect("job");
    let mut outside = suspended_child();
    job.terminate_member(outside.id()).expect("outside");
    let mut inside = suspended_child();
    job.assign(inside.as_handle()).expect("assign");
    job.terminate_member(inside.id()).expect("inside");
    assert_eq!(inside.wait().expect("wait").code(), Some(1));
    assert!(outside.try_wait().expect("poll").is_none());
    outside.kill().expect("kill");
    outside.wait().expect("wait");
}

#[test]
fn an_empty_job_lists_nothing() {
    let job = JobObject::new(Limit::KillOnClose).expect("job");
    assert!(job.process_ids().expect("ids").is_empty());
    assert_eq!(job.active_processes().expect("count"), 0);
}

#[test]
fn a_member_that_cannot_be_opened_is_an_error() {
    let job = JobObject::new(Limit::KillOnClose).expect("job");
    assert!(job.terminate_member(u32::MAX - 2).is_err());
}

#[test]
fn an_exited_process_cannot_be_assigned() {
    let job = JobObject::new(Limit::KillOnClose).expect("job");
    let mut child = child(Some(0)).spawn().expect("spawn");
    child.wait().expect("wait");
    assert!(job.assign(child.as_handle()).is_err());
}
