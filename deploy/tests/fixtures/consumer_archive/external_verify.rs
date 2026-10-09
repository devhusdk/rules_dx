use std::path::{Path, PathBuf};

const BANNER_MARKER: &str = "dx-external-archive-marker";
const GREET_MESSAGE: &str = "Hello from the external native library, external";
const NOTICE_MARKER: &str = "external-release NOTICE";

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn read_identity(path: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(path).expect("read IDENTITY.txt");
    text.lines()
        .map(|line| {
            let (key, value) = line
                .split_once(": ")
                .expect("IDENTITY line must be 'key: value'");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn field(identity: &[(String, String)], key: &str) -> String {
    identity
        .iter()
        .find(|(name, _)| name == key)
        .unwrap_or_else(|| panic!("IDENTITY.txt must carry '{key}'"))
        .1
        .clone()
}

fn dotted_version(text: &str) -> (u64, u64) {
    let version = text
        .strip_prefix("glibc-")
        .unwrap_or_else(|| panic!("min_runtime must be a glibc floor, got {text:?}"));
    let (major, minor) = version
        .split_once('.')
        .unwrap_or_else(|| panic!("min_runtime must be glibc-<major>.<minor>, got {text:?}"));
    (
        major.parse().expect("major must be numeric"),
        minor.parse().expect("minor must be numeric"),
    )
}

fn host_glibc(stdout: &str) -> (u64, u64) {
    for token in stdout.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            if let (Ok(major), Ok(minor)) = (parts[0].parse(), parts[1].parse()) {
                return (major, minor);
            }
        }
    }
    panic!("ldd --version must report a glibc version, got {stdout:?}");
}

#[test]
fn external_archive_checksum_matches() {
    let tarball = data("DX_EXTERNAL_TARBALL");
    let checksum = data("DX_EXTERNAL_CHECKSUM");
    let text = std::fs::read_to_string(&checksum).expect("read checksum");
    let expected = text.split_whitespace().next().expect("checksum field");
    let hashed = dx_testing::run(
        &PathBuf::from("sha256sum"),
        &[tarball.to_string_lossy().as_ref()],
        &[],
    )
    .expect("sha256sum must execute");
    assert!(
        hashed.status.success(),
        "cannot hash {}\n{}",
        tarball.display(),
        hashed.combined()
    );
    let actual = hashed
        .stdout
        .split_whitespace()
        .next()
        .expect("digest field");
    assert_eq!(
        expected,
        actual,
        "checksum mismatch for {}",
        tarball.display()
    );
}

#[test]
fn external_archive_relocates_and_runs() {
    let tarball = data("DX_EXTERNAL_TARBALL");
    let scratch = dx_testing::mkscratch("external-release").expect("scratch");
    let listed = dx_testing::run(
        &PathBuf::from("tar"),
        &["-tzf", tarball.to_string_lossy().as_ref()],
        &[],
    )
    .expect("tar must execute");
    assert!(
        listed.status.success(),
        "cannot list {}\n{}",
        tarball.display(),
        listed.combined()
    );
    let mut members: Vec<&str> = listed.stdout.lines().collect();
    members.sort_unstable();
    assert_eq!(
        members,
        [
            "IDENTITY.txt",
            "NOTICE",
            "external_app",
            "resources/banner.txt"
        ],
        "unexpected tarball members in {}",
        tarball.display()
    );
    let extracted = dx_testing::run(
        &PathBuf::from("tar"),
        &[
            "-xzf",
            tarball.to_string_lossy().as_ref(),
            "-C",
            scratch.to_string_lossy().as_ref(),
        ],
        &[],
    )
    .expect("tar must execute");
    assert!(
        extracted.status.success(),
        "cannot extract {}\n{}",
        tarball.display(),
        extracted.combined()
    );
    let app = scratch.join("external_app");
    let banner = scratch.join("resources").join("banner.txt");
    let notice = scratch.join("NOTICE");
    let identity_file = scratch.join("IDENTITY.txt");
    assert!(app.is_file(), "extracted app must exist");
    assert!(banner.is_file(), "extracted resource must exist");
    let identity = read_identity(&identity_file);
    assert_eq!(field(&identity, "arch"), std::env::consts::ARCH);
    assert_eq!(field(&identity, "os"), std::env::consts::OS);
    assert_eq!(field(&identity, "linkage"), "dynamic");
    let floor = dotted_version(&field(&identity, "min_runtime"));
    let version =
        dx_testing::run(&PathBuf::from("ldd"), &["--version"], &[]).expect("ldd must execute");
    assert!(
        version.status.success(),
        "cannot query the host runtime\n{}",
        version.combined()
    );
    let host = host_glibc(&version.stdout);
    assert!(
        host >= floor,
        "host glibc {host:?} is below the recorded floor {floor:?}"
    );
    let linked = dx_testing::run(
        &PathBuf::from("ldd"),
        &[app.to_string_lossy().as_ref()],
        &[],
    )
    .expect("ldd must execute");
    assert!(
        linked.status.success(),
        "cannot inspect {}\n{}",
        app.display(),
        linked.combined()
    );
    assert!(
        !linked.combined().contains("not found"),
        "extracted app has missing loader dependencies\n{}",
        linked.combined()
    );
    let rundir = scratch.join("run");
    std::fs::create_dir_all(&rundir).expect("run dir");
    let output = std::process::Command::new(&app)
        .current_dir(&rundir)
        .output()
        .expect("relocated app must execute");
    assert!(
        output.status.success(),
        "relocated app failed\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(BANNER_MARKER),
        "relocated app must load its resource\n{stdout}"
    );
    assert!(
        stdout.contains(GREET_MESSAGE),
        "relocated app must call its native dependency\n{stdout}"
    );
    let notice_text = std::fs::read_to_string(&notice).expect("read NOTICE");
    assert!(
        notice_text.contains(NOTICE_MARKER),
        "archive must carry the license-words file\n{notice_text}"
    );
}
