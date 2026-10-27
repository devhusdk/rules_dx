use std::path::{Path, PathBuf};
use std::process::Command;

const LAUNCHERS: [&str; 5] = [
    "bazelisk-darwin-amd64",
    "bazelisk-darwin-arm64",
    "bazelisk-linux-amd64",
    "bazelisk-linux-arm64",
    "bazelisk-windows-amd64.exe",
];

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn expect_ok(&self) -> &Self {
        assert_eq!(
            self.code, 0,
            "stdout {}\nstderr {}",
            self.stdout, self.stderr
        );
        self
    }

    fn expect_refusal(&self, detail: &str) -> &Self {
        assert_ne!(self.code, 0, "expected a refusal naming {detail:?}");
        assert!(
            self.stderr.contains(detail),
            "stderr must name {detail:?}, got {}",
            self.stderr
        );
        self
    }
}

struct Bundle {
    parent: PathBuf,
    path: PathBuf,
}

impl Bundle {
    fn path(&self) -> &Path {
        &self.path
    }

    fn parent(&self) -> &Path {
        &self.parent
    }

    fn advisory(&self, name: &str) -> PathBuf {
        self.path.join("advisory").join(name)
    }

    fn manifest(&self, dir: &str) -> PathBuf {
        self.path.join(dir).join("SHA256SUMS")
    }
}

fn scratch() -> PathBuf {
    dx_testing::mkscratch("dx-bootstrap").unwrap_or_else(|error| panic!("test scratch: {error}"))
}

fn script() -> PathBuf {
    let rel = std::env::var("DX_BOOTSTRAP").expect("DX_BOOTSTRAP must name the bootstrap");
    dx_testing::resolve_runfiles(&rel)
}

fn source() -> PathBuf {
    let rel = std::env::var("DX_BOOTSTRAP_SRC").expect("DX_BOOTSTRAP_SRC must name the source");
    dx_testing::resolve_runfiles(&rel)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn write(path: &Path, text: &str) {
    let parent = path
        .parent()
        .unwrap_or_else(|| panic!("{} has a parent", path.display()));
    std::fs::create_dir_all(parent)
        .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    std::fs::write(path, text.as_bytes())
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// Returns the advisory sets the bootstrap accepts, read from its own constant.
fn accepted_sets() -> Vec<String> {
    read(&source())
        .lines()
        .find(|line| line.contains("ADVISORY_SETS: [&str;"))
        .and_then(|line| line.rsplit_once('[').map(|(_, rest)| rest))
        .and_then(|rest| rest.split_once(']').map(|(head, _)| head))
        .map(|names| {
            names
                .split(',')
                .filter_map(|name| name.split('"').nth(1))
                .map(|name| name.to_owned())
                .collect()
        })
        .expect("the bootstrap declares one fixed advisory set list")
}

fn snapshot_names() -> Vec<String> {
    accepted_sets()
        .into_iter()
        .map(|set| format!("{set}.json"))
        .collect()
}

fn meta_names() -> Vec<String> {
    accepted_sets()
        .into_iter()
        .map(|set| format!("{set}.meta.json"))
        .collect()
}

/// The date every fixture bundle records for its snapshots.
const BUNDLE_DATE: &str = "2026-01-01";

fn launcher_names() -> Vec<String> {
    LAUNCHERS.iter().map(|name| (*name).to_owned()).collect()
}

/// Writes the sha256sum-style manifest of one bundle subdir, one line per named file.
fn manifest(dir: &Path, names: &[String], trailing_newline: bool) {
    let mut text = String::new();
    for name in names {
        let digest = dx_digest::sha256_file_hex(&dir.join(name))
            .unwrap_or_else(|error| panic!("hash {name}: {error}"));
        text.push_str(&format!("{digest}  {name}\n"));
    }
    if !trailing_newline && text.ends_with('\n') {
        text.pop();
    }
    std::fs::write(dir.join("SHA256SUMS"), text.as_bytes()).expect("write manifest");
}

/// Every advisory file the manifest pins: snapshots, then their identities.
fn pinned_names() -> Vec<String> {
    let mut names = snapshot_names();
    names.extend(meta_names());
    names
}

fn bundle(name: &str) -> Bundle {
    let parent = scratch();
    let staged = parent.join(name);
    for launcher in LAUNCHERS {
        write(
            &staged.join("bazelisk").join(launcher),
            &format!("launcher {launcher}\n"),
        );
    }
    for set in accepted_sets() {
        write(
            &staged.join("advisory").join(format!("{set}.json")),
            &format!("{{\"set\":\"{set}\",\"advisories\":[]}}\n"),
        );
    }
    for set in accepted_sets() {
        let snapshot = staged.join("advisory").join(format!("{set}.json"));
        let sha = dx_digest::sha256_file_hex(&snapshot)
            .unwrap_or_else(|error| panic!("hash {set}: {error}"));
        write(
            &staged.join("advisory").join(format!("{set}.meta.json")),
            &format!(
                "{{\n  \"set\": \"{set}\",\n  \"url\": \"https://osv.dev/{set}.json\",\n  \
                 \"sha256\": \"{sha}\",\n  \"retrieved_at\": \"{BUNDLE_DATE}\",\n  \
                 \"path\": \".dx/advisory/{set}.json\"\n}}\n"
            ),
        );
    }
    manifest(&staged.join("bazelisk"), &launcher_names(), true);
    manifest(&staged.join("advisory"), &pinned_names(), true);
    let path = std::fs::canonicalize(&staged)
        .unwrap_or_else(|error| panic!("canonicalize {}: {error}", staged.display()));
    Bundle { parent, path }
}

fn bootstrap_offline(args: &[String], cwd: &Path) -> Run {
    let output = Command::new(script())
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|error| panic!("run bootstrap-offline: {error}"));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn bootstrap(bundle_arg: &str, cwd: &Path, home: &Path) -> Run {
    bootstrap_offline(
        &[
            "--bundle".to_owned(),
            bundle_arg.to_owned(),
            "--install-dir".to_owned(),
            home.join("install").to_string_lossy().into_owned(),
            "--workspace".to_owned(),
            home.join("workspace").to_string_lossy().into_owned(),
        ],
        cwd,
    )
}

fn run(bundle: &Bundle, home: &Path) -> Run {
    bootstrap(&bundle.path().to_string_lossy(), bundle.path(), home)
}

/// Returns the one advisory identity line the bootstrap writes for one set.
fn identity(home: &Path, set: &str) -> String {
    read(
        &home
            .join("workspace/.dx/advisory")
            .join(format!("{set}.meta.json")),
    )
    .trim()
    .to_owned()
}

/// Returns the `url` field of one identity line.
fn recorded_url(meta: &str) -> String {
    meta.split("\"url\": \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("identity needs a url: {meta}"))
        .to_owned()
}

fn assert_identity_binds_the_vendored_file(bundle: &Bundle, home: &Path, set: &str) {
    let meta = identity(home, set);
    let snapshot = bundle.advisory(&format!("{set}.json"));
    let digest =
        dx_digest::sha256_file_hex(&snapshot).unwrap_or_else(|error| panic!("hash {set}: {error}"));
    assert!(
        dx_digest::is_lower_hex(&digest, 32),
        "{set} digest must be 64 lowercase hex"
    );
    let url = recorded_url(&meta);
    let want = bundle.advisory(&format!("{set}.json"));
    let read_back = dx_path::uri_to_path(&url)
        .unwrap_or_else(|error| panic!("{set} url {url} is not a file URI: {error}"));
    assert_eq!(
        std::fs::canonicalize(&read_back).ok(),
        std::fs::canonicalize(&want).ok(),
        "{set} url {url} must name the vendored snapshot"
    );
    for field in [
        format!("\"set\": \"{set}\""),
        format!("\"sha256\": \"{digest}\""),
        format!("\"path\": \".dx/advisory/{set}.json\""),
        format!("\"retrieved_at\": \"{BUNDLE_DATE}\""),
    ] {
        assert!(
            meta.contains(&field),
            "{set} identity needs {field}, got {meta}"
        );
    }
    assert_eq!(
        meta.matches('{').count(),
        1,
        "{set} identity is one object: {meta}"
    );
    assert_eq!(
        meta.matches('}').count(),
        1,
        "{set} identity is one object: {meta}"
    );
}

#[test]
fn installs_the_host_launcher_and_pins_every_accepted_snapshot() {
    let bundle = bundle("bundle");
    let home = scratch();
    run(&bundle, &home).expect_ok();

    let installed: Vec<PathBuf> = ["bazel", "bazel.exe"]
        .iter()
        .map(|name| home.join("install").join(name))
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(installed.len(), 1, "exactly one launcher installs");
    let launcher = read(&installed[0]);
    assert!(
        LAUNCHERS
            .iter()
            .any(|name| launcher == format!("launcher {name}\n")),
        "installed bytes must come from the bundle: {launcher}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed[0])
            .expect("launcher metadata")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "installed launcher stays executable");
    }

    let sets = accepted_sets();
    assert!(
        sets.len() >= 6,
        "the curated set list stays pinned: {sets:?}"
    );
    for set in &sets {
        assert_eq!(
            read(
                &home
                    .join("workspace/.dx/advisory")
                    .join(format!("{set}.json"))
            ),
            read(&bundle.advisory(&format!("{set}.json"))),
            "{set} snapshot bytes"
        );
        assert_identity_binds_the_vendored_file(&bundle, &home, set);
    }
}

#[test]
fn a_relative_bundle_arg_still_records_an_absolute_file_url() {
    let bundle = bundle("bundle");
    let home = scratch();
    bootstrap("bundle", bundle.parent(), &home).expect_ok();
    assert_identity_binds_the_vendored_file(&bundle, &home, "cargo");
}

#[test]
fn a_tampered_snapshot_is_refused() {
    let bundle = bundle("bundle");
    write(
        &bundle.advisory("go.json"),
        "{\"advisories\":[{\"tampered\":true}]}\n",
    );
    run(&bundle, &scratch()).expect_refusal("checksum mismatch for go.json");
}

#[test]
fn a_snapshot_the_manifest_omits_is_refused() {
    let bundle = bundle("bundle");
    let pinned: Vec<String> = pinned_names()
        .into_iter()
        .filter(|name| name != "npm.json")
        .collect();
    manifest(&bundle.path().join("advisory"), &pinned, true);
    run(&bundle, &scratch()).expect_refusal("manifest does not pin advisory snapshot: npm.json");
}

#[test]
fn a_manifest_that_pins_nothing_is_refused() {
    let bundle = bundle("bundle");
    write(&bundle.manifest("advisory"), "# nothing pinned\n");
    run(&bundle, &scratch()).expect_refusal("manifest names no files");
}

#[test]
fn the_last_manifest_line_is_verified_without_a_trailing_newline() {
    let bundle = bundle("bundle");
    let names = pinned_names();
    let last = names.last().expect("a curated set").clone();
    manifest(&bundle.path().join("advisory"), &names, false);
    assert!(
        !read(&bundle.manifest("advisory")).ends_with('\n'),
        "the fixture must drop the trailing newline"
    );
    write(
        &bundle.advisory(&last),
        "{\"advisories\":[{\"tampered\":true}]}\n",
    );
    run(&bundle, &scratch()).expect_refusal(&format!("checksum mismatch for {last}"));
}

#[test]
fn an_unknown_advisory_set_is_refused() {
    let bundle = bundle("bundle");
    write(&bundle.advisory("pypi.json"), "{\"advisories\":[]}\n");
    let mut names = pinned_names();
    names.push("pypi.json".to_owned());
    manifest(&bundle.path().join("advisory"), &names, true);
    run(&bundle, &scratch()).expect_refusal("unknown advisory set: pypi");
}

#[test]
fn an_old_snapshot_keeps_its_bundle_date() {
    let bundle = bundle("bundle");
    let home = scratch();
    let result = run(&bundle, &home);
    result.expect_ok();
    for set in accepted_sets() {
        let meta = identity(&home, &set);
        assert!(
            meta.contains(&format!("\"retrieved_at\": \"{BUNDLE_DATE}\"")),
            "{set} keeps the bundle date: {meta}"
        );
    }
    assert!(!result.stdout.contains("retrieved_at"), "{}", result.stdout);
}

#[test]
fn a_snapshot_without_a_bundle_date_is_refused() {
    let bundle = bundle("bundle");
    std::fs::remove_file(bundle.advisory("go.meta.json")).expect("remove identity");
    let names: Vec<String> = pinned_names()
        .into_iter()
        .filter(|name| name != "go.meta.json")
        .collect();
    manifest(&bundle.path().join("advisory"), &names, true);
    run(&bundle, &scratch()).expect_refusal(
        "bundle has no recorded date for advisory snapshot: go.json (want advisory/go.meta.json)",
    );
}

#[test]
fn an_unpinned_bundle_identity_is_refused() {
    let bundle = bundle("bundle");
    manifest(&bundle.path().join("advisory"), &snapshot_names(), true);
    run(&bundle, &scratch())
        .expect_refusal("manifest does not pin advisory identity: cargo.meta.json");
}

#[test]
fn a_bundle_identity_is_not_an_unknown_set() {
    let bundle = bundle("bundle");
    let result = run(&bundle, &scratch());
    result.expect_ok();
    assert!(
        !result.stderr.contains("unknown advisory set"),
        "{}",
        result.stderr
    );
}

#[cfg(unix)]
#[test]
fn a_bundle_path_with_a_quote_or_backslash_is_recorded_percent_encoded() {
    let bundle = bundle("we\"ird\\bundle");
    let home = scratch();
    run(&bundle, &home).expect_ok();
    let meta = identity(&home, "cargo");
    let url = recorded_url(&meta);
    assert!(url.contains("%22"), "quote is encoded: {url}");
    assert!(url.contains("%5C"), "backslash is encoded: {url}");
    assert!(!url.contains('\\'), "no raw backslash: {url}");
    assert_identity_binds_the_vendored_file(&bundle, &home, "cargo");
}

#[test]
fn a_flag_without_its_value_is_refused() {
    let bundle = bundle("bundle");
    bootstrap_offline(
        &[
            "--bundle".to_owned(),
            bundle.path().to_string_lossy().into_owned(),
            "--workspace".to_owned(),
        ],
        bundle.path(),
    )
    .expect_refusal("--workspace needs a DIR");
}

#[test]
fn a_bad_argument_prints_usage_to_stderr_only() {
    let bundle = bundle("bundle");
    let result = bootstrap_offline(&["--nope".to_owned()], bundle.path());
    result.expect_refusal("unknown argument '--nope'");
    assert_eq!(result.stdout, "", "a refusal prints nothing on stdout");
    assert_eq!(
        result.stderr.matches("usage:").count(),
        1,
        "{}",
        result.stderr
    );
}

#[test]
fn help_prints_usage_once_and_exits_zero() {
    let bundle = bundle("bundle");
    let result = bootstrap_offline(&["--help".to_owned()], bundle.path());
    result.expect_ok();
    assert_eq!(
        result.stdout.matches("usage:").count(),
        1,
        "{}",
        result.stdout
    );
    assert_eq!(result.stderr, "");
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
    for entry in std::fs::read_dir(from)
        .unwrap_or_else(|error| panic!("read {}: {error}", from.display()))
        .flatten()
    {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", entry.path().display()));
        }
    }
}

#[test]
fn the_demo_offline_bundle_bootstraps() {
    let rel = std::env::var("DX_OFFLINE_DEMO").expect("DX_OFFLINE_DEMO must name the demo bundle");
    let manifest = dx_testing::resolve_runfiles(&rel);
    let built = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the demo manifest sits two levels under the bundle");
    let staged = scratch().join("offline_demo");
    copy_tree(built, &staged);
    let home = scratch();
    let bundle = std::fs::canonicalize(&staged).expect("canonical demo bundle");
    bootstrap(&bundle.to_string_lossy(), &home, &home).expect_ok();
    assert!(home.join("workspace/.dx/advisory/cargo.json").is_file());
    assert!(
        identity(&home, "cargo").contains("\"retrieved_at\": \"2026-01-01\""),
        "the demo bundle records its own date: {}",
        identity(&home, "cargo")
    );
    assert!(
        ["bazel", "bazel.exe"]
            .iter()
            .any(|name| home.join("install").join(name).is_file()),
        "the demo launcher installs"
    );
}

#[test]
fn offline_bundle_knows_every_host_launcher() {
    let rel = std::env::var("DX_OFFLINE_BZL").expect("DX_OFFLINE_BZL must name offline.bzl");
    let bzl = read(&dx_testing::resolve_runfiles(&rel));
    let start = bzl
        .find("_LAUNCHERS = [")
        .expect("offline.bzl lists launchers");
    let list = &bzl[start..];
    let list = &list[..list.find(']').expect("launcher list is closed")];
    let mut in_bzl: Vec<String> = list
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    in_bzl.sort();
    let mut in_rust: Vec<String> = read(&source())
        .split("Some(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|name| name.starts_with("bazelisk-"))
        .map(str::to_owned)
        .collect();
    in_rust.sort();
    assert_eq!(in_bzl, in_rust);
    assert_eq!(in_bzl, launcher_names());
}
