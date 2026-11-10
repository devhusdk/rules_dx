//! Finds the languages a workspace contains from the files in its tree.

use std::collections::BTreeSet;
use std::path::Path;

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

/// Files recognized by exact name, reported alongside the extensions.
const NAMED_FILES: &[&str] = &["CMakeLists.txt"];

/// The lowercased extensions, with their dot, of the files under `root`, plus the names in `NAMED_FILES`.
pub fn file_extensions(root: &Path) -> BTreeSet<String> {
    source_files(root)
        .iter()
        .filter_map(|path| {
            let name = path.rsplit('/').next()?;
            if NAMED_FILES.contains(&name) {
                return Some(name.to_owned());
            }
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

    const MODULE_FILE: &str = "MODULE.bazel";
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

    fn git(dir: &Path) {
        dx_process::capture::run_checked(
            dx_process::capture::git(dir).args(["init", "-q"]),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .expect("git init");
    }

    fn workspace(files: &[&str], with_git: bool) -> dx_test_scratch::TempDir {
        let dir = scratch("families");
        write(dir.path(), MODULE_FILE, MODULE);
        for file in files {
            write(dir.path(), file, "");
        }
        if with_git {
            git(dir.path());
        }
        dir
    }

    #[test]
    fn dx_directory_files_are_not_sources() {
        let dir = workspace(&["A.java", ".dx/config/ruff/extra.toml"], true);
        let extensions = file_extensions(dir.path());
        assert!(extensions.contains(".java"));
        assert!(!extensions.contains(".toml"));
    }

    #[test]
    fn a_tree_without_git_is_walked() {
        let dir = workspace(&["src/A.java", ".git/x.go", "bazel-out/y.go"], false);
        let extensions = file_extensions(dir.path());
        assert!(extensions.contains(".java"));
        assert!(!extensions.contains(".go"));
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
        let extensions = file_extensions(&root);
        assert!(extensions.contains(".java"));
        assert!(!extensions.contains(".go"));
    }

    #[test]
    fn bazelignore_dirs_are_skipped() {
        let dir = workspace(&["vendor/A.java", ".bazelignore"], true);
        write(dir.path(), ".bazelignore", "# vendored\nvendor/\n");
        assert!(!file_extensions(dir.path()).contains(".java"));
    }
}
