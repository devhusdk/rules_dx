use dx_deploy_tools::{archive_extract, archive_listing};
use dx_testing::{image_machine, mkscratch, resolve_runfiles};
use std::process::Command;

#[test]
fn dynamic_libstdcxx_app_ships_its_runtime_and_runs_from_the_archive() {
    let archive = resolve_runfiles(&std::env::var("ARCHIVE").expect("ARCHIVE"));
    let mut names: Vec<String> = archive_listing(&archive)
        .expect("list archive")
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "IDENTITY.txt",
            "NOTICE",
            "hello_cxx_gcc",
            "libgcc_s.so.1",
            "libstdc++.so.6",
            "notices/gcc_linux_libstdcxx",
        ]
    );

    let scratch = mkscratch("gcc-libstdcxx-archive").expect("scratch");
    let dir = scratch.join("out");
    archive_extract(&archive, &dir).expect("extract archive");

    let identity = std::fs::read_to_string(dir.join("IDENTITY.txt")).expect("IDENTITY");
    for want in [
        "runtime: gcc_linux_glibc 2.28 os_supplied",
        "runtime: gcc_linux_libstdcxx 16.2.0 app_local",
    ] {
        assert!(
            identity.lines().any(|line| line == want),
            "IDENTITY: {identity}"
        );
    }

    let app = dir.join("hello_cxx_gcc");
    for file in ["hello_cxx_gcc", "libstdc++.so.6", "libgcc_s.so.1"] {
        let bytes = std::fs::read(dir.join(file)).expect("read file");
        assert_eq!(
            image_machine(&bytes).as_deref(),
            Some("aarch64"),
            "{file} is not aarch64"
        );
    }

    let ldd = Command::new("ldd").arg(&app).output().expect("ldd");
    let ldd = String::from_utf8_lossy(&ldd.stdout).to_string();
    for lib in ["libstdc++.so.6", "libgcc_s.so.1"] {
        let line = ldd
            .lines()
            .find(|l| l.trim_start().starts_with(lib))
            .unwrap_or_else(|| panic!("ldd does not list {lib}:\n{ldd}"));
        let want = format!("=> {}/{lib}", dir.display());
        assert!(line.contains(&want), "{lib} not from the archive:\n{ldd}");
    }
    assert!(!ldd.contains("not found"), "ldd:\n{ldd}");
    assert!(!ldd.contains("dx_gcc"), "ldd:\n{ldd}");

    let out = Command::new(&app)
        .env_clear()
        .current_dir(&scratch)
        .output()
        .expect("run app");
    assert!(out.status.success(), "app failed: {out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "hello from c++"
    );
}
