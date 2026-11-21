use super::*;
use std::io::Read;
use std::sync::Mutex;

static STATICS: Mutex<()> = Mutex::new(());

#[cfg(any(target_os = "linux", target_os = "android"))]
fn info_from(pid: i32, code: i32) -> libc::siginfo_t {
    // SAFETY: siginfo_t is plain C data for which all-zero is valid.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    info.si_code = code;
    let align = std::mem::size_of::<usize>();
    let offset = (12 + align - 1) & !(align - 1);
    let base = std::ptr::addr_of_mut!(info).cast::<u8>();
    // SAFETY: si_pid follows the three ints of siginfo_t, padded to pointer alignment,
    // so base + offset stays inside info.
    let pid_field = unsafe { base.add(offset) }.cast::<i32>();
    // SAFETY: pid_field points at the aligned si_pid field of the live info.
    unsafe { pid_field.write(pid) };
    info
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn info_from(pid: i32, code: i32) -> libc::siginfo_t {
    // SAFETY: siginfo_t is plain C data for which all-zero is valid.
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
fn a_missing_signal_info_is_not_from_dx() {
    let _lock = STATICS.lock().expect("lock");
    assert!(!from_parent(None));
}

#[test]
fn only_user_sent_signals_from_dx_are_filtered() {
    let _lock = STATICS.lock().expect("lock");
    WATCHER_PARENT.store(4242, Ordering::SeqCst);
    assert!(from_parent(Some(&info_from(4242, 0))));
    assert_eq!(from_parent(Some(&info_from(4242, KERNEL.0))), KERNEL.1);
    assert!(!from_parent(Some(&info_from(77, 0))));
}

#[test]
fn the_signal_handler_reports_foreign_signals_and_keeps_errno() {
    let _lock = STATICS.lock().expect("lock");
    let (read, write) = crate::fd::pipe_nonblocking().expect("pipe");
    WATCHER_PARENT.store(-5, Ordering::SeqCst);
    WATCHER_REPORT.store(write.as_raw_fd(), Ordering::SeqCst);
    crate::errno::set(9);
    report_signal(libc::SIGTERM, std::ptr::null_mut(), std::ptr::null_mut());
    assert_eq!(crate::errno::get(), 9);
    let mut byte = [0u8; 1];
    let got = std::fs::File::from(read).read(&mut byte).expect("read");
    assert_eq!((got, i32::from(byte[0])), (1, libc::SIGTERM));
    WATCHER_REPORT.store(-1, Ordering::SeqCst);
}

#[test]
fn settling_reports_pending_stop_signals_before_the_settled_byte() {
    let _lock = STATICS.lock().expect("lock");
    let (read, write) = crate::fd::pipe_nonblocking().expect("pipe");
    WATCHER_REPORT.store(write.as_raw_fd(), Ordering::SeqCst);
    std::thread::spawn(move || {
        let blocked = Blocked::new(&[libc::SIGHUP]);
        // SAFETY: pthread_self has no preconditions.
        let me = unsafe { libc::pthread_self() };
        // SAFETY: pthread_kill targets this live thread, which has SIGHUP blocked.
        unsafe { libc::pthread_kill(me, libc::SIGHUP) };
        crate::errno::set(9);
        report_settled(libc::SIGUSR1, std::ptr::null_mut(), std::ptr::null_mut());
        assert_eq!(crate::errno::get(), 9);
        let hup = set_of(&[libc::SIGHUP]);
        let mut taken = 0;
        // SAFETY: hup and taken are valid; SIGHUP is pending and blocked, so sigwait returns.
        unsafe { libc::sigwait(&raw const hup, &raw mut taken) };
        assert_eq!(taken, libc::SIGHUP);
        drop(blocked);
    })
    .join()
    .expect("thread");
    WATCHER_REPORT.store(-1, Ordering::SeqCst);
    drop(write);
    let mut bytes = [0u8; 3];
    let got = std::fs::File::from(read).read(&mut bytes).expect("read");
    assert_eq!(got, 2);
    assert_eq!(
        bytes[..2],
        [u8::try_from(libc::SIGHUP).expect("signal"), SETTLED]
    );
}
