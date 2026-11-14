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
/// Asks the watcher to report its pending stop signals and then [`SETTLED`].
const SETTLE: i32 = libc::SIGUSR1;
/// The byte the watcher writes once every earlier stop signal is reported.
const SETTLED: u8 = 0;
/// How long [`TerminalJob::settle`] waits for the watcher.
const SETTLE_WAIT_MS: i32 = 1000;
/// The signals blocked while the watcher forks.
const WATCHED: [i32; 4] = [STOP_SIGNALS[0], STOP_SIGNALS[1], STOP_SIGNALS[2], SETTLE];
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

// LCOV_EXCL_START - reason: needs a controlling terminal in the foreground, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
        // SAFETY: getpgrp has no preconditions.
        let own = unsafe { libc::getpgrp() };
        // SAFETY: tty is an open descriptor.
        if unsafe { libc::tcgetpgrp(tty.as_raw_fd()) } != own {
            return None;
        }
        let (read, write) = report_pipe()?;
        // SAFETY: getpid has no preconditions.
        let parent = unsafe { libc::getpid() };
        WATCHER_PARENT.store(parent, Ordering::SeqCst);
        WATCHER_REPORT.store(write.as_raw_fd(), Ordering::SeqCst);
        let pid = {
            let _blocked = Blocked::new(&WATCHED);
            // SAFETY: fork takes no arguments; the child immediately enters `watch`.
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                // SAFETY: the forked copy runs only async-signal-safe libc calls.
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
        // SAFETY: setpgid and getpgid take plain pids and have no memory preconditions.
        if unsafe { libc::setpgid(pid, pid) } != 0 && unsafe { libc::getpgid(pid) } != pid {
            return None;
        }
        let handed = {
            let _blocked = Blocked::new(&[libc::SIGTTOU]);
            // SAFETY: tcsetpgrp takes the open tty descriptor and a pid; SIGTTOU is blocked.
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
        // SAFETY: the report fd is open and byte is a valid one-byte buffer.
        let read = unsafe {
            libc::read(
                self.report.as_raw_fd(),
                std::ptr::addr_of_mut!(byte).cast(),
                1,
            )
        };
        (read == 1 && byte != SETTLED).then_some(i32::from(byte))
    }

    /// Returns every stop signal the group got before this call, once the watcher confirms it reported them all.
    pub fn settle(&self) -> Vec<i32> {
        let mut reported = Vec::new();
        // SAFETY: kill takes plain integers and has no memory preconditions.
        if unsafe { libc::kill(self.group, SETTLE) } != 0 {
            return reported;
        }
        let fd = self.report.as_raw_fd();
        loop {
            let mut ready = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: ready is one valid pollfd and the count passed is 1.
            if unsafe { libc::poll(&mut ready, 1, SETTLE_WAIT_MS) } <= 0 {
                return reported;
            }
            let mut byte = 0u8;
            // SAFETY: byte is a valid one-byte buffer for the length passed.
            let read = unsafe { libc::read(fd, std::ptr::addr_of_mut!(byte).cast(), 1) };
            match (read, byte) {
                (1, SETTLED) => return reported,
                (1, signo) => reported.push(i32::from(signo)),
                _ => return reported,
            }
        }
    }

    /// Follows a job-control stop of the child: hands the terminal back,
    /// stops this process like the shell expects, then resumes the group.
    pub fn tend(&self, child: i32) {
        // SAFETY: siginfo_t is plain C data for which all-zero is valid.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: info is a valid siginfo_t out-pointer for waitid.
        let waited = unsafe {
            libc::waitid(
                libc::P_PID,
                child as libc::id_t,
                &mut info,
                libc::WSTOPPED | libc::WNOHANG,
            )
        };
        // SAFETY: waited == 0 means waitid filled info, so reading si_pid is valid.
        if waited != 0 || unsafe { info.si_pid() } == 0 || info.si_code != libc::CLD_STOPPED {
            return;
        }
        // SAFETY: info was filled by waitid for a stopped child, so si_status is valid.
        if !JOB_STOPS.contains(&unsafe { info.si_status() }) {
            return;
        }
        self.hand_over(self.group, self.own);
        // SAFETY: raise takes a signal number and has no memory preconditions.
        unsafe {
            libc::raise(libc::SIGTSTP);
        }
        self.hand_over(self.own, self.group);
        // SAFETY: kill takes plain integers and has no memory preconditions.
        unsafe {
            libc::kill(-self.group, libc::SIGCONT);
        }
    }

    /// Moves the terminal to `to` only while `from` still holds it.
    fn hand_over(&self, from: i32, to: i32) {
        let _blocked = Blocked::new(&[libc::SIGTTOU]);
        let fd = self.tty.as_raw_fd();
        // SAFETY: tcgetpgrp and tcsetpgrp take the open tty descriptor and plain pids.
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
        // SAFETY: kill and waitpid take plain pids; a null status pointer is permitted.
        unsafe {
            libc::kill(self.group, libc::SIGKILL);
            libc::waitpid(self.group, std::ptr::null_mut(), 0);
        }
    }
}
// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

/// Lets this thread write to the terminal while dx is in the background.
pub fn allow_background_tty_writes() {
    // SAFETY: sigset_t is plain C data for which all-zero is valid.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: set is a valid sigset_t and a null old-set pointer is permitted.
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
        // SAFETY: sigset_t is plain C data for which all-zero is valid.
        let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
        // SAFETY: sigset_t is plain C data for which all-zero is valid.
        let mut previous: libc::sigset_t = unsafe { std::mem::zeroed() };
        // SAFETY: set and previous are valid sigset_t values for these calls.
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
        // SAFETY: previous was saved by pthread_sigmask; a null old-set pointer is permitted.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}

/// Fills `fds` with a close-on-exec, non-blocking pipe; false if the pipe failed.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn raw_pipe(fds: &mut [i32; 2]) -> bool {
    // SAFETY: fds points to two ints, as pipe2 requires.
    unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) == 0 }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn raw_pipe(fds: &mut [i32; 2]) -> bool {
    // SAFETY: fds points to two ints, as pipe requires.
    let piped = unsafe { libc::pipe(fds.as_mut_ptr()) } == 0;
    for &fd in fds.iter().filter(|_| piped) {
        // SAFETY: fd is an open pipe descriptor owned by this function.
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    piped
}

/// A close-on-exec, non-blocking pipe: (read end, write end).
fn report_pipe() -> Option<(File, File)> {
    let mut fds = [0i32; 2];
    if !raw_pipe(&mut fds) {
        return None; // LCOV_EXCL_LINE - reason: pipe fails only without descriptors, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
    }
    let [read, write] = fds;
    // SAFETY: pipe returned two fresh descriptors this function owns.
    Some(unsafe { (File::from_raw_fd(read), File::from_raw_fd(write)) })
}

/// Whether a signal with this `si_code` was sent by a process rather than the kernel.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn user_sent(code: i32) -> bool {
    code <= 0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn user_sent(_code: i32) -> bool {
    true
}

/// Whether dx sent this signal itself, so the child already has it.
unsafe fn from_parent(info: *const libc::siginfo_t) -> bool {
    if info.is_null() {
        return false;
    }
    // SAFETY: info was checked non-null and the caller passes a valid siginfo_t.
    let sender = unsafe { (*info).si_pid() };
    if sender != WATCHER_PARENT.load(Ordering::Relaxed) {
        return false;
    }
    // SAFETY: info was checked non-null and the caller passes a valid siginfo_t.
    user_sent(unsafe { (*info).si_code })
}

extern "C" fn report_signal(signo: libc::c_int, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    let errno = crate::errno_slot();
    // SAFETY: errno_slot returns this thread's errno, valid for reads and writes.
    let saved = unsafe { *errno };
    // SAFETY: info is the siginfo_t the kernel passed to this handler.
    if !unsafe { from_parent(info) } {
        // SAFETY: report is async-signal-safe and signo is the delivered signal.
        unsafe { report(signo) };
    }
    // SAFETY: errno is the slot read above and still valid for writes.
    unsafe { *errno = saved };
}

extern "C" fn report_settled(_: libc::c_int, _: *mut libc::siginfo_t, _: *mut libc::c_void) {
    let errno = crate::errno_slot();
    // SAFETY: errno_slot returns this thread's errno, valid for reads and writes.
    let saved = unsafe { *errno };
    // SAFETY: sigset_t is plain C data for which all-zero is valid.
    let mut pending: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: pending is a valid sigset_t; report only writes to the watcher pipe.
    unsafe {
        libc::sigemptyset(&mut pending);
        libc::sigpending(&mut pending);
        for signo in STOP_SIGNALS {
            if libc::sigismember(&pending, signo) == 1 {
                report(signo);
            }
        }
        report(libc::c_int::from(SETTLED));
        *errno = saved;
    }
}

unsafe fn report(signo: libc::c_int) {
    let byte = signo as u8;
    // SAFETY: the pipe descriptor is open and byte is a valid one-byte buffer.
    unsafe {
        libc::write(
            WATCHER_REPORT.load(Ordering::Relaxed),
            std::ptr::addr_of!(byte).cast(),
            1,
        );
    }
}

// LCOV_EXCL_START - reason: runs only in the forked watcher, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
            // SAFETY: close_range takes plain integers and only closes descriptors.
            if first <= last && unsafe { libc::syscall(libc::SYS_close_range, first, last, 0) } != 0
            {
                closed = false;
            }
        }
        if closed {
            return;
        }
    }
    // SAFETY: rlimit is plain C data for which all-zero is valid.
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    let mut top = MAX_CLOSE;
    // SAFETY: limit is a valid rlimit out-pointer.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0
        && limit.rlim_cur < MAX_CLOSE as libc::rlim_t
    {
        top = limit.rlim_cur as i32;
    }
    let mut fd = 3;
    while fd < top {
        if fd != keep {
            // SAFETY: the forked watcher uses no descriptor other than the kept report pipe.
            unsafe {
                libc::close(fd);
            }
        }
        fd += 1;
    }
}

/// The watcher's whole life: lead G, report stop signals, die with dx.
unsafe fn watch(parent: i32, report: i32) -> ! {
    // SAFETY: this runs only in the forked child and calls async-signal-safe functions.
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
        for signo in WATCHED {
            libc::sigaddset(&mut unblock, signo);
        }
        for signo in WATCHED {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = if signo == SETTLE {
                report_settled as *const () as libc::sighandler_t
            } else {
                report_signal as *const () as libc::sighandler_t
            };
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            action.sa_mask = unblock;
            libc::sigaction(signo, &action, std::ptr::null_mut());
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

// LCOV_EXCL_STOP - reason: end of excluded branch, issue: 8104, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod terminal_tests;
