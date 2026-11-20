//! Windows Job objects.

#![expect(unsafe_code, reason = "Job objects have no safe std API")]

use crate::win32::{Access, open, owned_or_last_error, struct_size};
use std::io;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, HandleOrNull, OwnedHandle};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::TerminateProcess;

// The process id list is a u32 pair followed by usize ids, read here as whole words.
const _: () = assert!(size_of::<usize>() == 8, "dx supports only 64-bit Windows");

const LIST_ATTEMPTS: usize = 8;

/// The one limit a new job carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Closing the last handle kills every process in the job.
    KillOnClose,
    /// Members may start children outside the job with `CREATE_BREAKAWAY_FROM_JOB`.
    BreakawayOk,
}

/// An owned Job object, closed on drop.
#[derive(Debug)]
pub struct JobObject {
    handle: OwnedHandle,
}

impl JobObject {
    /// Creates an unnamed job with `limit`.
    pub fn new(limit: Limit) -> io::Result<JobObject> {
        // SAFETY: CreateJobObjectW accepts null attributes and a null name.
        let raw = unsafe { CreateJobObjectW(null(), null()) };
        // SAFETY: CreateJobObjectW returns a new handle owned by the caller, or null.
        let handle = unsafe { HandleOrNull::from_raw_handle(raw) };
        let handle = owned_or_last_error(handle)?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = match limit {
            Limit::KillOnClose => JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            Limit::BreakawayOk => JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        };
        let size = struct_size::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()?;
        // SAFETY: handle is a live job and info is the extended limit structure of the size passed.
        let applied = unsafe {
            SetInformationJobObject(
                handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size,
            )
        };
        if applied == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(JobObject { handle })
    }

    /// Puts `process` in the job.
    pub fn assign(&self, process: BorrowedHandle<'_>) -> io::Result<()> {
        // SAFETY: both handles are live for the call.
        let assigned = unsafe {
            AssignProcessToJobObject(self.handle.as_raw_handle(), process.as_raw_handle())
        };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Ends every process in the job with `exit_code`.
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        // SAFETY: the job handle is live.
        let ended = unsafe { TerminateJobObject(self.handle.as_raw_handle(), exit_code) };
        if ended == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(())
    }

    /// The ids of the processes in the job.
    pub fn process_ids(&self) -> io::Result<Vec<u32>> {
        let mut capacity = 64usize;
        for _ in 0..LIST_ATTEMPTS {
            let mut buffer = vec![0usize; 1 + capacity];
            let len = u32::try_from(buffer.len() * size_of::<usize>()).map_err(io::Error::other)?;
            // SAFETY: buffer is writable for len bytes and the job handle is live.
            let queried = unsafe {
                QueryInformationJobObject(
                    self.handle.as_raw_handle(),
                    JobObjectBasicProcessIdList,
                    buffer.as_mut_ptr().cast(),
                    len,
                    null_mut(),
                )
            } != 0;
            let err = io::Error::last_os_error();
            // The header word holds NumberOfAssignedProcesses, then NumberOfProcessIdsInList.
            let header = buffer.first().copied().unwrap_or(0);
            let assigned = header & 0xffff_ffff;
            let listed = header >> 32;
            if queried && assigned <= listed {
                return buffer
                    .iter()
                    .skip(1)
                    .take(listed.min(capacity))
                    .map(|&pid| u32::try_from(pid).map_err(io::Error::other))
                    .collect();
                // LCOV_EXCL_START - reason: the list outgrowing its buffer between two calls, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
            }
            if !queried && err.raw_os_error() != Some(ERROR_MORE_DATA.cast_signed()) {
                return Err(err);
            }
            capacity = (assigned + 16).max(capacity * 2);
        }
        Err(io::Error::other("the job's process list kept growing"))
        // LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }

    /// How many processes in the job are still running.
    pub fn active_processes(&self) -> io::Result<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let size = struct_size::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>()?;
        // SAFETY: info is writable for the size passed and the job handle is live.
        let queried = unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast(),
                size,
                null_mut(),
            )
        };
        if queried == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(info.ActiveProcesses)
    }

    /// Ends process `pid` with exit code 1 if it is in the job; a process outside is left alone.
    pub fn terminate_member(&self, pid: u32) -> io::Result<()> {
        let process = open(pid, Access::Terminate)?;
        let mut inside = 0;
        // SAFETY: both handles are live and inside is a writable BOOL.
        let checked = unsafe {
            IsProcessInJob(
                process.as_raw_handle(),
                self.handle.as_raw_handle(),
                &mut inside,
            )
        };
        if checked == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: Win32 call fails only when the OS is out of resources, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        if inside == 0 {
            return Ok(());
        }
        // SAFETY: process is a live handle with PROCESS_TERMINATE access.
        let ended = unsafe { TerminateProcess(process.as_raw_handle(), 1) };
        if ended == 0 {
            return Err(io::Error::last_os_error()); // LCOV_EXCL_LINE - reason: the member exited before it was ended, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
        Ok(())
    }
}
