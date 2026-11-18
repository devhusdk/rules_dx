#![deny(clippy::indexing_slicing, clippy::panic, clippy::string_slice)]

pub mod commit;
pub mod lease;
pub mod managed;

use std::fs::File;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

pub use managed::{
    EntryKind, LinkPolicy, Mechanism, PointerKind, classify, create_pointer, materialize_file,
    materialize_file_with, remove_managed, replace_pointer,
};

fn staging_for(path: &Path) -> io::Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(long_path(parent)?)?;
    }
    tempfile::Builder::new()
        .prefix(&staging_prefix(path))
        .suffix(STAGING_SUFFIX)
        .tempfile_in(long_path(parent.unwrap_or(Path::new(".")))?)
}

const STAGING_SUFFIX: &str = ".tmp";
const STAGING_NAME_MAX: usize = 200;

fn truncated_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = String::with_capacity(name.len().min(STAGING_NAME_MAX));
    for char in name.chars() {
        if out.len() + char.len_utf8() > STAGING_NAME_MAX {
            break;
        }
        out.push(char);
    }
    out
}

fn staging_prefix(path: &Path) -> String {
    format!(".{}.", truncated_name(path))
}

/// True when candidate is a staging file that `write_atomic` makes next to target.
pub fn is_staging_sibling(target: &Path, candidate: &Path) -> bool {
    if candidate == target
        || candidate.parent().map(dx_path::without_verbatim)
            != target.parent().map(dx_path::without_verbatim)
    {
        return false;
    }
    let Some(name) = candidate.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let prefix = staging_prefix(target);
    name.len() > prefix.len() + STAGING_SUFFIX.len()
        && name.starts_with(&prefix)
        && name.ends_with(STAGING_SUFFIX)
}

/// Removes a scratch file. A missing file counts as removed; any other error is logged with the path.
pub fn discard_file(path: impl AsRef<Path>) {
    let path = path.as_ref();
    log_discard_error(path, std::fs::remove_file(path));
}

/// Removes a scratch directory tree. A missing tree counts as removed; any other error is logged with the path.
pub fn discard_dir(path: impl AsRef<Path>) {
    let path = path.as_ref();
    log_discard_error(path, std::fs::remove_dir_all(path));
}

/// Writes a file that readers treat as a hint, such as a holder label; an error is logged with the path.
pub fn write_hint(path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
    let path = path.as_ref();
    if let Err(error) = std::fs::write(path, content) {
        tracing::info!(path = %path.display(), %error, "cannot write hint file");
    }
}

fn log_discard_error(path: &Path, result: io::Result<()>) {
    if let Err(error) = result
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::info!(path = %path.display(), %error, "cannot remove scratch path");
    }
}

const STALE_STAGING_AGE: Duration = Duration::from_hours(1);

/// Removes staging files next to target that an interrupted write left behind.
/// Files younger than an hour are kept since another process may still be writing them.
pub fn remove_stale_staging(target: &Path) {
    let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_staging_sibling(target, &path) {
            continue;
        }
        let age = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok());
        if age.is_some_and(|age| age >= STALE_STAGING_AGE) {
            discard_file(&path);
        }
    }
}

/// Writes content to path so readers see the old or the new file, never a partial one,
/// and syncs it so the new file survives a crash.
/// An existing file keeps its mode; a new one gets 0644.
pub fn write_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    write_staged(path, content, Durability::Synced)
}

/// Like [`write_atomic`] without the disk syncs, for files a crash may lose: scratch trees
/// thrown away after the run, and Bazel action outputs, which Bazel re-checks by digest.
pub fn write_scratch(path: &Path, content: &[u8]) -> io::Result<()> {
    write_staged(path, content, Durability::Unsynced)
}

fn write_staged(path: &Path, content: &[u8], durability: Durability) -> io::Result<()> {
    use std::io::Write as _;
    let mut staging = staging_for(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode =
            std::fs::metadata(path).map_or(0o644, |metadata| metadata.permissions().mode() & 0o777);
        staging
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    staging.write_all(content)?;
    persist(staging, path, durability)
}

/// Whether a staged file is flushed to disk around its rename.
#[derive(Clone, Copy)]
pub(crate) enum Durability {
    Synced,
    /// For files a crash may lose: scratch trees, and copies that stand in for a link,
    /// since links are not synced either.
    Unsynced,
}

/// Renames staging over dest. Synced flushes staging first, since without it a crash
/// after the rename can leave dest empty, then syncs dest's directory.
fn persist(
    staging: tempfile::NamedTempFile,
    dest: &Path,
    durability: Durability,
) -> io::Result<()> {
    match durability {
        Durability::Synced => {
            sync_outcome(staging.as_file().sync_all())?;
            staging.persist(long_path(dest)?).map_err(|err| err.error)?;
            sync_parent(dest)
        }
        Durability::Unsynced => {
            staging.persist(long_path(dest)?).map_err(|err| err.error)?;
            Ok(())
        }
    }
}

/// The result of a sync, where a filesystem that cannot sync counts as done, as it did
/// before writes were synced. macOS SMB shares fail F_FULLFSYNC with ENOTSUP.
fn sync_outcome(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if cannot_sync(&error) => Ok(()),
        other => other,
    }
}

/// Like [`sync_outcome`] for a directory. Linux filesystems without directory sync, such
/// as some shared-folder mounts, fail it with EINVAL.
#[cfg(unix)]
fn dir_sync_outcome(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        other => sync_outcome(other),
    }
}

fn cannot_sync(error: &io::Error) -> bool {
    // ENOTSUP in <sys/errno.h>. std maps it to Uncategorized there; only EOPNOTSUPP (102)
    // becomes Unsupported.
    const APPLE_ENOTSUP: i32 = 45;
    error.kind() == io::ErrorKind::Unsupported
        || (cfg!(target_vendor = "apple") && error.raw_os_error() == Some(APPLE_ENOTSUP))
}

/// Syncs the directory that holds path, so the renamed entry survives a crash.
#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // A directory the caller may write but not read (mode 0300 or 0711) cannot be opened.
    // The rename has already landed, so skip the sync rather than report a failed write.
    match File::open(parent) {
        Ok(dir) => dir_sync_outcome(dir.sync_all()),
        Err(_) => Ok(()),
    }
}

/// Nothing to do: Windows cannot open a directory as a plain file to sync it.
#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

pub(crate) fn persist_copy(
    mut source: File,
    mut staging: tempfile::NamedTempFile,
    dest: &Path,
    durability: Durability,
) -> io::Result<()> {
    io::copy(&mut source, staging.as_file_mut())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let executable = source.metadata()?.permissions().mode() & 0o111 != 0;
        let bits = if executable { 0o755 } else { 0o644 };
        staging
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(bits))?;
    }
    persist(staging, dest, durability)
}

/// Copies src to dest through a temp file, staged 0644 or 0755 when src is executable.
pub fn copy_atomic(src: &Path, dest: &Path) -> io::Result<()> {
    let source = File::open(src)?;
    persist_copy(source, staging_for(dest)?, dest, Durability::Synced)
}

/// Copies src to dest like [`copy_atomic`] without syncing, for files a crash may lose.
pub fn copy_scratch(src: &Path, dest: &Path) -> io::Result<()> {
    let source = File::open(src)?;
    persist_copy(source, staging_for(dest)?, dest, Durability::Unsynced)
}

/// A file written in place beside `dest` and renamed over it on [`StagedFile::commit`].
/// Dropping it without a commit removes the staging file and leaves `dest` untouched.
pub struct StagedFile {
    staging: tempfile::NamedTempFile,
    dest: std::path::PathBuf,
}

impl StagedFile {
    /// Opens a 0644 staging file beside `dest`, creating `dest`'s parent directories.
    pub fn create(dest: &Path) -> io::Result<Self> {
        let staging = staging_for(dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            staging
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o644))?;
        }
        Ok(Self {
            staging,
            dest: dest.to_owned(),
        })
    }

    pub fn file_mut(&mut self) -> &mut File {
        self.staging.as_file_mut()
    }

    /// The staging file's own path.
    pub fn path(&self) -> &Path {
        self.staging.path()
    }

    /// Syncs the staging file and renames it over `dest`.
    pub fn commit(self) -> io::Result<()> {
        persist(self.staging, &self.dest, Durability::Synced)
    }

    /// Like [`StagedFile::commit`] onto `dest`, a path in the directory of the one given
    /// to [`StagedFile::create`], for content whose name is known only once written.
    pub fn commit_as(self, dest: &Path) -> io::Result<()> {
        persist(self.staging, dest, Durability::Synced)
    }
}

/// The path spelled so Windows accepts it past the legacy length limit.
#[cfg(windows)]
fn long_path(path: &Path) -> io::Result<std::path::PathBuf> {
    Ok(dx_path::verbatim(&std::path::absolute(path)?))
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

    fn dir_entries(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .expect("list dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn discard_removes_present_and_accepts_missing_paths() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
        let file = scratch.path().join("a.bin");
        let dir = scratch.path().join("tree");
        std::fs::write(&file, b"x").expect("seed file");
        std::fs::create_dir_all(dir.join("nested")).expect("seed dir");
        std::fs::write(dir.join("nested").join("b.bin"), b"y").expect("seed nested");
        discard_file(&file);
        discard_dir(&dir);
        assert!(!file.exists());
        assert!(!dir.exists());
        discard_file(&file);
        discard_dir(&dir);
    }

    #[test]
    fn write_hint_writes_and_tolerates_a_missing_parent() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
        let file = scratch.path().join("holder");
        write_hint(&file, "label");
        assert_eq!(std::fs::read_to_string(&file).expect("read hint"), "label");
        let orphan = scratch.path().join("missing").join("holder");
        write_hint(&orphan, "label");
        assert!(!orphan.exists());
    }

    #[test]
    fn staged_file_commit_replaces_dest() {
        use std::io::Write as _;
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
        let dest = scratch.path().join("out").join("a.bin");
        write_atomic(&dest, b"old").expect("seed");
        let mut staged = StagedFile::create(&dest).expect("stage");
        staged.file_mut().write_all(b"new").expect("write");
        assert_eq!(std::fs::read(&dest).expect("read"), b"old");
        staged.commit().expect("commit");
        assert_eq!(std::fs::read(&dest).expect("read"), b"new");
        assert_eq!(
            dir_entries(&scratch.path().join("out")),
            vec!["a.bin".to_owned()]
        );
    }

    #[test]
    fn staged_file_commit_as_names_dest_after_writing() {
        use std::io::Write as _;
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
        let first = StagedFile::create(&scratch.path().join(".layer")).expect("stage");
        let mut second = StagedFile::create(&scratch.path().join(".layer")).expect("stage");
        assert_ne!(first.path(), second.path());
        assert_eq!(first.path().parent(), Some(scratch.path()));
        drop(first);
        second.file_mut().write_all(b"blob").expect("write");
        let dest = scratch.path().join("abc");
        second.commit_as(&dest).expect("commit");
        assert_eq!(std::fs::read(&dest).expect("read"), b"blob");
        assert_eq!(dir_entries(scratch.path()), vec!["abc".to_owned()]);
    }

    #[test]
    fn staged_file_drop_leaves_no_sibling() {
        use std::io::Write as _;
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
        let dest = scratch.path().join("a.bin");
        let mut staged = StagedFile::create(&dest).expect("stage");
        staged.file_mut().write_all(b"partial").expect("write");
        drop(staged);
        assert!(dir_entries(scratch.path()).is_empty());
    }

    #[test]
    fn writes_and_overwrites_without_stray_staging_files() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-parents-").unwrap();
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
            std::fs::write("dx-atomic-fs-bare-child-ok", b"ok").expect("sentinel");
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-bare-").unwrap();
        let original = std::env::current_dir().expect("cwd");
        let spawn = || {
            std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args(["tests::bare_name_stages_in_current_directory", "--exact"])
                .current_dir(scratch.path())
                .env("DX_ATOMIC_FS_BARE_CHILD", "run")
                .output()
                .expect("spawn bare-path child")
        };
        let ok = spawn();
        assert!(ok.status.success(), "{ok:?}");
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
            let scratch = dx_test_scratch::scratch("dx-atomic-fs-mode-").unwrap();
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
    fn non_unix_copies_replace_the_destination() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-bytes-").unwrap();
        let src = scratch.path().join("src.bin");
        std::fs::write(&src, b"payload\n").expect("write src");
        let dest = scratch.path().join("dest.bin");
        copy_atomic(&src, &dest).expect("copy");
        copy_atomic(&src, &dest).expect("copy over");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(not(unix))]
    fn non_unix_round_trips_bytes_without_mode_assertions() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-bytes-").unwrap();
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

    #[test]
    #[cfg(unix)]
    fn writes_into_a_directory_it_cannot_read() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-write-only-dir-").unwrap();
        let dir = scratch.path().join("drop");
        std::fs::create_dir(&dir).expect("mkdir");
        chmod(&dir, 0o300);
        let target = dir.join("out.txt");
        let first = write_atomic(&target, b"first\n");
        let second = write_atomic(&target, b"second\n");
        chmod(&dir, 0o755);
        first.expect("create in write-only dir");
        second.expect("replace in write-only dir");
        assert_eq!(std::fs::read(&target).expect("read back"), b"second\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_filesystem_that_cannot_sync_counts_as_synced() {
        assert!(sync_outcome(Ok(())).is_ok());
        assert!(sync_outcome(Err(io::ErrorKind::Unsupported.into())).is_ok());
        let failed = sync_outcome(Err(io::ErrorKind::Other.into())).expect_err("other errors stay");
        assert_eq!(failed.kind(), io::ErrorKind::Other);
        assert_eq!(
            cannot_sync(&io::Error::from_raw_os_error(45)),
            cfg!(target_vendor = "apple")
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_directory_that_cannot_sync_counts_as_synced() {
        assert!(dir_sync_outcome(Ok(())).is_ok());
        assert!(dir_sync_outcome(Err(io::ErrorKind::InvalidInput.into())).is_ok());
        assert!(dir_sync_outcome(Err(io::ErrorKind::Unsupported.into())).is_ok());
        let failed =
            dir_sync_outcome(Err(io::ErrorKind::Other.into())).expect_err("other errors stay");
        assert_eq!(failed.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn write_scratch_replaces_and_keeps_the_mode() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-scratch-").unwrap();
        let target = scratch.path().join("mirror").join("a.txt");
        write_scratch(&target, b"first\n").expect("create");
        write_scratch(&target, b"second\n").expect("replace");
        assert_eq!(std::fs::read(&target).expect("read back"), b"second\n");
        assert_eq!(names_in(&scratch.path().join("mirror")), ["a.txt"]);
        #[cfg(unix)]
        assert_eq!(mode_of(&target), 0o644);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn staging_file_is_a_staging_sibling() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-staging-sibling-").unwrap();
        let dir = scratch.path().to_path_buf();
        let target = long_path(&dir.join("out.sarif")).expect("long path");
        let staging = staging_for(&target).expect("staging");
        assert!(is_staging_sibling(&target, staging.path()));
    }

    #[test]
    fn is_staging_sibling_rejects_other_files() {
        let target = Path::new("/w/out.sarif");
        assert!(!is_staging_sibling(target, target));
        assert!(!is_staging_sibling(
            target,
            Path::new("/w/sub/.out.sarif.abc.tmp")
        ));
        assert!(!is_staging_sibling(target, Path::new("/w/.other.x.tmp")));
        assert!(!is_staging_sibling(target, Path::new("/w/.tmpabc")));
        assert!(is_staging_sibling(
            target,
            Path::new("/w/.out.sarif.abc.tmp")
        ));
    }

    #[test]
    fn truncated_name_cuts_on_a_char_boundary() {
        let name = format!("a{}", "\u{e9}".repeat(150));
        let cut = truncated_name(Path::new(&name));
        assert_eq!(cut.len(), STAGING_NAME_MAX - 1);
        assert!(name.starts_with(&cut));
    }

    #[test]
    fn a_path_without_a_file_name_is_not_a_staging_sibling() {
        assert!(!is_staging_sibling(Path::new("/w/out"), Path::new("/w/..")));
    }

    #[test]
    fn remove_stale_staging_ignores_bare_and_missing_parents() {
        remove_stale_staging(Path::new("bare-name-without-parent"));
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-stale-missing-").unwrap();
        remove_stale_staging(&scratch.path().join("missing").join("file"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_very_long_file_name_still_writes() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-long-name-").unwrap();
        let dir = scratch.path().to_path_buf();
        let target = dir.join("n".repeat(250));
        write_atomic(&target, b"x").expect("write");
        assert_eq!(std::fs::read(&target).expect("read"), b"x");
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
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-ro-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-exec-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-dest-").unwrap();
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
    #[cfg(unix)]
    fn copy_scratch_replaces_a_read_only_dest_and_keeps_the_exec_bit() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-scratch-").unwrap();
        let src = scratch.path().join("tool");
        std::fs::write(&src, b"#!/bin/sh\n").expect("write src");
        chmod(&src, 0o555);
        let out = scratch.path().join("out");
        std::fs::create_dir_all(&out).expect("mkdir out");
        let dest = out.join("tool");
        std::fs::write(&dest, b"old\n").expect("write dest");
        chmod(&dest, 0o444);
        copy_scratch(&src, &dest).expect("replace read-only dest");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"#!/bin/sh\n");
        assert_eq!(mode_of(&dest), 0o755);
        assert_eq!(names_in(&out), vec!["tool".to_owned()]);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn copy_atomic_missing_source_leaves_no_staging_file() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-copy-missing-").unwrap();
        let out = scratch.path().join("out");
        std::fs::create_dir_all(&out).expect("mkdir out");
        let err = copy_atomic(&scratch.path().join("absent"), &out.join("dest"))
            .expect_err("missing source");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(names_in(&out), [] as [String; 0]);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn remove_stale_staging_keeps_fresh_and_unrelated_files() {
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-stale-").unwrap();
        let dir = scratch.path();
        let target = dir.join("bazel");
        let old = dir.join(".bazel.abc.tmp");
        for name in [
            ".bazel.abc.tmp",
            ".bazel.fresh.tmp",
            ".other.abc.tmp",
            ".bashrc",
        ] {
            std::fs::write(dir.join(name), b"x").expect("write");
        }
        let long_ago = std::time::SystemTime::now() - Duration::from_hours(2);
        File::options()
            .write(true)
            .open(&old)
            .expect("open old")
            .set_modified(long_ago)
            .expect("age old");
        remove_stale_staging(&target);
        assert_eq!(
            names_in(dir),
            [".bashrc", ".bazel.fresh.tmp", ".other.abc.tmp"]
        );
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
        let _holding = dx_testing::holding();
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-").unwrap();
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
        let _holding = dx_testing::holding();
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-busy-").unwrap();
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
        let _holding = dx_testing::holding();
        let scratch = dx_test_scratch::scratch("dx-atomic-fs-lock-wait-").unwrap();
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
