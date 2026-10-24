use std::path::PathBuf;

fn release_file(suffix: &str) -> PathBuf {
    let files = std::env::var("DX_RELEASE_FILES").expect("DX_RELEASE_FILES must list the release");
    let rel = files
        .split_whitespace()
        .find(|path| path.ends_with(suffix))
        .unwrap_or_else(|| panic!("no release file ends with {suffix}: {files}"));
    dx_testing::resolve_runfiles(rel)
}

#[test]
fn the_aarch64_deployment_archives_the_aarch64_app_and_runs_on_the_host() {
    let program = release_file("/cross_release");
    let header = std::fs::read(&program).expect("read deployment program");
    assert_eq!(&header[..4], b"\x7fELF", "the program is an ELF executable");
    assert_eq!(
        u16::from_le_bytes([header[18], header[19]]),
        0x3e,
        "the deployment program must be x86_64, the execution platform"
    );

    let out = std::env::temp_dir().join(format!("dx-cross-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let run = dx_testing::run(&program, &[out.to_string_lossy().as_ref()], &[])
        .expect("the deployment program runs on the execution platform");
    assert!(run.status.success(), "{}", run.combined());

    let tarball = out.join("cross_release.tar.gz");
    let member = dx_testing::run(
        &PathBuf::from("tar"),
        &["-xzOf", tarball.to_string_lossy().as_ref(), "cross_app"],
        &[],
    )
    .expect("tar runs");
    assert!(member.status.success(), "{}", member.combined());
    assert!(
        member.stdout.contains("target-cpu=aarch64"),
        "the archive must carry the target-platform app: {}",
        member.stdout
    );
    assert_eq!(
        std::fs::read(&tarball).expect("released tarball"),
        std::fs::read(release_file("/cross_release.tar.gz")).expect("built tarball"),
    );
    std::fs::remove_dir_all(&out).expect("cleanup");
}
