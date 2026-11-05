use dx_deploy_tools::{archive_extract, archive_listing};
use dx_testing::{image_machine, mkscratch, resolve_runfiles};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn gcc_archive_runs_on_a_clean_environment_without_toolchain_files() {
    let archive = resolve_runfiles(&std::env::var("ARCHIVE").expect("ARCHIVE"));
    let mut names: Vec<String> = archive_listing(&archive)
        .expect("list archive")
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    names.sort();
    assert_eq!(names, ["IDENTITY.txt", "NOTICE", "hello_gcc"]);

    let scratch = mkscratch("gcc-archive").expect("scratch");
    let dir = scratch.join("out");
    archive_extract(&archive, &dir).expect("extract archive");

    let identity = std::fs::read_to_string(dir.join("IDENTITY.txt")).expect("IDENTITY");
    assert!(
        identity
            .lines()
            .any(|line| line == "runtime: gcc_linux_glibc 2.28 os_supplied"),
        "IDENTITY: {identity}"
    );

    let app = dir.join("hello_gcc");
    let meta = std::fs::symlink_metadata(&app).expect("stat app");
    assert!(meta.file_type().is_file(), "app is not a regular file");
    assert_eq!(meta.permissions().mode() & 0o777, 0o755);
    assert!(meta.len() > 1000, "app is {} bytes", meta.len());

    let bytes = std::fs::read(&app).expect("read app");
    assert_eq!(image_machine(&bytes).as_deref(), Some("aarch64"));

    let ldd = Command::new("ldd").arg(&app).output().expect("ldd");
    let ldd = String::from_utf8_lossy(&ldd.stdout).to_string();
    for bad in ["not found", "libstdc++", "libgcc_s", "dx_gcc"] {
        assert!(!ldd.contains(bad), "ldd mentions {bad:?}:\n{ldd}");
    }

    let out = Command::new(&app)
        .env_clear()
        .current_dir(&scratch)
        .output()
        .expect("run app");
    assert!(out.status.success(), "app failed: {out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hello from c");
}
