use dx_atomic_fs::{classify, materialize_file, write_atomic, EntryKind, LinkPolicy, Mechanism};
use normpath::BasePathBuf;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

pub const PINNED_TEMP_KEYS: &[&str] = &["TMPDIR", "TEMP", "TMP"];

pub const DETERMINISTIC_DEFAULTS: &[(&str, &str)] = &[("LANG", "C.UTF-8"), ("TZ", "UTC")];

pub const WINDOWS_RUNTIME_KEYS: &[&str] = &[
    "COMSPEC",
    "OS",
    "PATHEXT",
    "SYSTEMDRIVE",
    "SYSTEMROOT",
    "WINDIR",
];

fn fold_key(key: &OsStr, windows: bool) -> Vec<u8> {
    let bytes = key.as_encoded_bytes();
    if !windows {
        return bytes.to_vec();
    }
    bytes
        .iter()
        .map(|byte| {
            if byte.is_ascii_lowercase() {
                byte.to_ascii_uppercase()
            } else {
                *byte
            }
        })
        .collect()
}

fn is_folded_member(key: &OsStr, members: &[&str], windows: bool) -> bool {
    let folded = fold_key(key, windows);
    members.iter().any(|member| {
        if windows {
            folded
                == member
                    .bytes()
                    .map(|byte| byte.to_ascii_uppercase())
                    .collect::<Vec<u8>>()
        } else {
            folded == member.as_bytes()
        }
    })
}

fn find_slot(env: &[(OsString, OsString)], key: &OsStr, windows: bool) -> Option<usize> {
    let folded = fold_key(key, windows);
    env.iter()
        .position(|(have, _)| fold_key(have, windows) == folded)
}

pub fn hermetic_env_with_ambient(
    tmpdir: &Path,
    extra: &[(&str, &str)],
    ambient: &dyn Fn(&OsStr) -> Option<OsString>,
    windows: bool,
) -> Vec<(OsString, OsString)> {
    let tmp = tmpdir.as_os_str().to_os_string();
    let mut env: Vec<(OsString, OsString)> = Vec::new();
    env.push((OsString::from("TMPDIR"), tmp.clone()));
    if windows {
        env.push((OsString::from("TEMP"), tmp.clone()));
        env.push((OsString::from("TMP"), tmp));
    }
    for (key, value) in DETERMINISTIC_DEFAULTS {
        env.push((OsString::from(*key), OsString::from(*value)));
    }
    if windows {
        for key in WINDOWS_RUNTIME_KEYS {
            let probe = OsString::from(*key);
            if let Some(value) = ambient(probe.as_os_str()) {
                if find_slot(&env, probe.as_os_str(), true).is_none() {
                    env.push((probe, value));
                }
            }
        }
    }
    for (key, value) in extra {
        let key_os = OsString::from(*key);
        if is_folded_member(key_os.as_os_str(), PINNED_TEMP_KEYS, windows) {
            continue;
        }
        if is_folded_member(
            key_os.as_os_str(),
            &DETERMINISTIC_DEFAULTS
                .iter()
                .map(|(key, _)| *key)
                .collect::<Vec<&str>>(),
            windows,
        ) {
            continue;
        }
        if windows && is_folded_member(key_os.as_os_str(), WINDOWS_RUNTIME_KEYS, windows) {
            continue;
        }
        if let Some(slot) = find_slot(&env, key_os.as_os_str(), windows) {
            env[slot].1 = OsString::from(*value);
        } else {
            env.push((key_os, OsString::from(*value)));
        }
    }
    env
}

pub fn hermetic_env(tmpdir: &Path, extra: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    hermetic_env_with_ambient(tmpdir, extra, &|key| std::env::var_os(key), cfg!(windows))
}

pub const SPAWN_TIMEOUT: Duration = Duration::from_secs(120);

fn check_child_output_size(stdout: &[u8], stderr: &[u8]) -> io::Result<()> {
    let limit = crate::parsers::MAX_OUTPUT_BYTES;
    if stdout.len() > limit || stderr.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("tool output exceeds max size {limit} bytes"),
        ));
    }
    Ok(())
}

fn drain_pipe<R: Read + Send + 'static>(mut pipe: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let limit = crate::parsers::MAX_OUTPUT_BYTES;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let room = limit.saturating_add(1).saturating_sub(buf.len());
                    let take = n.min(room);
                    buf.extend_from_slice(&chunk[..take]);
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        buf
    })
}

fn join_drain(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> io::Result<Vec<u8>> {
    match handle {
        None => Ok(Vec::new()),
        Some(handle) => handle
            .join()
            .map_err(|_| io::Error::other("tool output reader thread panicked")),
    }
}

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
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout_reader = child.stdout.take().map(drain_pipe);
    let mut stderr_reader = child.stderr.take().map(drain_pipe);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = join_drain(stdout_reader.take());
                    let _ = join_drain(stderr_reader.take());
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("tool timed out after {}s", timeout.as_secs()),
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let stdout = join_drain(stdout_reader)?;
    let stderr = join_drain(stderr_reader)?;
    check_child_output_size(&stdout, &stderr)?;
    Ok(ChildOutput {
        code: status.code(),
        stdout,
        stderr,
    })
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

    fn ambient_map(vars: &[(&str, &str)]) -> impl Fn(&OsStr) -> Option<OsString> {
        let owned: Vec<(OsString, OsString)> = vars
            .iter()
            .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
            .collect();
        move |key: &OsStr| {
            owned
                .iter()
                .find(|(have, _)| have == key)
                .map(|(_, value)| value.clone())
        }
    }

    fn ambient_map_folded(vars: &[(&str, &str)]) -> impl Fn(&OsStr) -> Option<OsString> {
        let owned: Vec<(OsString, OsString)> = vars
            .iter()
            .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
            .collect();
        move |key: &OsStr| {
            let wanted: Vec<u8> = key
                .as_encoded_bytes()
                .iter()
                .map(|byte| byte.to_ascii_uppercase())
                .collect();
            owned
                .iter()
                .find(|(have, _)| {
                    have.as_encoded_bytes()
                        .iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect::<Vec<u8>>()
                        == wanted
                })
                .map(|(_, value)| value.clone())
        }
    }

    fn get(env: &[(OsString, OsString)], key: &str) -> Option<String> {
        env.iter()
            .find(|(have, _)| have == key)
            .map(|(_, value)| value.clone().into_string().expect("test values stay utf-8"))
    }

    fn count_key(env: &[(OsString, OsString)], key: &str, windows: bool) -> usize {
        let wanted = OsString::from(key);
        env.iter()
            .filter(|(have, _)| {
                if windows {
                    have.as_encoded_bytes()
                        .iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect::<Vec<u8>>()
                        == wanted
                            .as_encoded_bytes()
                            .iter()
                            .map(|byte| byte.to_ascii_uppercase())
                            .collect::<Vec<u8>>()
                } else {
                    have == &wanted
                }
            })
            .count()
    }

    #[test]
    fn hermetic_env_has_no_path() {
        let env = hermetic_env(Path::new("/tmp/dx"), &[("LD_LIBRARY_PATH", "/lib")]);
        assert_eq!(
            env,
            vec![
                (OsString::from("TMPDIR"), OsString::from("/tmp/dx")),
                (OsString::from("LANG"), OsString::from("C.UTF-8")),
                (OsString::from("TZ"), OsString::from("UTC")),
                (OsString::from("LD_LIBRARY_PATH"), OsString::from("/lib")),
            ]
        );
    }

    #[test]
    fn hermetic_env_tmpdir_is_never_shadowed() {
        let env = hermetic_env(
            Path::new("/tmp/dx"),
            &[("TMPDIR", "/evil"), ("LD_LIBRARY_PATH", "/lib")],
        );
        assert_eq!(count_key(&env, "TMPDIR", false), 1, "exactly one TMPDIR");
        assert_eq!(
            env[0],
            (OsString::from("TMPDIR"), OsString::from("/tmp/dx"))
        );
        assert!(env.contains(&(OsString::from("LD_LIBRARY_PATH"), OsString::from("/lib"))));
    }

    #[test]
    fn hermetic_env_pins_locale_and_time() {
        let env = hermetic_env(
            Path::new("/tmp/dx"),
            &[
                ("LANG", "de_DE.UTF-8"),
                ("TZ", "Europe/Berlin"),
                ("LC_ALL", "de_DE.UTF-8"),
            ],
        );
        assert_eq!(count_key(&env, "LANG", false), 1, "exactly one LANG");
        assert_eq!(count_key(&env, "TZ", false), 1, "exactly one TZ");
        assert!(env.contains(&(OsString::from("LANG"), OsString::from("C.UTF-8"))));
        assert!(env.contains(&(OsString::from("TZ"), OsString::from("UTC"))));
        assert!(env.contains(&(OsString::from("LC_ALL"), OsString::from("de_DE.UTF-8"))));
    }

    #[test]
    fn ambient_path_and_config_never_leak() {
        let ambient = ambient_map(&[
            ("PATH", "/usr/bin:/bin"),
            ("Path", "/evil"),
            ("USERPROFILE", "C:\\Users\\evil"),
            ("APPDATA", "C:\\Users\\evil\\AppData"),
            ("LOCALAPPDATA", "C:\\Users\\evil\\Local"),
            ("HOMEDRIVE", "C:"),
            ("HOMEPATH", "\\Users\\evil"),
            ("PROGRAMDATA", "C:\\ProgramData"),
            ("NPM_CONFIG_PREFIX", "/evil"),
            ("PYTHONPATH", "/evil"),
            ("TEMP", "/evil"),
            ("TMP", "/evil"),
            ("TMPDIR", "/evil"),
        ]);
        let env = hermetic_env_with_ambient(Path::new("/tmp/dx"), &[], &ambient, false);
        assert_eq!(get(&env, "TMPDIR"), Some("/tmp/dx".to_owned()));
        assert!(get(&env, "PATH").is_none(), "ambient PATH stays out");
        assert!(get(&env, "Path").is_none(), "ambient Path stays out");
        assert!(get(&env, "USERPROFILE").is_none());
        assert!(get(&env, "APPDATA").is_none());
        assert!(get(&env, "PROGRAMDATA").is_none());
        assert!(get(&env, "NPM_CONFIG_PREFIX").is_none());
        assert!(get(&env, "PYTHONPATH").is_none());
        assert!(get(&env, "TEMP").is_none(), "unix keeps no TEMP");
        assert!(get(&env, "TMP").is_none(), "unix keeps no TMP");
    }

    #[test]
    fn windows_temp_is_pinned_regardless_of_casing() {
        let ambient = ambient_map_folded(&[
            ("TEMP", "C:\\Users\\evil\\Temp"),
            ("TMP", "C:\\Users\\evil\\Temp"),
            ("TMPDIR", "/evil"),
            ("USERPROFILE", "C:\\Users\\evil"),
            ("APPDATA", "C:\\Users\\evil\\AppData"),
        ]);
        let env = hermetic_env_with_ambient(
            Path::new("C:\\scratch"),
            &[("tmpdir", "/evil"), ("Tmp", "/evil"), ("TEMP", "/evil")],
            &ambient,
            true,
        );
        assert_eq!(count_key(&env, "TMPDIR", true), 1);
        assert_eq!(count_key(&env, "TEMP", true), 1);
        assert_eq!(count_key(&env, "TMP", true), 1);
        for key in ["TMPDIR", "TEMP", "TMP"] {
            let values: Vec<String> = env
                .iter()
                .filter(|(have, _)| {
                    have.as_encoded_bytes()
                        .iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect::<Vec<u8>>()
                        == OsString::from(key)
                            .as_encoded_bytes()
                            .iter()
                            .map(|byte| byte.to_ascii_uppercase())
                            .collect::<Vec<u8>>()
                })
                .map(|(_, value)| value.clone().into_string().expect("utf-8 temp"))
                .collect();
            assert_eq!(values, vec!["C:\\scratch".to_owned()], "{key} stays pinned");
        }
        assert!(get(&env, "USERPROFILE").is_none());
        assert!(get(&env, "APPDATA").is_none());
    }

    #[test]
    fn windows_runtime_comes_only_from_ambient() {
        let ambient = ambient_map_folded(&[
            ("SystemRoot", "C:\\Windows"),
            ("SYSTEMDRIVE", "C:"),
            ("OS", "Windows_NT"),
            ("PATHEXT", ".EXE;.BAT"),
            ("WINDIR", "C:\\Windows"),
            ("COMSPEC", "C:\\Windows\\system32\\cmd.exe"),
            ("PATH", "C:\\Windows\\system32"),
            ("USERPROFILE", "C:\\Users\\evil"),
            ("PROGRAMDATA", "C:\\ProgramData"),
        ]);
        let env = hermetic_env_with_ambient(Path::new("C:\\scratch"), &[], &ambient, true);
        assert_eq!(
            get(&env, "SystemRoot").or(get(&env, "SYSTEMROOT")),
            Some("C:\\Windows".to_owned())
        );
        assert!(get(&env, "PATH").is_none(), "ambient PATH stays out");
        assert!(get(&env, "USERPROFILE").is_none());
        assert!(get(&env, "PROGRAMDATA").is_none());
        assert!(get(&env, "PROGRAMDATA").is_none());
        let folded: Vec<String> = env
            .iter()
            .map(|(key, _)| {
                String::from_utf8(
                    key.as_encoded_bytes()
                        .iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect(),
                )
                .expect("ascii keys")
            })
            .collect();
        assert_eq!(
            folded.iter().filter(|key| *key == "PROGRAMDATA").count(),
            0,
            "no duplicated PROGRAMDATA"
        );
    }

    #[test]
    fn windows_extra_cannot_shadow_reserved_keys() {
        let ambient = ambient_map_folded(&[("SystemRoot", "C:\\Windows")]);
        let env = hermetic_env_with_ambient(
            Path::new("C:\\scratch"),
            &[
                ("systemroot", "C:\\Evil"),
                ("SystemDrive", "X:"),
                ("lang", "de"),
                ("Path", "C:\\Evil"),
            ],
            &ambient,
            true,
        );
        let root = get(&env, "SystemRoot")
            .or(get(&env, "SYSTEMROOT"))
            .expect("runtime kept");
        assert_eq!(root, "C:\\Windows");
        assert!(env.contains(&(OsString::from("LANG"), OsString::from("C.UTF-8"))));
        assert_eq!(
            env.iter()
                .filter(|(key, _)| get(&env, &key.to_string_lossy()).is_some())
                .count(),
            env.len(),
            "every entry is reachable"
        );
        assert_eq!(count_key(&env, "SYSTEMROOT", true), 1);
        assert_eq!(count_key(&env, "LANG", true), 1);
        assert_eq!(
            get(&env, "Path"),
            Some("C:\\Evil".to_owned()),
            "PATH stays explicit"
        );
    }

    #[test]
    fn conflicting_extra_casing_has_one_outcome() {
        let none = ambient_map(&[]);
        let windows = hermetic_env_with_ambient(
            Path::new("C:\\scratch"),
            &[("PATH", "C:\\a"), ("Path", "C:\\b"), ("pAtH", "C:\\c")],
            &none,
            true,
        );
        assert_eq!(count_key(&windows, "PATH", true), 1);
        let value = windows
            .iter()
            .find(|(key, _)| {
                key.as_encoded_bytes()
                    .iter()
                    .map(|byte| byte.to_ascii_uppercase())
                    .collect::<Vec<u8>>()
                    == b"PATH".to_vec()
            })
            .map(|(_, value)| value.clone().into_string().expect("utf-8"))
            .expect("one PATH");
        assert_eq!(value, "C:\\c", "last extra wins deterministically");
        let unix = hermetic_env_with_ambient(
            Path::new("/tmp/dx"),
            &[("PATH", "/a"), ("Path", "/b")],
            &none,
            false,
        );
        assert_eq!(count_key(&unix, "PATH", false), 1);
        assert_eq!(count_key(&unix, "Path", false), 1);
        assert_eq!(get(&unix, "PATH"), Some("/a".to_owned()));
        assert_eq!(get(&unix, "Path"), Some("/b".to_owned()));
    }

    #[test]
    fn spawn_delivers_pinned_temp_and_blocks_config_discovery() {
        let scratch = Scratch::create(&std::env::temp_dir()).expect("scratch");
        let root = scratch.root().to_owned();
        let ambient = ambient_map(&[
            ("TEMP", "/evil"),
            ("TMP", "/evil"),
            ("TMPDIR", "/evil"),
            ("USERPROFILE", "C:\\Users\\evil"),
            ("APPDATA", "/evil"),
            ("PATH", "/evil"),
        ]);
        let env = hermetic_env_with_ambient(&root, &[], &ambient, false);
        let probe = dx_testing::process_probe();
        let pinned = root.to_string_lossy().into_owned();
        let flag = format!("--require-env=TMPDIR={pinned}");
        let out = spawn(
            &[
                probe.as_os_str(),
                OsStr::new(&flag),
                OsStr::new("--forbid-env=USERPROFILE"),
                OsStr::new("--forbid-env=APPDATA"),
            ],
            &root,
            &env,
        )
        .expect("probe sees production env");
        assert_eq!(out.code, Some(0));
        let printed = spawn(
            &[
                probe.as_os_str(),
                OsStr::new("--print-env=TMPDIR"),
                OsStr::new("--print-env=TEMP"),
            ],
            &root,
            &env,
        )
        .expect("probe prints temp");
        assert_eq!(printed.code, Some(0));
        let text = String::from_utf8(printed.stdout.clone()).expect("probe prints utf-8");
        let mut lines = text.split("\n");
        assert_eq!(
            lines.next(),
            Some(pinned.as_str()),
            "delivered TMPDIR is the scratch root"
        );
        assert_eq!(lines.next(), Some("<unset>"), "unix child sees no TEMP");
        scratch.close().expect("close");
    }

    #[test]
    fn spawn_runs_absolute_binaries_without_path() {
        let probe = dx_testing::process_probe();
        let cwd = std::env::temp_dir();
        let env = hermetic_env(&cwd, &[]);
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
        let env = hermetic_env(&cwd, &[]);
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
        let env = hermetic_env(&cwd, &[]);
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
        let env = hermetic_env(&cwd, &[]);
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
        let env = hermetic_env(&cwd, &[]);
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
