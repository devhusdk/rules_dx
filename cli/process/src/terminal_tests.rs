use super::*;
use std::sync::Mutex;

static STATICS: Mutex<()> = Mutex::new(());

#[test]
fn a_report_pipe_is_non_blocking_and_carries_bytes() {
    let (read, write) = report_pipe().expect("pipe");
    let mut empty = [0u8; 1];
    let none = unsafe { libc::read(read.as_raw_fd(), empty.as_mut_ptr().cast(), 1) };
    assert_eq!(none, -1);
    assert_eq!(
        unsafe { libc::write(write.as_raw_fd(), [7u8].as_ptr().cast(), 1) },
        1
    );
    let got = unsafe { libc::read(read.as_raw_fd(), empty.as_mut_ptr().cast(), 1) };
    assert_eq!((got, empty[0]), (1, 7));
}

#[test]
fn blocked_signals_come_back_when_dropped() {
    fn is_blocked(signo: i32) -> bool {
        let mut current: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), &mut current) };
        unsafe { libc::sigismember(&current, signo) == 1 }
    }
    assert!(!is_blocked(libc::SIGUSR2));
    {
        let _blocked = Blocked::new(&[libc::SIGUSR2]);
        assert!(is_blocked(libc::SIGUSR2));
    }
    assert!(!is_blocked(libc::SIGUSR2));
}

#[test]
fn background_tty_writes_block_sigttou_in_this_thread() {
    std::thread::spawn(|| {
        allow_background_tty_writes();
        let mut current: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), &mut current) };
        assert_eq!(unsafe { libc::sigismember(&current, libc::SIGTTOU) }, 1);
    })
    .join()
    .expect("thread");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn info_from(pid: i32, code: i32) -> libc::siginfo_t {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    info.si_code = code;
    let align = std::mem::size_of::<usize>();
    let offset = (12 + align - 1) & !(align - 1);
    // SAFETY: siginfo_t is a plain C struct and si_pid follows the three ints, padded to pointer alignment.
    unsafe {
        let base = std::ptr::addr_of_mut!(info).cast::<u8>();
        base.add(offset).cast::<i32>().write(pid);
    }
    info
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn info_from(pid: i32, code: i32) -> libc::siginfo_t {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    info.si_code = code;
    info.si_pid = pid;
    info
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const KERNEL: (i32, bool) = (libc::SI_KERNEL, false);
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const KERNEL: (i32, bool) = (0x80, true);

#[test]
fn a_null_signal_info_is_not_from_dx() {
    let _lock = STATICS.lock().unwrap();
    assert!(!unsafe { from_parent(std::ptr::null()) });
}

#[test]
fn only_user_sent_signals_from_dx_are_filtered() {
    let _lock = STATICS.lock().unwrap();
    WATCHER_PARENT.store(4242, Ordering::SeqCst);
    let user = info_from(4242, 0);
    assert!(unsafe { from_parent(&user) });
    let kernel = info_from(4242, KERNEL.0);
    assert_eq!(unsafe { from_parent(&kernel) }, KERNEL.1);
    let stranger = info_from(77, 0);
    assert!(!unsafe { from_parent(&stranger) });
}

#[test]
fn the_signal_handler_reports_foreign_signals_and_keeps_errno() {
    let _lock = STATICS.lock().unwrap();
    let (read, write) = report_pipe().expect("pipe");
    WATCHER_PARENT.store(-5, Ordering::SeqCst);
    WATCHER_REPORT.store(write.as_raw_fd(), Ordering::SeqCst);
    unsafe { *crate::errno_slot() = 9 };
    report_signal(libc::SIGTERM, std::ptr::null_mut(), std::ptr::null_mut());
    assert_eq!(unsafe { *crate::errno_slot() }, 9);
    let mut byte = [0u8; 1];
    let got = unsafe { libc::read(read.as_raw_fd(), byte.as_mut_ptr().cast(), 1) };
    assert_eq!((got, i32::from(byte[0])), (1, libc::SIGTERM));
    WATCHER_REPORT.store(-1, Ordering::SeqCst);
}
