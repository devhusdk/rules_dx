use std::collections::HashSet;
use std::path::Path;

use super::{
    owned_sources_expression, ownership_set_expression, run_label_query, PackageCache, QueryRunner,
    ResolveError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidenedScope {
    pub scope: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedSelection {
    pub targets: Vec<String>,
    pub widened: Vec<WidenedScope>,
}

impl AffectedSelection {
    pub fn complete(&self) -> bool {
        self.widened.is_empty()
    }

    pub fn explain_suffix(&self) -> String {
        if self.widened.is_empty() {
            return String::new();
        }
        let mut scopes: Vec<String> = self
            .widened
            .iter()
            .map(|widened| format!("{} ({})", widened.scope, widened.reason))
            .collect();
        scopes.sort();
        scopes.dedup();
        format!("; widened {} (conservative)", scopes.join(", "))
    }
}

fn git_failure(what: &str, detail: String) -> String {
    format!("hook git {what} failed: {detail}")
}

fn first_diagnostic(bytes: &[u8]) -> String {
    dx_output::first_diagnostic_line(bytes, 200, "no Git diagnostic")
}

fn git_dir_of(workspace: &Path) -> Option<std::path::PathBuf> {
    let dot_git = workspace.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let pointer = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = pointer.strip_prefix("gitdir: ")?.trim();
    let gitdir_path = Path::new(gitdir);
    if gitdir_path.is_absolute() {
        return Some(gitdir_path.to_path_buf());
    }
    Some(workspace.join(gitdir_path))
}

fn shallow_clone(workspace: &Path) -> bool {
    let Some(git_dir) = git_dir_of(workspace) else {
        return false;
    };
    if git_dir.join("shallow").is_file() {
        return true;
    }
    let Ok(commondir) = std::fs::read_to_string(git_dir.join("commondir")) else {
        return false;
    };
    git_dir.join(commondir.trim()).join("shallow").is_file()
}

fn shallow_error() -> String {
    "git history is shallow: fetch full history with `git fetch --unshallow` or pass explicit scopes".to_owned()
}

pub fn diff_name_status(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    extra: &[String],
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(extra.iter().cloned());
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("diff", error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(shallow_error());
        }
        return Err(git_failure("diff", first_diagnostic(&result.stderr)));
    }
    dx_adopt::parse_name_status_nul(&result.stdout)
}

pub fn merge_base(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    first: &str,
    second: &str,
) -> Result<String, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(
        ["merge-base", first, second]
            .iter()
            .map(ToString::to_string),
    );
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("merge-base", error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(shallow_error());
        }
        return Err(git_failure("merge-base", first_diagnostic(&result.stderr)));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|_| git_failure("merge-base", "output is not UTF-8".to_owned()))?;
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let base = lines
        .next()
        .ok_or_else(|| git_failure("merge-base", "output names no base commit".to_owned()))?;
    if lines.next().is_some() {
        return Err(git_failure(
            "merge-base",
            "output names more than one base commit".to_owned(),
        ));
    }
    Ok(base.to_owned())
}

fn parse_untracked(output: &[u8]) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let mut changes = Vec::with_capacity(fields.len());
    for field in fields {
        let path = std::str::from_utf8(field).map_err(|_| {
            "hook git path is not UTF-8: rename the file to a UTF-8 name".to_owned()
        })?;
        if path.is_empty() {
            return Err("hook git change list has an empty path".to_owned());
        }
        changes.push(dx_adopt::GitChange {
            path: path.to_owned(),
            from: None,
            kind: dx_adopt::ChangeKind::Added,
        });
    }
    Ok(changes)
}

pub fn collect_working_tree(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let staged = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--cached".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
        ],
    )?;
    let unstaged = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
        ],
    )?;
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(
        ["ls-files", "--others", "--exclude-standard", "-z"]
            .iter()
            .map(ToString::to_string),
    );
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("ls-files", error.to_string()))?;
    if result.code != Some(0) {
        return Err(git_failure("ls-files", first_diagnostic(&result.stderr)));
    }
    let untracked = parse_untracked(&result.stdout)?;
    let mut changes = staged;
    changes.extend(unstaged);
    changes.extend(untracked);
    Ok(dx_adopt::dedupe_changes(changes))
}

pub fn collect_range(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: &str,
    head: &str,
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let changes = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
            base.to_owned(),
            head.to_owned(),
        ],
    )?;
    Ok(dx_adopt::dedupe_changes(changes))
}

fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn is_workspace_input(path: &str) -> bool {
    let name = file_name_of(path);
    matches!(
        name,
        "MODULE.bazel" | "MODULE.bazel.lock" | "WORKSPACE" | "WORKSPACE.bazel" | ".bazelversion"
    ) || name.starts_with(".bazelrc")
        || name.ends_with(".lock")
}

fn is_build_input(path: &str) -> bool {
    let name = file_name_of(path);
    matches!(name, "BUILD" | "BUILD.bazel") || name.ends_with(".bzl")
}

fn fallback_reason(path: &str) -> String {
    if is_build_input(path) {
        format!("build input {path}")
    } else {
        format!("unowned {path}")
    }
}

fn valid_change_path(path: &str) -> Result<(), ResolveError> {
    if path.is_empty() {
        return Err(ResolveError::EmptyScope);
    }
    if Path::new(path).is_absolute() || path.split('/').any(|part| part == "..") {
        return Err(ResolveError::OutsideWorkspace {
            scope: path.to_owned(),
        });
    }
    if path.chars().any(char::is_control) {
        return Err(ResolveError::UnsupportedName {
            scope: path.to_owned(),
        });
    }
    Ok(())
}

fn push_widened(widened: &mut Vec<WidenedScope>, scope: String, reason: String) {
    if !widened
        .iter()
        .any(|existing| existing.scope == scope && existing.reason == reason)
    {
        widened.push(WidenedScope { scope, reason });
    }
}

pub fn resolve_affected(
    changes: &[dx_adopt::GitChange],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<AffectedSelection, ResolveError> {
    let mut targets: Vec<String> = Vec::new();
    let mut widened: Vec<WidenedScope> = Vec::new();
    let mut candidates: Vec<(String, String)> = Vec::new();
    let mut cache = PackageCache::default();
    for change in changes {
        let mut paths = vec![change.path.clone()];
        if let Some(from) = &change.from {
            paths.push(from.clone());
        }
        for path in paths {
            valid_change_path(&path)?;
            if is_workspace_input(&path) {
                push_widened(
                    &mut widened,
                    "//...".to_owned(),
                    format!("workspace input {path}"),
                );
                continue;
            }
            if !workspace.join(&path).is_file() && !workspace.join(&path).is_dir() {
                let reason = if change.kind == dx_adopt::ChangeKind::Deleted {
                    format!("deleted {path}")
                } else if change.from.as_deref() == Some(path.as_str()) {
                    format!("renamed from {path}")
                } else {
                    format!("removed {path}")
                };
                push_widened(
                    &mut widened,
                    dx_adopt::nearest_package_pattern(workspace, &path),
                    reason,
                );
                continue;
            }
            match cache.file_label(workspace, &path, &path) {
                Ok(label) => candidates.push((path, label)),
                Err(ResolveError::NotAPackage { .. }) => push_widened(
                    &mut widened,
                    dx_adopt::nearest_package_pattern(workspace, &path),
                    format!("no enclosing package for {path}"),
                ),
                Err(error) => return Err(error),
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    if !candidates.is_empty() {
        let labels: Vec<String> = candidates.iter().map(|(_, label)| label.clone()).collect();
        let owners = run_label_query(
            &ownership_set_expression(&labels),
            workspace,
            runner,
            startup_options,
        )?;
        if owners.is_empty() {
            for (path, _) in &candidates {
                push_widened(
                    &mut widened,
                    dx_adopt::nearest_package_pattern(workspace, path),
                    fallback_reason(path),
                );
            }
        } else if candidates.len() == 1 {
            targets.extend(owners);
        } else {
            let sources = run_label_query(
                &owned_sources_expression(&owners),
                workspace,
                runner,
                startup_options,
            )?;
            let owned: HashSet<&str> = sources.iter().map(String::as_str).collect();
            for (path, label) in &candidates {
                if !owned.contains(label.as_str()) {
                    push_widened(
                        &mut widened,
                        dx_adopt::nearest_package_pattern(workspace, path),
                        fallback_reason(path),
                    );
                }
            }
            targets.extend(owners);
        }
    }
    targets.sort();
    targets.dedup();
    widened.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    widened.dedup_by(|next, current| next.scope == current.scope && next.reason == current.reason);
    for widened_scope in &widened {
        if !targets.contains(&widened_scope.scope) {
            targets.push(widened_scope.scope.clone());
        }
    }
    targets.sort();
    targets.dedup();
    Ok(AffectedSelection { targets, widened })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, NeverQuery, QueryResult};
    use crate::test_support::strings;
    use std::cell::RefCell;
    use std::path::PathBuf;

    struct FakeQuery {
        calls: RefCell<Vec<(Vec<String>, PathBuf)>>,
        outputs: RefCell<Vec<QueryResult>>,
    }

    impl FakeQuery {
        fn new(outputs: Vec<QueryResult>) -> Self {
            FakeQuery {
                calls: RefCell::new(Vec::new()),
                outputs: RefCell::new(outputs),
            }
        }

        fn ok(lines: &str) -> QueryResult {
            QueryResult {
                code: Some(0),
                stdout: lines.as_bytes().to_vec(),
                stderr: Vec::new(),
            }
        }

        fn failed(stderr: &str) -> QueryResult {
            QueryResult {
                code: Some(2),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            }
        }

        fn raw(stdout: Vec<u8>) -> QueryResult {
            QueryResult {
                code: Some(0),
                stdout,
                stderr: Vec::new(),
            }
        }

        fn calls(&self) -> Vec<(Vec<String>, PathBuf)> {
            self.calls.borrow().clone()
        }
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
            self.calls
                .borrow_mut()
                .push((argv.to_vec(), cwd.to_path_buf()));
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    fn write(workspace: &Path, rel: &str, text: &str) {
        let full = workspace.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write file");
    }

    fn change(path: &str, kind: dx_adopt::ChangeKind) -> dx_adopt::GitChange {
        dx_adopt::GitChange {
            path: path.to_owned(),
            from: None,
            kind,
        }
    }

    fn renamed(from: &str, path: &str) -> dx_adopt::GitChange {
        dx_adopt::GitChange {
            path: path.to_owned(),
            from: Some(from.to_owned()),
            kind: dx_adopt::ChangeKind::Renamed,
        }
    }

    #[test]
    fn deleted_changes_widen_to_package_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let selection = resolve_affected(
            &[change("pkg/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &crate::resolve::NeverQuery,
            &[],
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].scope, "//pkg/...");
        assert_eq!(selection.widened[0].reason, "deleted pkg/gone.py");
        assert!(!selection.complete());
        let suffix = selection.explain_suffix();
        assert!(suffix.contains("//pkg/..."), "{suffix}");
        assert!(suffix.contains("deleted pkg/gone.py"), "{suffix}");
        assert!(suffix.contains("conservative"), "{suffix}");
    }

    #[test]
    fn rename_keeps_target_and_widens_missing_source() {
        let scratch = dx_test_scratch::scratch("dx-changed-rename-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n")]);
        let selection = resolve_affected(
            &[renamed("pkg/was.py", "pkg/a.py")],
            &workspace,
            &query,
            &[],
        )
        .expect("rename resolves");
        assert_eq!(
            selection.targets,
            strings(&["//pkg/...", "//pkg:lib"]),
            "existing side keeps its owner while the missing source widens"
        );
        assert_eq!(query.calls().len(), 1, "one batched ownership query");
        assert!(selection
            .explain_suffix()
            .contains("renamed from pkg/was.py"));
    }

    #[test]
    fn attributed_build_inputs_keep_precise_owners() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-owned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/space name.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::ok("//pkg:BUILD.bazel\n//pkg:space name.py\n"),
        ]);
        let selection = resolve_affected(
            &[
                change("pkg/space name.py", dx_adopt::ChangeKind::Added),
                change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
        )
        .expect("attributed");
        assert_eq!(selection.targets, strings(&["//pkg:lib"]));
        assert!(selection.complete());
        assert!(selection.explain_suffix().is_empty());
        assert_eq!(query.calls().len(), 2, "ownership plus attribution");
    }

    #[test]
    fn unowned_build_inputs_widen_to_their_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-unowned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let selection = resolve_affected(
            &[
                change("pkg/a.py", dx_adopt::ChangeKind::Modified),
                change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
        )
        .expect("unowned widens");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
        assert_eq!(
            query.calls().len(),
            1,
            "no attribution query on empty owners"
        );
        let mut reasons: Vec<&str> = selection
            .widened
            .iter()
            .map(|widened| widened.reason.as_str())
            .collect();
        reasons.sort();
        assert_eq!(
            reasons,
            vec!["build input pkg/BUILD.bazel", "unowned pkg/a.py"]
        );
    }

    #[test]
    fn single_unowned_file_widens_without_attribution_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-single-unowned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("\n")]);
        let selection = resolve_affected(
            &[change("pkg/a.py", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
        )
        .expect("single unowned widens");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn workspace_inputs_widen_to_repository_without_query() {
        for changed in [
            "MODULE.bazel",
            "MODULE.bazel.lock",
            "Cargo.lock",
            ".bazelrc",
        ] {
            let scratch = dx_test_scratch::scratch("dx-changed-workspace-");
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "");
            write(&workspace, changed, "x\n");
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &crate::resolve::NeverQuery,
                &[],
            )
            .expect("workspace input widens");
            assert_eq!(selection.targets, strings(&["//..."]), "{changed}");
            assert_eq!(selection.widened.len(), 1, "{changed}");
            assert_eq!(
                selection.widened[0].reason,
                format!("workspace input {changed}"),
                "{changed}"
            );
        }
    }

    #[test]
    fn affected_matches_plain_resolve_for_attributed_files() {
        let workspace_files = ["pkg/a.py", "pkg/b.py"];
        let owners = "//pkg:lib\n//pkg:extra\n";
        let sources = "//pkg:a.py\n//pkg:b.py\n";
        let mut affected_targets = Vec::new();
        let mut resolved_targets = Vec::new();
        for _ in 0..2 {
            let scratch = dx_test_scratch::scratch("dx-changed-parity-");
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "");
            write(&workspace, "pkg/a.py", "x = 1\n");
            write(&workspace, "pkg/b.py", "x = 1\n");
            let affected_query =
                FakeQuery::new(vec![FakeQuery::ok(owners), FakeQuery::ok(sources)]);
            let affected = resolve_affected(
                &workspace_files
                    .iter()
                    .map(|path| change(path, dx_adopt::ChangeKind::Modified))
                    .collect::<Vec<_>>(),
                &workspace,
                &affected_query,
                &[],
            )
            .expect("affected");
            assert!(affected.complete());
            assert_eq!(affected_query.calls().len(), 2);
            affected_targets.push(affected.targets);
            let resolved_query =
                FakeQuery::new(vec![FakeQuery::ok(owners), FakeQuery::ok(sources)]);
            let resolved = resolve(
                &workspace_files
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                &workspace,
                &resolved_query,
                &[],
            )
            .expect("resolve");
            resolved_targets.push(resolved.targets);
        }
        assert_eq!(affected_targets[0], affected_targets[1]);
        assert_eq!(affected_targets, resolved_targets);
    }

    #[test]
    fn resolve_rejects_deleted_paths_that_affected_widens() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-parity-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        let err = resolve(&strings(&["pkg/gone.py"]), &workspace, &NeverQuery, &[])
            .expect_err("deleted path has no worktree file");
        assert_eq!(
            err,
            ResolveError::PathNotFound {
                scope: "pkg/gone.py".to_owned(),
            }
        );
        let selection = resolve_affected(
            &[change("pkg/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &NeverQuery,
            &[],
        )
        .expect("affected widens instead of failing");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
    }

    #[test]
    fn control_names_fail_before_any_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-control-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/ok.py", "x = 1\n");
        let err = resolve_affected(
            &[change("pkg/we\nird.py", dx_adopt::ChangeKind::Added)],
            &workspace,
            &NeverQuery,
            &[],
        )
        .expect_err("control characters fail");
        assert_eq!(
            err,
            ResolveError::UnsupportedName {
                scope: "pkg/we\nird.py".to_owned(),
            }
        );
    }

    #[test]
    fn escaping_paths_fail_before_any_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-escape-");
        let workspace = scratch.path().to_path_buf();
        for scope in ["../escape.py", "pkg/../../escape.py", "/abs/path.py"] {
            let err = resolve_affected(
                &[change(scope, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &NeverQuery,
                &[],
            )
            .expect_err("escape fails");
            assert_eq!(
                err,
                ResolveError::OutsideWorkspace {
                    scope: scope.to_owned(),
                },
                "{scope}"
            );
        }
    }

    #[test]
    fn files_without_enclosing_package_widen_to_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-no-package-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "docs/guide.md", "# guide\n");
        let selection = resolve_affected(
            &[change("docs/guide.md", dx_adopt::ChangeKind::Added)],
            &workspace,
            &NeverQuery,
            &[],
        )
        .expect("no package widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(
            selection.widened[0].reason,
            "no enclosing package for docs/guide.md"
        );
    }

    #[test]
    fn nested_packages_widen_to_the_nearest_pattern() {
        let scratch = dx_test_scratch::scratch("dx-changed-nested-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "outer/BUILD.bazel", "");
        write(&workspace, "outer/inner/BUILD.bazel", "");
        let selection = resolve_affected(
            &[change("outer/inner/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &NeverQuery,
            &[],
        )
        .expect("nested widens");
        assert_eq!(selection.targets, strings(&["//outer/inner/..."]));
    }

    #[test]
    fn spaced_names_resolve_verbatim() {
        let scratch = dx_test_scratch::scratch("dx-changed-spaced-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/space name.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n")]);
        let selection = resolve_affected(
            &[change("pkg/space name.py", dx_adopt::ChangeKind::Added)],
            &workspace,
            &query,
            &[],
        )
        .expect("spaced resolves");
        assert_eq!(selection.targets, strings(&["//pkg:lib"]));
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0]
                .0
                .last()
                .expect("expression")
                .contains("\"//pkg:space name.py\""),
            "{:?}",
            calls[0].0
        );
    }

    #[test]
    fn ownership_failures_stay_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-changed-owner-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::failed("ERROR: no such package\nmore\n")]);
        let err = resolve_affected(
            &[change("pkg/a.py", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
        )
        .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "kind('rule', rdeps(//..., set(\"//pkg:a.py\"), 1))".to_owned(),
                detail: "ERROR: no such package".to_owned(),
            }
        );
    }

    #[test]
    fn attribution_failures_stay_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-changed-attrib-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        write(&workspace, "pkg/b.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::failed("ERROR: no such package\nmore\n"),
        ]);
        let err = resolve_affected(
            &[
                change("pkg/a.py", dx_adopt::ChangeKind::Modified),
                change("pkg/b.py", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
        )
        .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "deps(set(\"//pkg:lib\"), 1)".to_owned(),
                detail: "ERROR: no such package".to_owned(),
            }
        );
    }

    #[test]
    fn partially_owned_batches_widen_only_the_orphans() {
        let scratch = dx_test_scratch::scratch("dx-changed-partial-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/owned.py", "x = 1\n");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![
            FakeQuery::ok("//pkg:lib\n"),
            FakeQuery::ok("//pkg:lib\n//pkg:owned.py\n"),
        ]);
        let selection = resolve_affected(
            &[
                change("pkg/owned.py", dx_adopt::ChangeKind::Modified),
                change("pkg/orphan.py", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
        )
        .expect("partial widens");
        assert_eq!(selection.targets, strings(&["//pkg/...", "//pkg:lib"]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].reason, "unowned pkg/orphan.py");
    }

    #[test]
    fn working_tree_collects_staged_unstaged_and_untracked() {
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/b.py\0".to_vec()),
            FakeQuery::raw(b"pkg/c.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let mut paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["pkg/a.py", "pkg/b.py", "pkg/c.py"]);
        let untracked = changes
            .iter()
            .find(|change| change.path == "pkg/c.py")
            .expect("untracked");
        assert_eq!(untracked.kind, dx_adopt::ChangeKind::Added);
        let calls = query.calls();
        assert_eq!(calls.len(), 3);
        assert!(calls[0].0.contains(&"--cached".to_owned()), "{calls:?}");
        assert!(!calls[1].0.contains(&"--cached".to_owned()), "{calls:?}");
        assert!(calls[2].0.contains(&"ls-files".to_owned()), "{calls:?}");
        assert!(
            calls
                .iter()
                .all(|(argv, _)| argv.contains(&"-z".to_owned())),
            "{calls:?}"
        );
    }

    #[test]
    fn working_tree_dedupes_across_inputs() {
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-dedupe-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"pkg/a.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "pkg/a.py");
    }

    #[test]
    fn untracked_names_must_be_utf8() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-utf8-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(vec![0xff, 0x00]),
        ]);
        let err =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect_err("utf8");
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn range_diffs_base_against_head() {
        let scratch = dx_test_scratch::scratch("dx-changed-range-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::raw(b"M\0pkg/a.py\0".to_vec())]);
        let changes = collect_range(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            "base-sha",
            "head-sha",
        )
        .expect("range");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "pkg/a.py");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains(&"base-sha".to_owned()), "{calls:?}");
        assert!(calls[0].0.contains(&"head-sha".to_owned()), "{calls:?}");
        assert!(calls[0].0.contains(&"-z".to_owned()), "{calls:?}");
    }

    #[test]
    fn merge_base_returns_the_trimmed_sha() {
        let scratch = dx_test_scratch::scratch("dx-changed-merge-base-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::raw(b"  abc123\n".to_vec())]);
        let base = merge_base(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            "main",
            "HEAD",
        )
        .expect("base");
        assert_eq!(base, "abc123");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains(&"merge-base".to_owned()), "{calls:?}");
    }

    #[test]
    fn merge_base_rejects_empty_and_ambiguous_output() {
        for (name, stdout) in [("empty", b"".to_vec()), ("two", b"a\nb\n".to_vec())] {
            let scratch = dx_test_scratch::scratch("dx-changed-merge-base-bad-");
            let workspace = scratch.path().to_path_buf();
            let query = FakeQuery::new(vec![FakeQuery::raw(stdout)]);
            let err = merge_base(
                Path::new("/hermetic/git"),
                &workspace,
                &query,
                "main",
                "HEAD",
            )
            .expect_err("bad base");
            assert!(err.contains("merge-base"), "{name}: {err}");
        }
    }

    #[test]
    fn shallow_clone_turns_diff_failure_into_actionable_error() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-");
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join(".git")).expect("git dir");
        std::fs::write(workspace.join(".git/shallow"), "abc\n").expect("shallow");
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: no such commit")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("shallow");
        assert!(err.contains("shallow"), "{err}");
        assert!(err.contains("fetch --unshallow"), "{err}");
        assert_eq!(query.calls().len(), 1, "shallow needs no extra process");
    }

    #[test]
    fn worktree_shallow_marker_is_found_through_the_gitdir_pointer() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-wt-");
        let workspace = scratch.path().to_path_buf();
        let common = scratch.path().join("common.git");
        let gitdir = common.join("worktrees/wt");
        std::fs::create_dir_all(&gitdir).expect("gitdir");
        std::fs::write(
            workspace.join(".git"),
            format!("gitdir: {}\n", gitdir.to_string_lossy()),
        )
        .expect("pointer");
        std::fs::write(common.join("worktrees/wt/commondir"), "../..\n").expect("commondir");
        std::fs::write(common.join("shallow"), "abc\n").expect("shallow");
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: no such commit")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("shallow worktree");
        assert!(err.contains("shallow"), "{err}");
    }

    #[test]
    fn non_shallow_diff_failure_stays_plain() {
        let scratch = dx_test_scratch::scratch("dx-changed-diff-fail-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: bad revision")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("plain failure");
        assert!(err.contains("hook git diff failed"), "{err}");
        assert!(err.contains("bad revision"), "{err}");
    }
}
