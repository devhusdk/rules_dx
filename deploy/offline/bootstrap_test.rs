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
    manifest(&staged.join("bazelisk"), &launcher_names(), true);
    manifest(&staged.join("advisory"), &snapshot_names(), true);
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

fn assert_identity_binds_the_vendored_file(bundle: &Bundle, home: &Path, set: &str) {
    let meta = identity(home, set);
    let snapshot = bundle.advisory(&format!("{set}.json"));
    let digest =
        dx_digest::sha256_file_hex(&snapshot).unwrap_or_else(|error| panic!("hash {set}: {error}"));
    assert!(
        dx_digest::is_lower_hex(&digest, 32),
        "{set} digest must be 64 lowercase hex"
    );
    for field in [
        format!("\"set\": \"{set}\""),
        format!(
            "\"url\": \"file://{}/advisory/{set}.json\"",
            bundle.path().to_string_lossy()
        ),
        format!("\"sha256\": \"{digest}\""),
        format!("\"path\": \".dx/advisory/{set}.json\""),
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
    let pinned: Vec<String> = snapshot_names()
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
    let names = snapshot_names();
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
    let mut names = snapshot_names();
    names.push("pypi.json".to_owned());
    manifest(&bundle.path().join("advisory"), &names, true);
    run(&bundle, &scratch()).expect_refusal("unknown advisory set: pypi");
}

#[test]
fn a_bundle_path_it_cannot_spell_in_json_is_refused() {
    let bundle = bundle("we\"ird/bundle");
    run(&bundle, &scratch()).expect_refusal("bundle path must stay plain text");
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
