use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{rlocation, Run};

pub const APPLE_SUPPORT: &str = "2.9.1";

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn checkout(repo: &str) -> PathBuf {
    let module = rlocation(&format!("{repo}+/MODULE.bazel"))
        .or_else(|_| rlocation("_main/MODULE.bazel"))
        .unwrap_or_else(|error| panic!("runfile MODULE.bazel of {repo}: {error}"));
    let real = std::fs::canonicalize(&module).unwrap_or_else(|error| panic!("{module:?}: {error}"));
    real.parent()
        .unwrap_or_else(|| panic!("no checkout above {real:?}"))
        .to_path_buf()
}

/// The rules_dx checkout, the main repo when run from the root and an external repo from here.
pub fn core_source() -> PathBuf {
    checkout("rules_dx")
}

/// The family checkout, the main repo when run from here and an external repo from the root.
pub fn family_source() -> PathBuf {
    checkout("rules_dx_apple")
}

pub fn module(extra: &str) -> String {
    format!(
        "module(name = \"ledger\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"dx\")\nbazel_dep(name = \"rules_dx_apple\", version = \"0.0.0\", repo_name = \"dx_apple\")\nbazel_dep(name = \"apple_support\", version = \"{APPLE_SUPPORT}\", repo_name = \"xcode_support\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.27\")\nbazel_dep(name = \"platforms\", version = \"1.1.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\nlocal_path_override(module_name = \"rules_dx_apple\", path = \"{}\")\n{extra}",
        core_source().display(),
        family_source().display()
    )
}

pub fn consumer(prefix: &str, module_extra: &str, bazelrc: &str) -> Case {
    let rules = core_source();
    let root = nested::case(prefix);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(&root.join("MODULE.bazel"), &module(module_extra));
    write(
        &root.join(".bazelrc"),
        &format!("common --enable_bzlmod\nbuild:dx_dev --compilation_mode=fastbuild\nbuild --remote_download_outputs=all\n{bazelrc}"),
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
