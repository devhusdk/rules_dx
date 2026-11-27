#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(prefix: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("scratch tempdir")
    }

    const HOLD: &str = "DX_TEST_WAIT_HOLD";

    #[test]
    fn holds_until_stdin_closes() {
        if std::env::var_os(HOLD).is_some() {
            drop(std::io::Read::read_to_end(
                &mut std::io::stdin(),
                &mut Vec::new(),
            ));
        }
    }

    fn holder() -> std::process::Child {
        std::process::Command::new(std::env::current_exe().expect("exe"))
            .args(["--exact", "tests::holds_until_stdin_closes"])
            .env(HOLD, "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawn")
    }

    const LONG: std::time::Duration = std::time::Duration::from_secs(60);

    #[test]
    fn within_returns_the_work_or_names_what_never_finished() {
        assert_eq!(within("one", LONG, || 1), Ok(1));
        let (_keep, never) = std::sync::mpsc::channel::<()>();
        let error = within("the never", std::time::Duration::ZERO, move || never.recv())
            .expect_err("never finishes");
        assert!(error.contains("the never did not happen"), "{error}");
    }

    #[test]
    fn path_appears_sees_a_file_created_below_a_missing_directory() {
        let dir = scratch("dx-wait-path-");
        let path = dir.path().join("later/deeper/file");
        let error = path_appears(&path, std::time::Duration::ZERO).expect_err("absent");
        assert!(error.contains("appearing did not happen"), "{error}");
        let created = path.clone();
        let writer = std::thread::spawn(move || {
            std::fs::create_dir_all(created.parent().expect("parent"))?;
            std::fs::write(&created, "x")
        });
        path_appears(&path, LONG).unwrap();
        writer.join().expect("writer").unwrap();
    }

    #[test]
    fn watch_until_rechecks_a_condition_no_file_event_announces() {
        let dir = scratch("dx-wait-recheck-");
        let mut checks = 0;
        watch_until(&dir.path().join("quiet"), "the third check", LONG, || {
            checks += 1;
            checks >= 3
        })
        .unwrap();
        assert_eq!(checks, 3);
    }

    #[test]
    fn lock_free_waits_for_the_holder_to_let_go() {
        let dir = scratch("dx-wait-lock-");
        let path = dir.path().join("alive.lock");
        let held = std::fs::File::create(&path).unwrap();
        held.lock().unwrap();
        let error = lock_free(&path, std::time::Duration::ZERO).expect_err("held");
        assert!(error.contains("coming free did not happen"), "{error}");
        drop(held);
        lock_free(&path, LONG).unwrap();
    }

    #[test]
    fn pid_gone_and_exit_within_follow_a_child_to_its_end() {
        let mut child = holder();
        let pid = child.id();
        assert!(pid_gone(pid, std::time::Duration::ZERO).is_err());
        drop(child.stdin.take());
        pid_gone(pid, LONG).unwrap();
        assert!(matches!(
            exit_within(child, LONG).unwrap(),
            Exit::Exited(status) if status.success()
        ));
    }

    #[test]
    fn exit_within_kills_a_child_that_outlives_the_limit() {
        let mut child = holder();
        let _stdin = child.stdin.take();
        assert!(matches!(
            exit_within(child, std::time::Duration::ZERO).unwrap(),
            Exit::Killed(status) if !status.success()
        ));
    }
}
