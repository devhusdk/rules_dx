use std::path::{Path, PathBuf};

const BANNER: &str = "dx-external-archive-marker\nbanner-external-v1\n";
const NOTICE: &str =
    "external-release NOTICE\nThird-party licenses for the external consumer application.\n";
const GREET_MESSAGE: &str = "Hello from the external native library, external";
const MEMBERS: [&str; 4] = [
    "IDENTITY.txt",
    "NOTICE",
    "external_app",
    "resources/banner.txt",
];

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn tar(args: &[&str]) -> String {
    let out = dx_testing::run(&PathBuf::from("tar"), args, &[]).expect("tar must execute");
    assert!(
        out.status.success(),
        "tar {args:?} failed\n{}",
        out.combined()
    );
    out.stdout
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn verify(env: &str, name: &str) {
    let tarball = data(env);
    let tarball_arg = tarball.to_string_lossy().into_owned();
    let listed = tar(&["-tzf", &tarball_arg]);
    let mut members: Vec<&str> = listed.lines().collect();
    members.sort_unstable();
    assert_eq!(
        members,
        MEMBERS,
        "unexpected members in {}",
        tarball.display()
    );
    for member in &members {
        assert!(
            !member.starts_with('/')
                && !member.starts_with("external/")
                && !member.split('/').any(|part| part == ".." || part == "."),
            "member {member:?} in {} must be relative and contained",
            tarball.display()
        );
    }
    let scratch = dx_testing::mkscratch(name).expect("scratch");
    tar(&[
        "-xzf",
        &tarball_arg,
        "-C",
        scratch.to_string_lossy().as_ref(),
    ]);
    assert_eq!(
        read(&scratch.join("NOTICE")),
        NOTICE,
        "NOTICE bytes in {name}"
    );
    assert_eq!(
        read(&scratch.join("resources").join("banner.txt")),
        BANNER,
        "banner bytes in {name}"
    );
    let identity = read(&scratch.join("IDENTITY.txt"));
    assert!(
        identity.starts_with(&format!("name: {name}\n")),
        "IDENTITY.txt must name {name}\n{identity}"
    );
    let rundir = scratch.join("run");
    std::fs::create_dir_all(&rundir).expect("run dir");
    let output = std::process::Command::new(scratch.join("external_app"))
        .current_dir(&rundir)
        .output()
        .expect("extracted app must execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "extracted app from {name} failed\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("external_app resource: dx-external-archive-marker"),
        "app from {name} must load its resource\n{stdout}"
    );
    assert!(
        stdout.contains(GREET_MESSAGE),
        "app from {name} must call its native dependency\n{stdout}"
    );
}

#[test]
fn workspace_package_archive_has_relative_layout() {
    verify("DX_WORKSPACE_TARBALL", "external_release");
}

#[test]
fn external_root_package_archive_has_relative_layout() {
    verify("DX_REPO_ROOT_TARBALL", "repo_root_release");
}

#[test]
fn external_nested_package_archive_has_relative_layout() {
    verify("DX_REPO_NESTED_TARBALL", "repo_nested_release");
}
