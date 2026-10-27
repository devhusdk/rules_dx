//! Staging helpers shared by every publisher: runfiles, outdirs, private dirs.

use std::io::{self, Write};
use std::path::Path;

pub fn bin_usage(prog: &str, usage: &str) -> i32 {
    eprintln!("usage: {prog} {usage}");
    1
}

pub fn bin_cannot(prog: &str, action: &str, target: &Path, error: impl std::fmt::Display) -> i32 {
    eprintln!("{prog}: cannot {action} {}: {error}", target.display());
    1
}

/// Copies one file and verifies byte identity.
pub fn copy_verified(src: &Path, dest: &Path) -> io::Result<String> {
    dx_atomic_fs::copy_atomic(src, dest)?;
    let want = dx_digest::sha256_file_hex(src)?;
    let got = dx_digest::sha256_file_hex(dest)?;
    if want != got {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "byte mismatch for {} (expected sha256 {want}, got {got})",
                src.display(),
            ),
        ));
    }
    Ok(want)
}

/// Resolves one runfiles rlocation through the Bazel runfiles library.
pub fn resolve_runfile(rloc: &str) -> io::Result<std::path::PathBuf> {
    if rloc.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty runfiles rlocation",
        ));
    }
    let runfiles = runfiles::Runfiles::create().map_err(|error| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("cannot open runfiles for '{rloc}': {error}"),
        )
    })?;
    match runfiles.rlocation_from(rloc, "") {
        Some(path) if path.exists() => Ok(path),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("runfile not found for '{rloc}'"),
        )),
    }
}

/// Returns the deploy output directory.
pub fn deploy_outdir(argv: &[String]) -> std::path::PathBuf {
    if argv.len() >= 2 {
        return absolute_from_invocation(Path::new(&argv[1]));
    }
    std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

/// Resolves a relative path against the directory the user ran bazel from.
fn absolute_from_invocation(path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = std::env::var_os("BUILD_WORKING_DIRECTORY")
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join(path)
}

/// Serializes one JSON document pretty-printed with a trailing newline.
pub(crate) fn render_json(value: &serde_json::Value) -> std::io::Result<String> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}

/// Writes one owner-only file.
pub fn write_secure_file(
    directory: &Path,
    name: &str,
    content: &str,
) -> io::Result<std::path::PathBuf> {
    let path = directory.join(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(content.as_bytes())?;
    Ok(path)
}

pub(crate) fn package_relative_paths(
    crate_files: &[std::path::PathBuf],
    manifest_name: &str,
    label: &str,
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    let bad = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    if crate_files.is_empty() {
        return Err(bad(format!("{label}: need at least one source file")));
    }
    let mut manifests: Vec<&std::path::PathBuf> = crate_files
        .iter()
        .filter(|src| src.file_name().and_then(|name| name.to_str()) == Some(manifest_name))
        .collect();
    manifests.sort_by_key(|src| src.components().count());
    let Some(first) = manifests.first() else {
        return Err(bad(format!(
            "{label}: no {manifest_name} among the source files"
        )));
    };
    if let Some(second) = manifests.get(1) {
        if second.components().count() == first.components().count() {
            return Err(bad(format!(
                "{label}: ambiguous package root, found '{}' and '{}'",
                first.display(),
                second.display()
            )));
        }
    }
    let root = first.parent().unwrap_or_else(|| Path::new(""));
    let mut mapped: Vec<(std::path::PathBuf, String)> = Vec::new();
    for src in crate_files {
        let rel = src.strip_prefix(root).map_err(|_| {
            bad(format!(
                "{label}: '{}' is outside the package root '{}'",
                src.display(),
                root.display()
            ))
        })?;
        let mut parts: Vec<&str> = Vec::new();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(part) => match part.to_str() {
                    Some(part) if !part.is_empty() => parts.push(part),
                    _ => return Err(bad(format!("{label}: non-UTF-8 path '{}'", src.display()))),
                },
                _ => {
                    return Err(bad(format!(
                        "{label}: unsafe package path '{}'",
                        rel.display()
                    )))
                }
            }
        }
        if parts.is_empty() {
            return Err(bad(format!(
                "{label}: empty package path for '{}'",
                src.display()
            )));
        }
        mapped.push((src.clone(), parts.join("/")));
    }
    mapped.sort_by(|left, right| left.1.cmp(&right.1));
    for pair in mapped.windows(2) {
        let (left, right) = (&pair[0].1, &pair[1].1);
        if left == right {
            return Err(bad(format!("{label}: path collision on '{left}'")));
        }
        if right.starts_with(&format!("{left}/")) {
            return Err(bad(format!(
                "{label}: '{left}' is both a file and a directory"
            )));
        }
    }
    Ok(mapped)
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

pub(crate) fn tempfile_stage_dir(prefix: &str) -> io::Result<std::path::PathBuf> {
    for counter in 0..100 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("{prefix}{}-{nanos}-{counter}", std::process::id()));
        match create_private_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot stage tempdir",
    ))
}

/// Owner-only temp directory that is removed when dropped, including on panic.
pub(crate) struct PrivateStageDir(std::path::PathBuf);

impl PrivateStageDir {
    pub(crate) fn new(prefix: &str) -> io::Result<Self> {
        tempfile_stage_dir(prefix).map(Self)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for PrivateStageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn which_on_path(tool: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    let sep = ';';
    #[cfg(not(windows))]
    let sep = ':';
    for dir in path.split(sep) {
        if dir.is_empty() {
            continue;
        }
        let candidate = std::path::Path::new(dir).join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        for suffix in dx_path::host::EXECUTABLE_SUFFIXES {
            let exe = std::path::Path::new(dir).join(format!("{tool}{suffix}"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn bin_shims_share_usage_and_failure_exit() {
        assert_eq!(bin_usage("dx_archiver", "<src> <dst>"), 1);
        assert_eq!(
            bin_cannot(
                "dx_archiver",
                "write",
                Path::new("/tmp/out.tar.gz"),
                "read-only filesystem"
            ),
            1
        );
    }

    #[test]
    fn render_json_escapes_control_and_markup() {
        let value = serde_json::json!({"k": "a\"b\\c\nd\u{1}<&>"});
        let text = render_json(&value).expect("renders");
        assert_eq!(text, "{\n  \"k\": \"a\\\"b\\\\c\\nd\\u0001<&>\"\n}\n");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).expect("parses"),
            value
        );
        assert_eq!(
            quick_xml::escape::escape("<a href=\"x\">&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;&lt;/a&gt;"
        );
    }

    #[test]
    fn stage_helpers_create_and_report_paths() {
        let dir = tempfile_stage_dir("dx-deploy-stage-").expect("stage dir");
        assert!(dir.is_dir());
        assert!(dir.to_string_lossy().contains("dx-deploy-stage-"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&dir).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "stage dir must stay owner-only");
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
        assert!(tempfile_stage_dir("dx-deploy-stage-").is_ok());
    }

    #[test]
    fn copy_verified_reports_source_digest() {
        let scratch = scratch_dir();
        let src = scratch.path().join("src.bin");
        std::fs::write(&src, b"payload").expect("write src");
        let dest = scratch.path().join("dest.bin");
        std::fs::write(&dest, b"stale-bytes").expect("write dest");
        let digest = copy_verified(&src, &dest).expect("copy");
        assert_eq!(digest, dx_digest::sha256_file_hex(&src).expect("hex"));
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload");
    }

    #[test]
    #[cfg(unix)]
    fn copy_verified_restages_a_read_only_source() {
        let scratch = scratch_dir();
        for (src_mode, want_mode) in [(0o555, 0o755), (0o444, 0o644)] {
            let src = scratch.path().join(format!("src-{src_mode:o}.bin"));
            std::fs::write(&src, b"payload").expect("write src");
            set_mode(&src, src_mode);
            let dest = scratch.path().join(format!("dest-{src_mode:o}.bin"));
            let first = copy_verified(&src, &dest).expect("first copy");
            let second = copy_verified(&src, &dest).expect("second copy");
            assert_eq!(first, second);
            assert_eq!(std::fs::read(&dest).expect("read dest"), b"payload");
            assert_eq!(mode_bits(&dest), want_mode);
        }
    }

    #[test]
    fn which_on_path_finds_present_and_missing_tools() {
        if env_test_in_child("stage::tests::which_on_path_finds_present_and_missing_tools") {
            return;
        }
        with_env(&[("PATH", Some(""))], || {
            assert!(which_on_path("definitely-not-a-real-tool-xyz").is_none());
        });
        let dir = scratch_dir();
        let tool = dir.path().join("dx_fake_tool");
        std::fs::write(&tool, b"#!/bin/sh\n").expect("write tool");
        let path = format!("{}{}", dir.path().to_string_lossy(), {
            if cfg!(windows) {
                ";"
            } else {
                ":"
            }
        });
        with_env(&[("PATH", Some(&path))], || {
            assert_eq!(
                which_on_path("dx_fake_tool").as_deref(),
                Some(tool.as_path())
            );
            assert!(which_on_path("dx_missing_tool").is_none());
        });
    }

    #[test]
    fn deploy_outdir_prefers_arg_then_env_then_cwd() {
        if env_test_in_child("stage::tests::deploy_outdir_prefers_arg_then_env_then_cwd") {
            return;
        }
        let scratch = scratch_dir();
        let explicit = scratch.path().join("explicit");
        let argv = vec!["prog".to_owned(), explicit.to_string_lossy().into_owned()];
        assert_eq!(deploy_outdir(&argv), explicit);
        with_env(&[("BUILD_WORKSPACE_DIRECTORY", None)], || {
            let fallback = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&["prog".to_owned()]), fallback);
        });
        let from_env = scratch.path().join("from-env");
        with_env(
            &[(
                "BUILD_WORKSPACE_DIRECTORY",
                Some(from_env.to_string_lossy().as_ref()),
            )],
            || {
                assert_eq!(deploy_outdir(&["prog".to_owned()]), from_env);
            },
        );
        let relative = vec!["prog".to_owned(), "dist".to_owned()];
        let invocation = scratch.path().join("inv");
        with_env(
            &[(
                "BUILD_WORKING_DIRECTORY",
                Some(invocation.to_string_lossy().as_ref()),
            )],
            || {
                assert_eq!(deploy_outdir(&relative), invocation.join("dist"));
                assert_eq!(deploy_outdir(&argv), explicit);
            },
        );
        with_env(&[("BUILD_WORKING_DIRECTORY", None)], || {
            let cwd = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&relative), cwd.join("dist"));
        });
        with_env(&[("BUILD_WORKING_DIRECTORY", Some(""))], || {
            let cwd = std::env::current_dir().expect("cwd");
            assert_eq!(deploy_outdir(&relative), cwd.join("dist"));
        });
    }

    #[test]
    fn resolve_runfile_reads_dir_and_manifest() {
        if env_test_in_child("stage::tests::resolve_runfile_reads_dir_and_manifest") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let payload = runfiles.join("demo/app.bin");
        std::fs::create_dir_all(payload.parent().expect("parent")).expect("nested");
        std::fs::write(&payload, b"payload").expect("write payload");
        let manifest = scratch.path().join("manifest.txt");
        std::fs::write(
            &manifest,
            format!(
                "demo/app.bin {}\nother/missing.bin /nope\nmalformed\n",
                payload.display()
            ),
        )
        .expect("write manifest");

        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                assert_eq!(resolve_runfile("demo/app.bin").expect("from dir"), payload);
            },
        );
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(scratch.path().to_string_lossy().as_ref()),
                ),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                assert_eq!(
                    resolve_runfile("demo/app.bin").expect("from manifest"),
                    payload
                );
            },
        );
        with_env(
            &[("RUNFILES_DIR", None), ("RUNFILES_MANIFEST_FILE", None)],
            || {
                let empty = resolve_runfile("").expect_err("empty rloc");
                assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
                let missing = resolve_runfile("demo/absent-xyz").expect_err("missing rloc");
                assert_eq!(missing.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn resolve_runfile_reads_manifest_values_with_spaces() {
        if env_test_in_child("stage::tests::resolve_runfile_reads_manifest_values_with_spaces") {
            return;
        }
        let scratch = scratch_dir();
        let payload = scratch.path().join("real dir").join("app.bin");
        std::fs::create_dir_all(payload.parent().expect("parent")).expect("nested");
        std::fs::write(&payload, b"spaced").expect("write payload");
        let manifest = scratch.path().join("spaced.manifest");
        std::fs::write(
            &manifest,
            format!("_main/pkg/app.bin {}\n", payload.display()),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let found = resolve_runfile("_main/pkg/app.bin").expect("spaced value");
                assert_eq!(std::fs::read(found).expect("bytes"), b"spaced");
                let error = resolve_runfile("_main/pkg/absent.bin").expect_err("missing key");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert!(error.to_string().contains("_main/pkg/absent.bin"));
            },
        );
    }

    #[test]
    fn resolve_runfile_reads_escaped_manifest_entries() {
        if env_test_in_child("stage::tests::resolve_runfile_reads_escaped_manifest_entries") {
            return;
        }
        let scratch = scratch_dir();
        let spaced = scratch.path().join("real dir").join("a b.bin");
        std::fs::create_dir_all(spaced.parent().expect("parent")).expect("nested");
        std::fs::write(&spaced, b"spaced").expect("write spaced");
        let slashed = scratch.path().join("slash.bin");
        std::fs::write(&slashed, b"slashed").expect("write slashed");
        let manifest = scratch.path().join("escaped.manifest");
        std::fs::write(
            &manifest,
            format!(
                " _main/pkg/a\\sb.bin {}\n _main/pkg/c\\bd\\ne {}\n",
                spaced.display(),
                slashed.display()
            ),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let found = resolve_runfile("_main/pkg/a b.bin").expect("escaped space");
                assert_eq!(std::fs::read(found).expect("bytes"), b"spaced");
                let found = resolve_runfile("_main/pkg/c\\d\ne").expect("escaped slash");
                assert_eq!(std::fs::read(found).expect("bytes"), b"slashed");
                let error = resolve_runfile("_main/pkg/a\\sb.bin").expect_err("raw escape");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn resolve_runfile_maps_a_repository_name_through_the_manifest() {
        if env_test_in_child("stage::tests::resolve_runfile_maps_a_repository_name_through_the_manifest") {
            return;
        }
        let scratch = scratch_dir();
        let payload = scratch.path().join("dep.bin");
        std::fs::write(&payload, b"external").expect("write payload");
        let mapping = scratch.path().join("repo_mapping");
        std::fs::write(&mapping, ",dep,dep+1.0.0+\n").expect("write mapping");
        let manifest = scratch.path().join("mapped.manifest");
        std::fs::write(
            &manifest,
            format!(
                "_repo_mapping {}\ndep+1.0.0+/pkg/app.bin {}\n",
                mapping.display(),
                payload.display()
            ),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let found = resolve_runfile("dep/pkg/app.bin").expect("mapped name");
                assert_eq!(std::fs::read(found).expect("bytes"), b"external");
                let error = resolve_runfile("other/pkg/app.bin").expect_err("unmapped name");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn secure_file_is_owner_only_and_refuses_existing() {
        let scratch = scratch_dir();
        let path = write_secure_file(scratch.path(), "secret", "one").expect("secure");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "one");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "secret must stay owner-only");
        }
        let error = write_secure_file(scratch.path(), "secret", "two").expect_err("existing");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "one");
        #[cfg(unix)]
        {
            let target = scratch.path().join("target");
            std::fs::write(&target, "keep").expect("write target");
            std::os::unix::fs::symlink(&target, scratch.path().join("link")).expect("symlink");
            assert!(write_secure_file(scratch.path(), "link", "leak").is_err());
            assert_eq!(
                std::fs::read_to_string(&target).expect("read target"),
                "keep"
            );
        }
    }

    #[test]
    fn private_dir_refuses_existing_dir() {
        let scratch = scratch_dir();
        let error = create_private_dir(scratch.path()).expect_err("existing");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        let child = scratch.path().join("child");
        create_private_dir(&child).expect("child");
        assert!(child.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&child)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "private dir must stay owner-only");
        }
    }

    #[test]
    fn private_stage_dir_is_removed_on_drop() {
        let dir = PrivateStageDir::new("dx-private-stage-test-").expect("dir");
        let path = dir.path().to_path_buf();
        write_secure_file(&path, "secret", "x").expect("write");
        assert!(path.join("secret").exists());
        drop(dir);
        assert!(!path.exists());
    }
}
