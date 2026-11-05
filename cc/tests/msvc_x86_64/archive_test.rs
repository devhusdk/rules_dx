use dx_deploy_tools::{archive_extract, archive_listing};
use dx_testing::{mkscratch, resolve_runfiles};
use std::process::Command;

#[test]
fn msvc_archive_ships_the_vc_runtime_and_runs_on_a_clean_path() {
    let archive = resolve_runfiles(&std::env::var("ARCHIVE").expect("ARCHIVE"));
    let names: Vec<String> = archive_listing(&archive)
        .expect("list archive")
        .into_iter()
        .map(|(name, _, _)| name.to_lowercase())
        .collect();
    for want in [
        "vcruntime140.dll",
        "msvcp140.dll",
        "hello_msvc.exe",
        "notice",
    ] {
        assert!(
            names.iter().any(|name| name == want),
            "{want} not in {names:?}"
        );
    }
    for bad in ["cl.exe", "link.exe", "vcruntime140.lib", "msvcprt.lib"] {
        assert!(
            !names.iter().any(|name| name.ends_with(bad)),
            "{bad} shipped: {names:?}"
        );
    }

    let scratch = mkscratch("msvc-archive").expect("scratch");
    let dir = scratch.join("out");
    archive_extract(&archive, &dir).expect("extract archive");

    let identity = std::fs::read_to_string(dir.join("IDENTITY.txt")).expect("IDENTITY");
    for prefix in ["runtime: msvc_vcruntime ", "runtime: windows_ucrt "] {
        assert!(
            identity.lines().any(|line| line.starts_with(prefix)),
            "IDENTITY: {identity}"
        );
    }

    let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
    let out = Command::new(dir.join("hello_msvc.exe"))
        .env_clear()
        .env("SystemRoot", &system)
        .env("PATH", format!("{system}\\System32"))
        .current_dir(&scratch)
        .output()
        .expect("run app");
    assert!(out.status.success(), "app failed: {out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "hello from c++"
    );
}
