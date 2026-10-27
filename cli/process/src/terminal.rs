//! A watcher-led foreground group on the parent's terminal for one streamed child.
//!
//! The watcher never execs, so it calls only async-signal-safe libc functions.

use super::STOP_SIGNALS;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Only one run in a process may hold the terminal at a time.
static HELD: AtomicBool = AtomicBool::new(false);
/// dx's pid, read by the watcher's signal handler.
static WATCHER_PARENT: AtomicI32 = AtomicI32::new(0);
/// The pipe end the watcher's signal handler reports into.
static WATCHER_REPORT: AtomicI32 = AtomicI32::new(-1);

const JOB_STOPS: [i32; 3] = [libc::SIGTSTP, libc::SIGTTIN, libc::SIGTTOU];
const MAX_CLOSE: i32 = 65536;

/// Releases the one-holder guard when the job ends.
struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        HELD.store(false, Ordering::SeqCst);
    }
}

/// A child's foreground process group on the parent's terminal.
pub struct TerminalJob {
    tty: File,
    own: i32,
    group: i32,
    report: File,
    _guard: Guard,
}

impl TerminalJob {
    /// Starts a watcher-led foreground group, or `None` when this process
    /// has no terminal in the foreground or any step fails.
    pub fn start() -> Option<TerminalJob> {
        if HELD.swap(true, Ordering::SeqCst) {
            return None;
        }
        let guard = Guard;
        let tty = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let own = unsafe { libc::getpgrp() };
        if unsafe { libc::tcgetpgrp(tty.as_raw_fd()) } != own {
            return None;
        }
        let (read, write) = report_pipe()?;
        let parent = unsafe { libc::getpid() };
        WATCHER_PARENT.store(parent, Ordering::SeqCst);
        WATCHER_REPORT.store(write.as_raw_fd(), Ordering::SeqCst);
        let pid = {
            let _blocked = Blocked::new(&STOP_SIGNALS);
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                // SAFETY: the forked copy runs only async-signal-safe libc
                // calls and never returns.
                unsafe { watch(parent, write.as_raw_fd()) }
            }
            pid
        };
        drop(write);
        if pid < 0 {
            return None;
        }
        let job = TerminalJob {
            tty,
            own,
            group: pid,
            report: read,
            _guard: guard,
        };
        if unsafe { libc::setpgid(pid, pid) } != 0 && unsafe { libc::getpgid(pid) } != pid {
            return None;
        }
        let handed = {
            let _blocked = Blocked::new(&[libc::SIGTTOU]);
            unsafe { libc::tcsetpgrp(job.tty.as_raw_fd(), pid) == 0 }
        };
        handed.then_some(job)
    }

    /// The foreground group the child joins.
    pub fn pgid(&self) -> i32 {
        self.group
    }

    /// The next stop signal the terminal delivered to the group, if any.
    pub fn take_reported(&self) -> Option<i32> {
        let mut byte = 0u8;
        let read = unsafe {
            libc::read(
                self.report.as_raw_fd(),
                std::ptr::addr_of_mut!(byte).cast(),
                1,
            )
        };
        (read == 1).then_some(i32::from(byte))
    }

    /// Follows a job-control stop of the child: hands the terminal back,
    /// stops this process like the shell expects, then resumes the group.
    pub fn tend(&self, child: i32) {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let waited = unsafe {
            libc::waitid(
                libc::P_PID,
                child as libc::id_t,
                &mut info,
                libc::WSTOPPED | libc::WNOHANG,
            )
        };
        if waited != 0 || unsafe { info.si_pid() } == 0 || info.si_code != libc::CLD_STOPPED {
            return;
        }
        if !JOB_STOPS.contains(&unsafe { info.si_status() }) {
            return;
        }
        self.hand_over(self.group, self.own);
        unsafe {
            libc::raise(libc::SIGTSTP);
        }
        self.hand_over(self.own, self.group);
        unsafe {
            libc::kill(-self.group, libc::SIGCONT);
        }
    }

    /// Moves the terminal to `to` only while `from` still holds it.
    fn hand_over(&self, from: i32, to: i32) {
        let _blocked = Blocked::new(&[libc::SIGTTOU]);
        let fd = self.tty.as_raw_fd();
        unsafe {
            if libc::tcgetpgrp(fd) == from {
                libc::tcsetpgrp(fd, to);
            }
        }
    }
}

impl Drop for TerminalJob {
    fn drop(&mut self) {
        self.hand_over(self.group, self.own);
        unsafe {
            libc::kill(self.group, libc::SIGKILL);
            libc::waitpid(self.group, std::ptr::null_mut(), 0);
        }
    }
}

/// Lets this thread write to the terminal while dx is in the background.
pub fn allow_background_tty_writes() {
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTTOU);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}

/// Blocks signals in the calling thread until dropped.
struct Blocked {
    previous: libc::sigset_t,
}

impl Blocked {
    fn new(signals: &[i32]) -> Blocked {
        let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
        let mut previous: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut set);
            for &signo in signals {
                libc::sigaddset(&mut set, signo);
            }
            libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut previous);
        }
        Blocked { previous }
    }
}

impl Drop for Blocked {
    fn drop(&mut self) {
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}

/// A close-on-exec, non-blocking pipe: (read end, write end).
fn report_pipe() -> Option<(File, File)> {
    let mut fds = [0i32; 2];
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
            return None;
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return None;
        }
        for fd in fds {
            unsafe {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
    }
    let [read, write] = fds;
    // SAFETY: pipe returned two fresh descriptors this function owns.
    Some(unsafe { (File::from_raw_fd(read), File::from_raw_fd(write)) })
}

/// Whether dx sent this signal itself, so the child already has it.
unsafe fn from_parent(info: *const libc::siginfo_t) -> bool {
    if info.is_null() {
        return false;
    }
    let sender = unsafe { (*info).si_pid() };
    if sender != WATCHER_PARENT.load(Ordering::Relaxed) {
        return false;
    }
    if cfg!(any(target_os = "linux", target_os = "android")) {
        unsafe { (*info).si_code <= 0 }
    } else {
        true
    }
}

extern "C" fn report_signal(signo: libc::c_int, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    if unsafe { from_parent(info) } {
        return;
    }
    let byte = signo as u8;
    unsafe {
        libc::write(
            WATCHER_REPORT.load(Ordering::Relaxed),
            std::ptr::addr_of!(byte).cast(),
            1,
        );
    }
}

/// Closes every descriptor from 3 up except `keep`.
unsafe fn close_others(keep: i32) {
    #[cfg(target_os = "linux")]
    {
        let all = libc::c_uint::MAX;
        let ranges: [(libc::c_uint, libc::c_uint); 2] = if keep >= 3 {
            [
                (3, (keep - 1) as libc::c_uint),
                (keep as libc::c_uint + 1, all),
            ]
        } else {
            [(3, all), (1, 0)]
        };
        let mut closed = true;
        for (first, last) in ranges {
            if first <= last && unsafe { libc::syscall(libc::SYS_close_range, first, last, 0) } != 0
            {
                closed = false;
            }
        }
        if closed {
            return;
        }
    }
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    let mut top = MAX_CLOSE;
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0
        && limit.rlim_cur < MAX_CLOSE as libc::rlim_t
    {
        top = limit.rlim_cur as i32;
    }
    let mut fd = 3;
    while fd < top {
        if fd != keep {
            unsafe {
                libc::close(fd);
            }
        }
        fd += 1;
    }
}

/// The watcher's whole life: lead G, report stop signals, die with dx.
unsafe fn watch(parent: i32, report: i32) -> ! {
    unsafe {
        libc::setpgid(0, 0);
        close_others(report);
        #[cfg(any(target_os = "linux", target_os = "android"))]
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        if libc::getppid() != parent {
            libc::_exit(0);
        }
        for signo in JOB_STOPS {
            libc::signal(signo, libc::SIG_IGN);
        }
        let mut unblock: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut unblock);
        for signo in STOP_SIGNALS {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = report_signal as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signo, &action, std::ptr::null_mut());
            libc::sigaddset(&mut unblock, signo);
        }
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &unblock, std::ptr::null_mut());
        let second = libc::timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        loop {
            if libc::getppid() != parent {
                libc::_exit(0);
            }
            libc::nanosleep(&second, std::ptr::null_mut());
        }
    }
}
