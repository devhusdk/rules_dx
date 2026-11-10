//! Plans the `bazel_dep(rules_dx_<family>)` lines for the languages a workspace contains.
//!
//! A family counts when a file with one of its extensions is in the tree. A dep whose files
//! are gone is reported and never removed.

use std::collections::BTreeSet;
use std::path::Path;

use dx_families::Family;
use dx_module_bazel::ModuleFile;

use crate::AdoptError;
use crate::module_pin::ModuleEdit;

const MODULE_FILE: &str = "MODULE.bazel";

/// The planned `MODULE.bazel` edit, if any, and what the planner left alone.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct FamilyDeps {
    pub edit: Option<ModuleEdit>,
    pub notes: Vec<String>,
}

fn is_rules_dx_module(name: &str) -> bool {
    name == "rules_dx" || name.starts_with("rules_dx_")
}

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

/// Plans the family deps of the workspace at `root`. Writes nothing.
pub fn plan_family_deps(root: &Path, families: &[Family]) -> Result<FamilyDeps, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(FamilyDeps::default());
    };
    let read_include = |label: &str| -> Result<String, dx_error::DxError> {
        let rel = dx_module_bazel::include_rel("", label).map_err(dx_error::DxError::failure)?;
        std::fs::read_to_string(root.join(&rel))
            .map_err(|e| dx_error::DxError::failure(format!("cannot read {rel}: {e}")))
    };
    let loaded =
        dx_module_bazel::load_with_includes_labeled(&text, &read_include).map_err(refuse)?;
    let mut deps = BTreeSet::new();
    let mut pin = None;
    for (_, file) in &loaded {
        for call in file.calls() {
            match call.name.as_str() {
                "module" if call.str("name").is_some_and(is_rules_dx_module) => {
                    return Ok(FamilyDeps::default());
                }
                "bazel_dep" => {
                    let Some(name) = call.str("name") else {
                        continue;
                    };
                    if name == "rules_dx" {
                        pin = call.str("version").map(str::to_owned);
                    }
                    deps.insert(name.to_owned());
                }
                _ => {}
            }
        }
    }
    let Some(pin) = pin else {
        return Ok(FamilyDeps::default());
    };

    let extensions = file_extensions(root);
    let mut new_text = text.clone();
    let mut notes = Vec::new();
    for family in families {
        let Some(module) = &family.module else {
            continue;
        };
        let found = family
            .file_extensions
            .iter()
            .any(|ext| extensions.contains(&ext.to_ascii_lowercase()));
        let declared = deps.contains(module);
        if found && !declared {
            new_text = ModuleFile::parse(&new_text)
                .map_err(refuse)?
                .insert_bazel_dep(module, &pin)
                .map_err(refuse)?;
        } else if declared && !found {
            notes.push(format!(
                "{module} is a bazel_dep but no {} files were found; remove it by hand if unused",
                family.file_extensions.join(", ")
            ));
        }
    }
    let edit = (new_text != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new: new_text,
    });
    Ok(FamilyDeps { edit, notes })
}

/// The quality families of `dx.toml` and the files that mean one is present: an extension with
/// its dot, or an exact file name. Families found only by file content are left out.
const QUALITY_FAMILIES: &[(&str, &[&str])] = &[
    ("astro", &[".astro"]),
    ("bicep", &[".bicep"]),
    (
        "cc",
        &[
            ".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".ipp", ".inl",
        ],
    ),
    ("core", &[".yaml", ".yml"]),
    ("d", &[".d"]),
    ("dart", &[".dart"]),
    ("dockerfile", &["Dockerfile", ".dockerfile"]),
    ("fortran", &[".f", ".f03", ".f08", ".f90", ".f95"]),
    ("go", &[".go"]),
    ("go_module", &["go.mod"]),
    ("graphql", &[".gql", ".graphql"]),
    ("haskell", &[".hs"]),
    ("helm", &["Chart.yaml"]),
    ("html", &[".htm", ".html"]),
    ("java", &[".java"]),
    ("javascript", &[".cjs", ".js", ".jsx", ".mjs"]),
    ("json", &[".json"]),
    ("julia", &[".jl"]),
    ("kotlin", &[".kt", ".kts"]),
    ("markdown", &[".md"]),
    ("mdx", &[".mdx"]),
    ("nix", &[".nix"]),
    ("ocaml", &[".ml", ".mli"]),
    ("powershell", &[".ps1", ".psd1", ".psm1"]),
    ("python", &[".py", ".pyi"]),
    ("rust", &[".rs"]),
    ("scala", &[".sc", ".scala"]),
    ("shell", &[".bash", ".sh"]),
    (
        "starlark",
        &[".bazel", ".bzl", ".star", "BUILD", "WORKSPACE"],
    ),
    ("svelte", &[".svelte"]),
    ("swift", &[".swift"]),
    ("text", &[".txt"]),
    ("toml", &[".toml"]),
    ("typescript", &[".cts", ".mts", ".ts", ".tsx"]),
    ("vue", &[".vue"]),
    ("yaml", &[".yaml", ".yml"]),
    ("zig", &[".zig"]),
];

/// The `[families.<id>]` ids of `dx.toml` whose files occur under `root`.
pub fn detected_families(root: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut extensions = BTreeSet::new();
    for path in source_files(root) {
        let name = path.rsplit('/').next().unwrap_or_default();
        if let Some((stem, ext)) = name.rsplit_once('.')
            && !stem.is_empty()
        {
            extensions.insert(format!(".{}", ext.to_ascii_lowercase()));
        }
        names.insert(name.to_owned());
    }
    QUALITY_FAMILIES
        .iter()
        .filter(|(_, marks)| {
            marks.iter().any(|mark| {
                if mark.starts_with('.') {
                    extensions.contains(*mark)
                } else {
                    names.contains(*mark)
                }
            })
        })
        .map(|(id, _)| (*id).to_owned())
        .collect()
}

/// The lowercased extensions, with their dot, of the files under `root`.
pub fn file_extensions(root: &Path) -> BTreeSet<String> {
    source_files(root)
        .iter()
        .filter_map(|path| {
            let name = path.rsplit('/').next()?;
            let (stem, ext) = name.rsplit_once('.')?;
            (!stem.is_empty()).then(|| format!(".{}", ext.to_ascii_lowercase()))
        })
        .collect()
}

/// Whether a BUILD file under `root` declares a `dx_deployment`.
pub(crate) fn has_dx_deployment(root: &Path) -> bool {
    source_files(root).iter().any(|path| {
        let name = path.rsplit('/').next().unwrap_or(path);
        matches!(name, "BUILD" | "BUILD.bazel")
            && std::fs::read_to_string(root.join(path))
                .is_ok_and(|text| text.contains("dx_deployment("))
    })
}

/// The workspace files outside `.dx` and the `.bazelignore`d directories.
fn source_files(root: &Path) -> Vec<String> {
    let ignored = bazelignore(root);
    let mut paths = git_files(root).unwrap_or_else(|| walk(root));
    paths.retain(|path| {
        path != dx_path::state::DX_DIR
            && !path.starts_with(".dx/")
            && !ignored.iter().any(|dir| {
                path.strip_prefix(dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
            })
    });
    paths
}

fn bazelignore(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(".bazelignore")).unwrap_or_default();
    text.lines()
        .map(|line| line.trim().trim_end_matches('/'))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches("./").to_owned())
        .collect()
}

fn git_files(root: &Path) -> Option<Vec<String>> {
    if !root.join(".git").exists() {
        return None;
    }
    let mut git = dx_process::capture::git_isolated(root);
    if let Some(parent) = root
        .canonicalize()
        .ok()
        .and_then(|dir| dir.parent().map(Path::to_path_buf))
    {
        git.env("GIT_CEILING_DIRECTORIES", parent);
    }
    let out = dx_process::capture::run_checked(
        git.args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ]),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .ok()?;
    let text = out.stdout_text();
    Some(
        text.split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// Every file path under `root`, `/`-separated, without following symlinks.
fn walk(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let rel = format!("{prefix}{name}");
            if kind.is_dir() {
                if name != ".git" && name != dx_path::state::DX_DIR && !name.starts_with("bazel-") {
                    pending.push((entry.path(), format!("{rel}/")));
                }
            } else if kind.is_file() {
                files.push(rel);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};

    const MODULE: &str =
        "module(name = \"app\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";

    #[test]
    fn detected_families_are_dx_toml_ids() {
        let dir = scratch("dx-detected-families-");
        assert!(detected_families(dir.path()).is_empty());
        write(dir.path(), "a.py", "");
        write(dir.path(), "src/b.rs", "");
        write(dir.path(), "Dockerfile", "");
        assert_eq!(
            detected_families(dir.path()),
            BTreeSet::from(["dockerfile", "python", "rust"].map(str::to_owned))
        );
    }

    fn table() -> Vec<Family> {
        let family = |name: &str, module: &str, ext: &str| Family {
            name: name.to_owned(),
            module: Some(module.to_owned()),
            file_extensions: vec![ext.to_owned()],
            ..Family::default()
        };
        vec![
            family("go", "rules_dx_go", ".go"),
            family("jvm", "rules_dx_jvm", ".java"),
            family("rust", "rules_dx_rust", ".rs"),
            Family {
                name: "core".to_owned(),
                file_extensions: vec![".md".to_owned()],
                ..Family::default()
            },
        ]
    }

    fn git(dir: &Path) {
        dx_process::capture::run_checked(
            dx_process::capture::git(dir).args(["init", "-q"]),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .expect("git init");
    }

    fn workspace(module: &str, files: &[&str], with_git: bool) -> dx_test_scratch::TempDir {
        let dir = scratch("families");
        write(dir.path(), MODULE_FILE, module);
        for file in files {
            write(dir.path(), file, "");
        }
        if with_git {
            git(dir.path());
        }
        dir
    }

    fn new_text(deps: &FamilyDeps) -> &str {
        &deps.edit.as_ref().expect("an edit").new
    }

    #[test]
    fn adds_a_dep_for_each_language_after_rules_dx() {
        let dir = workspace(MODULE, &["A.java", "b.go", "c.md"], true);
        let deps = plan_family_deps(dir.path(), &table()).expect("plan");
        assert_eq!(
            new_text(&deps),
            format!(
                "{MODULE}bazel_dep(name = \"rules_dx_go\", version = \"1.0.0\")\n\
                 bazel_dep(name = \"rules_dx_jvm\", version = \"1.0.0\")\n"
            )
        );
        assert!(deps.notes.is_empty());
    }

    #[test]
    fn dx_directory_files_are_not_sources() {
        let dir = workspace(MODULE, &["A.java", ".dx/config/ruff/extra.toml"], true);
        let extensions = file_extensions(dir.path());
        assert!(extensions.contains(".java"));
        assert!(!extensions.contains(".toml"));
    }

    #[test]
    fn a_second_run_plans_nothing() {
        let dir = workspace(MODULE, &["A.java", "b.go"], true);
        let deps = plan_family_deps(dir.path(), &table()).expect("plan");
        write(dir.path(), MODULE_FILE, new_text(&deps));
        let again = plan_family_deps(dir.path(), &table()).expect("plan");
        assert_eq!(again, FamilyDeps::default());
    }

    #[test]
    fn a_dep_without_files_is_reported_and_kept() {
        let module = format!("{MODULE}bazel_dep(name = \"rules_dx_go\", version = \"1.0.0\")\n");
        let dir = workspace(&module, &["a.md"], true);
        let deps = plan_family_deps(dir.path(), &table()).expect("plan");
        assert_eq!(
            deps.notes,
            ["rules_dx_go is a bazel_dep but no .go files were found; remove it by hand if unused"]
        );
        assert_eq!(deps.edit, None);
    }

    #[test]
    fn no_rules_dx_dep_plans_nothing() {
        let dir = workspace("module(name = \"app\")\n", &["A.java"], true);
        assert_eq!(
            plan_family_deps(dir.path(), &table()).expect("plan"),
            FamilyDeps::default()
        );
    }

    #[test]
    fn rules_dx_itself_plans_nothing() {
        let module =
            "module(name = \"rules_dx\")\nbazel_dep(name = \"rules_dx\", version = \"1\")\n";
        let dir = workspace(module, &["A.java"], true);
        assert_eq!(
            plan_family_deps(dir.path(), &table()).expect("plan"),
            FamilyDeps::default()
        );
    }

    #[test]
    fn a_rules_dx_family_module_plans_nothing() {
        let module =
            "module(name = \"rules_dx_jvm\")\nbazel_dep(name = \"rules_dx\", version = \"1\")\n";
        let dir = workspace(module, &["A.java"], true);
        assert_eq!(
            plan_family_deps(dir.path(), &table()).expect("plan"),
            FamilyDeps::default()
        );
    }

    #[test]
    fn a_tree_without_git_is_walked() {
        let dir = workspace(MODULE, &["src/A.java", ".git/x.go", "bazel-out/y.go"], false);
        let deps = plan_family_deps(dir.path(), &table()).expect("plan");
        let text = new_text(&deps);
        assert!(text.contains("rules_dx_jvm"), "{text}");
        assert!(!text.contains("rules_dx_go"), "{text}");
    }

    #[test]
    fn a_rust_tree_gets_the_rust_module() {
        let dir = workspace(MODULE, &["src/lib.rs"], false);
        let deps = plan_family_deps(dir.path(), &table()).expect("plan");
        let text = new_text(&deps);
        assert!(text.contains("rules_dx_rust"), "{text}");
        assert!(!text.contains("rules_dx_jvm"), "{text}");
    }

    #[test]
    fn a_broken_git_dir_under_a_parent_repo_is_walked() {
        let outer = scratch("families");
        git(outer.path());
        let root = outer.path().join("app");
        write(&root, MODULE_FILE, MODULE);
        for file in ["src/A.java", ".git/x.go", "bazel-out/y.go"] {
            write(&root, file, "");
        }
        let deps = plan_family_deps(&root, &table()).expect("plan");
        let text = new_text(&deps);
        assert!(text.contains("rules_dx_jvm"), "{text}");
        assert!(!text.contains("rules_dx_go"), "{text}");
    }

    #[test]
    fn bazelignore_dirs_are_skipped() {
        let dir = workspace(MODULE, &["vendor/A.java", ".bazelignore"], true);
        write(dir.path(), ".bazelignore", "# vendored\nvendor/\n");
        assert_eq!(
            plan_family_deps(dir.path(), &table()).expect("plan"),
            FamilyDeps::default()
        );
    }

    #[test]
    fn families_without_a_module_are_never_added() {
        let dir = workspace(MODULE, &["README.md"], true);
        assert_eq!(
            plan_family_deps(dir.path(), &table()).expect("plan"),
            FamilyDeps::default()
        );
    }
}
