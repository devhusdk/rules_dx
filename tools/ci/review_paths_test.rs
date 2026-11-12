use dx_testing::{read_runfiles, resolve_runfiles};
use std::io::Write;
use std::process::{Command, Stdio};

const LIST: &str = ".github/review-paths.txt";
const SCRIPT: &str = "tools/ci/review_paths.sh";

fn matched(input: &str) -> String {
    let mut child = Command::new("bash")
        .arg(resolve_runfiles(SCRIPT))
        .arg(resolve_runfiles(LIST))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn review_paths.sh");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "review_paths.sh failed");
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn list_holds_the_review_prefixes() {
    let text = read_runfiles(LIST);
    let prefixes: Vec<_> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(
        prefixes,
        [
            "cli/process/",
            "cli/worktree/",
            "cli/plan/src/slots",
            "cli/atomic_fs/",
            "deploy/private/release/",
            ".github/workflows/release",
            "core/",
            "MODULE.bazel",
            "tools/testing/root_layout_test.rs",
            "docs/design/layout.md",
            "tools/testing/language_skeleton_allowlist.txt",
        ]
    );
}

#[test]
fn every_workflow_prefix_names_a_parsed_workflow() {
    let workflows: Vec<String> = ci_workflow::all()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let text = read_runfiles(LIST);
    let prefixes: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with(".github/workflows/"))
        .collect();
    assert!(!prefixes.is_empty(), "{LIST} lists no workflow prefix");
    for prefix in prefixes {
        let covered: Vec<&String> = workflows
            .iter()
            .filter(|path| path.starts_with(prefix))
            .collect();
        assert!(!covered.is_empty(), "{prefix} matches no workflow");
        for path in covered {
            assert!(
                !ci_workflow::jobs(path).is_empty(),
                "{path} under {prefix} declares no job"
            );
        }
    }
}

#[test]
fn matches_listed_paths_only() {
    let out = matched("cli/worktree/src/lib.rs\ncli/cli/src/main.rs\ncli/plan/src/slots.rs\n");
    assert_eq!(out, "cli/worktree/src/lib.rs\ncli/plan/src/slots.rs\n");
}

#[test]
fn prints_nothing_and_succeeds_without_a_match() {
    assert_eq!(matched("cli/cli/src/main.rs\n"), "");
}
