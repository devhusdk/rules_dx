//! Plans the `MODULE.bazel` lines that `avro_cc_library` needs: the `avro-cpp` dependency and the
//! `override_repo` that binds `dx_avro_cpp` to it. A line that is already there is left alone,
//! so a second run plans nothing.

use std::path::Path;

use dx_module_bazel::{ModuleFile, Value};

use crate::AdoptError;
use crate::lock_hubs::LockHubs;
use crate::module_pin::ModuleEdit;

const MODULE_FILE: &str = "MODULE.bazel";
const MODULE_NAME: &str = "avro-cpp";
const EXTENSION_BZL: &str = "@rules_dx//avro:extensions.bzl";
const EXTENSION: &str = "avro_cc";
const REPO: &str = "dx_avro_cpp";

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

/// Whether a BUILD file under `root` calls `avro_cc_library`.
fn uses_avro_cc(root: &Path) -> bool {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if dir != root && dir.join(MODULE_FILE).is_file() {
            continue;
        }
        for name in ["BUILD.bazel", "BUILD"] {
            if std::fs::read_to_string(dir.join(name)).is_ok_and(|text| {
                text.lines()
                    .any(|line| line.trim_start().starts_with("avro_cc_library("))
            }) {
                return true;
            }
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let skip = name.starts_with('.')
                || name.starts_with("bazel-")
                || matches!(name.as_str(), "node_modules" | "target");
            if !skip && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(entry.path());
            }
        }
    }
    false
}

/// Whether an `override_repo` of `binding` already binds `dx_avro_cpp`.
fn overrides(file: &ModuleFile, binding: &str) -> bool {
    file.calls_named("override_repo").any(|call| {
        matches!(call.positional.first(), Some(Value::Ident(name)) if name == binding)
            && call.named.iter().any(|(key, _)| key == REPO)
    })
}

/// Plans the `avro-cpp` dependency and `override_repo` for a root that has `avro_cc_library`.
/// Writes nothing.
pub fn plan_avro_cpp(root: &Path) -> Result<LockHubs, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(LockHubs::default());
    };
    if !uses_avro_cc(root) {
        return Ok(LockHubs::default());
    }
    let mut file = ModuleFile::parse(&text).map_err(refuse)?;
    let mut out = text.clone();
    if file.bazel_dep(MODULE_NAME).is_err() {
        out = file
            .insert_bazel_dep(MODULE_NAME, dx_versions::AVRO_CPP_VERSION)
            .map_err(refuse)?;
        file = ModuleFile::parse(&out).map_err(refuse)?;
    }
    let existing = file.extension_bindings(EXTENSION_BZL, EXTENSION);
    let bound = existing.first().map(|(name, _)| (*name).to_owned());
    if !bound.as_deref().is_some_and(|name| overrides(&file, name)) {
        let eol = if out.contains("\r\n") { "\r\n" } else { "\n" };
        let binding = bound.clone().unwrap_or_else(|| EXTENSION.to_owned());
        if !out.ends_with('\n') {
            out.push_str(eol);
        }
        if !out.trim().is_empty() {
            out.push_str(eol);
        }
        if bound.is_none() {
            out.push_str(&format!(
                "{binding} = use_extension(\"{EXTENSION_BZL}\", \"{EXTENSION}\"){eol}"
            ));
        }
        out.push_str(&format!(
            "override_repo({binding}, {REPO} = \"{MODULE_NAME}\"){eol}"
        ));
        ModuleFile::parse(&out).map_err(refuse)?;
    }
    let edit = (out != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new: out,
    });
    Ok(LockHubs {
        edit,
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};

    const MODULE: &str =
        "module(name = \"app\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";
    const BUILD: &str = "avro_cc_library(\n    name = \"user_cc\",\n)\n";

    fn apply(dir: &Path, edit: &ModuleEdit) {
        write(dir, MODULE_FILE, &edit.new);
    }

    #[test]
    fn adds_the_dependency_and_override_the_first_time() {
        let dir = scratch("avro_cpp_first");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "svc/BUILD.bazel", BUILD);
        let edit = plan_avro_cpp(dir.path()).expect("plan").edit.expect("edit");
        let want = format!(
            "{MODULE}bazel_dep(name = \"avro-cpp\", version = \"{}\")\n\navro_cc = use_extension(\"@rules_dx//avro:extensions.bzl\", \"avro_cc\")\noverride_repo(avro_cc, dx_avro_cpp = \"avro-cpp\")\n",
            dx_versions::AVRO_CPP_VERSION
        );
        assert_eq!(edit.new, want);
    }

    #[test]
    fn a_second_run_plans_nothing() {
        let dir = scratch("avro_cpp_again");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "svc/BUILD.bazel", BUILD);
        let edit = plan_avro_cpp(dir.path()).expect("plan").edit.expect("edit");
        apply(dir.path(), &edit);
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), LockHubs::default());
    }

    #[test]
    fn keeps_a_dependency_and_extension_the_user_wrote() {
        let dir = scratch("avro_cpp_user");
        let module = format!(
            "{MODULE}bazel_dep(name = \"avro-cpp\", version = \"1.11.0\")\nx = use_extension(\"@rules_dx//avro:extensions.bzl\", \"avro_cc\")\n"
        );
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "BUILD.bazel", BUILD);
        let edit = plan_avro_cpp(dir.path()).expect("plan").edit.expect("edit");
        assert_eq!(
            edit.new,
            format!("{module}\noverride_repo(x, dx_avro_cpp = \"avro-cpp\")\n")
        );
    }

    #[test]
    fn plans_nothing_without_avro_cc_library() {
        let dir = scratch("avro_cpp_none");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "BUILD.bazel", "avro_library(name = \"u\")\n");
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), LockHubs::default());
    }
}
