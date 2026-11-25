use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LEASES_DIR_NAME: &str = dx_path::state::LEASES;

pub const LEASE_FILE_SUFFIX: &str = ".lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenerationUse {
    Environment,
    Generated,
}

impl GenerationUse {
    #[must_use]
    pub const fn dir_name(self) -> &'static str {
        match self {
            GenerationUse::Environment => dx_path::state::ENVIRONMENTS,
            GenerationUse::Generated => dx_path::state::GENERATED,
        }
    }

    #[must_use]
    pub fn parse(kind: &str) -> Option<GenerationUse> {
        [GenerationUse::Environment, GenerationUse::Generated]
            .into_iter()
            .find(|candidate| candidate.dir_name() == kind)
    }
}

#[derive(Debug)]
pub struct SharedLease {
    file: File,
}

impl SharedLease {
    #[must_use]
    pub fn into_file(self) -> File {
        self.file
    }
}

#[derive(Debug)]
pub struct ExclusiveLease {
    file: File,
}

impl ExclusiveLease {
    #[must_use]
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

fn with_path(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

fn open_lease_file(path: &Path) -> io::Result<File> {
    open_lock_file(path, true)
}

fn busy(path: &Path) -> io::Error {
    with_path(path, &io::Error::from(io::ErrorKind::WouldBlock))
}

pub fn acquire_shared(
    root: &Path,
    kind: GenerationUse,
    hex: &str,
    timeout: Duration,
) -> io::Result<SharedLease> {
    acquire_shared_at(&lease_path(root, kind, hex)?, timeout)
}

pub fn acquire_shared_at(path: &Path, timeout: Duration) -> io::Result<SharedLease> {
    let file = open_lease_file(path)?;
    let start = std::time::Instant::now();
    loop {
        match file.try_lock_shared() {
            Ok(()) => return Ok(SharedLease { file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                if start.elapsed() >= timeout {
                    return Err(busy(path));
                }
                std::thread::sleep(super::LOCK_POLL);
            }
            Err(std::fs::TryLockError::Error(other)) => return Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        }
    }
}

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
    if create && let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| with_path(parent, &error))?;
    } // LCOV_EXCL_LINE - reason: lease paths always carry a parent, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path)
        .map_err(|error| with_path(path, &error))
}

pub fn try_lock_file(path: &Path, create: bool) -> io::Result<Option<File>> {
    let file = open_lock_file(path, create)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

pub fn try_lock_file_shared(path: &Path, create: bool) -> io::Result<Option<File>> {
    let file = open_lock_file(path, create)?;
    match file.try_lock_shared() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
    }
}

pub fn acquire_exclusive_at(path: &Path, timeout: Duration) -> io::Result<ExclusiveLease> {
    let file = open_lease_file(path)?;
    match super::lock_exclusive(&file, timeout) {
        Ok(()) => Ok(ExclusiveLease { file }),
        Err(std::fs::TryLockError::WouldBlock) => Err(busy(path)),
        Err(std::fs::TryLockError::Error(other)) => Err(other), // LCOV_EXCL_LINE - reason: non-contention lock error, issue: 2040, policy: docs/cli/commands/build-test-coverage.md
    }
}

pub fn shared_held(root: &Path, kind: GenerationUse, hex: &str) -> io::Result<bool> {
    Ok(try_acquire_exclusive(root, kind, hex)?.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn root(scratch: &std::path::Path) -> PathBuf {
        dx_path::state::local_dir(scratch)
    }

    #[test]
    fn lease_errors_name_the_path() {
        let scratch = dx_test_scratch::scratch("dx-lease-context-").unwrap();
        let _holding = dx_testing::holding();
        let path = scratch.path().join("one.busy");
        let held = acquire_exclusive_at(&path, Duration::ZERO).expect("free");
        for error in [
            acquire_shared_at(&path, Duration::ZERO).expect_err("busy"),
            acquire_exclusive_at(&path, Duration::ZERO).expect_err("busy"),
        ] {
            assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            assert!(error.to_string().contains("one.busy"), "{error}");
        }
        drop(held);
        let blocked = scratch.path().join("one.busy").join("sub").join("x.lock");
        let error = try_lock_file(&blocked, true).expect_err("under a file");
        assert!(error.to_string().contains("one.busy"), "{error}");
        let error = try_lock_file(&scratch.path().join("none.lock"), false).expect_err("missing");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("none.lock"), "{error}");
        scratch.close().expect("cleanup");
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
            HEX_A.get(..63).expect("string slice within bounds"),
        ] {
            assert!(
                lease_path(&base, GenerationUse::Environment, bad).is_err(),
                "malformed identity {bad:?} must fail"
            );
        }
    }

    #[test]
    fn shared_blocks_exclusive_until_released() {
        let scratch = dx_test_scratch::scratch("dx-lease-contention-").unwrap();
        let _holding = dx_testing::holding();
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
        free(
            &lease_path(&root, GenerationUse::Environment, HEX_A).expect("path"),
            "releasing the shared lease frees the generation",
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_locked_path_reports_its_holder() {
        let scratch = dx_test_scratch::scratch("dx-lease-path-").unwrap();
        let _holding = dx_testing::holding();
        let path = scratch.path().join("sub").join("one.lock");
        assert!(
            try_lock_file(&path.with_file_name("none.lock"), false).is_err(),
            "a missing file is not created"
        );
        let held = try_lock_file(&path, true).expect("lock").expect("free");
        assert!(try_lock_file(&path, false).expect("probe").is_none());
        drop(held);
        free(&path, "release of the held path");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_shared_file_lock_blocks_only_exclusive() {
        let scratch = dx_test_scratch::scratch("dx-lease-shared-file-").unwrap();
        let _holding = dx_testing::holding();
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
        let exclusive = dx_test_wait::locked(&path, RELEASE)
            .unwrap_or_else(|error| panic!("release of both shared holders: {error}"));
        assert!(try_lock_file_shared(&path, false).expect("probe").is_none());
        drop(exclusive);
        dx_test_wait::locked_shared(&path, RELEASE)
            .unwrap_or_else(|error| panic!("release of the exclusive holder: {error}"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_shared_lease_at_a_path_blocks_exclusive_and_times_out_behind_it() {
        let scratch = dx_test_scratch::scratch("dx-lease-shared-at-").unwrap();
        let _holding = dx_testing::holding();
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
        let scratch = dx_test_scratch::scratch("dx-lease-at-").unwrap();
        let _holding = dx_testing::holding();
        let path = scratch.path().join("nested").join("pool.lock");
        let first = acquire_exclusive_at(&path, Duration::from_millis(100)).expect("first");
        let blocked = acquire_exclusive_at(&path, Duration::from_millis(100));
        assert_eq!(
            blocked.map(|_| ()).map_err(|error| error.kind()),
            Err(io::ErrorKind::WouldBlock)
        );
        drop(first);
        free(&path, "release of the first holder");
    }

    #[test]
    fn exclusive_is_available_when_idle() {
        let scratch = dx_test_scratch::scratch("dx-lease-idle-").unwrap();
        let _holding = dx_testing::holding();
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
        let scratch = dx_test_scratch::scratch("dx-lease-shared-handoff-").unwrap();
        let _holding = dx_testing::holding();
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
        free(
            &lease_path(&root, GenerationUse::Environment, HEX_A).expect("path"),
            "the generation frees once the handed-off handle drops",
        );
        scratch.close().expect("cleanup");
    }

    fn spawn_child(test: &str, root: &Path, mode: &str) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(format!("lease::tests::{test}"))
            .arg("--exact")
            .env("DX_LEASE_CHILD", mode)
            .env("DX_LEASE_ROOT", root)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("spawn lease child")
    }

    const RELEASE: Duration = Duration::from_secs(30);

    fn wait_for(path: &Path, what: &str) {
        dx_test_wait::path_appears(path, RELEASE).unwrap_or_else(|error| panic!("{what}: {error}"));
    }

    fn free(path: &Path, what: &str) {
        dx_test_wait::lock_free(path, RELEASE).unwrap_or_else(|error| panic!("{what}: {error}"));
    }

    fn child_main(root: &Path, mode: &str) {
        let ready = root.join("child-ready");
        match mode {
            "die" => {
                // LCOV_EXCL_START - reason: runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
                let _held = acquire_shared(root, GenerationUse::Generated, HEX_B, Duration::ZERO)
                    .expect("child acquires");
                std::fs::write(&ready, b"ready").expect("signal ready");
                drop(io::Read::read_to_end(&mut io::stdin(), &mut Vec::new()));
            }
            // LCOV_EXCL_STOP - reason: end runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
                    Err(error) => panic!("contention probe wants WouldBlock, got {error:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
                };
                std::fs::write(root.join("child-verdict"), verdict).expect("signal verdict");
            }
            other => panic!("unknown child mode {other:?}"), // LCOV_EXCL_LINE - reason: test failure arm, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        }
    }

    #[test]
    fn killed_holder_releases_the_generation() {
        // LCOV_EXCL_START - reason: runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        if let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        {
            let root = std::env::var_os("DX_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child root");
            child_main(&root, &mode);
            return;
        }
        // LCOV_EXCL_STOP - reason: end runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        let _spawning = dx_testing::spawning();
        let scratch = dx_test_scratch::scratch("dx-lease-crash-").unwrap();
        let root = root(scratch.path());
        let mut child = spawn_child("killed_holder_releases_the_generation", &root, "die");
        wait_for(&root.join("child-ready"), "child ready");
        assert!(
            shared_held(&root, GenerationUse::Generated, HEX_B).expect("held probe"),
            "the live child blocks exclusivity"
        );
        child.kill().expect("kill the holder");
        let status = child.wait().expect("reap the holder");
        assert!(!status.success(), "the holder dies abnormally");
        free(
            &lease_path(&root, GenerationUse::Generated, HEX_B).expect("path"),
            "a killed holder must release its lease",
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn shared_acquire_fails_closed_while_cleanup_holds_exclusive() {
        // LCOV_EXCL_START - reason: runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        if let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        {
            let root = std::env::var_os("DX_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child root");
            child_main(&root, &mode);
            return;
        }
        // LCOV_EXCL_STOP - reason: end runs only in the spawned child, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
        let _spawning = dx_testing::spawning();
        let scratch = dx_test_scratch::scratch("dx-lease-contend-").unwrap();
        let root = root(scratch.path());
        let exclusive = try_acquire_exclusive(&root, GenerationUse::Generated, HEX_B)
            .expect("probe the idle generation")
            .expect("cleanup holds the generation while it prunes");
        let mut blocked = spawn_child(
            "shared_acquire_fails_closed_while_cleanup_holds_exclusive",
            &root,
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
        free(
            &lease_path(&root, GenerationUse::Generated, HEX_B).expect("path"),
            "release of the exclusive cleanup hold",
        );
        std::fs::remove_file(root.join("child-verdict")).expect("reset the verdict");
        let mut admitted = spawn_child(
            "shared_acquire_fails_closed_while_cleanup_holds_exclusive",
            &root,
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
