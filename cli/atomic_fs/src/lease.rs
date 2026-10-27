//! OS-held shared generation-use leases under one managed root.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Directory inside a managed root that holds generation lease files.
pub const LEASES_DIR_NAME: &str = "leases";

/// Variable carrying `kind:hex` lease entries to a controlled child.
pub const LEASE_ENV_VAR: &str = "DX_MANAGED_LEASES";

/// Suffix of every generation lease file.
pub const LEASE_FILE_SUFFIX: &str = ".lock";

/// Which managed generation family one lease protects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenerationUse {
    Environment,
    Generated,
}

impl GenerationUse {
    /// Generation directory this use protects.
    pub const fn dir_name(self) -> &'static str {
        match self {
            GenerationUse::Environment => "environments",
            GenerationUse::Generated => "generated",
        }
    }

    /// Parses the `kind` half of a `kind:hex` lease entry.
    pub fn parse(kind: &str) -> Option<GenerationUse> {
        [GenerationUse::Environment, GenerationUse::Generated]
            .into_iter()
            .find(|candidate| candidate.dir_name() == kind)
    }
}

/// A held shared generation-use lease; dropping releases it.
#[derive(Debug)]
pub struct SharedLease {
    file: File,
}

impl SharedLease {
    /// Hands the locked file to a caller that keeps the raw handle.
    pub fn into_file(self) -> File {
        self.file
    }
}

/// A held exclusive generation lease; dropping releases it.
#[derive(Debug)]
pub struct ExclusiveLease {
    file: File,
}

impl ExclusiveLease {
    /// Hands the locked file to a caller that keeps the raw handle.
    pub fn into_file(self) -> File {
        self.file
    }
}

fn valid_hex(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Names the lease file for one generation; rejects malformed identity.
pub fn lease_path(root: &Path, kind: GenerationUse, hex: &str) -> io::Result<PathBuf> {
    if !valid_hex(hex) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("refusing to lease {hex:?}: want 64 lowercase hex chars"),
        ));
    }
    Ok(root
        .join(LEASES_DIR_NAME)
        .join(kind.dir_name())
        .join(format!("{hex}{LEASE_FILE_SUFFIX}")))
}

fn open_lease_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    } // LCOV_EXCL_LINE - reason: lease paths always carry a parent, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// Holds a shared lease, waiting up to `timeout` for exclusive holders.
pub fn acquire_shared(
    root: &Path,
    kind: GenerationUse,
    hex: &str,
    timeout: Duration,
) -> io::Result<SharedLease> {
    acquire_shared_at(&lease_path(root, kind, hex)?, timeout)
}

/// Holds a shared lease on the file at `path`, waiting up to `timeout` for exclusive holders.
pub fn acquire_shared_at(path: &Path, timeout: Duration) -> io::Result<SharedLease> {
    let file = open_lease_file(path)?;
    let start = std::time::Instant::now();
    loop {
        match file.try_lock_shared() {
            Ok(()) => return Ok(SharedLease { file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                if start.elapsed() >= timeout {
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                std::thread::sleep(super::LOCK_POLL);
            }
            Err(std::fs::TryLockError::Error(other)) => return Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        }
    }
}

/// Holds the exclusive lease when no shared holder exists.
pub fn try_acquire_exclusive(
    root: &Path,
    kind: GenerationUse,
    hex: &str,
) -> io::Result<Option<ExclusiveLease>> {
    let path = lease_path(root, kind, hex)?;
    let file = open_lease_file(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(ExclusiveLease { file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

fn open_lock_file(path: &Path, create: bool) -> io::Result<File> {
    if create {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        } // LCOV_EXCL_LINE - reason: lease paths always carry a parent, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path)
}

/// Locks the file at `path` exclusively without waiting; `None` means another holder has it.
pub fn try_lock_file(path: &Path, create: bool) -> io::Result<Option<File>> {
    let file = open_lock_file(path, create)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

/// Locks the file at `path` shared without waiting; `None` means an exclusive holder has it.
pub fn try_lock_file_shared(path: &Path, create: bool) -> io::Result<Option<File>> {
    let file = open_lock_file(path, create)?;
    match file.try_lock_shared() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

/// Holds the exclusive lease file at `path`, waiting up to `timeout`.
pub fn acquire_exclusive_at(path: &Path, timeout: Duration) -> io::Result<ExclusiveLease> {
    let file = open_lease_file(path)?;
    match super::lock_exclusive(&file, timeout) {
        Ok(()) => Ok(ExclusiveLease { file }),
        Err(std::fs::TryLockError::WouldBlock) => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 2040, policy: docs/cli/commands/build-test-coverage.md
    }
}

/// Reports whether a live shared holder blocks exclusive acquisition.
pub fn shared_held(root: &Path, kind: GenerationUse, hex: &str) -> io::Result<bool> {
    Ok(try_acquire_exclusive(root, kind, hex)?.is_none())
}

/// Renders lease entries for [`LEASE_ENV_VAR`].
pub fn inherit_value(entries: &[(GenerationUse, &str)]) -> String {
    entries
        .iter()
        .map(|(kind, hex)| format!("{}:{hex}", kind.dir_name()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Holds one shared lease per [`LEASE_ENV_VAR`] entry; absent means none.
pub fn acquire_shared_from_env(root: &Path, timeout: Duration) -> io::Result<Vec<SharedLease>> {
    let Some(raw) = std::env::var_os(LEASE_ENV_VAR) else {
        return Ok(Vec::new());
    };
    let Some(raw) = raw.to_str() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{LEASE_ENV_VAR} is not UTF-8"),
        ));
    };
    let mut held = Vec::new();
    for entry in raw.split(',') {
        let Some((kind, hex)) = entry.split_once(':') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{LEASE_ENV_VAR} entry {entry:?} wants kind:hex"),
            ));
        };
        let Some(parsed) = GenerationUse::parse(kind) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{LEASE_ENV_VAR} entry {entry:?} names no generation family"),
            ));
        };
        held.push(acquire_shared(root, parsed, hex, timeout)?);
    }
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn root(scratch: &std::path::Path) -> PathBuf {
        scratch.join(".dx")
    }

    #[test]
    fn parse_round_trips_dir_name() {
        for kind in [GenerationUse::Environment, GenerationUse::Generated] {
            assert_eq!(GenerationUse::parse(kind.dir_name()), Some(kind));
        }
        assert_eq!(GenerationUse::parse("setups"), None);
    }

    #[test]
    fn lease_paths_stay_inside_the_managed_root() {
        let base = PathBuf::from("/ws/.dx");
        let path = lease_path(&base, GenerationUse::Environment, HEX_A).expect("path");
        assert_eq!(
            path,
            base.join("leases")
                .join("environments")
                .join(format!("{HEX_A}.lock"))
        );
        let path = lease_path(&base, GenerationUse::Generated, HEX_B).expect("path");
        assert_eq!(
            path,
            base.join("leases")
                .join("generated")
                .join(format!("{HEX_B}.lock"))
        );
        for bad in [
            "",
            "abc",
            &"A".repeat(64),
            &"g".repeat(64),
            "../escape",
            &HEX_A[..63],
        ] {
            assert!(
                lease_path(&base, GenerationUse::Environment, bad).is_err(),
                "malformed identity {bad:?} must fail"
            );
        }
    }

    #[test]
    fn shared_blocks_exclusive_until_released() {
        let scratch = dx_test_scratch::scratch("dx-lease-contention-");
        let root = root(scratch.path());
        let shared = acquire_shared(
            &root,
            GenerationUse::Environment,
            HEX_A,
            Duration::from_secs(10),
        )
        .expect("shared");
        assert!(
            shared_held(&root, GenerationUse::Environment, HEX_A).expect("probe"),
            "a held shared lease blocks exclusivity"
        );
        assert!(
            try_acquire_exclusive(&root, GenerationUse::Environment, HEX_A)
                .expect("probe")
                .is_none(),
            "exclusive acquisition reports the holder instead of waiting"
        );
        drop(shared);
        eventually("releasing the shared lease frees the generation", || {
            try_acquire_exclusive(&root, GenerationUse::Environment, HEX_A)
                .expect("probe after release")
        });
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_locked_path_reports_its_holder() {
        let scratch = dx_test_scratch::scratch("dx-lease-path-");
        let path = scratch.path().join("sub").join("one.lock");
        assert!(
            try_lock_file(&path.with_file_name("none.lock"), false).is_err(),
            "a missing file is not created"
        );
        let held = try_lock_file(&path, true).expect("lock").expect("free");
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(held);
        eventually("release of the held path", || {
            try_lock_file(&path, false).expect("probe")
        });
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_shared_file_lock_blocks_only_exclusive() {
        let scratch = dx_test_scratch::scratch("dx-lease-shared-file-");
        let path = scratch.path().join("sub").join("reserve.lock");
        let first = try_lock_file_shared(&path, true)
            .expect("lock")
            .expect("free");
        let second = try_lock_file_shared(&path, false)
            .expect("lock")
            .expect("shared holders coexist");
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(first);
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(second);
        let exclusive = eventually("release of both shared holders", || {
            try_lock_file(&path, false).expect("probe")
        });
        assert!(try_lock_file_shared(&path, false).expect("probe").is_none());
        drop(exclusive);
        eventually("release of the exclusive holder", || {
            try_lock_file_shared(&path, false).expect("probe")
        });
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_shared_lease_at_a_path_blocks_exclusive_and_times_out_behind_it() {
        let scratch = dx_test_scratch::scratch("dx-lease-shared-at-");
        let path = scratch.path().join("sub").join("one.busy");
        let shared = acquire_shared_at(&path, Duration::ZERO).expect("shared");
        let again = acquire_shared_at(&path, Duration::ZERO).expect("shared holders coexist");
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(shared);
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(again);
        let exclusive = acquire_exclusive_at(&path, Duration::from_secs(10)).expect("free");
        let waited = acquire_shared_at(&path, Duration::from_millis(100))
            .expect_err("an exclusive holder blocks shared");
        assert_eq!(waited.kind(), io::ErrorKind::WouldBlock);
        drop(exclusive);
        acquire_shared_at(&path, Duration::from_secs(10)).expect("free after release");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn exclusive_lease_at_a_path_excludes_a_second_holder_until_dropped() {
        let scratch = dx_test_scratch::scratch("dx-lease-at-");
        let path = scratch.path().join("nested").join("pool.lock");
        let first = acquire_exclusive_at(&path, Duration::from_millis(100)).expect("first");
        let blocked = acquire_exclusive_at(&path, Duration::from_millis(100));
        assert_eq!(
            blocked.map(|_| ()).map_err(|error| error.kind()),
            Err(io::ErrorKind::WouldBlock)
        );
        drop(first);
        eventually(
            "release of the first holder",
            || match acquire_exclusive_at(&path, Duration::from_millis(100)) {
                Ok(lease) => Some(lease),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
                Err(error) => panic!("acquire after drop: {error:?}"),
            },
        );
    }

    #[test]
    fn exclusive_is_available_when_idle() {
        let scratch = dx_test_scratch::scratch("dx-lease-idle-");
        let root = root(scratch.path());
        assert!(
            !shared_held(&root, GenerationUse::Generated, HEX_B).expect("idle probe"),
            "an idle generation reports no holder"
        );
        let exclusive = try_acquire_exclusive(&root, GenerationUse::Generated, HEX_B)
            .expect("probe")
            .expect("idle generation grants exclusivity");
        let file = exclusive.into_file();
        assert!(
            try_acquire_exclusive(&root, GenerationUse::Generated, HEX_B)
                .expect("handoff probe")
                .is_none(),
            "a handed-off handle keeps the generation locked"
        );
        drop(file);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn shared_handoff_keeps_the_generation_locked() {
        let scratch = dx_test_scratch::scratch("dx-lease-shared-handoff-");
        let root = root(scratch.path());
        let shared = acquire_shared(
            &root,
            GenerationUse::Environment,
            HEX_A,
            Duration::from_secs(10),
        )
        .expect("shared");
        let file = shared.into_file();
        assert!(
            shared_held(&root, GenerationUse::Environment, HEX_A).expect("handoff probe"),
            "a handed-off shared handle keeps the generation locked"
        );
        drop(file);
        eventually(
            "the generation frees once the handed-off handle drops",
            || {
                (!shared_held(&root, GenerationUse::Environment, HEX_A).expect("final probe"))
                    .then_some(())
            },
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn inherit_value_round_trips_through_the_child_entry() {
        assert_eq!(inherit_value(&[]), "");
        let rendered = inherit_value(&[
            (GenerationUse::Environment, HEX_A),
            (GenerationUse::Generated, HEX_B),
        ]);
        assert_eq!(rendered, format!("environments:{HEX_A},generated:{HEX_B}"));
        for bad in ["no-separator", "unknown-kind", "environments:short"] {
            let entry = if bad == "unknown-kind" {
                format!("unknown-kind:{HEX_A}")
            } else {
                bad.to_owned()
            };
            std::env::set_var(LEASE_ENV_VAR, entry);
            assert!(
                acquire_shared_from_env(Path::new("/nonexistent"), Duration::ZERO).is_err(),
                "malformed inheritance fails closed"
            );
        }
        std::env::remove_var(LEASE_ENV_VAR);
        assert!(
            acquire_shared_from_env(Path::new("/nonexistent"), Duration::ZERO)
                .expect("absent inheritance")
                .is_empty(),
            "an absent inheritance entry holds no lease"
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt as _;
            std::env::set_var(
                LEASE_ENV_VAR,
                std::ffi::OsStr::from_bytes(b"environments:\xff\xfe"),
            );
            let err = acquire_shared_from_env(Path::new("/nonexistent"), Duration::ZERO)
                .expect_err("non-UTF-8 inheritance fails closed");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            std::env::remove_var(LEASE_ENV_VAR);
        }
    }

    fn spawn_child(test: &str, root: &Path, entries: &str, mode: &str) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(format!("lease::tests::{test}"))
            .arg("--exact")
            .env(LEASE_ENV_VAR, entries)
            .env("DX_LEASE_CHILD", mode)
            .env("DX_LEASE_ROOT", root)
            .spawn()
            .expect("spawn lease child")
    }

    fn wait_for(path: &Path, what: &str) {
        let start = std::time::Instant::now();
        while !path.exists() {
            if start.elapsed() > Duration::from_secs(30) {
                panic!("timed out waiting for {what}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn eventually<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
        let start = std::time::Instant::now();
        loop {
            if let Some(value) = probe() {
                return value;
            }
            if start.elapsed() > Duration::from_secs(30) {
                panic!("timed out waiting for {what}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn child_main(root: &Path, mode: &str) {
        let ready = root.join("child-ready");
        let release = root.join("child-release");
        match mode {
            "hold" => {
                let held = acquire_shared_from_env(root, Duration::from_secs(30))
                    .expect("child acquires its inherited lease");
                assert_eq!(held.len(), 1);
                std::fs::write(&ready, b"ready").expect("signal ready");
                wait_for(&release, "release");
                drop(held);
            }
            "die" => {
                let _held = acquire_shared(root, GenerationUse::Generated, HEX_B, Duration::ZERO)
                    .expect("child acquires");
                std::fs::write(&ready, b"ready").expect("signal ready");
                wait_for(&release, "release");
            }
            "contend" => {
                let verdict = match acquire_shared(
                    root,
                    GenerationUse::Generated,
                    HEX_B,
                    Duration::from_secs(1),
                ) {
                    Ok(held) => {
                        drop(held);
                        "acquired"
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => "blocked",
                    Err(error) => panic!("contention probe wants WouldBlock, got {error:?}"),
                };
                std::fs::write(root.join("child-verdict"), verdict).expect("signal verdict");
            }
            other => panic!("unknown child mode {other:?}"),
        }
    }

    #[test]
    fn killed_holder_releases_the_generation() {
        if let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        {
            let root = std::env::var_os("DX_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child root");
            child_main(&root, &mode);
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-lease-crash-");
        let root = root(scratch.path());
        let mut child = spawn_child("killed_holder_releases_the_generation", &root, "", "die");
        wait_for(&root.join("child-ready"), "child ready");
        assert!(
            shared_held(&root, GenerationUse::Generated, HEX_B).expect("held probe"),
            "the live child blocks exclusivity"
        );
        child.kill().expect("kill the holder");
        let status = child.wait().expect("reap the holder");
        assert!(!status.success(), "the holder dies abnormally");
        eventually("a killed holder must release its lease", || {
            (!shared_held(&root, GenerationUse::Generated, HEX_B).expect("release probe"))
                .then_some(())
        });
        scratch.close().expect("cleanup");
    }

    #[test]
    fn surviving_child_keeps_the_generation_after_its_parent_releases() {
        if let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        {
            let root = std::env::var_os("DX_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child root");
            child_main(&root, &mode);
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-lease-inherit-");
        let root = root(scratch.path());
        let parent = acquire_shared(
            &root,
            GenerationUse::Environment,
            HEX_A,
            Duration::from_secs(10),
        )
        .expect("parent acquires");
        let entries = inherit_value(&[(GenerationUse::Environment, HEX_A)]);
        let mut child = spawn_child(
            "surviving_child_keeps_the_generation_after_its_parent_releases",
            &root,
            &entries,
            "hold",
        );
        wait_for(&root.join("child-ready"), "child ready");
        drop(parent);
        assert!(
            shared_held(&root, GenerationUse::Environment, HEX_A).expect("inherited probe"),
            "the surviving child still protects the generation after its parent releases"
        );
        std::fs::write(root.join("child-release"), b"go").expect("release the child");
        let status = child.wait().expect("reap the child");
        assert!(status.success(), "the child exits cleanly");
        eventually("the generation frees once every holder exits", || {
            (!shared_held(&root, GenerationUse::Environment, HEX_A).expect("final probe"))
                .then_some(())
        });
        scratch.close().expect("cleanup");
    }

    #[test]
    fn shared_acquire_fails_closed_while_cleanup_holds_exclusive() {
        if let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        {
            let root = std::env::var_os("DX_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child root");
            child_main(&root, &mode);
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-lease-contend-");
        let root = root(scratch.path());
        let exclusive = try_acquire_exclusive(&root, GenerationUse::Generated, HEX_B)
            .expect("probe the idle generation")
            .expect("cleanup holds the generation while it prunes");
        let mut blocked = spawn_child(
            "shared_acquire_fails_closed_while_cleanup_holds_exclusive",
            &root,
            "",
            "contend",
        );
        wait_for(&root.join("child-verdict"), "contention verdict");
        let status = blocked.wait().expect("reap the contender");
        assert!(status.success(), "the contender exits cleanly");
        assert_eq!(
            std::fs::read_to_string(root.join("child-verdict")).expect("read the verdict"),
            "blocked",
            "a shared acquire during an exclusive cleanup hold fails instead of waiting forever"
        );
        drop(exclusive);
        eventually("release of the exclusive cleanup hold", || {
            (!shared_held(&root, GenerationUse::Generated, HEX_B).expect("release probe"))
                .then_some(())
        });
        std::fs::remove_file(root.join("child-verdict")).expect("reset the verdict");
        let mut admitted = spawn_child(
            "shared_acquire_fails_closed_while_cleanup_holds_exclusive",
            &root,
            "",
            "contend",
        );
        wait_for(&root.join("child-verdict"), "admission verdict");
        let status = admitted.wait().expect("reap the admitted child");
        assert!(status.success(), "the admitted child exits cleanly");
        assert_eq!(
            std::fs::read_to_string(root.join("child-verdict")).expect("read the verdict"),
            "acquired",
            "releasing the exclusive hold admits shared acquirers again"
        );
        scratch.close().expect("cleanup");
    }
}
