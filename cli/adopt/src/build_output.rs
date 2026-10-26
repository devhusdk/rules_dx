use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use crate::git_changes::{
    dedupe_changes, parse_name_status_nul, ChangeKind, DEFAULT_BRANCH_CANDIDATES,
};

/// Largest file a change may add without being listed in the allow file.
pub const MAX_FILE_BYTES: u64 = 1_048_576;

/// Root file that lists oversized paths a change may keep, one per line.
pub const ALLOW_FILE: &str = ".dx-large-files";

/// Directory names that always hold build output.
pub const BUILD_OUTPUT_DIRS: [&str; 4] = ["target", "node_modules", "__pycache__", ".gradle"];

/// Lines `dx init` writes to `.gitignore`.
pub const IGNORE_LINES: [&str; 9] = [
    "target/",
    "/bazel-*",
    "node_modules/",
    "__pycache__/",
    "*.pyc",
    ".gradle/",
    "/build/",
    "/.dx/ide/",
    "/.vscode/.rules_rust_analyzer/",
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
    let Ok(manifest) = std::fs::read_to_string(root.join(parent.join("/")).join("Cargo.toml"))
    else {
        return false;
    };
    manifest.lines().any(|line| {
        line.split_once('=').is_some_and(|(key, value)| {
            key.trim() == "build" && value.trim().trim_matches('"').starts_with("build/")
        })
    })
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

fn allowed(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join(ALLOW_FILE))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

fn is_allowed(allow: &[String], path: &str) -> bool {
    allow.iter().any(|entry| {
        if entry.ends_with('/') {
            path.starts_with(entry.as_str())
        } else {
            entry == path
        }
    })
}

fn classify_kinds(root: &Path, paths: &[(String, bool)]) -> Vec<(BuildOutputFinding, bool)> {
    let allow = allowed(root);
    let mut findings = Vec::new();
    let mut seen_dirs = BTreeSet::new();
    for (path, added) in paths {
        if is_allowed(&allow, path) {
            continue;
        }
        if *added {
            if let Some(dir) = output_dir(root, path) {
                if seen_dirs.insert(dir.clone()) {
                    findings.push((
                        BuildOutputFinding {
                            path: path.clone(),
                            reason: format!("build output under {dir}/"),
                        },
                        false,
                    ));
                }
                continue;
            }
        }
        if let Ok(meta) = std::fs::symlink_metadata(root.join(path)) {
            if meta.is_file() && meta.len() > MAX_FILE_BYTES {
                findings.push((
                    BuildOutputFinding {
                        path: path.clone(),
                        reason: format!(
                            "{} bytes is over the {MAX_FILE_BYTES} byte limit; list it in {ALLOW_FILE} to keep it",
                            meta.len()
                        ),
                    },
                    true,
                ));
            }
        }
    }
    findings
}

/// Classifies changed paths as build output or oversized files. The flag marks added paths.
pub fn classify(root: &Path, paths: &[(String, bool)]) -> Vec<BuildOutputFinding> {
    classify_kinds(root, paths)
        .into_iter()
        .map(|(finding, _)| finding)
        .collect()
}

/// Result of scanning a change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub findings: Vec<BuildOutputFinding>,
    pub committed_skipped: Option<String>,
}

struct Git<'a> {
    root: &'a Path,
    configure: &'a dyn Fn(&mut Command),
}

impl Git<'_> {
    fn run(&self, args: &[&str]) -> Option<std::process::Output> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(self.root).args(args);
        (self.configure)(&mut cmd);
        cmd.output().ok()
    }

    fn ok_text(&self, args: &[&str]) -> Option<String> {
        let output = self.run(args)?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    fn stdout(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let output = self
            .run(args)
            .ok_or_else(|| "git did not start".to_owned())?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let line = stderr.lines().next().unwrap_or("git failed").trim();
        Err(line.to_owned())
    }
}

fn default_base(git: &Git<'_>) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(head) = git.ok_text(&[
        "symbolic-ref",
        "--quiet",
        "--short",
        "refs/remotes/origin/HEAD",
    ]) {
        candidates.push(head);
    }
    candidates.extend(DEFAULT_BRANCH_CANDIDATES.iter().map(|c| (*c).to_owned()));
    candidates.into_iter().find(|candidate| {
        let spec = format!("{candidate}^{{commit}}");
        git.ok_text(&["rev-parse", "--verify", "--quiet", &spec])
            .is_some()
    })
}

fn lfs_paths(git: &Git<'_>, paths: &[String]) -> Result<BTreeSet<String>, String> {
    let mut args = vec!["check-attr", "-z", "filter", "--"];
    args.extend(paths.iter().map(String::as_str));
    let stdout = git.stdout(&args)?;
    let fields: Vec<&[u8]> = stdout.split(|byte| *byte == 0).collect();
    Ok(fields
        .chunks(3)
        .filter(|chunk| chunk.len() == 3 && chunk[2] == b"lfs")
        .filter_map(|chunk| std::str::from_utf8(chunk[0]).ok().map(ToOwned::to_owned))
        .collect())
}

/// Scans the change: untracked files, staged and unstaged edits, and commits since the default branch.
pub fn scan(root: &Path) -> Result<ScanReport, String> {
    scan_with(root, &|_| {})
}

/// `scan` with a hook that configures every git command, so tests can isolate git.
fn scan_with(root: &Path, configure: &dyn Fn(&mut Command)) -> Result<ScanReport, String> {
    let git = &Git { root, configure };
    let mut report = ScanReport::default();
    let Some(inside) = git.run(&["rev-parse", "--is-inside-work-tree"]) else {
        return Ok(report);
    };
    if !inside.status.success() {
        return Ok(report);
    }
    let mut paths: Vec<(String, bool)> = Vec::new();
    let others = git.stdout(&["ls-files", "-z", "--others", "--exclude-standard"])?;
    for field in others.split(|byte| *byte == 0).filter(|f| !f.is_empty()) {
        let path = std::str::from_utf8(field)
            .map_err(|_| "git path is not UTF-8: rename the file to a UTF-8 name".to_owned())?;
        paths.push((path.to_owned(), true));
    }
    let has_head = git
        .ok_text(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .is_some();
    if has_head {
        let mut reference = "HEAD".to_owned();
        match default_base(git).and_then(|base| git.ok_text(&["merge-base", &base, "HEAD"])) {
            Some(base) => reference = base,
            None => {
                report.committed_skipped = Some(
                    "no default branch or merge base found; checked the working tree only"
                        .to_owned(),
                );
            }
        }
        let diff = git.stdout(&[
            "diff",
            "--relative",
            "--name-status",
            "-z",
            "--diff-filter=ACMR",
            &reference,
        ])?;
        for change in dedupe_changes(parse_name_status_nul(&diff)?) {
            let added = matches!(
                change.kind,
                ChangeKind::Added | ChangeKind::Copied | ChangeKind::Renamed
            );
            paths.push((change.path, added));
        }
    } else {
        let cached = git.stdout(&["ls-files", "-z", "--cached"])?;
        for field in cached.split(|byte| *byte == 0).filter(|f| !f.is_empty()) {
            let path = std::str::from_utf8(field)
                .map_err(|_| "git path is not UTF-8: rename the file to a UTF-8 name".to_owned())?;
            paths.push((path.to_owned(), true));
        }
    }
    paths.sort();
    paths.dedup_by(|next, current| next.0 == current.0);
    let found = classify_kinds(root, &paths);
    let oversized: Vec<String> = found
        .iter()
        .filter(|(_, size)| *size)
        .map(|(finding, _)| finding.path.clone())
        .collect();
    let lfs = if oversized.is_empty() {
        BTreeSet::new()
    } else {
        lfs_paths(git, &oversized)?
    };
    report.findings = found
        .into_iter()
        .map(|(finding, _)| finding)
        .filter(|finding| !lfs.contains(&finding.path))
        .collect();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repo directory at `sandbox/repo`; derefs to the repo path.
    struct Fixture {
        _sandbox: dx_test_scratch::TempDir,
        repo: std::path::PathBuf,
    }

    impl std::ops::Deref for Fixture {
        type Target = std::path::PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.repo
        }
    }

    fn sandbox_of(root: &Path) -> &Path {
        root.parent().expect("sandbox")
    }

    fn temp() -> Fixture {
        let sandbox = dx_test_scratch::scratch("dx-build-output-");
        let repo = sandbox
            .path()
            .canonicalize()
            .expect("canonical")
            .join("repo");
        std::fs::create_dir_all(&repo).expect("dir");
        Fixture {
            _sandbox: sandbox,
            repo,
        }
    }

    fn scan_isolated(root: &Path) -> Result<ScanReport, String> {
        let sandbox = sandbox_of(root);
        scan_with(root, &|c| {
            dx_test_scratch::isolate_git(c, sandbox);
        })
    }

    fn names(root: &Path, paths: &[&str]) -> Vec<String> {
        let owned: Vec<(String, bool)> = paths.iter().map(|p| ((*p).to_owned(), true)).collect();
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
        std::fs::write(
            root.join("crate/Cargo.toml"),
            "[package]\nbuild = \"build/script.rs\"\n",
        )
        .expect("file");
        let found = names(&root, &["crate/build/script.rs"]);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn build_output_in_a_cargo_workspace_root_is_rejected() {
        let root = temp();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("file");
        let found = names(&root, &["build/out.o"]);
        assert_eq!(found, ["build/out.o"]);
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
                ("big.bin".to_owned(), true),
                ("kept.bin".to_owned(), true),
                ("small.txt".to_owned(), true),
            ],
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "big.bin");
        assert!(findings[0].reason.contains(ALLOW_FILE));
    }

    fn git(root: &Path, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args);
        dx_test_scratch::isolate_git(&mut cmd, sandbox_of(root));
        let status = cmd.status().expect("git");
        assert!(status.success(), "{args:?}");
    }

    fn write(root: &Path, rel: &str, body: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, body).expect("write");
    }

    fn big() -> Vec<u8> {
        vec![0u8; (MAX_FILE_BYTES + 1) as usize]
    }

    fn repo_on_main() -> Fixture {
        let root = temp();
        git(&root, &["init", "-q", "-b", "main"]);
        write(&root, "README.md", b"x");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        root
    }

    fn found(root: &Path) -> Vec<String> {
        scan_isolated(root)
            .expect("scan")
            .findings
            .into_iter()
            .map(|f| f.path)
            .collect()
    }

    #[test]
    fn source_paths_named_like_output_pass_on_main_and_branches() {
        let root = repo_on_main();
        write(&root, "src/main/java/com/acme/target/Foo.java", b"x");
        write(&root, "build/ci/Dockerfile", b"x");
        write(&root, "testdata/node_modules/pkg/package.json", b"{}");
        git(&root, &["add", "-f", "-A"]);
        git(&root, &["commit", "-q", "-m", "legacy"]);
        git(&root, &["switch", "-q", "-c", "feature"]);
        write(&root, "src/lib.rs", b"x");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "work"]);
        assert!(found(&root).is_empty());
        git(&root, &["switch", "-q", "main"]);
        assert!(scan_isolated(&root)
            .expect("scan")
            .committed_skipped
            .is_none());
    }

    #[test]
    fn output_is_flagged_committed_staged_and_untracked() {
        let root = repo_on_main();
        git(&root, &["switch", "-q", "-c", "feature"]);
        write(&root, "cli/target/debug/dx", b"x");
        assert_eq!(found(&root), ["cli/target/debug/dx"]);
        git(&root, &["add", "-f", "-A"]);
        assert_eq!(found(&root), ["cli/target/debug/dx"]);
        git(&root, &["commit", "-q", "-m", "oops"]);
        assert_eq!(found(&root), ["cli/target/debug/dx"]);
    }

    #[test]
    fn ignored_output_never_counts() {
        let root = repo_on_main();
        write(&root, ".gitignore", b"target/\n");
        write(&root, "target/debug/out", b"x");
        assert!(found(&root).is_empty());
    }

    #[test]
    fn modified_files_are_only_size_checked() {
        let root = repo_on_main();
        write(&root, "target/x", b"x");
        git(&root, &["add", "-f", "-A"]);
        git(&root, &["commit", "-q", "-m", "legacy"]);
        git(&root, &["switch", "-q", "-c", "feature"]);
        write(&root, "target/x", b"y");
        assert!(found(&root).is_empty());
        write(&root, "target/x", &big());
        assert_eq!(found(&root), ["target/x"]);
    }

    #[test]
    fn allow_file_exempts_both_rules() {
        let root = repo_on_main();
        git(&root, &["switch", "-q", "-c", "feature"]);
        write(&root, "target/a", b"x");
        write(&root, "big.bin", &big());
        write(&root, "vendor/big2.bin", &big());
        write(&root, ALLOW_FILE, b"target/a\nvendor/\n");
        assert_eq!(found(&root), ["big.bin"]);
        write(&root, ALLOW_FILE, b"vendor\n");
        assert_eq!(found(&root), ["big.bin", "target/a", "vendor/big2.bin"]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_to_large_files_pass() {
        let root = repo_on_main();
        write(&root, "outside.bin", &big());
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "big"]);
        std::os::unix::fs::symlink("outside.bin", root.join("link.bin")).expect("link");
        assert_eq!(found(&root), Vec::<String>::new());
    }

    #[test]
    fn lfs_files_pass() {
        let root = repo_on_main();
        write(&root, ".gitattributes", b"*.bin filter=lfs\n");
        write(&root, "model.bin", &big());
        write(&root, "other.dat", &big());
        assert_eq!(found(&root), ["other.dat"]);
    }

    #[test]
    fn missing_default_branch_checks_the_working_tree_and_says_so() {
        let root = temp();
        git(&root, &["init", "-q", "-b", "trunk"]);
        write(&root, "a.rs", b"x");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        write(&root, "target/x", b"x");
        let report = scan_isolated(&root).expect("scan");
        assert_eq!(report.findings.len(), 1);
        assert!(report.committed_skipped.is_some());
    }

    #[test]
    fn unborn_head_has_no_note() {
        let root = temp();
        git(&root, &["init", "-q", "-b", "main"]);
        write(&root, "target/x", b"x");
        let report = scan_isolated(&root).expect("scan");
        assert_eq!(report.findings.len(), 1);
        assert!(report.committed_skipped.is_none());
    }

    #[test]
    fn scan_outside_git_finds_nothing() {
        let root = temp();
        let version = Command::new("git")
            .arg("--version")
            .output()
            .expect("git runs");
        assert!(version.status.success(), "git is installed");
        write(&root, "target/x", b"x");
        assert_eq!(scan_isolated(&root), Ok(ScanReport::default()));
    }

    #[test]
    fn corrupt_index_is_an_error() {
        let root = repo_on_main();
        std::fs::write(root.join(".git/index"), b"garbage").expect("index");
        let error = scan_isolated(&root).expect_err("error");
        assert!(!error.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_names_are_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let root = repo_on_main();
        let name = std::ffi::OsStr::from_bytes(b"bad\xff.txt");
        std::fs::write(root.join(name), b"x").expect("write");
        assert!(scan_isolated(&root).is_err());
    }

    #[test]
    fn init_ignore_lines_cover_the_build_directories() {
        for dir in ["target", "node_modules", "__pycache__", ".gradle"] {
            assert!(IGNORE_LINES.contains(&format!("{dir}/").as_str()), "{dir}");
        }
        for line in ["/.dx/ide/", "/.vscode/.rules_rust_analyzer/"] {
            assert!(IGNORE_LINES.contains(&line), "{line}");
        }
    }

    #[test]
    fn isolation_overrides_ambient_excludes() {
        let root = repo_on_main();
        let sandbox = sandbox_of(&root);
        let ambient = sandbox.join("ambient");
        write(&ambient, "ignore", b"target/\n");
        write(
            &ambient,
            "gitconfig",
            format!(
                "[core]\n\texcludesFile = {}\n",
                ambient.join("ignore").display()
            )
            .as_bytes(),
        );
        write(&root, "cli/target/debug/dx", b"x");
        let report = scan_with(&root, &|c| {
            c.env("GIT_CONFIG_GLOBAL", ambient.join("gitconfig"));
            c.env("XDG_CONFIG_HOME", &ambient);
            dx_test_scratch::isolate_git(c, sandbox);
        })
        .expect("scan");
        let paths: Vec<String> = report.findings.into_iter().map(|f| f.path).collect();
        assert_eq!(paths, ["cli/target/debug/dx"]);
    }
}
