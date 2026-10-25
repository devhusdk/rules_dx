#[cfg(target_os = "macos")]
mod darwin {
    use std::ffi::{c_int, c_void};
    use std::sync::atomic::{AtomicU32, Ordering};

    const PROC_PIDLISTFDS: c_int = 1;
    const PROC_PIDTBSDINFO: c_int = 3;
    const PROC_PIDFDVNODEINFO: c_int = 1;
    const PROX_FDTYPE_VNODE: u32 = 1;
    const MAX_FDS: usize = 1024;
    const NO_DEVICE: u32 = u32::MAX;

    static MASTER: AtomicU32 = AtomicU32::new(0);

    #[repr(C)]
    struct BsdInfo {
        head: [u32; 27],
        tdev: u32,
        tail: [u32; 6],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FdInfo {
        fd: i32,
        fdtype: u32,
    }

    #[repr(C)]
    struct VnodeFdInfo {
        file: [u64; 3],
        stat: [u32; 29],
        rdev: u32,
        tail: [u64; 4],
    }

    const _: () = assert!(std::mem::size_of::<BsdInfo>() == 136);
    const _: () = assert!(std::mem::size_of::<VnodeFdInfo>() == 176);

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_void,
            buffersize: c_int,
        ) -> c_int;
        fn proc_pidfdinfo(
            pid: c_int,
            fd: c_int,
            flavor: c_int,
            buffer: *mut c_void,
            buffersize: c_int,
        ) -> c_int;
    }

    fn size_of<T>() -> c_int {
        c_int::try_from(std::mem::size_of::<T>()).unwrap_or(c_int::MAX)
    }

    fn controlling_terminal() -> Option<u32> {
        let own = c_int::try_from(std::process::id()).ok()?;
        let mut info = std::mem::MaybeUninit::<BsdInfo>::zeroed();
        let written = unsafe {
            proc_pidinfo(
                own,
                PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size_of::<BsdInfo>(),
            )
        };
        if written != size_of::<BsdInfo>() {
            return None;
        }
        let tdev = unsafe { info.assume_init() }.tdev;
        (tdev != 0 && tdev != NO_DEVICE).then_some(tdev)
    }

    /// Device number of the master side of the pty whose replica is `replica`.
    pub(crate) fn master_of(replica: u32) -> Option<u32> {
        use std::os::unix::fs::MetadataExt as _;
        let ptmx = u32::try_from(std::fs::metadata("/dev/ptmx").ok()?.rdev()).ok()?;
        Some((ptmx & 0xff00_0000) | (replica & 0x00ff_ffff))
    }

    /// Remembers the master side of this process's controlling terminal.
    pub fn watch_terminal() {
        if let Some(master) = controlling_terminal().and_then(master_of) {
            MASTER.store(master, Ordering::SeqCst);
        }
    }

    fn device_of(pid: c_int, fd: c_int) -> Option<u32> {
        let mut info = std::mem::MaybeUninit::<VnodeFdInfo>::zeroed();
        let written = unsafe {
            proc_pidfdinfo(
                pid,
                fd,
                PROC_PIDFDVNODEINFO,
                info.as_mut_ptr().cast(),
                size_of::<VnodeFdInfo>(),
            )
        };
        (written == size_of::<VnodeFdInfo>()).then(|| unsafe { info.assume_init() }.rdev)
    }

    /// Whether `pid` holds the device `master` open; safe inside a signal handler.
    pub(crate) fn holds(pid: c_int, master: u32) -> bool {
        let mut fds = [FdInfo { fd: 0, fdtype: 0 }; MAX_FDS];
        let written = unsafe {
            proc_pidinfo(
                pid,
                PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr().cast(),
                size_of::<[FdInfo; MAX_FDS]>(),
            )
        };
        let Ok(written) = usize::try_from(written) else {
            return false;
        };
        let count = (written / std::mem::size_of::<FdInfo>()).min(MAX_FDS);
        fds[..count]
            .iter()
            .any(|fd| fd.fdtype == PROX_FDTYPE_VNODE && device_of(pid, fd.fd) == Some(master))
    }

    /// Whether `sender` writes to this process's terminal.
    pub fn sent_by_terminal(sender: i32) -> bool {
        let master = MASTER.load(Ordering::SeqCst);
        master != 0 && holds(sender, master)
    }
}

#[cfg(target_os = "macos")]
pub use darwin::{sent_by_terminal, watch_terminal};

/// Remembers the master side of this process's controlling terminal.
#[cfg(not(target_os = "macos"))]
pub fn watch_terminal() {}

/// Whether `sender` writes to this process's terminal.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
pub fn sent_by_terminal(_sender: i32) -> bool {
    false
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::darwin::{holds, master_of};
    use std::ffi::{c_char, c_int, CStr};
    use std::os::fd::{FromRawFd as _, OwnedFd};
    use std::os::unix::fs::MetadataExt as _;

    unsafe extern "C" {
        fn posix_openpt(flags: c_int) -> c_int;
        fn grantpt(fd: c_int) -> c_int;
        fn unlockpt(fd: c_int) -> c_int;
        fn ptsname(fd: c_int) -> *const c_char;
    }

    #[test]
    fn a_process_holding_a_pty_master_is_its_terminal() {
        use std::os::fd::AsRawFd as _;
        let fd = unsafe { posix_openpt(0x0002 | 0x0002_0000) };
        assert!(fd >= 0, "posix_openpt: {}", std::io::Error::last_os_error());
        let master = unsafe { OwnedFd::from_raw_fd(fd) };
        assert_eq!(unsafe { grantpt(master.as_raw_fd()) }, 0);
        assert_eq!(unsafe { unlockpt(master.as_raw_fd()) }, 0);
        let name = unsafe { CStr::from_ptr(ptsname(master.as_raw_fd())) }
            .to_str()
            .expect("utf-8 pty name")
            .to_owned();
        let replica =
            u32::try_from(std::fs::metadata(&name).expect("replica").rdev()).expect("device fits");
        let device = master_of(replica).expect("ptmx");
        let own = c_int::try_from(std::process::id()).expect("pid fits");
        assert!(holds(own, device), "{name} master {device:#x} not found");
        drop(master);
        assert!(!holds(own, device), "{name} master {device:#x} still held");
    }
}
