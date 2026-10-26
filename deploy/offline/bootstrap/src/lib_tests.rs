//! Tests for the offline bootstrap helpers.

use std::path::PathBuf;

use super::{
    host_launcher, manifest_entry, parse_args, pinned, run, today_utc, usage, Command, Options,
    ADVISORY_SETS,
};

fn scratch(name: &str) -> PathBuf {
    dx_testing::mkscratch(name).unwrap_or_else(|error| panic!("test scratch: {error}"))
}

#[test]
fn every_host_maps_to_the_launcher_its_bundle_carries() {
    assert_eq!(
        host_launcher("linux", "x86_64"),
        Some("bazelisk-linux-amd64")
    );
    assert_eq!(
        host_launcher("linux", "aarch64"),
        Some("bazelisk-linux-arm64")
    );
    assert_eq!(
        host_launcher("macos", "x86_64"),
        Some("bazelisk-darwin-amd64")
    );
    assert_eq!(
        host_launcher("macos", "aarch64"),
        Some("bazelisk-darwin-arm64")
    );
    assert_eq!(
        host_launcher("windows", "x86_64"),
        Some("bazelisk-windows-amd64.exe")
    );
    assert_eq!(host_launcher("freebsd", "x86_64"), None);
    assert_eq!(host_launcher("windows", "aarch64"), None);
}

#[test]
fn the_curated_set_list_stays_pinned() {
    assert_eq!(
        ADVISORY_SETS,
        ["cargo", "npm", "maven", "nuget", "go", "rubygems"]
    );
}

#[test]
fn a_manifest_line_splits_into_a_digest_and_a_name() {
    assert_eq!(
        manifest_entry("abc123  cargo.json"),
        Some(("abc123".to_string(), "cargo.json".to_string()))
    );
    assert_eq!(
        manifest_entry("  abc123   cargo.json  "),
        Some(("abc123".to_string(), "cargo.json".to_string()))
    );
}

#[test]
fn blank_and_comment_manifest_lines_name_nothing() {
    assert_eq!(manifest_entry(""), None);
    assert_eq!(manifest_entry("   "), None);
    assert_eq!(manifest_entry("# nothing pinned"), None);
    assert_eq!(manifest_entry("  # indented comment"), None);
}

#[test]
fn a_manifest_without_a_name_still_reports_the_digest() {
    assert_eq!(
        manifest_entry("abc123"),
        Some(("abc123".to_string(), String::new()))
    );
}

#[test]
fn a_pinned_digest_is_found_by_file_name() {
    let manifest = "aaa  one.json\nbbb  two.json\nccc  three.json\n";
    assert_eq!(pinned(manifest, "two.json").as_deref(), Some("bbb"));
    assert_eq!(pinned(manifest, "missing.json"), None);
}

#[test]
fn the_last_manifest_line_is_read_without_a_trailing_newline() {
    let manifest = "aaa  one.json\nbbb  two.json";
    assert_eq!(pinned(manifest, "two.json").as_deref(), Some("bbb"));
}

#[test]
fn today_is_a_plain_utc_date() {
    let today = today_utc();
    assert_eq!(today.len(), 10, "today looks like YYYY-MM-DD: {today}");
    assert_eq!(
        today
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '-')
            .count(),
        10,
        "today carries only digits and dashes: {today}"
    );
}

#[test]
fn a_missing_bundle_is_refused_before_anything_else() {
    let options = Options {
        bundle: None,
        install_dir: "/tmp/install".to_string(),
        workspace: "/tmp/workspace".to_string(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        "missing --bundle DIR (vendored offline bundle)"
    );
    let empty = Options {
        bundle: Some(String::new()),
        ..options
    };
    assert_eq!(
        run(&empty, "linux", "x86_64", "2026-01-01"),
        "missing --bundle DIR (vendored offline bundle)"
    );
}

#[test]
fn a_bundle_dir_that_is_absent_is_refused() {
    let options = Options {
        bundle: Some("/nonexistent/offline/bundle".to_string()),
        install_dir: "/tmp/install".to_string(),
        workspace: "/tmp/workspace".to_string(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        "bundle dir does not exist: /nonexistent/offline/bundle"
    );
}

#[test]
fn a_bundle_without_the_two_bundle_dirs_is_refused() {
    let root = scratch("dx-bootstrap-shape-");
    let options = Options {
        bundle: Some(root.to_string_lossy().into_owned()),
        install_dir: root.join("bin").to_string_lossy().into_owned(),
        workspace: root.to_string_lossy().into_owned(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        format!("bundle has no bazelisk/ dir: {}", root.to_string_lossy())
    );
    std::fs::create_dir(root.join("bazelisk")).expect("bazelisk dir");
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        format!("bundle has no advisory/ dir: {}", root.to_string_lossy())
    );
}

#[test]
fn an_unsupported_host_is_refused() {
    let root = scratch("dx-bootstrap-host-");
    std::fs::create_dir(root.join("bazelisk")).expect("bazelisk dir");
    std::fs::create_dir(root.join("advisory")).expect("advisory dir");
    let options = Options {
        bundle: Some(root.to_string_lossy().into_owned()),
        install_dir: root.join("bin").to_string_lossy().into_owned(),
        workspace: root.to_string_lossy().into_owned(),
    };
    assert_eq!(
        run(&options, "plan9", "risc-v", "2026-01-01"),
        "unsupported host plan9-risc-v"
    );
}

#[test]
fn the_usage_line_names_the_required_bundle() {
    assert!(usage().contains("--bundle DIR"));
}

fn bundle_with_launcher(name: &str, bytes: &[u8]) -> (PathBuf, Options) {
    let root = scratch(name);
    let bazelisk = root.join("bazelisk");
    let advisory = root.join("advisory");
    std::fs::create_dir(&bazelisk).expect("bazelisk dir");
    std::fs::create_dir(&advisory).expect("advisory dir");
    let asset = host_launcher("linux", "x86_64").expect("launcher");
    std::fs::write(bazelisk.join(asset), bytes).expect("launcher");
    let sum = dx_digest::sha256_file_hex(&bazelisk.join(asset)).expect("digest");
    std::fs::write(bazelisk.join("SHA256SUMS"), format!("{sum}  {asset}\n")).expect("manifest");
    let set = ADVISORY_SETS[0];
    std::fs::write(advisory.join(format!("{set}.json")), b"{}").expect("snapshot");
    let sum = dx_digest::sha256_file_hex(&advisory.join(format!("{set}.json"))).expect("digest");
    std::fs::write(advisory.join("SHA256SUMS"), format!("{sum}  {set}.json\n")).expect("manifest");
    let options = Options {
        bundle: Some(root.to_string_lossy().into_owned()),
        install_dir: root.join("bin").to_string_lossy().into_owned(),
        workspace: root.join("ws").to_string_lossy().into_owned(),
    };
    (root, options)
}

#[test]
fn a_non_utf8_launcher_is_installed_byte_for_byte() {
    let bytes = [0x7f, b'E', b'L', b'F', 0xff, 0x80, 0x00, 0xc3, 0x28];
    let (root, options) = bundle_with_launcher("dx-bootstrap-binary-", &bytes);
    assert_eq!(run(&options, "linux", "x86_64", "2026-01-01"), "");
    let installed = std::fs::read(root.join("bin").join("bazel")).expect("installed launcher");
    assert_eq!(installed, bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.join("bin").join("bazel"))
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }
}

#[test]
#[cfg(unix)]
fn a_read_only_bundle_installs_twice() {
    use std::os::unix::fs::PermissionsExt;
    let (root, options) = bundle_with_launcher("dx-bootstrap-read-only-", b"launcher");
    let asset = host_launcher("linux", "x86_64").expect("launcher");
    let mut read_only = vec![root.join("bazelisk").join(asset)];
    read_only.extend(
        ADVISORY_SETS
            .iter()
            .map(|set| root.join("advisory").join(format!("{set}.json")))
            .filter(|path| path.exists()),
    );
    for path in &read_only {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    }
    assert_eq!(run(&options, "linux", "x86_64", "2026-01-01"), "");
    assert_eq!(run(&options, "linux", "x86_64", "2026-01-01"), "");
    let set = ADVISORY_SETS[0];
    let snapshot = root
        .join("ws")
        .join(".dx")
        .join("advisory")
        .join(format!("{set}.json"));
    assert_eq!(std::fs::read(&snapshot).expect("snapshot"), b"{}");
    assert_eq!(
        std::fs::metadata(&snapshot)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn a_launcher_that_cannot_be_copied_is_reported() {
    let (root, options) = bundle_with_launcher("dx-bootstrap-copyfail-", b"\xff\xfe");
    std::fs::create_dir_all(root.join("bin").join("bazel")).expect("blocking dir");
    let refusal = run(&options, "linux", "x86_64", "2026-01-01");
    assert!(refusal.starts_with("cannot copy "), "{refusal}");
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| (*arg).to_string()).collect()
}

#[test]
fn help_is_its_own_command() {
    assert_eq!(parse_args(&args(&["-h"])), Ok(Command::Help));
    assert_eq!(parse_args(&args(&["--help"])), Ok(Command::Help));
}

#[test]
fn a_bad_argument_is_a_plain_refusal() {
    assert_eq!(
        parse_args(&args(&["--nope"])),
        Err("unknown argument '--nope'".to_string())
    );
    assert_eq!(
        parse_args(&args(&["--bundle"])),
        Err("--bundle needs a DIR".to_string())
    );
}

#[test]
fn flags_parse_into_bootstrap_options() {
    let parsed = parse_args(&args(&[
        "--bundle",
        "b",
        "--install-dir",
        "i",
        "--workspace",
        "w",
    ]));
    assert_eq!(
        parsed,
        Ok(Command::Bootstrap(Options {
            bundle: Some("b".to_string()),
            install_dir: "i".to_string(),
            workspace: "w".to_string(),
        }))
    );
}

#[test]
fn the_identity_url_is_a_file_uri_of_the_bundle() {
    let (root, mut options) = bundle_with_launcher("dx bootstrap url-", b"launcher");
    options.bundle = Some(root.to_string_lossy().into_owned());
    assert_eq!(run(&options, "linux", "x86_64", "2026-01-01"), "");
    let meta = std::fs::read_to_string(root.join("ws/.dx/advisory/cargo.meta.json"))
        .expect("identity");
    assert!(meta.contains("%20"), "a space is percent-encoded: {meta}");
    assert!(!meta.contains('\\'), "no backslash in the URL: {meta}");
    assert!(meta.contains("\"url\": \"file://"), "file URL: {meta}");
}
