//! Plans a `bazel_dep` for each rule set the workspace's BUILD files load from that the root
//! module does not declare, at the version rules_dx itself depends on, and the TypeScript
//! compiler repository `ts_project` reads when TypeScript rules are loaded.

use std::collections::BTreeSet;
use std::path::Path;

use dx_module_bazel::ModuleFile;

use crate::AdoptError;
use crate::load_graph::build_file_labels;
use crate::lock_hubs::LockHubs;
use crate::module_deps::{is_rules_dx_root, module_deps};
use crate::module_pin::ModuleEdit;
use crate::preflight::RULES_DX_DEPS;

const MODULE_FILE: &str = "MODULE.bazel";
const RULES_DX: &str = "rules_dx";
const TS_MODULE: &str = "aspect_rules_ts";
const TS_COMPILER: &str = "npm_typescript";
const TS_EXTENSION: &str = "typescript";

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
    let labels = build_file_labels(root).map_err(refuse)?;
    let loaded: BTreeSet<&str> = labels
        .iter()
        .filter_map(|label| label_repo(label))
        .collect();
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
    let dx_repo = ModuleFile::parse(&text)
        .map_err(refuse)?
        .repo_name(RULES_DX)
        .unwrap_or(RULES_DX)
        .to_owned();
    let ts_prefix = format!("@{dx_repo}//{TS_EXTENSION}:");
    let loads_ts = loaded.contains(TS_MODULE)
        || labels.iter().any(|label| {
            label.starts_with(&ts_prefix) || label.starts_with(&format!("@{ts_prefix}"))
        });
    if loads_ts && !is_rules_dx_root(root) && !imports_ts_compiler(root, &text)? {
        new = ModuleFile::parse(&new)
            .and_then(|file| {
                file.insert_extension_block(
                    TS_EXTENSION,
                    &format!("@{dx_repo}//{TS_EXTENSION}:extensions.bzl"),
                    TS_EXTENSION,
                    &[],
                    &[TS_COMPILER],
                )
            })
            .map_err(refuse)?;
        notes.push(format!(
            "BUILD files load TypeScript rules; {MODULE_FILE} needs use_repo {TS_COMPILER} from the rules_dx {TS_EXTENSION} extension"
        ));
    }
    let edit = (new != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new,
    });
    Ok(LockHubs { edit, notes })
}

fn label_repo(label: &str) -> Option<&str> {
    let rest = label.strip_prefix('@')?;
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    Some(rest.split_once("//").map_or(rest, |(repo, _)| repo))
}

fn imports_ts_compiler(root: &Path, text: &str) -> Result<bool, AdoptError> {
    let read_include = |label: &str| -> Result<String, AdoptError> {
        let rel = dx_module_bazel::include_rel("", label).map_err(refuse)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| refuse(format!("cannot read included {label}: {e}")))
    };
    let modules = dx_module_bazel::load_with_includes(text, &read_include)
        .map_err(|error| error.into_error(refuse))?;
    let extension = format!("@{RULES_DX}//{TS_EXTENSION}:extensions.bzl");
    Ok(modules.iter().any(|module| {
        module
            .hub_bindings(&extension, TS_EXTENSION)
            .iter()
            .any(|(binding, _)| {
                module
                    .use_repos(binding)
                    .iter()
                    .any(|(repo, _)| *repo == TS_COMPILER)
            })
    }))
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
                "{MODULE}bazel_dep(name = \"aspect_rules_js\", version = \"{}\")\nbazel_dep(name = \"aspect_rules_ts\", version = \"{}\")\n{TS_BLOCK}",
                version("aspect_rules_js"),
                version("aspect_rules_ts"),
            )
        );
        assert_eq!(hubs.notes.len(), 3, "{:?}", hubs.notes);
    }

    const TS_BLOCK: &str = "\ntypescript = use_extension(\"@rules_dx//typescript:extensions.bzl\", \"typescript\")\nuse_repo(typescript, \"npm_typescript\")\n";

    #[test]
    fn rules_dx_typescript_loads_import_the_compiler_repository() {
        let hubs = plan(&[
            ("MODULE.bazel", MODULE),
            (
                "web/BUILD.bazel",
                "load(\"@rules_dx//typescript:defs.bzl\", \"typescript_library\")\n",
            ),
        ]);
        let edit = hubs.edit.expect("edit");
        assert_eq!(edit.new, format!("{MODULE}{TS_BLOCK}"));
    }

    #[test]
    fn an_imported_compiler_repository_is_left_alone() {
        for module in [
            format!("{MODULE}include(\"//:ts.MODULE.bazel\")\n"),
            format!(
                "{MODULE}bazel_dep(name = \"aspect_rules_ts\", version = \"{}\")\nts = use_extension(\"@aspect_rules_ts//ts:extensions.bzl\", \"typescript\")\nuse_repo(ts, \"npm_typescript\")\n",
                version("aspect_rules_ts")
            ),
        ] {
            let hubs = plan(&[
                ("MODULE.bazel", module.as_str()),
                ("ts.MODULE.bazel", TS_BLOCK),
                (
                    "web/BUILD.bazel",
                    "load(\"@aspect_rules_ts//ts:defs.bzl\", \"ts_project\")\nload(\"@rules_dx//typescript:defs.bzl\", \"typescript_library\")\n",
                ),
            ]);
            let new = hubs.edit.map(|edit| edit.new);
            assert!(
                new.as_deref()
                    .is_none_or(|text| !text.contains("typescript = use_extension")),
                "{new:?}"
            );
        }
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
