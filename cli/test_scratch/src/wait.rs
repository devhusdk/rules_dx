use std::fs::File;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

pub fn within<T: Send + 'static>(
    what: &str,
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || drop(sender.send(work())));
    receiver.recv_timeout(limit).map_err(|error| match error {
        RecvTimeoutError::Timeout => format!("{what} did not happen within {limit:?}"),
        RecvTimeoutError::Disconnected => format!("{what}: the waiting thread panicked"),
    })
}

pub fn path_appears(path: &Path, limit: Duration) -> Result<(), String> {
    let what = format!("{} appearing", path.display());
    watch_until(path, &what, limit, || path.exists())
}

const RECHECK_EVERY: Duration = Duration::from_millis(50);

pub fn watch_until(
    path: &Path,
    what: &str,
    limit: Duration,
    mut done: impl FnMut() -> bool,
) -> Result<(), String> {
    use notify::{RecursiveMode, Watcher as _};
    let deadline = Instant::now() + limit;
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent to watch", path.display()))?;
    let watched = parent
        .ancestors()
        .find(|dir| dir.is_dir())
        .ok_or_else(|| format!("no ancestor of {} exists", path.display()))?;
    let mode = if watched == parent {
        RecursiveMode::NonRecursive
    } else {
        RecursiveMode::Recursive
    };
    let (sender, events) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(sender)
        .map_err(|error| format!("watch {}: {error}", watched.display()))?;
    watcher
        .watch(watched, mode)
        .map_err(|error| format!("watch {}: {error}", watched.display()))?;
    while !done() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!("{what} did not happen within {limit:?}"));
        }
        match events.recv_timeout(left.min(RECHECK_EVERY)) {
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(format!("the watcher on {} stopped", watched.display()));
            }
        }
    }
    Ok(())
}

fn lock_within(
    path: &Path,
    limit: Duration,
    lock: fn(&File) -> std::io::Result<()>,
) -> Result<File, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let what = format!("the lock on {} coming free", path.display());
    within(&what, limit, move || lock(&file).map(|()| file))?
        .map_err(|error| format!("lock {}: {error}", path.display()))
}

pub fn locked(path: &Path, limit: Duration) -> Result<File, String> {
    lock_within(path, limit, File::lock)
}

pub fn locked_shared(path: &Path, limit: Duration) -> Result<File, String> {
    lock_within(path, limit, File::lock_shared)
}

pub fn lock_free(path: &Path, limit: Duration) -> Result<(), String> {
    locked(path, limit)?
        .unlock()
        .map_err(|error| format!("unlock {}: {error}", path.display()))
}

pub fn pid_gone(pid: u32, limit: Duration) -> Result<(), String> {
    match dx_os_sys::exit::wait_gone(pid, limit) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!("process {pid} still runs after {limit:?}")),
        Err(error) => Err(format!("wait for process {pid}: {error}")),
    }
}

pub fn child_exits(child: &mut Child, limit: Duration) -> Result<ExitStatus, String> {
    pid_gone(child.id(), limit)?;
    child
        .wait()
        .map_err(|error| format!("reap process {}: {error}", child.id()))
}

#[derive(Debug)]
pub enum Exit {
    Exited(ExitStatus),
    Killed(ExitStatus),
}

pub fn exit_within(mut child: Child, limit: Duration) -> std::io::Result<Exit> {
    if dx_os_sys::exit::wait_gone(child.id(), limit)? {
        return child.wait().map(Exit::Exited);
    }
    child.kill()?;
    child.wait().map(Exit::Killed)
}
