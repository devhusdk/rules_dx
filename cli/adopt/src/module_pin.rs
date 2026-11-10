//! Edits the version of the rules_dx `bazel_dep` in `MODULE.bazel` or an included file.

use std::path::Path;

use dx_module_bazel::ModuleFile;

use crate::AdoptError;

const MODULE_FILE: &str = "MODULE.bazel";

/// One planned file change: the workspace-relative path and its text before and after.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleEdit {
    pub path: String,
    pub old: String,
    pub new: String,
}

fn refuse(path: &str, detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: path.to_owned(),
        detail: detail.to_string(),
    }
}

fn is_one_line_dep(line: &str) -> bool {
    line.trim_end()
        .strip_prefix("bazel_dep(name = \"rules_dx\", version = \"")
        .and_then(|rest| rest.strip_suffix("\")"))
        .is_some_and(|version| !version.contains('"'))
}

/// Plans setting the rules_dx `bazel_dep` version to `version`. Writes nothing.
pub fn plan_rules_dx_version(root: &Path, version: &str) -> Result<ModuleEdit, AdoptError> {
    if version.is_empty() {
        return Err(AdoptError::EmptyVersion);
    }
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))
    };
    let root_text = read(MODULE_FILE).map_err(|detail| refuse(MODULE_FILE, detail))?;
    let read_include = |label: &str| -> Result<String, dx_error::DxError> {
        let rel = dx_module_bazel::include_rel("", label).map_err(dx_error::DxError::failure)?;
        read(&rel).map_err(dx_error::DxError::failure)
    };
    let files = dx_module_bazel::load_with_includes_labeled(&root_text, &read_include)
        .map_err(|detail| refuse(MODULE_FILE, detail))?;

    let mut found: Vec<(String, &ModuleFile)> = Vec::new();
    for (label, file) in &files {
        let path = match label {
            None => MODULE_FILE.to_owned(),
            Some(label) => dx_module_bazel::include_rel("", label)
                .map_err(|detail| refuse(MODULE_FILE, detail))?,
        };
        let count = file
            .calls()
            .iter()
            .filter(|call| call.name == "bazel_dep" && call.str("name") == Some("rules_dx"))
            .count();
        for _ in 0..count {
            found.push((path.clone(), file));
        }
    }
    match found.as_slice() {
        [] => Err(refuse(
            MODULE_FILE,
            "no bazel_dep(name = \"rules_dx\") in it or its includes",
        )),
        [(path, _)] if path != MODULE_FILE => Err(refuse(
            path,
            "the rules_dx bazel_dep must be in MODULE.bazel itself, not an included file; move it to one line in the root MODULE.bazel",
        )),
        [(path, file)] => {
            let new = file
                .set_bazel_dep_version("rules_dx", version)
                .map_err(|e| refuse(path, e))?;
            let old = read(path).map_err(|detail| refuse(path, detail))?;
            if !old.lines().any(is_one_line_dep) {
                return Err(refuse(
                    path,
                    "the rules_dx bazel_dep must be one line in the root MODULE.bazel: bazel_dep(name = \"rules_dx\", version = \"...\")",
                ));
            }
            Ok(ModuleEdit {
                path: path.clone(),
                old,
                new,
            })
        }
        many => {
            let mut paths: Vec<&str> = many.iter().map(|(path, _)| path.as_str()).collect();
            paths.dedup();
            Err(refuse(
                MODULE_FILE,
                format!(
                    "{} bazel_dep(name = \"rules_dx\") calls, in {}",
                    many.len(),
                    paths.join(", ")
                ),
            ))
        }
    }
}

/// Writes a planned edit, refusing when the file changed since it was planned.
pub fn write_module_edit(root: &Path, edit: &ModuleEdit) -> Result<(), AdoptError> {
    let full = root.join(&edit.path);
    let current = std::fs::read_to_string(&full).map_err(|e| AdoptError::ReadFile {
        path: edit.path.clone(),
        detail: e.to_string(),
    })?;
    if current != edit.old {
        return Err(refuse(&edit.path, "changed since the edit was planned"));
    }
    dx_atomic_fs::write_atomic(&full, edit.new.as_bytes()).map_err(|e| AdoptError::WriteFile {
        path: edit.path.clone(),
        detail: e.to_string(),
    })
}

/// Writes a planned edit; a file planned from nothing is created with its directories.
pub fn write_edit_or_create(root: &Path, edit: &ModuleEdit) -> Result<(), AdoptError> {
    let full = root.join(&edit.path);
    if !edit.old.is_empty() || full.exists() {
        return write_module_edit(root, edit);
    }
    let write_error = |e: std::io::Error| AdoptError::WriteFile {
        path: edit.path.clone(),
        detail: e.to_string(),
    };
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(write_error)?;
    }
    dx_atomic_fs::write_atomic(&full, edit.new.as_bytes()).map_err(write_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};

    const DEP: &str = "bazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";

    fn plan(files: &[(&str, &str)]) -> Result<ModuleEdit, AdoptError> {
        let dir = scratch("module_pin");
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        plan_rules_dx_version(dir.path(), "2.0.0")
    }

    fn message(result: Result<ModuleEdit, AdoptError>) -> String {
        result.expect_err("refused").to_string()
    }

    #[test]
    fn edits_the_dep_in_module_bazel() {
        let edit = plan(&[("MODULE.bazel", DEP)]).expect("plan");
        assert_eq!(edit.path, "MODULE.bazel");
        assert_eq!(edit.old, DEP);
        assert_eq!(edit.new, DEP.replace("1.0.0", "2.0.0"));
    }

    #[test]
    fn refuses_the_dep_in_an_included_file() {
        let root = "include(\"//tools:dx.MODULE.bazel\")\n";
        let err =
            plan(&[("MODULE.bazel", root), ("tools/dx.MODULE.bazel", DEP)]).expect_err("refused");
        assert!(err.to_string().contains("MODULE.bazel itself"), "{err}");
        assert!(err.to_string().contains("one line"), "{err}");
    }

    #[test]
    fn refuses_without_a_dep() {
        let msg = message(plan(&[("MODULE.bazel", "module(name = \"app\")\n")]));
        assert!(
            msg.contains("MODULE.bazel") && msg.contains("no bazel_dep"),
            "{msg}"
        );
    }

    #[test]
    fn refuses_two_deps_in_one_file() {
        let text = format!("{DEP}{DEP}");
        let msg = message(plan(&[("MODULE.bazel", &text)]));
        assert!(msg.contains("2 bazel_dep"), "{msg}");
    }

    #[test]
    fn refuses_one_dep_in_each_of_two_files() {
        let root = format!("include(\"//:more.MODULE.bazel\")\n{DEP}");
        let msg = message(plan(&[("MODULE.bazel", &root), ("more.MODULE.bazel", DEP)]));
        assert!(
            msg.contains("2 bazel_dep") && msg.contains("more.MODULE.bazel"),
            "{msg}"
        );
    }

    #[test]
    fn refuses_a_computed_version() {
        let text = "V = \"1.0.0\"\nbazel_dep(name = \"rules_dx\", version = V)\n";
        let msg = message(plan(&[("MODULE.bazel", text)]));
        assert!(msg.contains("MODULE.bazel"), "{msg}");
    }

    #[test]
    fn refuses_an_empty_version() {
        let dir = scratch("module_pin_empty");
        write(dir.path(), "MODULE.bazel", DEP);
        assert_eq!(
            plan_rules_dx_version(dir.path(), ""),
            Err(AdoptError::EmptyVersion)
        );
    }

    #[test]
    fn refuses_a_multi_line_dep() {
        let text = "bazel_dep(\n    name = \"rules_dx\",\n    version = \"1.0.0\",\n)\n";
        let err = plan(&[("MODULE.bazel", text)]).expect_err("refused");
        assert!(err.to_string().contains("one line"), "{err}");
    }

    #[test]
    fn keeps_comments_and_formatting() {
        let text = "# top\nmodule(name = \"app\")   # trailing\n\n# pin\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n\nbazel_dep(name = \"other\", version = \"1.0.0\")\n";
        let edit = plan(&[("MODULE.bazel", text)]).expect("plan");
        assert_eq!(edit.new, text.replacen("\"1.0.0\"", "\"2.0.0\"", 1));
    }

    #[test]
    fn writes_the_plan_and_rejects_a_stale_one() {
        let dir = scratch("module_pin_write");
        write(dir.path(), "MODULE.bazel", DEP);
        let edit = plan_rules_dx_version(dir.path(), "2.0.0").expect("plan");
        write_module_edit(dir.path(), &edit).expect("write");
        let text = std::fs::read_to_string(dir.path().join("MODULE.bazel")).expect("read");
        assert_eq!(text, edit.new);
        let err = write_module_edit(dir.path(), &edit).expect_err("stale");
        assert!(err.to_string().contains("changed since"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_leaves_module_bazel_unchanged() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("module_pin_failed_write");
        write(dir.path(), "MODULE.bazel", DEP);
        let edit = plan_rules_dx_version(dir.path(), "2.0.0").expect("plan");
        let root = dir.path();
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        let result = write_module_edit(root, &edit);
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(
            result.is_err(),
            "write into a read-only directory must fail"
        );
        let text = std::fs::read_to_string(root.join("MODULE.bazel")).expect("read");
        assert_eq!(text, DEP);
    }
}
