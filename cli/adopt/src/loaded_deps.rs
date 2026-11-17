//! Plans a `bazel_dep` for each rule set the workspace's BUILD files load from that the root
//! module does not declare, at the version `rules_dx` itself depends on, and the TypeScript
//! compiler repository `ts_project` reads when BUILD files load `aspect_rules_ts`. BUILD files that load
//! from an `npm_translate_lock` hub get `aspect_rules_js`, whose platform labels the hub's links
//! select on.

use std::collections::BTreeSet;
use std::path::Path;

use dx_module_bazel::{Arg, ModuleFile, NewTag};
use dx_path::label::repo_of;

use crate::AdoptError;
use crate::load_graph::build_file_labels;
use crate::lock_hubs::LockHubs;
use crate::module_deps::{is_rules_dx_root, module_deps};
use crate::module_pin::ModuleEdit;
use crate::preflight::RULES_DX_DEPS;

const MODULE_FILE: &str = "MODULE.bazel";
const RULES_DX: &str = "rules_dx";
const JS_MODULE: &str = "aspect_rules_js";
const TS_MODULE: &str = "aspect_rules_ts";
const TS_COMPILER: &str = "npm_typescript";
const TS_EXTENSION: &str = "typescript";
const TS_BINDING: &str = "rules_ts";

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
    let loaded: BTreeSet<&str> = labels.iter().filter_map(|label| repo_of(label)).collect();
    let modules = included_modules(root, &text)?;
    let links_npm = modules.iter().any(|module| {
        module
            .npm_translate_locks()
            .unwrap_or_default()
            .iter()
            .any(|lock| loaded.contains(lock.name))
    });
    let mut new = text.clone();
    let mut notes = Vec::new();
    for (name, version) in RULES_DX_DEPS {
        let needed = loaded.contains(*name) || (*name == JS_MODULE && links_npm);
        if !needed || declared.contains(*name) {
            continue;
        }
        new = ModuleFile::parse(&new)
            .and_then(|file| file.insert_bazel_dep(name, version))
            .map_err(refuse)?;
        notes.push(format!(
            "BUILD files load from {name}; {MODULE_FILE} needs bazel_dep {name} {version}"
        ));
    }
    if loaded.contains(TS_MODULE) && !is_rules_dx_root(root) && !imports_ts_compiler(&modules) {
        let ts_repo = ModuleFile::parse(&new)
            .map_err(refuse)?
            .repo_name(TS_MODULE)
            .unwrap_or(TS_MODULE)
            .to_owned();
        let deps = NewTag {
            name: "deps".to_owned(),
            args: vec![
                (
                    "integrity".to_owned(),
                    Arg::Str(dx_versions::TYPESCRIPT_INTEGRITY.to_owned()),
                ),
                (
                    "version".to_owned(),
                    Arg::Str(dx_versions::TYPESCRIPT_VERSION.to_owned()),
                ),
            ],
        };
        new = ModuleFile::parse(&new)
            .and_then(|file| {
                file.insert_extension_block(
                    TS_BINDING,
                    &format!("@{ts_repo}//ts:extensions.bzl"),
                    TS_EXTENSION,
                    &[deps],
                    &[TS_COMPILER],
                )
            })
            .map_err(refuse)?;
        notes.push(format!(
            "BUILD files load TypeScript rules; {MODULE_FILE} needs use_repo {TS_COMPILER} from the {TS_MODULE} {TS_EXTENSION} extension"
        ));
    }
    let edit = (new != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new,
    });
    Ok(LockHubs { edit, notes })
}

fn included_modules(root: &Path, text: &str) -> Result<Vec<ModuleFile>, AdoptError> {
    let read_include = |label: &str| -> Result<String, AdoptError> {
        let rel = dx_module_bazel::include_rel("", label).map_err(refuse)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| refuse(format!("cannot read included {label}: {e}")))
    };
    dx_module_bazel::load_with_includes(text, &read_include)
        .map_err(|error| error.into_error(refuse))
}

fn imports_ts_compiler(modules: &[ModuleFile]) -> bool {
    let extension = format!("@{RULES_DX}//{TS_EXTENSION}:extensions.bzl");
    modules.iter().any(|module| {
        module
            .hub_bindings(&extension, TS_EXTENSION)
            .iter()
            .any(|(binding, _)| {
                module
                    .use_repos(binding)
                    .iter()
                    .any(|(repo, _)| *repo == TS_COMPILER)
            })
    })
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
                "load(\"@aspect_rules_js//js:defs.bzl\", \"js_library\")\nload(\"@vendored//:defs.bzl\", \"npm_link_all_packages\")\n",
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
                "{MODULE}bazel_dep(name = \"aspect_rules_js\", version = \"{}\")\nbazel_dep(name = \"aspect_rules_ts\", version = \"{}\")\n{}",
                version("aspect_rules_js"),
                version("aspect_rules_ts"),
                ts_block(),
            )
        );
        assert_eq!(hubs.notes.len(), 3, "{:?}", hubs.notes);
    }

    fn ts_block() -> String {
        format!(
            "\nrules_ts = use_extension(\"@aspect_rules_ts//ts:extensions.bzl\", \"typescript\")\nrules_ts.deps(\n    integrity = \"{}\",\n    version = \"{}\",\n)\nuse_repo(rules_ts, \"npm_typescript\")\n",
            dx_versions::TYPESCRIPT_INTEGRITY,
            dx_versions::TYPESCRIPT_VERSION,
        )
    }

    #[test]
    fn rules_dx_typescript_loads_need_no_module_lines() {
        let hubs = plan(&[
            ("MODULE.bazel", MODULE),
            (
                "web/BUILD.bazel",
                "load(\"@rules_dx//typescript:defs.bzl\", \"typescript_library\")\n",
            ),
        ]);
        assert!(hubs.edit.is_none(), "{:?}", hubs.edit.map(|edit| edit.new));
        assert!(hubs.notes.is_empty(), "{:?}", hubs.notes);
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
                (
                    "ts.MODULE.bazel",
                    "typescript = use_extension(\"@rules_dx//typescript:extensions.bzl\", \"typescript\")\nuse_repo(typescript, \"npm_typescript\")\n",
                ),
                (
                    "web/BUILD.bazel",
                    "load(\"@aspect_rules_ts//ts:defs.bzl\", \"ts_project\")\nload(\"@rules_dx//typescript:defs.bzl\", \"typescript_library\")\n",
                ),
            ]);
            let new = hubs.edit.map(|edit| edit.new);
            assert!(
                new.as_deref()
                    .is_none_or(|text| !text.contains("rules_ts = use_extension")),
                "{new:?}"
            );
        }
    }

    #[test]
    fn loads_from_an_npm_hub_get_aspect_rules_js() {
        let module = format!("{MODULE}include(\"//:js.MODULE.bazel\")\n");
        let hubs = plan(&[
            ("MODULE.bazel", module.as_str()),
            (
                "js.MODULE.bazel",
                "npm = use_extension(\"@rules_dx//javascript:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"npm\",\n    pnpm_lock = \"//:pnpm-lock.yaml\",\n)\nuse_repo(npm, \"npm\")\n",
            ),
            (
                "BUILD.bazel",
                "load(\"@npm//:defs.bzl\", \"npm_link_all_packages\")\n",
            ),
        ]);
        let edit = hubs.edit.expect("edit");
        assert_eq!(
            edit.new,
            format!(
                "{MODULE}bazel_dep(name = \"aspect_rules_js\", version = \"{}\")\ninclude(\"//:js.MODULE.bazel\")\n",
                version("aspect_rules_js")
            )
        );
    }

    #[test]
    fn an_npm_hub_nothing_loads_from_adds_no_dep() {
        let module = format!(
            "{MODULE}npm = use_extension(\"@rules_dx//javascript:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"npm\",\n    pnpm_lock = \"//:pnpm-lock.yaml\",\n)\nuse_repo(npm, \"npm\")\n"
        );
        let hubs = plan(&[
            ("MODULE.bazel", module.as_str()),
            (
                "BUILD.bazel",
                "load(\"@rules_dx//rust:defs.bzl\", \"rust_library\")\n",
            ),
        ]);
        assert_eq!(hubs.edit, None);
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
