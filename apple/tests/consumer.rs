use std::path::Path;

use dx_nested::{self as nested, Case};
use dx_testing::{rules_dx_source, Run};

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

pub fn module(extra: &str) -> String {
    format!(
        "module(name = \"ledger\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"dx\")\nbazel_dep(name = \"apple_support\", version = \"{APPLE_SUPPORT}\", repo_name = \"xcode_support\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.27\")\nbazel_dep(name = \"platforms\", version = \"1.1.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n{extra}",
        rules_dx_source().display()
    )
}

pub fn consumer(prefix: &str, module_extra: &str, bazelrc: &str) -> Case {
    let rules = rules_dx_source();
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
