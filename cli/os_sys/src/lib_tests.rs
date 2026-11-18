use super::*;

#[test]
fn stop_signals_are_sigint_sigterm_and_sighup() {
    assert_eq!(STOP_SIGNALS, [2, 15, 1]);
}

#[cfg(unix)]
#[test]
fn an_errno_guard_restores_errno() {
    errno::set(9);
    {
        let _guard = errno::ErrnoGuard::save();
        errno::set(4);
        assert_eq!(errno::get(), 4);
    }
    assert_eq!(errno::get(), 9);
}

#[cfg(unix)]
#[test]
fn a_nonblocking_pipe_carries_bytes_and_never_blocks() {
    use std::io::{ErrorKind, Read, Write};
    let (read, write) = fd::pipe_nonblocking().expect("pipe");
    let (mut read, mut write) = (std::fs::File::from(read), std::fs::File::from(write));
    let mut byte = [0u8; 1];
    assert_eq!(
        read.read(&mut byte).expect_err("empty").kind(),
        ErrorKind::WouldBlock
    );
    assert!(!fd::wait_readable(&read, std::time::Duration::ZERO).expect("poll"));
    write.write_all(&[7]).expect("write");
    assert!(fd::wait_readable(&read, std::time::Duration::from_secs(1)).expect("poll"));
    assert_eq!(read.read(&mut byte).expect("read"), 1);
    assert_eq!(byte, [7]);
}

#[cfg(unix)]
#[test]
fn set_inheritable_clears_close_on_exec() {
    let (read, _write) = fd::pipe_nonblocking().expect("pipe");
    fd::set_inheritable(&read).expect("fcntl");
    assert!(
        !rustix::io::fcntl_getfd(&read)
            .expect("fcntl")
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
}

#[test]
fn the_space_of_the_temp_dir_is_known() {
    let space = fs::space(&std::env::temp_dir()).expect("statvfs");
    assert!(space.total > 0);
    assert!(space.free <= space.total);
}

#[test]
fn the_space_of_a_missing_dir_is_an_error() {
    assert!(fs::space(&std::env::temp_dir().join("no/such/dx/dir")).is_err());
}

#[cfg(unix)]
#[test]
fn a_file_is_not_a_terminal() {
    let file = std::fs::File::open("/dev/null").expect("open");
    assert!(tty::columns(&file).is_err());
    assert!(tty::foreground_group(&file).is_err());
}

#[cfg(unix)]
fn scratch_file(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("dx-os-sys-{}-{name}", std::process::id()));
    std::fs::write(&path, "").expect("scratch file");
    path
}

#[cfg(unix)]
#[test]
fn the_user_ids_own_a_new_file() {
    use std::os::unix::fs::MetadataExt;
    let file = scratch_file("owned");
    let owner = std::fs::metadata(&file).expect("metadata").uid();
    std::fs::remove_file(&file).expect("remove");
    assert_eq!(process::user_id(), owner);
    assert_eq!(process::effective_user_id(), owner);
}

#[cfg(target_os = "linux")]
#[test]
#[expect(unsafe_code, reason = "F_OFD_SETLK has no safe std API")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the fcntl lock constants fit the short flock fields"
)]
fn the_write_lock_probe_sees_ofd_locks_and_ignores_flock() {
    use std::os::fd::AsRawFd;
    let path = scratch_file("lock");
    let probe = std::fs::File::open(&path).expect("open");
    assert!(!fs::write_locked(&probe).expect("probe"));

    let flocked = std::fs::File::open(&path).expect("open");
    flocked.try_lock().expect("flock");
    assert!(!fs::write_locked(&probe).expect("probe"));
    drop(flocked);

    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open");
    // SAFETY: a zeroed `flock` is valid; the fields the call reads are set below.
    let mut request: libc::flock = unsafe { std::mem::zeroed() };
    request.l_type = libc::F_WRLCK as libc::c_short;
    request.l_whence = libc::SEEK_SET as libc::c_short;
    // SAFETY: the fd is open and `request` is a valid `flock`.
    let status = unsafe { libc::fcntl(held.as_raw_fd(), libc::F_OFD_SETLK, &mut request) };
    assert_eq!(status, 0, "{}", std::io::Error::last_os_error());
    assert!(fs::write_locked(&probe).expect("probe"));
    drop(held);
    assert!(!fs::write_locked(&probe).expect("probe"));
    std::fs::remove_file(&path).expect("remove");
}
