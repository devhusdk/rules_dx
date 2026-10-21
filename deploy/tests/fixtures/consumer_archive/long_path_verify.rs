use std::path::PathBuf;

const MEMBER: &str = "resources/textures/textures/textures/textures/textures/textures/textures/textures/textures/textures/icon.png";

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn tar(args: &[&str]) -> dx_testing::Run {
    let out = dx_testing::run(&PathBuf::from("tar"), args, &[]).expect("tar must execute");
    assert!(
        out.status.success(),
        "tar {args:?} failed\n{}",
        out.combined()
    );
    out
}

#[test]
fn long_resource_path_round_trips_through_standard_tar() {
    let tarball = data("DX_LONG_TARBALL");
    let tarball = tarball.to_string_lossy();
    assert_eq!(MEMBER.len(), 108);
    let listed = tar(&["-tzf", tarball.as_ref()]);
    assert!(
        listed.stdout.lines().any(|line| line == MEMBER),
        "long member missing\n{}",
        listed.stdout
    );
    let scratch = dx_testing::mkscratch("long-path").expect("scratch");
    tar(&[
        "-xzf",
        tarball.as_ref(),
        "-C",
        scratch.to_string_lossy().as_ref(),
    ]);
    let extracted = std::fs::read(scratch.join(MEMBER)).expect("read extracted member");
    assert!(String::from_utf8_lossy(&extracted).contains("dx-consumer-shared-marker"));
}
