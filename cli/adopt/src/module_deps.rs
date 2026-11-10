//! The `bazel_dep` and `module` names of a workspace's `MODULE.bazel`, includes followed.

use std::collections::BTreeSet;
use std::path::Path;

/// The `bazel_dep` names of the consumer's `MODULE.bazel`, includes followed, plus its own
/// module name, so a family module counts as installed in itself. Empty when it cannot be read.
pub fn module_deps(root: &Path) -> BTreeSet<String> {
    let mut deps = load_calls(root, "bazel_dep");
    deps.extend(load_calls(root, "module"));
    deps
}

/// Whether the workspace at `root` is the `rules_dx` module itself, which carries every family.
pub fn is_rules_dx_root(root: &Path) -> bool {
    load_calls(root, "module").contains("rules_dx")
}

fn load_calls(root: &Path, call_name: &str) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(root.join("MODULE.bazel")) else {
        return BTreeSet::new();
    };
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| format!("cannot read included {label}: {e}"))
    };
    let Ok(modules) = dx_module_bazel::load_with_includes(&text, &read_include) else {
        return BTreeSet::new();
    };
    modules
        .iter()
        .flat_map(|module| module.calls())
        .filter(|call| call.name == call_name)
        .filter_map(|call| call.str("name").map(str::to_owned))
        .collect()
}
