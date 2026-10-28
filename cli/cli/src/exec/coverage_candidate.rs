//! The commit a coverage run measured and the tracked sources no Bazel rule uses.

use super::common::git_output;
use dx_plan::resolve::{MemoizedGraphQuery, QueryRunner, RuleGraph};
use std::collections::HashMap;
use std::path::Path;

/// The checkout a coverage run measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) sha: String,
    /// True when a tracked file differs from `HEAD`.
    pub(super) dirty: bool,
}

/// Why the tracked-source check could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Unavailable {
    /// The workspace is not a Git work tree; this is never enforced.
    NotGit,
    /// Git or the rule-graph query failed inside a work tree; this is an accounting error.
    Failed(String),
}

fn inside_work_tree(workspace: &Path, runner: &dyn QueryRunner) -> bool {
    git_output(
        runner,
        workspace,
        "rev-parse --is-inside-work-tree",
        &["rev-parse", "--is-inside-work-tree"],
    )
    .is_ok_and(|out| String::from_utf8_lossy(&out).trim() == "true")
}

/// The `HEAD` commit and whether tracked files differ from it, or None outside a work tree.
pub(super) fn candidate(workspace: &Path, runner: &dyn QueryRunner) -> Option<Candidate> {
    if !inside_work_tree(workspace, runner) {
        return None;
    }
    let sha = git_output(
        runner,
        workspace,
        "rev-parse HEAD",
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
    )
    .ok()?;
    let sha = String::from_utf8_lossy(&sha).trim().to_owned();
    if sha.is_empty() {
        return None;
    }
    let status = git_output(
        runner,
        workspace,
        "status",
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .ok()?;
    Some(Candidate {
        sha,
        dirty: !status.iter().all(u8::is_ascii_whitespace),
    })
}

fn has_package(workspace: &Path, file: &str, cache: &mut HashMap<String, bool>) -> bool {
    let mut dir = file.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut walked: Vec<&str> = Vec::new();
    let found = loop {
        if let Some(known) = cache.get(dir) {
            break *known;
        }
        walked.push(dir);
        if dx_path::bazel_package::is_package_dir(&workspace.join(dir)) {
            break true;
        }
        if dir.is_empty() {
            break false;
        }
        dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
    };
    for seen in walked {
        cache.insert(seen.to_owned(), found);
    }
    found
}

/// Tracked covered-language files in a Bazel package that no rule of the workspace graph uses.
///
/// The graph is the unconfigured `kind('rule', //...)` query, so every `select()` branch and
/// every manual rule counts as a user. Files outside any package and `.bazelignore` trees are skipped.
pub(super) fn tracked_orphans(
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, Unavailable> {
    if !inside_work_tree(workspace, runner) {
        return Err(Unavailable::NotGit);
    }
    let listed = git_output(
        runner,
        workspace,
        "ls-files",
        &["ls-files", "-z", "--cached"],
    )
    .map_err(Unavailable::Failed)?;
    let tracked: Vec<String> = listed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect();
    let ignored = dx_adopt::load_graph::read_bazelignore(workspace).map_err(Unavailable::Failed)?;
    let mut cache = HashMap::new();
    let candidates: Vec<String> = dx_lcov::orphan_candidates(&tracked, &ignored)
        .into_iter()
        .filter(|path| has_package(workspace, path, &mut cache))
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let memo = MemoizedGraphQuery::new(runner);
    let graph = RuleGraph::load(workspace, &memo, startup_options)
        .map_err(|error| Unavailable::Failed(format!("rule graph query failed: {error}")))?;
    let used = graph.main_repo_input_paths();
    Ok(candidates
        .into_iter()
        .filter(|path| !used.contains(path))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::test_support::ScriptQuery;
    use dx_testing::cli::strings;

    fn script() -> ScriptQuery {
        ScriptQuery::default()
    }

    fn workspace(files: &[&str], ignore: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for file in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "").expect("write");
        }
        if !ignore.is_empty() {
            std::fs::write(dir.path().join(".bazelignore"), ignore).expect("ignore");
        }
        dir
    }

    fn run(
        dir: &tempfile::TempDir,
        tracked: &str,
        rules: &[String],
    ) -> Result<Vec<String>, Unavailable> {
        let query = script();
        query.script_checkout_git(0, "true\n");
        query.script_checkout_git(0, tracked);
        query.script_graph(rules);
        tracked_orphans(dir.path(), &query, &[])
    }

    #[test]
    fn candidate_reads_head_and_dirty() {
        let query = script();
        query.script_checkout_git(0, "true\n");
        query.script_checkout_git(0, "abc123\n");
        query.script_checkout_git(0, " M a.rs\n");
        let got = candidate(Path::new("/w"), &query).expect("candidate");
        assert_eq!((got.sha.as_str(), got.dirty), ("abc123", true));
        let query = script();
        query.script_checkout_git(0, "true\n");
        query.script_checkout_git(0, "abc123\n");
        query.script_checkout_git(0, "");
        assert!(!candidate(Path::new("/w"), &query).expect("candidate").dirty);
    }

    #[test]
    fn candidate_is_none_outside_git() {
        let query = script();
        query.script_checkout_git(128, "");
        assert_eq!(candidate(Path::new("/w"), &query), None);
    }

    #[test]
    fn not_a_work_tree_is_unavailable() {
        let dir = workspace(&["BUILD.bazel"], "");
        let query = script();
        query.script_checkout_git(128, "");
        assert_eq!(
            tracked_orphans(dir.path(), &query, &[]),
            Err(Unavailable::NotGit)
        );
    }

    #[test]
    fn git_failure_is_an_error() {
        let dir = workspace(&["BUILD.bazel"], "");
        let query = script();
        query.script_checkout_git(0, "true\n");
        query.script_checkout_git(1, "");
        assert!(matches!(
            tracked_orphans(dir.path(), &query, &[]),
            Err(Unavailable::Failed(message)) if message.contains("ls-files")
        ));
    }

    #[test]
    fn query_failure_is_an_error() {
        let dir = workspace(&["BUILD.bazel", "a.rs"], "");
        let query = script();
        query.script_checkout_git(0, "true\n");
        query.script_checkout_git(0, "a.rs\0");
        query.script_failure(3, "ERROR: partial\n");
        assert!(matches!(
            tracked_orphans(dir.path(), &query, &[]),
            Err(Unavailable::Failed(message)) if message.contains("rule graph")
        ));
    }

    #[test]
    fn used_files_are_not_orphans_and_one_real_orphan_is_listed() {
        let dir = workspace(
            &[
                "BUILD.bazel",
                "p/BUILD.bazel",
                "p/used.rs",
                "p/data.go",
                "p/arm.rs",
                "p/manual.rs",
                "p/orphan.rs",
            ],
            "",
        );
        let tracked = "p/used.rs\0p/data.go\0p/arm.rs\0p/manual.rs\0p/orphan.rs\0p/BUILD.bazel\0";
        let rules = [
            dx_testing::cli::graph_rule("//p:lib", &["//p:used.rs", "//p:arm.rs"], &[]),
            dx_testing::cli::graph_rule("//p:t", &["//p:data.go"], &[]),
            dx_testing::cli::graph_rule("//p:m", &["//p:manual.rs"], &["manual"]),
        ];
        assert_eq!(run(&dir, tracked, &rules), Ok(strings(&["p/orphan.rs"])));
    }

    #[test]
    fn ignored_dx_and_unpackaged_files_are_skipped() {
        let dir = workspace(
            &["p/BUILD.bazel", "p/a.rs", "vendor/x.rs", "loose/y.rs"],
            "vendor\n",
        );
        let tracked = "vendor/x.rs\0.dx/s.rs\0loose/y.rs\0p/a.rs\0";
        let rules = [dx_testing::cli::graph_rule("//p:lib", &["//p:a.rs"], &[])];
        assert_eq!(run(&dir, tracked, &rules), Ok(Vec::new()));
    }
}
