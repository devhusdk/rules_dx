use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, Run};

pub const APPLE_SUPPORT: &str = "1.24.2";

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let anchor = resolve_runfiles("apple/provisioning/probe.sh");
    let real = std::fs::canonicalize(&anchor).unwrap_or_else(|error| panic!("{anchor:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

pub fn module(extra: &str) -> String {
    format!(
        "module(name = \"ledger\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"dx\")\nbazel_dep(name = \"apple_support\", version = \"{APPLE_SUPPORT}\", repo_name = \"xcode_support\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n{extra}",
        rules_dx_root().display()
    )
}

pub fn consumer(prefix: &str, module_extra: &str, bazelrc: &str) -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch(prefix).unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(&root.join("MODULE.bazel"), &module(module_extra));
    write(
        &root.join(".bazelrc"),
        &format!("startup --max_idle_secs=120\ncommon --enable_bzlmod\nbuild:dx_dev --compilation_mode=fastbuild\nbuild --remote_download_outputs=all\n{bazelrc}"),
    );
    root
}

pub fn fails_with(run: &Run, needle: &str) {
    assert_ne!(run.status.code(), Some(0), "{}", run.combined());
    assert!(
        run.combined().contains(needle),
        "want {needle:?} in:\n{}",
        run.combined()
    );
}
