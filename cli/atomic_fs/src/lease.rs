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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationUse {
    Environment,
    Generated,
}

impl GenerationUse {
    /// Generation directory this use protects.
    pub fn dir_name(self) -> &'static str {
        match self {
            GenerationUse::Environment => "environments",
            GenerationUse::Generated => "generated",
        }
    }

    /// Parses the `kind` half of a `kind:hex` lease entry.
    pub fn parse(kind: &str) -> Option<GenerationUse> {
        match kind {
            "environments" => Some(GenerationUse::Environment),
            "generated" => Some(GenerationUse::Generated),
            _ => None,
        }
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
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// Holds the shared lock on one open lease file, waiting up to `timeout`.
fn lock_shared(file: &File, timeout: Duration) -> io::Result<()> {
    let start = std::time::Instant::now();
    loop {
        match file.try_lock_shared() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => {
                if start.elapsed() >= timeout {
                    return Err(io::Error::from(io::ErrorKind::WouldBlock));
                }
                std::thread::sleep(super::LOCK_POLL);
            }
            Err(std::fs::TryLockError::Error(other)) => return Err(other),
        }
    }
}

/// Reports whether the exclusive lock on one open lease file is free.
fn try_lock_exclusive(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(other)) => Err(other),
    }
}

/// Holds a shared lease, waiting up to `timeout` for exclusive holders.
pub fn acquire_shared(
    root: &Path,
    kind: GenerationUse,
    hex: &str,
    timeout: Duration,
) -> io::Result<SharedLease> {
    let path = lease_path(root, kind, hex)?;
    let file = open_lease_file(&path)?;
    lock_shared(&file, timeout)?;
    Ok(SharedLease { file })
}

/// Holds the exclusive lease when no shared holder exists.
pub fn try_acquire_exclusive(
    root: &Path,
    kind: GenerationUse,
    hex: &str,
) -> io::Result<Option<ExclusiveLease>> {
    let path = lease_path(root, kind, hex)?;
    let file = open_lease_file(&path)?;
    if try_lock_exclusive(&file)? {
        Ok(Some(ExclusiveLease { file }))
    } else {
        Ok(None)
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
        assert!(
            try_acquire_exclusive(&root, GenerationUse::Environment, HEX_A)
                .expect("probe after release")
                .is_some(),
            "releasing the shared lease frees the generation"
        );
        scratch.close().expect("cleanup");
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
    fn inherit_value_round_trips_through_the_child_entry() {
        assert_eq!(inherit_value(&[]), "");
        let rendered = inherit_value(&[
            (GenerationUse::Environment, HEX_A),
            (GenerationUse::Generated, HEX_B),
        ]);
        assert_eq!(rendered, format!("environments:{HEX_A},generated:{HEX_B}"));
        std::env::remove_var(LEASE_ENV_VAR);
        assert!(
            acquire_shared_from_env(Path::new("/nonexistent"), Duration::ZERO)
                .expect("absent inheritance")
                .is_empty(),
            "absent inheritance holds no leases"
        );
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
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt as _;
            std::env::set_var(
                LEASE_ENV_VAR,
                std::ffi::OsStr::from_bytes(b"environments:\xff"),
            );
            assert!(
                acquire_shared_from_env(Path::new("/nonexistent"), Duration::ZERO).is_err(),
                "non-UTF-8 inheritance fails closed"
            );
        }
        std::env::remove_var(LEASE_ENV_VAR);
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

    fn wait_for(path: &Path, what: &str, timeout: Duration) {
        let start = std::time::Instant::now();
        while !path.exists() {
            if start.elapsed() > timeout {
                panic!("timed out waiting for {what}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn released_within(
        root: &Path,
        kind: GenerationUse,
        hex: &str,
        timeout: Duration,
    ) -> io::Result<bool> {
        let start = std::time::Instant::now();
        loop {
            if !shared_held(root, kind, hex)? {
                return Ok(true);
            }
            if start.elapsed() > timeout {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn maybe_run_as_child() -> bool {
        let Some(mode) =
            std::env::var_os("DX_LEASE_CHILD").and_then(|mode| mode.to_str().map(str::to_owned))
        else {
            return false;
        };
        let root = std::env::var_os("DX_LEASE_ROOT")
            .map(PathBuf::from)
            .expect("child root");
        child_main(&root, &mode);
        true
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
                wait_for(&release, "release", Duration::from_secs(30));
                drop(held);
            }
            "die" => {
                let _held = acquire_shared(root, GenerationUse::Generated, HEX_B, Duration::ZERO)
                    .expect("child acquires");
                std::fs::write(&ready, b"ready").expect("signal ready");
                wait_for(&release, "release", Duration::from_secs(30));
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
                    Err(error) => panic!("contention probe wants WouldBlock, got {error:?}"), // LCOV_EXCL_LINE - reason: defensive probe arm, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
                };
                std::fs::write(root.join("child-verdict"), verdict).expect("signal verdict");
            }
            other => panic!("unknown child mode {other:?}"), // LCOV_EXCL_LINE - reason: literal modes only, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        }
    }

    #[test]
    fn killed_holder_releases_the_generation() {
        if maybe_run_as_child() {
            return; // LCOV_EXCL_LINE - reason: killed child never flushes, issue: 1381, policy: docs/cli/commands/build-test-coverage.md
        }
        let scratch = dx_test_scratch::scratch("dx-lease-crash-");
        let root = root(scratch.path());
        let mut child = spawn_child("killed_holder_releases_the_generation", &root, "", "die");
        wait_for(
            &root.join("child-ready"),
            "child ready",
            Duration::from_secs(30),
        );
        assert!(
            shared_held(&root, GenerationUse::Generated, HEX_B).expect("held probe"),
            "the live child blocks exclusivity"
        );
        child.kill().expect("kill the holder");
        let status = child.wait().expect("reap the holder");
        assert!(!status.success(), "the holder dies abnormally");
        assert!(
            released_within(
                &root,
                GenerationUse::Generated,
                HEX_B,
                Duration::from_secs(30)
            )
            .expect("release probe"),
            "a killed holder must release its lease"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn surviving_child_keeps_the_generation_after_its_parent_releases() {
        if maybe_run_as_child() {
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
        wait_for(
            &root.join("child-ready"),
            "child ready",
            Duration::from_secs(30),
        );
        drop(parent);
        assert!(
            shared_held(&root, GenerationUse::Environment, HEX_A).expect("inherited probe"),
            "the surviving child still protects the generation after its parent releases"
        );
        std::fs::write(root.join("child-release"), b"go").expect("release the child");
        let status = child.wait().expect("reap the child");
        assert!(status.success(), "the child exits cleanly");
        assert!(
            !shared_held(&root, GenerationUse::Environment, HEX_A).expect("final probe"),
            "the generation frees once every holder exits"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn shared_acquire_fails_closed_while_cleanup_holds_exclusive() {
        if maybe_run_as_child() {
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
        wait_for(
            &root.join("child-verdict"),
            "contention verdict",
            Duration::from_secs(30),
        );
        let status = blocked.wait().expect("reap the contender");
        assert!(status.success(), "the contender exits cleanly");
        assert_eq!(
            std::fs::read_to_string(root.join("child-verdict")).expect("read the verdict"),
            "blocked",
            "a shared acquire during an exclusive cleanup hold fails instead of waiting forever"
        );
        drop(exclusive);
        std::fs::remove_file(root.join("child-verdict")).expect("reset the verdict");
        let mut admitted = spawn_child(
            "shared_acquire_fails_closed_while_cleanup_holds_exclusive",
            &root,
            "",
            "contend",
        );
        wait_for(
            &root.join("child-verdict"),
            "admission verdict",
            Duration::from_secs(30),
        );
        let status = admitted.wait().expect("reap the admitted child");
        assert!(status.success(), "the admitted child exits cleanly");
        assert_eq!(
            std::fs::read_to_string(root.join("child-verdict")).expect("read the verdict"),
            "acquired",
            "releasing the exclusive hold admits shared acquirers again"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn shared_into_file_keeps_the_lock_until_dropped() {
        let scratch = dx_test_scratch::scratch("dx-lease-handoff-");
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
            try_acquire_exclusive(&root, GenerationUse::Environment, HEX_A)
                .expect("handoff probe")
                .is_none(),
            "a handed-off handle keeps the generation locked"
        );
        drop(file);
        assert!(
            try_acquire_exclusive(&root, GenerationUse::Environment, HEX_A)
                .expect("probe after drop")
                .is_some(),
            "dropping the handed-off handle frees the generation"
        );
        scratch.close().expect("cleanup");
    }

    /// Linux O_PATH handles accept no locks, so the kernel fails each lock
    /// call with EBADF: a deterministic OS error at the lock boundary.
    #[cfg(target_os = "linux")]
    #[test]
    fn lock_failures_surface_os_errors_instead_of_waiting() {
        use std::os::unix::fs::OpenOptionsExt as _;
        let probe = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0o10000000)
            .open("/")
            .expect("open an O_PATH probe");
        let error = lock_shared(&probe, Duration::ZERO).expect_err("lock must fail");
        assert!(
            error.kind() != io::ErrorKind::WouldBlock && error.raw_os_error().is_some(),
            "a lock failure surfaces its OS error, got {error:?}"
        );
        let error = try_lock_exclusive(&probe).expect_err("exclusive probe must fail");
        assert!(
            error.kind() != io::ErrorKind::WouldBlock && error.raw_os_error().is_some(),
            "an exclusive probe surfaces its OS error, got {error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_parentless_path_fails_closed() {
        assert!(
            open_lease_file(Path::new("/")).is_err(),
            "a parentless lease path fails instead of creating directories"
        );
    }

    #[test]
    #[should_panic(expected = "timed out waiting")]
    fn waiting_gives_up_after_its_bound() {
        let scratch = dx_test_scratch::scratch("dx-lease-wait-");
        wait_for(
            &scratch.path().join("never-appears"),
            "missing sentinel",
            Duration::from_millis(50),
        );
    }

    #[test]
    fn release_poll_times_out_while_a_holder_stays_alive() {
        let scratch = dx_test_scratch::scratch("dx-lease-poll-");
        let root = root(scratch.path());
        let held = acquire_shared(
            &root,
            GenerationUse::Generated,
            HEX_B,
            Duration::from_secs(10),
        )
        .expect("holder acquires");
        assert!(
            !released_within(
                &root,
                GenerationUse::Generated,
                HEX_B,
                Duration::from_millis(50)
            )
            .expect("bounded poll"),
            "a live holder keeps the generation past a short bound"
        );
        drop(held);
        assert!(
            released_within(
                &root,
                GenerationUse::Generated,
                HEX_B,
                Duration::from_secs(10)
            )
            .expect("release poll"),
            "a released generation polls free"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn released_holder_exits_cleanly_and_frees_the_generation() {
        if maybe_run_as_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-lease-release-");
        let root = root(scratch.path());
        let mut child = spawn_child(
            "released_holder_exits_cleanly_and_frees_the_generation",
            &root,
            "",
            "die",
        );
        wait_for(
            &root.join("child-ready"),
            "child ready",
            Duration::from_secs(30),
        );
        assert!(
            shared_held(&root, GenerationUse::Generated, HEX_B).expect("held probe"),
            "the live child blocks exclusivity"
        );
        std::fs::write(root.join("child-release"), b"go").expect("release the child");
        let status = child.wait().expect("reap the child");
        assert!(status.success(), "the released child exits cleanly");
        assert!(
            !shared_held(&root, GenerationUse::Generated, HEX_B).expect("final probe"),
            "the generation frees once the holder exits"
        );
        scratch.close().expect("cleanup");
    }
}
