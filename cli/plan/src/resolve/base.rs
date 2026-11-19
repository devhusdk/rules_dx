//! Resolves the base branch and merge-base that `--affected` and the changed-line gate compare against.

use super::{QueryRunner, ResolveError, merge_base, missing_base_shallow_error, shallow_clone};
use dx_adopt::DEFAULT_BRANCH_CANDIDATES;
use std::path::{Path, PathBuf};

/// Why a base could not be resolved: a git problem or a plan error.
#[derive(Debug)]
pub enum PlanError {
    Git(String),
    Resolve(ResolveError),
}

impl From<ResolveError> for PlanError {
    fn from(error: ResolveError) -> Self {
        PlanError::Resolve(error)
    }
}

fn git_text(runner: &dyn QueryRunner, workspace: &Path, args: &[&str]) -> Option<String> {
    let mut git_argv = vec!["git".to_owned()];
    git_argv.extend(args.iter().map(ToString::to_string));
    let result = runner.run_query(&git_argv, workspace).ok()?;
    if result.code != Some(0) {
        return None;
    }
    let text = String::from_utf8(result.stdout).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn resolves_to_commit(runner: &dyn QueryRunner, workspace: &Path, name: &str) -> bool {
    let spec = format!("{name}^{{commit}}");
    git_text(
        runner,
        workspace,
        &["rev-parse", "--verify", "--quiet", &spec],
    )
    .is_some()
}

/// Fails with a plain message when `workspace` is not inside a git work tree, so
/// a missing repository is not misreported as a missing default branch.
fn require_work_tree(runner: &dyn QueryRunner, workspace: &Path) -> Result<(), PlanError> {
    match git_text(runner, workspace, &["rev-parse", "--is-inside-work-tree"]).as_deref() {
        Some("true") => Ok(()),
        _ => Err(PlanError::Git(format!(
            "{} is not inside a git work tree; --affected compares against git history, so run it in a git checkout",
            workspace.display()
        ))),
    }
}

/// The `[affected] base` ref from `dx.toml`, when set.
fn configured_base(workspace: &Path) -> Result<Option<String>, PlanError> {
    match dx_config::load(workspace) {
        Ok(config) => Ok(config.and_then(|config| config.affected.base)),
        Err(problem) => Err(PlanError::Resolve(ResolveError::Config {
            message: problem.to_string(),
        })),
    }
}

/// The branch to take the merge-base with: `--base`, else `dx.toml` `[affected] base`,
/// else the detected default branch.
fn base_branch(
    runner: &dyn QueryRunner,
    workspace: &Path,
    requested: Option<&str>,
) -> Result<String, PlanError> {
    let (name, source) = match requested {
        Some(name) => (name.to_owned(), "--base"),
        None => match configured_base(workspace)? {
            Some(name) => (name, "dx.toml [affected] base"),
            None => return default_branch(runner, workspace),
        },
    };
    if resolves_to_commit(runner, workspace, &name) {
        return Ok(name);
    }
    if shallow_clone(workspace) {
        return Err(PlanError::Git(missing_base_shallow_error()));
    }
    Err(PlanError::Git(format!(
        "{source} {name:?} does not name a commit"
    )))
}

fn default_branch(runner: &dyn QueryRunner, workspace: &Path) -> Result<String, PlanError> {
    let head = git_text(
        runner,
        workspace,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    );
    let candidates = head
        .into_iter()
        .chain(DEFAULT_BRANCH_CANDIDATES.iter().map(|c| (*c).to_owned()));
    for candidate in candidates {
        if resolves_to_commit(runner, workspace, &candidate) {
            return Ok(candidate);
        }
    }
    if shallow_clone(workspace) {
        return Err(PlanError::Git(missing_base_shallow_error()));
    }
    Err(PlanError::Git(format!(
        "cannot find the default branch: tried origin/HEAD, {}",
        DEFAULT_BRANCH_CANDIDATES.join(", ")
    )))
}

/// The commit a `--since` ref names.
pub fn since_base(
    workspace: &Path,
    runner: &dyn QueryRunner,
    rev: &str,
) -> Result<String, PlanError> {
    require_work_tree(runner, workspace)?;
    let spec = format!("{rev}^{{commit}}");
    git_text(
        runner,
        workspace,
        &["rev-parse", "--verify", "--quiet", &spec],
    )
    .ok_or_else(|| PlanError::Git(format!("--since {rev:?} does not name a commit")))
}

pub fn affected_base(
    workspace: &Path,
    runner: &dyn QueryRunner,
    requested: Option<&str>,
) -> Result<String, PlanError> {
    require_work_tree(runner, workspace)?;
    named_base(workspace, runner, requested).map(|(_, base)| base)
}

/// The base branch (see [`base_branch`]) and its merge-base with `HEAD`.
pub fn named_base(
    workspace: &Path,
    runner: &dyn QueryRunner,
    requested: Option<&str>,
) -> Result<(String, String), PlanError> {
    let git = PathBuf::from("git");
    let branch = base_branch(runner, workspace, requested)?;
    let base = merge_base(&git, workspace, runner, &branch, "HEAD")
        .map_err(|error| PlanError::Git(error.to_string()))?;
    Ok((branch, base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::QueryResult;
    use dx_test_scratch::write;
    use std::process::Command as Process;

    struct GitRunner;

    impl QueryRunner for GitRunner {
        fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
            let out = Process::new(&argv[0])
                .args(&argv[1..])
                .current_dir(cwd)
                .output()?;
            Ok(QueryResult {
                code: out.status.code(),
                stdout: out.stdout,
                stderr: out.stderr,
            })
        }
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_test_scratch::git(root, root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        write(root, "a.txt", "a\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "base"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-b", "topic"]).unwrap();
        dir
    }

    #[test]
    fn missing_default_branch_is_a_git_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        dx_test_scratch::git(
            dir.path(),
            dir.path(),
            &["init", "--quiet", "--initial-branch=dev"],
        )
        .unwrap();
        write(dir.path(), "x", "x").unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["add", "."]).unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["commit", "--quiet", "-m", "x"]).unwrap();
        let runner = GitRunner;
        assert!(matches!(
            affected_base(dir.path(), &runner, None),
            Err(PlanError::Git(_))
        ));
    }

    #[test]
    fn dangling_origin_head_falls_through_to_origin_main() {
        let dir = repo();
        let root = dir.path();
        dx_test_scratch::git(
            root,
            root,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        )
        .unwrap();
        dx_test_scratch::git(
            root,
            root,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/master",
            ],
        )
        .unwrap();
        let runner = GitRunner;
        let Ok((branch, _)) = named_base(root, &runner, None) else {
            panic!("expected a base");
        };
        assert_eq!(branch, "origin/main");
    }

    #[test]
    fn missing_default_branch_error_names_the_refs_tried() {
        let dir = tempfile::tempdir().expect("tempdir");
        dx_test_scratch::git(
            dir.path(),
            dir.path(),
            &["init", "--quiet", "--initial-branch=dev"],
        )
        .unwrap();
        write(dir.path(), "x", "x").unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["add", "."]).unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["commit", "--quiet", "-m", "x"]).unwrap();
        let runner = GitRunner;
        let Err(PlanError::Git(message)) = affected_base(dir.path(), &runner, None) else {
            panic!("expected a git error");
        };
        assert!(message.contains("origin/main"), "{message}");
    }

    #[test]
    fn missing_default_branch_in_a_shallow_clone_names_the_fix() {
        let dir = tempfile::tempdir().expect("tempdir");
        dx_test_scratch::git(
            dir.path(),
            dir.path(),
            &["init", "--quiet", "--initial-branch=dev"],
        )
        .unwrap();
        write(dir.path(), "x", "x").unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["add", "."]).unwrap();
        dx_test_scratch::git(dir.path(), dir.path(), &["commit", "--quiet", "-m", "x"]).unwrap();
        write(dir.path(), ".git/shallow", "").unwrap();
        let runner = GitRunner;
        let Err(PlanError::Git(message)) = affected_base(dir.path(), &runner, None) else {
            panic!("expected a git error");
        };
        assert!(message.contains("shallow"), "{message}");
        assert!(message.contains("git fetch origin"), "{message}");
    }

    #[test]
    fn outside_git_says_so_instead_of_blaming_the_default_branch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let runner = GitRunner;
        let Err(PlanError::Git(message)) = affected_base(dir.path(), &runner, None) else {
            panic!("expected a git error");
        };
        assert!(message.contains("not inside a git work tree"), "{message}");
        let Err(PlanError::Git(message)) = since_base(dir.path(), &runner, "HEAD") else {
            panic!("expected a git error");
        };
        assert!(message.contains("not inside a git work tree"), "{message}");
    }

    #[test]
    fn base_comes_from_the_flag_then_dx_toml_then_detection() {
        let dir = repo();
        let root = dir.path();
        dx_test_scratch::git(root, root, &["branch", "develop", "main"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "release", "main"]).unwrap();
        let runner = GitRunner;
        let branch = |requested| named_base(root, &runner, requested).map(|(branch, _)| branch);
        assert!(matches!(branch(None).as_deref(), Ok("main")));
        write(
            root,
            "dx.toml",
            "schema_version = 1\n\n[affected]\nbase = \"develop\"\n",
        )
        .unwrap();
        assert!(matches!(branch(None).as_deref(), Ok("develop")));
        assert!(matches!(branch(Some("release")).as_deref(), Ok("release")));
        let Err(PlanError::Git(message)) = branch(Some("nope")) else {
            panic!("expected a git error");
        };
        assert!(message.contains("--base \"nope\""), "{message}");
        write(
            root,
            "dx.toml",
            "schema_version = 1\n\n[affected]\nbase = \"gone\"\n",
        )
        .unwrap();
        let Err(PlanError::Git(message)) = branch(None) else {
            panic!("expected a git error");
        };
        assert!(
            message.contains("dx.toml [affected] base \"gone\""),
            "{message}"
        );
    }

    #[test]
    fn since_rejects_a_ref_that_names_no_commit() {
        let dir = repo();
        let runner = GitRunner;
        let Err(PlanError::Git(message)) = since_base(dir.path(), &runner, "nope") else {
            panic!("expected a git error");
        };
        assert!(message.contains("nope"), "{message}");
    }
}
