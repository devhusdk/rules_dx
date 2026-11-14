//! Plans a `bazel_dep` for each rule set the workspace's BUILD files load from that the root
//! module does not declare, at the version rules_dx itself depends on.

use std::path::Path;

use dx_module_bazel::ModuleFile;

use crate::AdoptError;
use crate::load_graph::build_file_repos;
use crate::lock_hubs::LockHubs;
use crate::module_deps::module_deps;
use crate::module_pin::ModuleEdit;
use crate::preflight::RULES_DX_DEPS;

const MODULE_FILE: &str = "MODULE.bazel";

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

/// Plans the `bazel_dep` lines for the rule sets BUILD files load from but `MODULE.bazel` and
/// its includes do not declare. Writes nothing.
pub fn plan_loaded_deps(root: &Path) -> Result<LockHubs, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(LockHubs::default());
    };
    let declared = module_deps(root);
    let loaded = build_file_repos(root).map_err(refuse)?;
    let mut new = text.clone();
    let mut notes = Vec::new();
    for (name, version) in RULES_DX_DEPS {
        if !loaded.contains(*name) || declared.contains(*name) {
            continue;
        }
        new = ModuleFile::parse(&new)
            .and_then(|file| file.insert_bazel_dep(name, version))
            .map_err(refuse)?;
        notes.push(format!(
            "BUILD files load from {name}; {MODULE_FILE} needs bazel_dep {name} {version}"
        ));
    }
    let edit = (new != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new,
    });
    Ok(LockHubs { edit, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str =
        "module(name = \"app\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";

    fn plan(files: &[(&str, &str)]) -> LockHubs {
        let dir = dx_test_scratch::scratch("loaded-deps");
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        }
        plan_loaded_deps(dir.path()).expect("plan")
    }

    fn version(name: &str) -> &'static str {
        RULES_DX_DEPS
            .iter()
            .find(|(dep, _)| *dep == name)
            .map(|(_, version)| *version)
            .expect("pinned")
    }

    #[test]
    fn loaded_rule_sets_get_a_bazel_dep_at_the_rules_dx_pin() {
        let hubs = plan(&[
            ("MODULE.bazel", MODULE),
            (
                "web/BUILD.bazel",
                "load(\"@aspect_rules_js//js:defs.bzl\", \"js_library\")\nload(\"@npm//:defs.bzl\", \"npm_link_all_packages\")\n",
            ),
            (
                "ts/BUILD",
                "load(\"@aspect_rules_ts//ts:defs.bzl\", \"ts_project\")\nload(\"@aspect_rules_js//npm:defs.bzl\", \"npm_package\")\nload(\"//tools:defs.bzl\", \"x\")\n",
            ),
        ]);
        let edit = hubs.edit.expect("edit");
        assert_eq!(
            edit.new,
            format!(
                "{MODULE}bazel_dep(name = \"aspect_rules_js\", version = \"{}\")\nbazel_dep(name = \"aspect_rules_ts\", version = \"{}\")\n",
                version("aspect_rules_js"),
                version("aspect_rules_ts"),
            )
        );
        assert_eq!(hubs.notes.len(), 2, "{:?}", hubs.notes);
    }

    #[test]
    fn declared_and_unpinned_repos_are_left_alone() {
        let hubs = plan(&[
            (
                "MODULE.bazel",
                "module(name = \"app\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\ninclude(\"//:js.MODULE.bazel\")\n",
            ),
            (
                "js.MODULE.bazel",
                "bazel_dep(name = \"aspect_rules_js\", version = \"0.1.0\")\n",
            ),
            (
                "BUILD.bazel",
                "load(\"@aspect_rules_js//js:defs.bzl\", \"js_library\")\nload(\"@rules_dx//rust:defs.bzl\", \"rust_library\")\nload(\"@app//:defs.bzl\", \"x\")\nload(\"@somewhere_else//:defs.bzl\", \"y\")\n",
            ),
        ]);
        assert_eq!(hubs.edit, None);
        assert!(hubs.notes.is_empty(), "{:?}", hubs.notes);
    }

    #[test]
    fn ignored_directories_are_not_read() {
        let hubs = plan(&[
            ("MODULE.bazel", MODULE),
            (".bazelignore", "vendor\n"),
            (
                "vendor/BUILD.bazel",
                "load(\"@aspect_rules_ts//ts:defs.bzl\", \"ts_project\")\n",
            ),
        ]);
        assert_eq!(hubs.edit, None);
    }

    #[test]
    fn a_workspace_without_module_bazel_plans_nothing() {
        let hubs = plan(&[(
            "BUILD.bazel",
            "load(\"@aspect_rules_js//js:defs.bzl\", \"js_library\")\n",
        )]);
        assert_eq!(hubs, LockHubs::default());
    }
}
