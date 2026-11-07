//! `dx/generated-drift`: committed `.dx` files that differ from what the generator renders.

use std::collections::BTreeSet;
use std::path::Path;

use dx_output::{DiagnosticEvent, Severity};
use dx_plan::resolve::QueryRunner;

use super::common::git_output;

const TOOL: &str = "dx";
const DRIFT_RULE: &str = "dx/generated-drift";
const PIN_RULE: &str = "dx/pin-mismatch";

/// One finding per tracked `.dx` file that is not what the generator renders, and one for a
/// `.dx/pins` version that is not this dx. `rewritten` is what the pre-command hook changed.
pub(super) fn findings(
    workspace: &Path,
    runner: &dyn QueryRunner,
    rewritten: &[String],
) -> Vec<DiagnosticEvent> {
    if !workspace.join("MODULE.bazel").is_file() {
        return Vec::new();
    }
    let Ok(tracked) = git_output(
        runner,
        workspace,
        "ls-files",
        &["ls-files", "-z", "--", ".dx"],
    ) else {
        return Vec::new();
    };
    let tracked: BTreeSet<String> = tracked
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
        .map(|raw| String::from_utf8_lossy(raw).into_owned())
        .collect();
    let mut found = pin_mismatch(workspace, &tracked);
    let Ok(planned) = dx_generate::plan(workspace, &mut dx_adopt::detected_families) else {
        return found;
    };
    for path in &tracked {
        let Some(bytes) = planned.get(path) else {
            if rewritten.contains(path) {
                found.push(drift(path, "is not rendered by the generator any more"));
            }
            continue;
        };
        let staged = git_output(runner, workspace, "show", &["show", &format!(":{path}")]).ok();
        let on_disk = std::fs::read(workspace.join(path)).ok();
        if staged.as_ref() != Some(bytes)
            || on_disk.as_ref() != Some(bytes)
            || rewritten.contains(path)
        {
            found.push(drift(path, "differs from the generator output"));
        }
    }
    found
}

fn drift(path: &str, what: &str) -> DiagnosticEvent {
    DiagnosticEvent {
        rule: Some(DRIFT_RULE.to_owned()),
        path: Some(path.to_owned()),
        ..DiagnosticEvent::new(
            Severity::Error,
            TOOL,
            format!("{path} {what}; run `dx upgrade` or commit .dx/"),
        )
    }
}

fn pin_mismatch(workspace: &Path, tracked: &BTreeSet<String>) -> Vec<DiagnosticEvent> {
    let running = env!("CARGO_PKG_VERSION");
    if !tracked.contains(".dx/pins") || running == "0.0.0" {
        return Vec::new();
    }
    match dx_adopt::read_version_pin(workspace) {
        Ok(pinned) if !pinned.is_empty() && pinned != running => vec![DiagnosticEvent {
            rule: Some(PIN_RULE.to_owned()),
            path: Some(".dx/pins".to_owned()),
            ..DiagnosticEvent::new(
                Severity::Error,
                TOOL,
                format!(".dx/pins is {pinned} but this dx is {running}; run `dx upgrade`"),
            )
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_plan::resolve::QueryResult;
    use std::io;
    use std::process::Command;

    const BASE: &str = ".dx/config/ruff/base.toml";

    struct Git;

    impl QueryRunner for Git {
        fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
            let output = Command::new(&argv[0])
                .args(&argv[1..])
                .current_dir(cwd)
                .output()?;
            Ok(QueryResult {
                code: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=dx", "-c", "user.email=dx@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(root)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// A workspace with a python file whose generated `.dx` is committed.
    fn committed() -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch("dx-drift-");
        let root = scratch.path();
        dx_test_scratch::write(root, "MODULE.bazel", "");
        dx_test_scratch::write(root, "dx.toml", "schema_version = 1\n");
        dx_test_scratch::write(root, "pkg/a.py", "");
        dx_generate::sync(
            root,
            dx_generate::Mode::Full,
            &mut dx_adopt::detected_families,
        )
        .expect("sync");
        git(root, &["init", "--quiet"]);
        git(root, &["add", ".dx", "MODULE.bazel", "dx.toml", "pkg"]);
        git(root, &["commit", "--quiet", "-m", "base"]);
        scratch
    }

    fn rules(root: &Path, rewritten: &[String]) -> Vec<(Option<String>, Option<String>)> {
        findings(root, &Git, rewritten)
            .into_iter()
            .map(|found| (found.rule, found.path))
            .collect()
    }

    #[test]
    fn a_clean_checkout_passes() {
        let scratch = committed();
        assert!(rules(scratch.path(), &[]).is_empty());
    }

    #[test]
    fn a_hand_edit_is_drift() {
        let scratch = committed();
        let root = scratch.path();
        dx_test_scratch::write(root, BASE, "edited\n");
        let want = vec![(Some(DRIFT_RULE.to_owned()), Some(BASE.to_owned()))];
        assert_eq!(rules(root, &[]), want);
    }

    #[test]
    fn a_committed_edit_the_hook_rewrote_is_drift() {
        let scratch = committed();
        let root = scratch.path();
        dx_test_scratch::write(root, BASE, "edited\n");
        git(root, &["add", BASE]);
        git(root, &["commit", "--quiet", "-m", "edit"]);
        dx_generate::sync(
            root,
            dx_generate::Mode::Full,
            &mut dx_adopt::detected_families,
        )
        .expect("hook");
        let want = vec![(Some(DRIFT_RULE.to_owned()), Some(BASE.to_owned()))];
        assert_eq!(rules(root, &[BASE.to_owned()]), want);
    }

    #[test]
    fn untracked_dx_is_not_drift() {
        let scratch = committed();
        let root = scratch.path();
        git(root, &["rm", "--quiet", "--cached", "-r", ".dx"]);
        dx_test_scratch::write(root, BASE, "edited\n");
        assert!(rules(root, &[BASE.to_owned()]).is_empty());
    }
}
