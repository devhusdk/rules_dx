//! Shared test fixtures: the env lock, scratch dirs and child-process env tests.

use crate::{gzip_compress, tar_archive};
use std::path::Path;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn scratch_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("scratch")
}

pub(crate) fn with_env<T>(vars: &[(&str, Option<&str>)], body: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut saved: Vec<(&str, Option<String>)> = Vec::new();
    for (key, value) in vars {
        saved.push((*key, std::env::var(key).ok()));
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    let out = body();
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    out
}

const ENV_CHILD: &str = "DX_DEPLOY_RULES_ENV_CHILD";

pub(crate) fn env_test_in_child(test: &str) -> bool {
    if std::env::var_os(ENV_CHILD).is_some() {
        return false;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut before: Vec<_> = std::env::vars_os().collect();
    before.sort();
    let run = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([test, "--exact"])
        .env(ENV_CHILD, "1")
        .output()
        .expect("spawn env child");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stdout.contains("running 1 test"),
        "child did not run exactly one test: {stdout}{stderr}"
    );
    assert!(run.status.success(), "{stdout}{stderr}");
    let mut after: Vec<_> = std::env::vars_os().collect();
    after.sort();
    assert_eq!(after, before, "parent environment changed");
    true
}

pub(crate) fn write_crate_file(root: &Path, rel: &str, body: &str) -> std::path::PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, body).expect("write");
    path
}

pub(crate) fn write_nupkg(path: &Path, entries: &[(&str, &str)]) {
    use std::io::Write as _;
    let file = std::fs::File::create(path).expect("create nupkg");
    let mut writer = zip::ZipWriter::new(file);
    for (name, body) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(body.as_bytes()).expect("write entry");
    }
    writer.finish().expect("finish nupkg");
}

pub(crate) fn nuspec(id: &str, version: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><package xmlns=\"http://schemas.microsoft.com/packaging/2013/05/nuspec.xsd\"><metadata><id>{id}</id><version>{version}</version><authors>a</authors></metadata></package>"
    )
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[cfg(unix)]
pub(crate) fn mode_bits(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

pub(crate) fn oci_test_layer(gzip: bool) -> Vec<u8> {
    let tar =
        tar_archive([("hello", b"hello world\n".as_slice(), 0o644)].into_iter()).expect("tar");
    if gzip {
        gzip_compress(&tar).expect("gzip")
    } else {
        tar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_child_panic_leaves_parent_environment() {
        if std::env::var_os(ENV_CHILD).is_some() {
            with_env(&[("PATH", Some("poisoned"))], || {
                panic!("child panics inside a mutated environment")
            });
        }
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut before: Vec<_> = std::env::vars_os().collect();
        before.sort();
        let run = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "test_support::tests::env_child_panic_leaves_parent_environment",
                "--exact",
            ])
            .env(ENV_CHILD, "1")
            .output()
            .expect("spawn panicking child");
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(
            stdout.contains("child panics inside a mutated environment"),
            "child must panic inside with_env: {stdout}{stderr}"
        );
        assert!(!run.status.success(), "{stdout}{stderr}");
        let mut after: Vec<_> = std::env::vars_os().collect();
        after.sort();
        assert_eq!(after, before, "parent environment changed");
    }
}
