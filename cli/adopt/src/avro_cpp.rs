//! Plans the files and `MODULE.bazel` lines `avro_cc_library` needs.

use std::path::Path;

use dx_module_bazel::{Call, ModuleFile, Value};

use crate::AdoptError;
use crate::module_pin::ModuleEdit;

const MODULE_FILE: &str = "MODULE.bazel";
const MODULE_NAME: &str = "avro-cpp";
const FMT_MODULE: &str = "fmt";
const CC_MODULE: &str = "rules_cc";
const EXTENSION_BZL: &str = "@rules_dx//avro:extensions.bzl";
const EXTENSION: &str = "avro_cc";
const REPO: &str = "dx_avro_cpp";
const PATCH_DIR: &str = "third_party/rules_dx/avro-cpp";
const PATCH_LABEL: &str = "//third_party/rules_dx/avro-cpp:iterator.patch";
const PATCH: &str = include_str!("../../../patches/avro-cpp/iterator.patch");
const PATCH_BUILD: &str = "exports_files([\"iterator.patch\"])\n";

/// The edits `dx generate --apply` writes for `avro_cc_library`, BUILD and patch before
/// `MODULE.bazel`.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct AvroCpp {
    pub edits: Vec<ModuleEdit>,
    pub notes: Vec<String>,
}

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

/// What the walk of one module root found.
#[derive(Default)]
struct Found {
    avro_cc: bool,
    schema: bool,
}

/// Walks `root` up to nested modules for `avro_cc_library` calls and `.avsc` or `.avdl` files.
fn scan(root: &Path) -> Found {
    let mut found = Found::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if dir != root && dir.join(MODULE_FILE).is_file() {
            continue;
        }
        for name in ["BUILD.bazel", "BUILD"] {
            let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
                continue;
            };
            if ModuleFile::parse_build(&text)
                .ok()
                .is_some_and(|file| file.calls_named("avro_cc_library").next().is_some())
            {
                found.avro_cc = true;
                return found;
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
            if skip {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(entry.path());
            } else if name.ends_with(".avsc") || name.ends_with(".avdl") {
                found.schema = true;
            }
        }
    }
    found
}

/// Whether an `override_repo` of `binding` already binds `dx_avro_cpp`.
fn overrides(file: &ModuleFile, binding: &str) -> bool {
    file.calls_named("override_repo").any(|call| {
        matches!(call.positional.first(), Some(Value::Ident(name)) if name == binding)
            && call.named.iter().any(|(key, _)| key == REPO)
    })
}

/// The `*_override` call for module `name`.
fn module_override<'a>(file: &'a ModuleFile, name: &str) -> Option<&'a Call> {
    file.calls()
        .iter()
        .find(|call| call.name.ends_with("_override") && call.str("module_name") == Some(name))
}

fn append(out: &mut String, block: &str) {
    let eol = if out.contains("\r\n") { "\r\n" } else { "\n" };
    if !out.ends_with('\n') {
        out.push_str(eol);
    }
    if !out.trim().is_empty() {
        out.push_str(eol);
    }
    out.push_str(&block.replace('\n', eol));
}

/// Plans a file under `root` holding `want`, unless it already does.
fn file_edit(root: &Path, path: String, want: &str) -> Option<ModuleEdit> {
    let old = std::fs::read_to_string(root.join(&path)).unwrap_or_default();
    (old != want).then(|| ModuleEdit {
        path,
        old,
        new: want.to_owned(),
    })
}

/// Plans the patched avro-cpp and the fmt `single_version_override`s, and the patch they name.
fn plan_overrides(
    root: &Path,
    out: &mut String,
    edits: &mut Vec<ModuleEdit>,
) -> Result<(), AdoptError> {
    let file = ModuleFile::parse(out).map_err(refuse)?;
    if module_override(&file, MODULE_NAME).is_none() {
        append(
            out,
            &format!(
                "single_version_override(\n    module_name = \"{MODULE_NAME}\",\n    patch_strip = 1,\n    patches = [\"{PATCH_LABEL}\"],\n    version = \"{}\",\n)\n",
                dx_versions::AVRO_CPP_VERSION
            ),
        );
    }
    if module_override(&file, FMT_MODULE).is_none() {
        append(
            out,
            &format!(
                "single_version_override(\n    module_name = \"{FMT_MODULE}\",\n    version = \"{}\",\n)\n",
                dx_versions::FMT_VERSION
            ),
        );
    }
    let file = ModuleFile::parse(out).map_err(refuse)?;
    let patched = module_override(&file, MODULE_NAME)
        .is_some_and(|call| call.str_list("patches").contains(&PATCH_LABEL));
    if patched {
        edits.extend(file_edit(
            root,
            format!("{PATCH_DIR}/BUILD.bazel"),
            PATCH_BUILD,
        ));
        edits.extend(file_edit(
            root,
            format!("{PATCH_DIR}/iterator.patch"),
            PATCH,
        ));
    }
    Ok(())
}

/// Whether `file` declares `module(name = "rules_dx")`.
fn is_rules_dx(file: &ModuleFile) -> bool {
    file.calls_named("module")
        .any(|call| call.str("name") == Some("rules_dx"))
}

/// Plans what a root that has `avro_cc_library`, or Avro schemas Gazelle turns into one,
/// needs. Writes nothing.
pub fn plan_avro_cpp(root: &Path) -> Result<AvroCpp, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(AvroCpp::default());
    };
    let found = scan(root);
    if !found.avro_cc && !found.schema {
        return Ok(AvroCpp::default());
    }
    let mut file = ModuleFile::parse(&text).map_err(refuse)?;
    if is_rules_dx(&file) {
        return Ok(AvroCpp::default());
    }
    if !found.avro_cc && file.bazel_dep(CC_MODULE).is_err() {
        return Ok(AvroCpp::default());
    }
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
        let binding = bound.clone().unwrap_or_else(|| EXTENSION.to_owned());
        let mut block = String::new();
        if bound.is_none() {
            block.push_str(&format!(
                "{binding} = use_extension(\"{EXTENSION_BZL}\", \"{EXTENSION}\")\n"
            ));
        }
        block.push_str(&format!(
            "override_repo({binding}, {REPO} = \"{MODULE_NAME}\")\n"
        ));
        append(&mut out, &block);
    }
    let mut edits = Vec::new();
    plan_overrides(root, &mut out, &mut edits)?;
    ModuleFile::parse(&out).map_err(refuse)?;
    if out != text {
        edits.push(ModuleEdit {
            path: MODULE_FILE.to_owned(),
            old: text,
            new: out,
        });
    }
    Ok(AvroCpp {
        edits,
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

    fn overrides_block() -> String {
        format!(
            "\nsingle_version_override(\n    module_name = \"avro-cpp\",\n    patch_strip = 1,\n    patches = [\"//third_party/rules_dx/avro-cpp:iterator.patch\"],\n    version = \"{}\",\n)\n\nsingle_version_override(\n    module_name = \"fmt\",\n    version = \"{}\",\n)\n",
            dx_versions::AVRO_CPP_VERSION,
            dx_versions::FMT_VERSION
        )
    }

    fn paths(plan: &AvroCpp) -> Vec<&str> {
        plan.edits.iter().map(|edit| edit.path.as_str()).collect()
    }

    fn module_edit(plan: &AvroCpp) -> &ModuleEdit {
        plan.edits
            .iter()
            .find(|edit| edit.path == MODULE_FILE)
            .expect("MODULE.bazel edit")
    }

    fn apply(dir: &Path, plan: &AvroCpp) {
        for edit in &plan.edits {
            write(dir, &edit.path, &edit.new);
        }
    }

    fn fires(build_path: &str, build: &str) -> bool {
        let dir = scratch("avro_cpp_fires");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), build_path, build);
        !plan_avro_cpp(dir.path()).expect("plan").edits.is_empty()
    }

    #[test]
    fn adds_the_dependency_overrides_and_patch_the_first_time() {
        let dir = scratch("avro_cpp_first");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "svc/BUILD.bazel", BUILD);
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        assert_eq!(
            paths(&plan),
            [
                "third_party/rules_dx/avro-cpp/BUILD.bazel",
                "third_party/rules_dx/avro-cpp/iterator.patch",
                MODULE_FILE,
            ]
        );
        assert_eq!(plan.edits[0].new, "exports_files([\"iterator.patch\"])\n");
        assert_eq!(plan.edits[1].new, PATCH);
        let want = format!(
            "{MODULE}bazel_dep(name = \"avro-cpp\", version = \"{}\")\n\navro_cc = use_extension(\"@rules_dx//avro:extensions.bzl\", \"avro_cc\")\noverride_repo(avro_cc, dx_avro_cpp = \"avro-cpp\")\n{}",
            dx_versions::AVRO_CPP_VERSION,
            overrides_block()
        );
        assert_eq!(module_edit(&plan).new, want);
    }

    #[test]
    fn a_second_run_plans_nothing() {
        let dir = scratch("avro_cpp_again");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "svc/BUILD.bazel", BUILD);
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        apply(dir.path(), &plan);
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }

    #[test]
    fn keeps_a_dependency_and_extension_the_user_wrote() {
        let dir = scratch("avro_cpp_user");
        let module = format!(
            "{MODULE}bazel_dep(name = \"avro-cpp\", version = \"1.11.0\")\nx = use_extension(\"@rules_dx//avro:extensions.bzl\", \"avro_cc\")\n"
        );
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "BUILD.bazel", BUILD);
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        assert_eq!(
            module_edit(&plan).new,
            format!(
                "{module}\noverride_repo(x, dx_avro_cpp = \"avro-cpp\")\n{}",
                overrides_block()
            )
        );
    }

    #[test]
    fn keeps_an_override_the_user_wrote_and_writes_no_patch() {
        let dir = scratch("avro_cpp_archive");
        let module = format!(
            "{MODULE}archive_override(\n    module_name = \"avro-cpp\",\n    urls = [\"https://example.com/avro.tar.gz\"],\n)\n"
        );
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "BUILD.bazel", BUILD);
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        assert_eq!(paths(&plan), [MODULE_FILE]);
        let new = &module_edit(&plan).new;
        assert_eq!(new.matches("module_name = \"avro-cpp\"").count(), 1);
        assert!(new.contains("module_name = \"fmt\""));
    }

    #[test]
    fn rewrites_a_stale_patch() {
        let dir = scratch("avro_cpp_stale");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "BUILD.bazel", BUILD);
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        apply(dir.path(), &plan);
        write(
            dir.path(),
            "third_party/rules_dx/avro-cpp/iterator.patch",
            "old\n",
        );
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        assert_eq!(
            paths(&plan),
            ["third_party/rules_dx/avro-cpp/iterator.patch"]
        );
        assert_eq!(plan.edits[0].old, "old\n");
        assert_eq!(plan.edits[0].new, PATCH);
    }

    #[test]
    fn fires_for_a_nested_package() {
        assert!(fires("a/b/c/BUILD.bazel", BUILD));
    }

    #[test]
    fn fires_for_spaced_and_same_line_calls() {
        assert!(fires("BUILD.bazel", "avro_cc_library (name = \"u\")\n"));
        assert!(fires(
            "BUILD",
            "load(\"@rules_dx//avro:defs.bzl\", \"avro_cc_library\"); avro_cc_library(name = \"u\")\n"
        ));
    }

    #[test]
    fn plans_nothing_for_a_nested_module() {
        let dir = scratch("avro_cpp_nested_module");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "sub/MODULE.bazel", "module(name = \"sub\")\n");
        write(dir.path(), "sub/BUILD.bazel", BUILD);
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }

    #[test]
    fn fires_for_a_schema_with_rules_cc_and_no_build_file() {
        let dir = scratch("avro_cpp_schema_rules_cc");
        let module = format!("{MODULE}bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "svc/user.avsc", "{\"type\": \"string\"}\n");
        let plan = plan_avro_cpp(dir.path()).expect("plan");
        assert!(
            module_edit(&plan)
                .new
                .contains("override_repo(avro_cc, dx_avro_cpp = \"avro-cpp\")")
        );
    }

    #[test]
    fn plans_nothing_for_a_schema_without_rules_cc() {
        let dir = scratch("avro_cpp_schema_no_rules_cc");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "svc/user.avsc", "{\"type\": \"string\"}\n");
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }

    #[test]
    fn plans_nothing_for_a_schema_in_a_nested_module() {
        let dir = scratch("avro_cpp_schema_nested_module");
        let module = format!("{MODULE}bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "sub/MODULE.bazel", "module(name = \"sub\")\n");
        write(dir.path(), "sub/user.avdl", "protocol P {}\n");
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }

    #[test]
    fn plans_nothing_in_rules_dx() {
        let dir = scratch("avro_cpp_rules_dx");
        write(dir.path(), MODULE_FILE, "module(name = \"rules_dx\")\n");
        write(dir.path(), "BUILD.bazel", BUILD);
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }

    #[test]
    fn plans_nothing_without_avro_cc_library() {
        let dir = scratch("avro_cpp_none");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "BUILD.bazel", "avro_library(name = \"u\")\n");
        assert_eq!(plan_avro_cpp(dir.path()).expect("plan"), AvroCpp::default());
    }
}
