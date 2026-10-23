use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Largest file a change may add without being listed in the allow file.
pub const MAX_FILE_BYTES: u64 = 1_048_576;

/// Root file that lists oversized paths a change may keep, one per line.
pub const ALLOW_FILE: &str = ".dx-large-files";

/// Directory names that always hold build output.
pub const BUILD_OUTPUT_DIRS: [&str; 4] = ["target", "node_modules", "__pycache__", ".gradle"];

/// Lines `dx init` writes to `.gitignore`.
pub const IGNORE_LINES: [&str; 7] = [
    "target/",
    "/bazel-*",
    "node_modules/",
    "__pycache__/",
    "*.pyc",
    ".gradle/",
    "/build/",
];

const SOURCE_MANIFESTS: [&str; 9] = [
    "BUILD",
    "BUILD.bazel",
    "Cargo.toml",
    "package.json",
    "pom.xml",
    "go.mod",
    "pyproject.toml",
    "CMakeLists.txt",
    "build.gradle",
];

/// One path that must not be part of a change, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOutputFinding {
    pub path: String,
    pub reason: String,
}

fn has_manifest(root: &Path, dir: &str) -> bool {
    let base = root.join(dir);
    SOURCE_MANIFESTS
        .iter()
        .any(|name| base.join(name).is_file())
}

fn cargo_build_dir(root: &Path, parent: &[&str]) -> bool {
    root.join(parent.join("/")).join("Cargo.toml").is_file()
}

fn output_dir(root: &Path, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    for (index, part) in parts.iter().enumerate().take(parts.len().saturating_sub(1)) {
        let dir = parts[..=index].join("/");
        if part.starts_with("bazel-") && index == 0 {
            return Some(dir);
        }
        if BUILD_OUTPUT_DIRS.contains(part) {
            return Some(dir);
        }
        if *part == "build" && !has_manifest(root, &dir) && !cargo_build_dir(root, &parts[..index])
        {
            return Some(dir);
        }
    }
    None
}

fn allowed(root: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(root.join(ALLOW_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

/// Classifies workspace paths as build output or oversized files.
pub fn classify(root: &Path, paths: &[String]) -> Vec<BuildOutputFinding> {
    let allow = allowed(root);
    let mut findings = Vec::new();
    let mut seen_dirs = BTreeSet::new();
    for path in paths {
        if let Some(dir) = output_dir(root, path) {
            if seen_dirs.insert(dir.clone()) {
                findings.push(BuildOutputFinding {
                    path: path.clone(),
                    reason: format!("build output under {dir}/"),
                });
            }
            continue;
        }
        if allow.contains(path) {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(root.join(path)) {
            if meta.is_file() && meta.len() > MAX_FILE_BYTES {
                findings.push(BuildOutputFinding {
                    path: path.clone(),
                    reason: format!(
                        "{} bytes is over the {MAX_FILE_BYTES} byte limit; list it in {ALLOW_FILE} to keep it",
                        meta.len()
                    ),
                });
            }
        }
    }
    findings
}

/// Scans tracked and untracked unignored files. Returns nothing outside a Git checkout.
pub fn scan(root: &Path) -> Vec<BuildOutputFinding> {
    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut paths: Vec<String> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .filter_map(|field| std::str::from_utf8(field).ok().map(ToOwned::to_owned))
        .collect();
    paths.sort();
    paths.dedup();
    classify(root, &paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dx-build-output-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn names(root: &Path, paths: &[&str]) -> Vec<String> {
        let owned: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        classify(root, &owned).into_iter().map(|f| f.path).collect()
    }

    #[test]
    fn known_build_directories_are_rejected() {
        let root = temp();
        let found = names(
            &root,
            &[
                "cli/target/debug/dx",
                "cli/target/debug/dx.d",
                "bazel-out/x",
                "web/node_modules/a/index.js",
                "py/__pycache__/m.pyc",
                "app/.gradle/x",
                "build/out.o",
                "src/main.rs",
            ],
        );
        assert_eq!(
            found,
            [
                "cli/target/debug/dx",
                "bazel-out/x",
                "web/node_modules/a/index.js",
                "py/__pycache__/m.pyc",
                "app/.gradle/x",
                "build/out.o"
            ]
        );
    }

    #[test]
    fn build_directories_inside_source_trees_pass() {
        let root = temp();
        std::fs::create_dir_all(root.join("tools/site/build")).expect("dir");
        std::fs::write(root.join("tools/site/build/BUILD.bazel"), "").expect("file");
        let found = names(&root, &["tools/site/build/src/lib.rs"]);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn build_directory_beside_a_cargo_manifest_passes() {
        let root = temp();
        std::fs::create_dir_all(root.join("crate")).expect("dir");
        std::fs::write(root.join("crate/Cargo.toml"), "").expect("file");
        let found = names(&root, &["crate/build/script.rs"]);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn build_output_next_to_a_manifest_is_rejected() {
        let root = temp();
        std::fs::write(root.join("BUILD.bazel"), "").expect("file");
        std::fs::create_dir_all(root.join("app")).expect("dir");
        std::fs::write(root.join("app/build.gradle"), "").expect("file");
        let found = names(&root, &["build/out.o", "app/build/outputs/app.apk"]);
        assert_eq!(found, ["build/out.o", "app/build/outputs/app.apk"]);
    }

    #[test]
    fn oversized_files_need_an_allow_entry() {
        let root = temp();
        let big = vec![0u8; (MAX_FILE_BYTES + 1) as usize];
        std::fs::write(root.join("big.bin"), &big).expect("big");
        std::fs::write(root.join("kept.bin"), &big).expect("kept");
        std::fs::write(root.join("small.txt"), "x").expect("small");
        std::fs::write(root.join(ALLOW_FILE), "# kept\nkept.bin\n").expect("allow");
        let findings = classify(
            &root,
            &[
                "big.bin".to_owned(),
                "kept.bin".to_owned(),
                "small.txt".to_owned(),
            ],
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "big.bin");
        assert!(findings[0].reason.contains(ALLOW_FILE));
    }

    #[test]
    fn scan_reads_git_tracked_and_untracked_files() {
        let root = temp();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .status()
                .expect("git");
            assert!(status.success());
        };
        git(&["init", "-q"]);
        std::fs::create_dir_all(root.join("target/debug")).expect("dir");
        std::fs::write(root.join("target/debug/out"), "x").expect("out");
        std::fs::write(root.join("a.rs"), "x").expect("src");
        let found = scan(&root);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "target/debug/out");
        std::fs::write(root.join(".gitignore"), "target/\n").expect("ignore");
        assert!(scan(&root).is_empty());
    }

    #[test]
    fn scan_outside_git_finds_nothing() {
        let root = temp();
        std::fs::create_dir_all(root.join("target")).expect("dir");
        std::fs::write(root.join("target/x"), "x").expect("x");
        assert!(scan(&root).is_empty());
    }

    #[test]
    fn init_ignore_lines_cover_the_build_directories() {
        for dir in ["target", "node_modules", "__pycache__", ".gradle"] {
            assert!(IGNORE_LINES.contains(&format!("{dir}/").as_str()), "{dir}");
        }
    }
}
