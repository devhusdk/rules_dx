#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod commit;
pub mod lease;
pub mod managed;

use std::fs::File;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

pub use managed::{
    classify, materialize_file, materialize_file_with, remove_managed, replace_pointer, EntryKind,
    LinkPolicy, Mechanism, PointerKind,
};

fn staging_for(path: &Path) -> io::Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent)?;
    }
    tempfile::NamedTempFile::new_in(parent.unwrap_or(Path::new(".")))
}

pub fn write_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let mut staging = staging_for(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or(0o644);
        staging
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    staging.write_all(content)?;
    staging
        .persist(long_path(path)?)
        .map_err(|err| err.error)?;
    Ok(())
}

/// Copies src to dest through a temp file, staged 0644 or 0755 when src is executable.
pub fn copy_atomic(src: &Path, dest: &Path) -> io::Result<()> {
    let mut source = File::open(src)?;
    let mut staging = staging_for(dest)?;
    io::copy(&mut source, staging.as_file_mut())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let executable = source.metadata()?.permissions().mode() & 0o111 != 0;
        let mode = if executable { 0o755 } else { 0o644 };
        staging
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    staging.persist(dest).map_err(|err| err.error)?;
    Ok(())
}

/// The path spelled so Windows accepts it past the legacy length limit.
#[cfg(windows)]
fn long_path(path: &Path) -> io::Result<std::path::PathBuf> {
    let absolute = std::path::absolute(path)?;
    if absolute.as_os_str().as_encoded_bytes().starts_with(br"\\") {
        return Ok(absolute);
    }
    let mut verbatim = std::ffi::OsString::from(r"\\?\");
    verbatim.push(absolute.as_os_str());
    Ok(verbatim.into())
}

/// The path unchanged, since only Windows limits its length.
#[cfg(not(windows))]
fn long_path(path: &Path) -> io::Result<std::path::PathBuf> {
    Ok(path.to_owned())
}

pub const LOCK_POLL: Duration = Duration::from_millis(50);

pub fn lock_exclusive(file: &File, timeout: Duration) -> Result<(), std::fs::TryLockError> {
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => {
                if start.elapsed() >= timeout {
                    return Err(std::fs::TryLockError::WouldBlock);
                }
                std::thread::sleep(LOCK_POLL);
            }
            Err(other) => return Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn writes_and_overwrites_without_stray_staging_files() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-");
        let dir = scratch.path().to_path_buf();
        let nested = dir.join("sub").join("a.txt");
        assert_eq!(
            std::fs::read(&nested).map_err(|e| e.kind()),
            Err(io::ErrorKind::NotFound)
        );
        write_atomic(&nested, b"hello\n").expect("write");
        assert_eq!(std::fs::read(&nested).expect("read back"), b"hello\n");
        let entries: Vec<_> = std::fs::read_dir(dir.join("sub"))
            .expect("list target dir")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(entries, vec!["a.txt".to_owned()]);
        write_atomic(&nested, b"updated\n").expect("overwrite");
        assert_eq!(
            std::fs::read(&nested).expect("read overwrite"),
            b"updated\n"
        );
        let entries: Vec<_> = std::fs::read_dir(dir.join("sub"))
            .expect("list target dir after overwrite")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(entries, vec!["a.txt".to_owned()]);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn creates_parent_directories() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-parents-");
        let nested = scratch.path().join("a").join("b").join("c.txt");
        write_atomic(&nested, b"deep\n").expect("deep write");
        assert_eq!(std::fs::read(&nested).expect("deep read"), b"deep\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn bare_name_stages_in_current_directory() {
        if std::env::var_os("DX_ATOMIC_FS_BARE_CHILD").is_some() {
            let bare = PathBuf::from("dx-atomic-fs-bare-tmp.txt");
            write_atomic(&bare, b"bare\n").expect("bare write");
            assert_eq!(std::fs::read(&bare).expect("bare read"), b"bare\n");
            std::fs::remove_file(&bare).expect("bare cleanup");
            if std::env::var_os("DX_ATOMIC_FS_BARE_CHILD").as_deref()
                == Some(std::ffi::OsStr::new("panic"))
            {
                write_atomic(&bare, b"poisoned\n").expect("poison write");
                panic!("intentional child failure");
            }
            std::fs::write("dx-atomic-fs-bare-child-ok", b"ok").expect("sentinel");
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-bare-");
        let original = std::env::current_dir().expect("cwd");
        let spawn = |mode: &str| {
            std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args(["tests::bare_name_stages_in_current_directory", "--exact"])
                .current_dir(scratch.path())
                .env("DX_ATOMIC_FS_BARE_CHILD", mode)
                .output()
                .expect("spawn bare-path child")
        };
        let poisoned = spawn("panic");
        assert!(
            !poisoned.status.success(),
            "panicking child must fail: {}",
            String::from_utf8_lossy(&poisoned.stdout)
        );
        assert_eq!(
            std::env::current_dir().expect("cwd"),
            original,
            "child cwd dies with the child"
        );
        let ok = spawn("run");
        assert!(
            ok.status.success(),
            "child failed: {}{}",
            String::from_utf8_lossy(&ok.stdout),
            String::from_utf8_lossy(&ok.stderr)
        );
        assert!(
            scratch.path().join("dx-atomic-fs-bare-child-ok").exists(),
            "child ran the assertions"
        );
        assert_eq!(std::env::current_dir().expect("cwd"), original);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn preserves_existing_mode_on_overwrite() {
        {
            use std::os::unix::fs::PermissionsExt as _;
            let scratch = dx_test_scratch::scratch("dx-atomic-fs-mode-");
            let executable = scratch.path().join("run.sh");
            write_atomic(&executable, b"#!/bin/sh\necho hi\n").expect("create executable");
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
                .expect("chmod 755");
            write_atomic(&executable, b"#!/bin/sh\necho updated\n").expect("overwrite executable");
            assert_eq!(
                std::fs::read(&executable).expect("read back"),
                b"#!/bin/sh\necho updated\n"
            );
            assert_eq!(
                std::fs::metadata(&executable)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
            let private = scratch.path().join("secret.txt");
            write_atomic(&private, b"secret\n").expect("create private");
            std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o600))
                .expect("chmod 600");
            write_atomic(&private, b"rotated\n").expect("overwrite private");
            assert_eq!(std::fs::read(&private).expect("read back"), b"rotated\n");
            assert_eq!(
                std::fs::metadata(&private)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            let crlf = scratch.path().join("crlf.txt");
            write_atomic(&crlf, b"line\r\n").expect("create crlf");
            write_atomic(&crlf, b"updated\r\n").expect("overwrite crlf");
            assert_eq!(std::fs::read(&crlf).expect("read back"), b"updated\r\n");
            scratch.close().expect("cleanup");
        }
    }

    #[test]
    #[cfg(not(unix))]
    fn non_unix_round_trips_bytes_without_mode_assertions() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-bytes-");
        let target = scratch.path().join("roundtrip.txt");
        write_atomic(&target, b"hello\n").expect("write");
        assert_eq!(std::fs::read(&target).expect("read back"), b"hello\n");
        write_atomic(&target, b"updated\r\n").expect("overwrite crlf");
        assert_eq!(std::fs::read(&target).expect("read back"), b"updated\r\n");
        write_atomic(&target, b"no-newline").expect("overwrite bare");
        assert_eq!(std::fs::read(&target).expect("read back"), b"no-newline");
        scratch.close().expect("cleanup");
    }

    #[cfg(unix)]
    fn chmod(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    }

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("list dir")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    #[cfg(unix)]
    fn copy_atomic_restages_a_read_only_source() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-ro-");
        let src = scratch.path().join("src.txt");
        std::fs::write(&src, b"payload\n").expect("write src");
        chmod(&src, 0o444);
        let out = scratch.path().join("out");
        let dest = out.join("dest.txt");
        copy_atomic(&src, &dest).expect("first copy");
        copy_atomic(&src, &dest).expect("second copy");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload\n");
        assert_eq!(mode_of(&dest), 0o644);
        assert_eq!(names_in(&out), vec!["dest.txt".to_owned()]);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn copy_atomic_keeps_exec_bit_as_0755() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-exec-");
        let src = scratch.path().join("tool");
        std::fs::write(&src, b"#!/bin/sh\n").expect("write src");
        chmod(&src, 0o555);
        let dest = scratch.path().join("staged-tool");
        copy_atomic(&src, &dest).expect("first copy");
        copy_atomic(&src, &dest).expect("second copy");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"#!/bin/sh\n");
        assert_eq!(mode_of(&dest), 0o755);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn copy_atomic_replaces_a_read_only_dest() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-dest-");
        let src = scratch.path().join("src.txt");
        std::fs::write(&src, b"new\n").expect("write src");
        let dest = scratch.path().join("dest.txt");
        std::fs::write(&dest, b"old\n").expect("write dest");
        chmod(&dest, 0o444);
        copy_atomic(&src, &dest).expect("replace read-only dest");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"new\n");
        assert_eq!(mode_of(&dest), 0o644);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn copy_atomic_missing_source_leaves_no_staging_file() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-missing-");
        let out = scratch.path().join("out");
        std::fs::create_dir_all(&out).expect("mkdir out");
        let err = copy_atomic(&scratch.path().join("absent"), &out.join("dest"))
            .expect_err("missing source");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(names_in(&out).is_empty());
        scratch.close().expect("cleanup");
    }

    fn open_lock_file(path: &Path) -> File {
        use std::fs::OpenOptions;
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .expect("open lock file")
    }

    #[test]
    fn lock_exclusive_acquires_uncontended() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-");
        let path = scratch.path().join(".commit.lock");
        let file = open_lock_file(&path);
        lock_exclusive(&file, Duration::from_secs(10)).expect("acquire");
        drop(file);
        let next = open_lock_file(&path);
        lock_exclusive(&next, Duration::from_secs(10)).expect("reacquire after release");
        drop(next);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn lock_exclusive_times_out_while_held() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-busy-");
        let path = scratch.path().join(".commit.lock");
        let held = open_lock_file(&path);
        lock_exclusive(&held, Duration::from_secs(10)).expect("hold lock");
        let waiter = open_lock_file(&path);
        assert!(matches!(
            lock_exclusive(&waiter, Duration::ZERO),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(held);
        drop(waiter);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn lock_exclusive_waits_then_acquires() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-wait-");
        let path = scratch.path().join(".commit.lock");
        let held = open_lock_file(&path);
        lock_exclusive(&held, Duration::from_secs(10)).expect("hold lock");
        let waiter_path = path.clone();
        let waiter = std::thread::spawn(move || {
            let file = open_lock_file(&waiter_path);
            lock_exclusive(&file, Duration::from_secs(30)).expect("waiter acquires");
        });
        std::thread::sleep(Duration::from_millis(500));
        drop(held);
        waiter.join().expect("waiter thread");
        scratch.close().expect("cleanup");
    }
}
