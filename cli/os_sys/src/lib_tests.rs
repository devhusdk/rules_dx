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

#[cfg(unix)]
#[test]
fn the_space_of_the_temp_dir_is_known() {
    let space = fs::space(&std::env::temp_dir()).expect("statvfs");
    assert!(space.total > 0);
    assert!(space.free <= space.total);
}

#[cfg(unix)]
#[test]
fn the_space_of_a_missing_dir_is_an_error() {
    assert!(fs::space(std::path::Path::new("/no/such/dx/dir")).is_err());
}

#[cfg(unix)]
#[test]
fn a_file_is_not_a_terminal() {
    let file = std::fs::File::open("/dev/null").expect("open");
    assert!(tty::columns(&file).is_err());
    assert!(tty::foreground_group(&file).is_err());
}

#[cfg(unix)]
#[test]
fn the_errno_slot_is_this_threads_errno() {
    crate::errno::set(7);
    assert!(!crate::errno::slot().is_null());
    assert_eq!(crate::errno::get(), 7);
}
