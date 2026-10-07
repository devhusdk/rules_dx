use dx_atomic_fs::{classify, materialize_file, write_atomic, EntryKind, LinkPolicy, Mechanism};
use dx_process::lifecycle::{
    self, CapturePolicy, ChildOutcome, EnvPolicy, ExitKind, SpawnSpec, StdinPolicy, TreePolicy,
};
use normpath::BasePathBuf;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorContents {
    Bytes(Vec<u8>),
    Link(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorFile {
    pub mirror_rel: PathBuf,
    pub contents: MirrorContents,
}

/// How one mirror file reached the scratch tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedContents {
    Bytes,
    Linked,
    Copied,
}

/// One mirror file materialized into the scratch tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    pub mirror_rel: PathBuf,
    pub contents: StagedContents,
}

#[derive(Debug)]
pub struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    pub fn create(parent: &Path) -> io::Result<Scratch> {
        let dir = tempfile::Builder::new()
            .prefix("dx-scratch-")
            .tempdir_in(parent)?;
        Ok(Scratch { dir })
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn resolve(&self, rel: &Path) -> io::Result<PathBuf> {
        let root = self.dir.path();
        let raw = rel.as_os_str().as_encoded_bytes();
        if raw.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mirror path is empty",
            ));
        }
        if raw.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mirror path contains null byte: {}", rel.display()),
            ));
        }
        if raw.contains(&b'\\') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mirror path contains backslash: {}", rel.display()),
            ));
        }
        let mut absolute = BasePathBuf::new(root.to_owned()).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("scratch root is not a base path: {error}"),
            )
        })?;
        for component in rel.components() {
            use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
            match component {
                Normal(part) => absolute.push(part),
                CurDir => {}
                ParentDir => {
                    if absolute.pop().is_err() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("mirror path escapes scratch: {}", rel.display()),
                        ));
                    }
                    if absolute.as_path() != root && !absolute.starts_with(root) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("mirror path escapes scratch: {}", rel.display()),
                        ));
                    }
                }
                RootDir | Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("mirror path is absolute: {}", rel.display()),
                    ));
                }
            }
        }
        if absolute.as_path() != root && !absolute.starts_with(root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mirror path escapes scratch: {}", rel.display()),
            ));
        }
        Ok(absolute.into_path_buf())
    }

    pub fn materialize(&self, files: &[MirrorFile]) -> io::Result<Vec<StagedFile>> {
        let root = self.dir.path();
        let mut staged = Vec::with_capacity(files.len());
        for file in files {
            let absolute = self.resolve(&file.mirror_rel)?;
            let parent = absolute.parent().ok_or_else(|| {
                // LCOV_EXCL_LINE - reason: defensive diverge, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("scratch path has no parent: {}", absolute.display()),
                )
            })?;
            ensure_no_symlink_prefix(root, parent, &file.mirror_rel)?;
            if classify(&absolute)? == EntryKind::Link {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "mirror path escapes scratch via symlink: {}",
                        file.mirror_rel.display()
                    ),
                ));
            }
            std::fs::create_dir_all(parent)?;
            ensure_no_symlink_prefix(root, parent, &file.mirror_rel)?;
            let contents = match &file.contents {
                MirrorContents::Bytes(bytes) => {
                    write_atomic(&absolute, bytes)?;
                    StagedContents::Bytes
                }
                MirrorContents::Link(source) => {
                    match materialize_file(source, &absolute, LinkPolicy::LinkOrCopy)? {
                        Mechanism::Linked => StagedContents::Linked,
                        Mechanism::Copied => StagedContents::Copied,
                    }
                }
            };
            staged.push(StagedFile {
                mirror_rel: file.mirror_rel.clone(),
                contents,
            });
        }
        Ok(staged)
    }

    pub fn close(self) -> io::Result<()> {
        self.dir.close()
    }
}

fn ensure_no_symlink_prefix(root: &Path, path: &Path, rel: &Path) -> io::Result<()> {
    let suffix = path.strip_prefix(root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("mirror path escapes scratch: {}", rel.display()),
        )
    })?;
    let mut current = root.to_owned();
    for component in suffix.components() {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
        match component {
            Normal(part) => current.push(part),
            CurDir => {}
            ParentDir => {
                current.pop();
            }
            RootDir | Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("mirror path escapes scratch: {}", rel.display()),
                ));
            }
        }
        match classify(&current)? {
            EntryKind::Link => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("mirror path escapes scratch via symlink: {}", rel.display()),
                ));
            }
            EntryKind::Missing | EntryKind::File | EntryKind::Directory => {}
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

const WINDOWS_AMBIENT_KEYS: &[&str] = &["SystemRoot"];

pub fn hermetic_env(
    scratch: &Path,
    extra: &[(OsString, OsString)],
    ambient: &[(OsString, OsString)],
) -> Vec<(OsString, OsString)> {
    build_env(scratch, extra, ambient, cfg!(windows))
}

fn fold_env_key(key: &OsStr, windows: bool) -> OsString {
    if windows {
        OsString::from(key.to_string_lossy().to_ascii_uppercase())
    } else {
        key.to_owned()
    }
}

fn push_env_once(
    out: &mut Vec<(OsString, OsString)>,
    seen: &mut HashSet<OsString>,
    key: OsString,
    value: OsString,
    windows: bool,
) {
    if seen.insert(fold_env_key(&key, windows)) {
        out.push((key, value));
    }
}

fn build_env(
    scratch: &Path,
    extra: &[(OsString, OsString)],
    ambient: &[(OsString, OsString)],
    windows: bool,
) -> Vec<(OsString, OsString)> {
    let scratch = scratch.as_os_str().to_owned();
    let mut out = Vec::with_capacity(5 + extra.len() + 1);
    let mut seen = HashSet::new();
    for (key, value) in [
        (OsString::from("TMPDIR"), scratch.clone()),
        (OsString::from("TEMP"), scratch.clone()),
        (OsString::from("TMP"), scratch.clone()),
        (OsString::from("LANG"), OsString::from("C.UTF-8")),
        (OsString::from("TZ"), OsString::from("UTC")),
    ] {
        push_env_once(&mut out, &mut seen, key, value, windows);
    }
    for (key, value) in extra {
        push_env_once(&mut out, &mut seen, key.clone(), value.clone(), windows);
    }
    if windows {
        let kept: Vec<OsString> = WINDOWS_AMBIENT_KEYS
            .iter()
            .map(|key| fold_env_key(OsStr::new(key), true))
            .collect();
        for (key, value) in ambient {
            if kept.contains(&fold_env_key(key, true)) {
                push_env_once(&mut out, &mut seen, key.clone(), value.clone(), windows);
            }
        }
    }
    out
}

pub const SPAWN_TIMEOUT: Duration = Duration::from_secs(120);

pub fn spawn(
    argv: &[impl AsRef<OsStr>],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    spawn_with_timeout(argv, cwd, env, SPAWN_TIMEOUT)
}

pub fn spawn_with_timeout(
    argv: &[impl AsRef<OsStr>],
    cwd: &Path,
    env: &[(OsString, OsString)],
    timeout: Duration,
) -> io::Result<ChildOutput> {
    let spec = SpawnSpec {
        argv: argv
            .iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect(),
        cwd: cwd.to_path_buf(),
        stdin: StdinPolicy::Inherit,
        tree: TreePolicy::Owned,
        timeout: Some(timeout),
    };
    let capture = CapturePolicy {
        max_output_bytes: crate::parsers::MAX_OUTPUT_BYTES,
    };
    let outcome = lifecycle::run(
        &spec,
        &EnvPolicy::Controlled {
            vars: env.to_vec(),
        },
        &capture,
    );
    match outcome {
        ChildOutcome::Completed {
            exit,
            stdout,
            stderr,
        } => Ok(ChildOutput {
            code: match exit {
                ExitKind::Code(code) => Some(code),
                ExitKind::Signaled { .. } => None,
            },
            stdout,
            stderr,
        }),
        ChildOutcome::TimedOut => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("tool timed out after {}s", timeout.as_secs()),
        )),
        ChildOutcome::Failed { error, .. } => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_materializes_and_cleans_up() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-materialize-")
            .tempdir_in(std::env::temp_dir())
            .expect("materialize parent");
        let parent = parent_tmp.path().to_path_buf();
        std::fs::create_dir_all(&parent).expect("materialize parent");
        let source = parent.join("native-taplo.toml");
        std::fs::write(&source, b"config = true\n").expect("closure source");
        let scratch = Scratch::create(&parent).expect("scratch");
        let root = scratch.root().to_owned();
        assert!(root.is_dir());
        let staged = scratch
            .materialize(&[
                MirrorFile {
                    mirror_rel: PathBuf::from("src/main.rs"),
                    contents: MirrorContents::Bytes(b"fn main() {}\n".to_vec()),
                },
                MirrorFile {
                    mirror_rel: PathBuf::from("taplo.toml"),
                    contents: MirrorContents::Link(source.clone()),
                },
            ])
            .expect("materialize");
        assert_eq!(
            std::fs::read(root.join("src/main.rs")).expect("read back"),
            b"fn main() {}\n"
        );
        assert_eq!(
            std::fs::read(root.join("taplo.toml")).expect("closure entry"),
            b"config = true\n"
        );
        let closure = staged
            .iter()
            .find(|file| file.mirror_rel == PathBuf::from("taplo.toml"))
            .expect("staged closure entry");
        let linked = std::fs::symlink_metadata(root.join("taplo.toml"))
            .expect("metadata")
            .file_type()
            .is_symlink();
        assert_eq!(
            closure.contents,
            if linked {
                StagedContents::Linked
            } else {
                StagedContents::Copied
            },
            "the reported mechanism matches what the scratch tree holds"
        );
        assert_eq!(
            staged[0],
            StagedFile {
                mirror_rel: PathBuf::from("src/main.rs"),
                contents: StagedContents::Bytes,
            }
        );
        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(root.join("taplo.toml")).expect("symlink preferred on unix"),
            source,
        );
        drop(scratch);
        assert!(!root.exists(), "scratch is removed on drop");
        parent_tmp.close().expect("materialize cleanup");
    }

    #[test]
    fn materialize_copies_closure_entry_when_link_path_exists() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-fallback-")
            .tempdir_in(std::env::temp_dir())
            .expect("fallback parent");
        let parent = parent_tmp.path().to_path_buf();
        std::fs::create_dir_all(&parent).expect("fallback parent");
        let source = parent.join("native.toml");
        std::fs::write(&source, b"config = true\n").expect("closure source");
        let scratch = Scratch::create(&parent).expect("scratch");
        let dest = scratch.root().join("taplo.toml");
        std::fs::write(&dest, b"stale\n").expect("pre-existing link path");
        let staged = scratch
            .materialize(&[MirrorFile {
                mirror_rel: PathBuf::from("taplo.toml"),
                contents: MirrorContents::Link(source),
            }])
            .expect("fallback copy");
        assert_eq!(
            staged[0].contents,
            StagedContents::Copied,
            "an occupied name cannot become a link, so the fallback copy reports itself"
        );
        assert_eq!(
            std::fs::read(&dest).expect("copied entry"),
            b"config = true\n"
        );
        assert!(
            !std::fs::symlink_metadata(&dest)
                .expect("metadata")
                .file_type()
                .is_symlink(),
            "fallback copies instead of linking"
        );
        scratch.close().expect("close");
        parent_tmp.close().expect("fallback cleanup");
    }

    #[test]
    fn resolve_rejects_escapes_and_absolute_paths() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        assert!(scratch.resolve(Path::new("../../evil")).is_err());
        assert!(scratch.resolve(Path::new("/absolute")).is_err());
        assert!(scratch
            .resolve(Path::new("../../../../../../../../evil"))
            .is_err());
        assert_eq!(
            scratch
                .resolve(Path::new("sub/../ok.rs"))
                .expect("contained dot-dot"),
            scratch.root().join("ok.rs")
        );
        assert_eq!(
            scratch
                .resolve(Path::new("./ok.rs"))
                .expect("dot component"),
            scratch.root().join("ok.rs")
        );
    }

    #[test]
    fn resolve_normalizes_dot_segments_via_normpath() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        assert_eq!(
            scratch.resolve(Path::new("a/b/../c.rs")).expect("a/b/../c"),
            scratch.root().join("a/c.rs")
        );
        assert_eq!(
            scratch.resolve(Path::new("./a.rs")).expect("dot slash"),
            scratch.root().join("a.rs")
        );
        assert_eq!(
            scratch.resolve(Path::new("a/b/")).expect("trailing slash"),
            scratch.root().join("a/b")
        );
        assert!(scratch.resolve(Path::new("a/../../evil")).is_err());
        assert!(scratch.resolve(Path::new("a/b/../../../evil")).is_err());
    }

    #[test]
    fn resolve_rejects_empty_null_and_backslash() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        assert!(scratch.resolve(Path::new("")).is_err(), "empty");
        assert!(scratch.resolve(Path::new("a\0b")).is_err(), "null byte");
        assert!(scratch.resolve(Path::new("a\\b")).is_err(), "backslash");
        assert!(
            scratch.resolve(Path::new("..\\evil")).is_err(),
            "mixed separators"
        );
        assert!(
            scratch.resolve(Path::new("\\\\server\\share")).is_err(),
            "UNC"
        );
        assert_eq!(
            scratch.resolve(Path::new("a/b/c.rs")).expect("nested"),
            scratch.root().join("a/b/c.rs")
        );
    }

    #[test]
    #[cfg(unix)]
    fn materialize_rejects_symlink_directory_escape() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-symlink-dir-")
            .tempdir_in(std::env::temp_dir())
            .expect("symlink parent");
        let parent = parent_tmp.path().to_path_buf();
        let outside = parent.join("outside");
        std::fs::create_dir_all(&outside).expect("outside dir");
        let scratch = Scratch::create(&parent).expect("scratch");
        let root = scratch.root().to_owned();
        std::os::unix::fs::symlink(&outside, root.join("evil")).expect("plant symlink dir");
        assert_eq!(
            std::fs::read_link(root.join("evil")).expect("symlink planted"),
            outside,
        );
        let err = scratch
            .materialize(&[MirrorFile {
                mirror_rel: PathBuf::from("evil/pwned"),
                contents: MirrorContents::Bytes(b"pwned\n".to_vec()),
            }])
            .expect_err("symlink prefix must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(!outside.join("pwned").exists(), "outside stays clean");
        scratch.close().expect("close");
        parent_tmp.close().expect("cleanup");
    }

    #[test]
    fn materialize_rejects_a_directory_source() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-mirror-dir-")
            .tempdir_in(std::env::temp_dir())
            .expect("mirror dir parent");
        let parent = parent_tmp.path().to_path_buf();
        let outside = parent.join("outside");
        std::fs::create_dir_all(outside.join("nested")).expect("outside dir");
        let scratch = Scratch::create(&parent).expect("scratch");
        let err = scratch
            .materialize(&[MirrorFile {
                mirror_rel: PathBuf::from("tree"),
                contents: MirrorContents::Link(outside.clone()),
            }])
            .expect_err("mirror entries are immutable files, never trees");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(classify(&outside).expect("outside"), EntryKind::Directory);
        assert!(outside.join("nested").is_dir(), "the tree is untouched");
        scratch.close().expect("close");
        parent_tmp.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn materialize_rejects_symlink_file_overwrite() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-symlink-file-")
            .tempdir_in(std::env::temp_dir())
            .expect("symlink parent");
        let parent = parent_tmp.path().to_path_buf();
        let outside = parent.join("secret");
        std::fs::write(&outside, b"secret\n").expect("outside file");
        let scratch = Scratch::create(&parent).expect("scratch");
        let root = scratch.root().to_owned();
        std::os::unix::fs::symlink(&outside, root.join("link")).expect("plant file symlink");
        let err = scratch
            .materialize(&[MirrorFile {
                mirror_rel: PathBuf::from("link"),
                contents: MirrorContents::Bytes(b"pwned\n".to_vec()),
            }])
            .expect_err("symlink target must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            std::fs::read(&outside).expect("outside read"),
            b"secret\n",
            "outside unchanged"
        );
        scratch.close().expect("close");
        parent_tmp.close().expect("cleanup");
    }

    #[test]
    fn create_reports_unusable_parents() {
        let missing = std::env::temp_dir().join("dx-no-such-parent-9f2c1d");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(Scratch::create(&missing.join("child")).is_err());
    }

    #[test]
    fn scratch_claims_distinct_prefixed_trees() {
        let parent_tmp = tempfile::Builder::new()
            .prefix("dx-distinct-")
            .tempdir_in(std::env::temp_dir())
            .expect("distinct parent");
        let parent = parent_tmp.path().to_path_buf();
        std::fs::create_dir_all(&parent).expect("distinct parent");
        let first = Scratch::create(&parent).expect("first");
        let second = Scratch::create(&parent).expect("second");
        assert_ne!(first.root(), second.root(), "claims never share a tree");
        for root in [first.root(), second.root()] {
            assert!(root.starts_with(&parent), "scratch lives under the parent");
            assert!(
                root.file_name()
                    .expect("scratch name")
                    .to_string_lossy()
                    .starts_with("dx-scratch-"),
                "scratch keeps the recognizable prefix",
            );
        }
        first.close().expect("close");
        second.close().expect("close");
        parent_tmp.close().expect("distinct cleanup");
    }

    #[test]
    fn scratch_names_are_unique() {
        let parent = std::env::temp_dir();
        let mut roots = Vec::new();
        for _ in 0..64 {
            let scratch = Scratch::create(&parent).expect("scratch");
            roots.push(scratch.root().to_owned());
            scratch.close().expect("close");
        }
        roots.sort();
        roots.dedup();
        assert_eq!(roots.len(), 64, "every scratch claims a distinct tree");
    }

    #[test]
    fn close_removes_tree_and_surfaces_errors() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        scratch.close().expect("close removes the tree");
        assert!(!root.exists(), "closed scratch is gone");
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        std::fs::remove_dir_all(&root).expect("pre-remove");
        assert!(scratch.close().is_err(), "missing tree is an error");
    }

    fn pair(key: &str, value: &str) -> (OsString, OsString) {
        (OsString::from(key), OsString::from(value))
    }

    fn value_of(env: &[(OsString, OsString)], key: &str) -> Option<OsString> {
        env.iter()
            .find(|(known, _)| known == key)
            .map(|(_, value)| value.clone())
    }

    fn hostile_ambient() -> Vec<(OsString, OsString)> {
        vec![
            pair("TMPDIR", "/evil-tmpdir"),
            pair("TEMP", "/evil-temp"),
            pair("TMP", "/evil-tmp"),
            pair("PATH", "/evil-bin"),
            pair("HOME", "/evil-home"),
            pair("USERPROFILE", "C:\\evil"),
            pair("APPDATA", "C:\\evil-appdata"),
            pair("LOCALAPPDATA", "C:\\evil-local"),
            pair("PYTHONPATH", "/evil-py"),
            pair("PYTHONHOME", "/evil-home-py"),
            pair("NODE_OPTIONS", "--evil"),
            pair("NODE_PATH", "/evil-node"),
            pair("SystemRoot", "C:\\Windows"),
        ]
    }

    #[test]
    fn hermetic_env_is_deterministic_for_an_empty_ambient() {
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(
            Path::new("/tmp/dx"),
            &[pair("LD_LIBRARY_PATH", "/lib")],
            &ambient,
        );
        assert_eq!(
            env,
            vec![
                pair("TMPDIR", "/tmp/dx"),
                pair("TEMP", "/tmp/dx"),
                pair("TMP", "/tmp/dx"),
                pair("LANG", "C.UTF-8"),
                pair("TZ", "UTC"),
                pair("LD_LIBRARY_PATH", "/lib"),
            ]
        );
    }

    #[test]
    fn hermetic_env_pins_temp_over_ambient_and_extra() {
        for windows in [false, true] {
            let env = build_env(
                Path::new("/tmp/dx"),
                &[pair("TMPDIR", "/evil"), pair("Temp", "/evil")],
                &hostile_ambient(),
                windows,
            );
            for key in ["TMPDIR", "TEMP", "TMP"] {
                assert_eq!(
                    value_of(&env, key),
                    Some(OsString::from("/tmp/dx")),
                    "{key} stays pinned (windows={windows})"
                );
            }
        }
    }

    #[test]
    fn hermetic_env_pins_locale_and_time() {
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(
            Path::new("/tmp/dx"),
            &[
                pair("LANG", "de_DE.UTF-8"),
                pair("TZ", "Europe/Berlin"),
                pair("LC_ALL", "de_DE.UTF-8"),
            ],
            &ambient,
        );
        assert_eq!(
            env.iter().filter(|(key, _)| key == "LANG").count(),
            1,
            "exactly one LANG"
        );
        assert_eq!(
            env.iter().filter(|(key, _)| key == "TZ").count(),
            1,
            "exactly one TZ"
        );
        assert!(env.contains(&pair("LANG", "C.UTF-8")));
        assert!(env.contains(&pair("TZ", "UTC")));
        assert!(env.contains(&pair("LC_ALL", "de_DE.UTF-8")));
    }

    #[test]
    fn hermetic_env_never_leaks_user_profile_or_config_discovery() {
        for windows in [false, true] {
            let env = build_env(Path::new("/tmp/dx"), &[], &hostile_ambient(), windows);
            for key in [
                "PATH",
                "HOME",
                "USERPROFILE",
                "APPDATA",
                "LOCALAPPDATA",
                "PYTHONPATH",
                "PYTHONHOME",
                "NODE_OPTIONS",
                "NODE_PATH",
            ] {
                assert!(
                    env.iter().all(|(known, _)| fold_env_key(known, windows)
                        != fold_env_key(OsStr::new(key), windows)),
                    "{key} never leaks (windows={windows})"
                );
            }
        }
    }

    #[test]
    fn hermetic_env_inherits_only_system_root_on_windows() {
        let unix = build_env(Path::new("/tmp/dx"), &[], &hostile_ambient(), false);
        assert!(
            unix.iter().all(|(key, _)| key != "SystemRoot"),
            "unix inherits nothing"
        );
        let windows = build_env(Path::new("/tmp/dx"), &[], &hostile_ambient(), true);
        assert_eq!(
            value_of(&windows, "SystemRoot"),
            Some(OsString::from("C:\\Windows"))
        );
        assert_eq!(
            windows
                .iter()
                .filter(|(key, _)| fold_env_key(key, true) == OsString::from("SYSTEMROOT"))
                .count(),
            1,
            "exactly one system root"
        );
    }

    #[test]
    fn hermetic_env_prefers_explicit_extra_over_inherited_system_root() {
        let env = build_env(
            Path::new("/tmp/dx"),
            &[pair("SystemRoot", "D:\\alternate")],
            &hostile_ambient(),
            true,
        );
        assert_eq!(
            value_of(&env, "SystemRoot"),
            Some(OsString::from("D:\\alternate"))
        );
    }

    #[test]
    fn hermetic_env_collapses_conflicting_casing_deterministically() {
        let env = build_env(
            Path::new("/tmp/dx"),
            &[pair("Path", "/first"), pair("PATH", "/second")],
            &[pair("SystemRoot", "C:\\one"), pair("SYSTEMROOT", "C:\\two")],
            true,
        );
        assert_eq!(value_of(&env, "Path"), Some(OsString::from("/first")));
        assert_eq!(
            env.iter()
                .filter(|(key, _)| fold_env_key(key, true) == OsString::from("PATH"))
                .count(),
            1,
            "one PATH folding"
        );
        assert_eq!(
            value_of(&env, "SystemRoot"),
            Some(OsString::from("C:\\one"))
        );
    }

    #[test]
    fn delivered_env_matches_the_policy() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        let env = hermetic_env(&root, &[], &hostile_ambient());
        let probe = dx_testing::process_probe();
        let root_text = root.to_string_lossy().into_owned();
        let require = |name: &str, value: &str| format!("--require-env={name}={value}");
        let forbid = |name: &str| format!("--forbid-env={name}");
        let argv = vec![
            probe.into_os_string(),
            OsString::from(require("TMPDIR", &root_text)),
            OsString::from(require("TEMP", &root_text)),
            OsString::from(require("TMP", &root_text)),
            OsString::from(require("LANG", "C.UTF-8")),
            OsString::from(require("TZ", "UTC")),
            OsString::from(forbid("PATH")),
            OsString::from(forbid("HOME")),
            OsString::from(forbid("USERPROFILE")),
            OsString::from(forbid("PYTHONPATH")),
            OsString::from(forbid("NODE_OPTIONS")),
        ];
        let checked = spawn(&argv, &root, &env).expect("probe runs");
        assert_eq!(
            checked.code,
            Some(0),
            "probe stderr: {}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let listed = spawn(
            &[argv[0].clone(), OsString::from("--print-env-all")],
            &root,
            &env,
        )
        .expect("probe lists");
        assert_eq!(listed.code, Some(0));
        let text = String::from_utf8(listed.stdout).expect("listing is text");
        let mut allowed = vec!["TMPDIR", "TEMP", "TMP", "LANG", "TZ"];
        if cfg!(windows) {
            allowed.push("SystemRoot");
        }
        for line in text.lines() {
            let (key, _) = line.split_once('=').expect("KEY=VALUE line");
            assert!(
                allowed.contains(&key),
                "delivered {key} is pinned or allowed"
            );
        }
        scratch.close().expect("close");
    }

    fn host_tool(name: &str) -> PathBuf {
        let exe = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        };
        let path = std::env::var_os("PATH").expect("the suite keeps its own PATH");
        std::env::split_paths(&path)
            .map(|dir| dir.join(&exe))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| panic!("{exe} is on PATH where the suite runs"))
    }

    #[test]
    fn pinned_node_starts_and_sees_pinned_temp() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        let ambient: Vec<(OsString, OsString)> = std::env::vars_os().collect();
        let env = hermetic_env(&root, &[], &ambient);
        let node = host_tool("node");
        let version = vec![node.clone().into_os_string(), OsString::from("--version")];
        let started = spawn(&version, &root, &env).expect("node starts");
        assert_eq!(started.code, Some(0));
        assert!(
            String::from_utf8_lossy(&started.stdout).starts_with('v'),
            "node reports a version"
        );
        let want = root.to_string_lossy().into_owned();
        let script = format!("console.log(process.env.TMPDIR === {want:?} ? 'pinned' : 'leaked')");
        let argv = vec![
            node.into_os_string(),
            OsString::from("--input-type=module"),
            OsString::from("--eval"),
            OsString::from(script),
        ];
        let temp = spawn(&argv, &root, &env).expect("node reads TMPDIR");
        assert_eq!(temp.code, Some(0));
        assert_eq!(String::from_utf8_lossy(&temp.stdout).trim(), "pinned");
        scratch.close().expect("close");
    }

    #[test]
    fn pinned_python_starts_and_sees_pinned_temp() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        let ambient: Vec<(OsString, OsString)> = std::env::vars_os().collect();
        let env = hermetic_env(&root, &[], &ambient);
        let python = host_tool("python3");
        let version = vec![python.clone().into_os_string(), OsString::from("--version")];
        let started = spawn(&version, &root, &env).expect("python starts");
        assert_eq!(started.code, Some(0));
        let want = root.to_string_lossy().into_owned();
        let script = format!(
            "import os;print('pinned' if os.environ.get('TMPDIR') == {want:?} else 'leaked')"
        );
        let argv = vec![
            python.into_os_string(),
            OsString::from("-c"),
            OsString::from(script),
        ];
        let temp = spawn(&argv, &root, &env).expect("python reads TMPDIR");
        assert_eq!(temp.code, Some(0));
        assert_eq!(String::from_utf8_lossy(&temp.stdout).trim(), "pinned");
        scratch.close().expect("close");
    }

    #[test]
    fn spawn_runs_absolute_binaries_without_path() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(&cwd, &[], &ambient);
        let ok = spawn(
            &[probe.as_os_str(), OsStr::new("--exit-code=0")],
            &cwd,
            &env,
        )
        .expect("spawn");
        assert_eq!(ok.code, Some(0));
        let fail = spawn(
            &[probe.as_os_str(), OsStr::new("--exit-code=3")],
            &cwd,
            &env,
        )
        .expect("spawn");
        assert_eq!(fail.code, Some(3));
        assert!(spawn(&[OsStr::new("/nonexistent-dx-tool")], &cwd, &env).is_err());
        let empty: Vec<&OsStr> = Vec::new();
        assert!(spawn(&empty, &cwd, &env).is_err());
    }

    #[test]
    fn spawn_enforces_timeout_and_kills_the_child() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(&cwd, &[], &ambient);
        let err = spawn_with_timeout(
            &[probe.as_os_str(), OsStr::new("--sleep-ms=30000")],
            &cwd,
            &env,
            Duration::from_millis(50),
        )
        .expect_err("timeout");
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn spawn_drains_large_output_without_faking_a_timeout() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(&cwd, &[], &ambient);
        let out = spawn_with_timeout(
            &[probe.as_os_str(), OsStr::new("--stdout-bytes=1048576")],
            &cwd,
            &env,
            Duration::from_secs(10),
        )
        .expect("large output");
        assert_eq!(out.code, Some(0));
        assert_eq!(out.stdout, vec![0u8; 1_048_576]);
        assert!(out.stderr.is_empty());
    }

    #[test]
    fn spawn_captures_exact_stdout_and_stderr_bytes() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(&cwd, &[], &ambient);
        let out = spawn_with_timeout(
            &[
                probe.as_os_str(),
                OsStr::new("--stdout-text=out:"),
                OsStr::new("--stdout-bytes=2"),
                OsStr::new("--stderr-text=err:"),
                OsStr::new("--stderr-bytes=1"),
                OsStr::new("--exit-code=5"),
            ],
            &cwd,
            &env,
            Duration::from_secs(10),
        )
        .expect("both streams");
        assert_eq!(out.code, Some(5));
        assert_eq!(out.stdout, b"out:\0\0");
        assert_eq!(out.stderr, b"err:\0");
    }

    #[test]
    fn spawn_rejects_oversized_output() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let ambient: Vec<(OsString, OsString)> = Vec::new();
        let env = hermetic_env(&cwd, &[], &ambient);
        let limit = crate::parsers::MAX_OUTPUT_BYTES;
        let flag = format!("--stdout-bytes={}", limit + 1);
        let err = spawn_with_timeout(
            &[probe.as_os_str(), OsStr::new(&flag)],
            &cwd,
            &env,
            Duration::from_secs(30),
        )
        .expect_err("oversize");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("max size"));
    }
}
