use dx_testing::read_runfiles;

const LIST: &str = ".github/review-paths.txt";

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
