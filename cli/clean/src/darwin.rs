use std::ffi::{c_int, c_void};
use std::path::PathBuf;

const PROC_PIDLISTFDS: c_int = 1;
const PROC_PIDFDVNODEPATHINFO: c_int = 2;
const PROC_PIDVNODEPATHINFO: c_int = 9;
const PROX_FDTYPE_VNODE: u32 = 1;

#[repr(C)]
struct VnodeInfoPath {
    info: [u64; 19],
    path: [u8; 1024],
}

#[repr(C)]
struct ProcVnodePathInfo {
    cdir: VnodeInfoPath,
    rdir: VnodeInfoPath,
}

#[repr(C)]
struct VnodeFdInfoWithPath {
    file: [u64; 3],
    vnode: VnodeInfoPath,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcFdInfo {
    fd: i32,
    fdtype: u32,
}

const _: () = assert!(std::mem::size_of::<ProcVnodePathInfo>() == 2352);
const _: () = assert!(std::mem::size_of::<VnodeFdInfoWithPath>() == 1200);

unsafe extern "C" {
    fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
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

fn path_of(vnode: &VnodeInfoPath) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    let end = vnode.path.iter().position(|byte| *byte == 0)?;
    if end == 0 {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(
        &vnode.path[..end],
    )))
}

fn byte_len<T>(count: usize) -> c_int {
    c_int::try_from(count.saturating_mul(std::mem::size_of::<T>())).unwrap_or(c_int::MAX)
}

/// Lists every process id the caller can see.
pub fn pids() -> Result<Vec<c_int>, String> {
    let mut capacity = 4096usize;
    loop {
        let mut buffer: Vec<c_int> = vec![0; capacity];
        let count = unsafe {
            proc_listallpids(buffer.as_mut_ptr().cast(), byte_len::<c_int>(buffer.len()))
        };
        let Ok(count) = usize::try_from(count) else {
            return Err(format!(
                "proc_listallpids failed: {}",
                std::io::Error::last_os_error()
            ));
        };
        if count == 0 {
            return Err("proc_listallpids listed no processes".to_owned());
        }
        if count < capacity {
            buffer.truncate(count);
            return Ok(buffer);
        }
        capacity = capacity.saturating_mul(2);
    }
}

fn cwd(pid: c_int) -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<ProcVnodePathInfo>::zeroed();
    let size = byte_len::<ProcVnodePathInfo>(1);
    let written = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    path_of(&info.cdir)
}

fn fds(pid: c_int) -> Vec<ProcFdInfo> {
    let needed = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    let Ok(needed) = usize::try_from(needed) else {
        return Vec::new();
    };
    let mut buffer =
        vec![ProcFdInfo { fd: 0, fdtype: 0 }; needed / std::mem::size_of::<ProcFdInfo>() + 32];
    let written = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDLISTFDS,
            0,
            buffer.as_mut_ptr().cast(),
            byte_len::<ProcFdInfo>(buffer.len()),
        )
    };
    let Ok(written) = usize::try_from(written) else {
        return Vec::new();
    };
    buffer.truncate(written / std::mem::size_of::<ProcFdInfo>());
    buffer
}

fn fd_path(pid: c_int, fd: c_int) -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<VnodeFdInfoWithPath>::zeroed();
    let size = byte_len::<VnodeFdInfoWithPath>(1);
    let written = unsafe {
        proc_pidfdinfo(
            pid,
            fd,
            PROC_PIDFDVNODEPATHINFO,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    path_of(&info.vnode)
}

/// The working directory and open file paths of one process.
pub fn open_paths(pid: c_int) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = cwd(pid).into_iter().collect();
    for fd in fds(pid) {
        if fd.fdtype == PROX_FDTYPE_VNODE {
            paths.extend(fd_path(pid, fd.fd));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_reports_its_cwd_and_an_open_file() {
        let scratch = dx_test_scratch::scratch("dx-clean-darwin-");
        let real = std::fs::canonicalize(scratch.path()).expect("real scratch");
        let file = real.join("held");
        let _held = std::fs::File::create(&file).expect("hold a file");
        let own = c_int::try_from(std::process::id()).expect("pid fits");
        assert!(pids().expect("list pids").contains(&own));
        let paths = open_paths(own);
        assert!(paths.contains(&file), "{paths:?}");
        let here = std::fs::canonicalize(std::env::current_dir().expect("cwd")).expect("real cwd");
        assert!(paths.contains(&here), "{paths:?}");
    }
}
