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
    let config = std::path::absolute(release_file("/cross_release_config.deploy.json"))
        .expect("absolute config path");
    let run = dx_testing::run(
        &program,
        &[out.to_string_lossy().as_ref()],
        &[("DX_DEPLOY_CONFIG", config.to_string_lossy().as_ref())],
    )
    .expect("the deployment program runs on the execution platform");
    assert!(run.status.success(), "{}", run.combined());

    let tarball = out.join("cross_release.tar.gz");
    let extracted = out.join("extracted");
    std::fs::create_dir_all(&extracted).expect("extract dir");
    let untar = dx_testing::run(
        &PathBuf::from("tar"),
        &[
            "-xzf",
            tarball.to_string_lossy().as_ref(),
            "-C",
            extracted.to_string_lossy().as_ref(),
            "cross_app",
        ],
        &[],
    )
    .expect("tar runs");
    assert!(untar.status.success(), "{}", untar.combined());
    let app = std::fs::read(extracted.join("cross_app")).expect("archived app");
    assert_eq!(
        &app[..4],
        b"\x7fELF",
        "the archived app is an ELF executable"
    );
    assert_eq!(
        u16::from_le_bytes([app[18], app[19]]),
        0xb7,
        "the archived app must be aarch64, the target platform"
    );
    assert_eq!(
        std::fs::read(&tarball).expect("released tarball"),
        std::fs::read(release_file("/cross_release.tar.gz")).expect("built tarball"),
    );
    std::fs::remove_dir_all(&out).expect("cleanup");
}
