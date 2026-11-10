use super::*;
use crate::{account, parse_source_manifest};

fn lines(values: &[u32]) -> BTreeSet<u32> {
    values.iter().copied().collect()
}

fn added(entries: &[(&str, &[u32])]) -> AddedLines {
    entries
        .iter()
        .map(|(path, values)| (path.to_string(), lines(values)))
        .collect()
}

fn lcov(path: &str, hits: &[(u32, u64)]) -> String {
    let mut out = format!("SF:{path}\n");
    for (line, count) in hits {
        out.push_str(&format!("DA:{line},{count}\n"));
    }
    out.push_str("end_of_record\n");
    out
}

fn sources(files: &[(&str, &str)]) -> impl Fn(&str) -> Result<String, LcovError> {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(path, text)| (path.to_string(), text.to_string()))
        .collect();
    move |path| {
        files.get(path).cloned().ok_or_else(|| LcovError::Io {
            message: format!("no such fixture: {path}"),
        })
    }
}

fn gate(
    manifest: &str,
    report: &[String],
    changes: &AddedLines,
    files: &[(&str, &str)],
) -> ChangedAccount {
    let manifests = [parse_source_manifest(manifest).unwrap()];
    let load = sources(files);
    let cell = account(&manifests, report, &load);
    changed_account(&cell, report, changes, &load)
}

const OWNS_A: &str = "v1 cell linux_x86_64\nprod source src/a.py\n";

#[test]
fn hunk_with_count_adds_each_line() {
    let diff = "diff --git a/src/a.py b/src/a.py\n--- a/src/a.py\n+++ b/src/a.py\n@@ -1,0 +2,3 @@\n+x\n+y\n+z\n";
    assert_eq!(parse_added_lines(diff), added(&[("src/a.py", &[2, 3, 4])]));
}

#[test]
fn hunk_without_count_adds_one_line() {
    let diff = "--- a/a.py\n+++ b/a.py\n@@ -5 +5 @@\n-old\n+new\n";
    assert_eq!(parse_added_lines(diff), added(&[("a.py", &[5])]));
}

#[test]
fn hunk_with_zero_count_adds_nothing() {
    let diff = "--- a/a.py\n+++ b/a.py\n@@ -3,2 +2,0 @@\n-gone\n-gone\n";
    assert!(parse_added_lines(diff).is_empty());
}

#[test]
fn several_hunks_and_files_accumulate() {
    let diff = "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1 +1 @@\n-a\n+A\n@@ -9,0 +10,2 @@\n+++ looks like a header\n+b\ndiff --git a/b.py b/b.py\n--- a/b.py\n+++ b/b.py\n@@ -0,0 +1 @@\n+c\n";
    assert_eq!(
        parse_added_lines(diff),
        added(&[("a.py", &[1, 10, 11]), ("b.py", &[1])])
    );
}

#[test]
fn context_lines_advance_the_new_side() {
    let diff = "--- a/a.py\n+++ b/a.py\n@@ -1,3 +1,3 @@\n keep\n-old\n+new\n keep\n";
    assert_eq!(parse_added_lines(diff), added(&[("a.py", &[2])]));
}

#[test]
fn renamed_file_with_edit_counts_the_new_path_only() {
    let diff = "diff --git a/old.py b/new.py\nsimilarity index 90%\nrename from old.py\nrename to new.py\n--- a/old.py\n+++ b/new.py\n@@ -2 +2 @@\n-a\n+b\n";
    assert_eq!(parse_added_lines(diff), added(&[("new.py", &[2])]));
}

#[test]
fn pure_rename_and_deletion_add_nothing() {
    let diff = "diff --git a/old.py b/new.py\nsimilarity index 100%\nrename from old.py\nrename to new.py\ndiff --git a/gone.py b/gone.py\ndeleted file mode 100644\n--- a/gone.py\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n";
    assert!(parse_added_lines(diff).is_empty());
}

#[test]
fn real_b_directory_stays_in_the_path() {
    let diff = "diff --git a/b/x.py b/b/x.py\n--- a/b/x.py\n+++ b/b/x.py\n@@ -0,0 +1 @@\n+a\n";
    assert_eq!(parse_added_lines(diff), added(&[("b/x.py", &[1])]));
}

#[test]
fn binary_file_adds_nothing() {
    let diff = "diff --git a/i.png b/i.png\nnew file mode 100644\nBinary files /dev/null and b/i.png differ\n";
    assert!(parse_added_lines(diff).is_empty());
}

#[test]
fn no_newline_marker_is_not_a_line() {
    let diff = "--- a/a.py\n+++ b/a.py\n@@ -1 +1,2 @@\n-a\n\\ No newline at end of file\n+a\n+b\n\\ No newline at end of file\n";
    assert_eq!(parse_added_lines(diff), added(&[("a.py", &[1, 2])]));
}

#[test]
fn paths_with_spaces_and_quotes_parse() {
    let diff = "--- a/x y.py\t\n+++ b/x y.py\t\n@@ -0,0 +1 @@\n+a\n--- \"a/q\\\"z.py\"\n+++ \"b/q\\\"z.py\"\n@@ -0,0 +1 @@\n+b\n--- \"a/\\303\\251.py\"\n+++ \"b/\\303\\251.py\"\n@@ -0,0 +1 @@\n+c\n";
    assert_eq!(
        parse_added_lines(diff),
        added(&[("x y.py", &[1]), ("q\"z.py", &[1]), ("\u{e9}.py", &[1])])
    );
}

#[test]
fn all_lines_counts_every_line() {
    assert_eq!(all_lines("a\nb\nc"), lines(&[1, 2, 3]));
    assert!(all_lines("").is_empty());
}

#[test]
fn covered_changed_lines_pass() {
    let report = [lcov("src/a.py", &[(1, 1), (2, 1), (3, 0)])];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[1, 2])]), &[]);
    assert_eq!((got.covered_lines, got.eligible_lines), (2, 2));
    assert_eq!(got.result(100), ChangedResult::Pass);
    assert!(got.result(100).ok());
}

#[test]
fn uncovered_changed_lines_fail_with_locations() {
    let report = [lcov("src/a.py", &[(1, 1), (2, 0), (3, 0)])];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[2, 3, 7])]), &[]);
    assert_eq!((got.covered_lines, got.eligible_lines), (0, 2));
    assert_eq!(
        got.uncovered,
        vec![
            ChangedLine {
                path: "src/a.py".to_string(),
                line: 2
            },
            ChangedLine {
                path: "src/a.py".to_string(),
                line: 3
            },
        ]
    );
    assert_eq!(got.result(1), ChangedResult::Fail);
    assert!(!got.result(1).ok());
}

#[test]
fn threshold_compares_exactly() {
    let report = [lcov("src/a.py", &[(1, 1), (2, 1), (3, 0)])];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[1, 2, 3])]), &[]);
    assert_eq!(got.result(67), ChangedResult::Fail);
    assert_eq!(got.result(66), ChangedResult::Pass);
    assert_eq!(got.result(0), ChangedResult::Pass);
}

#[test]
fn no_changed_executable_lines_is_not_applicable() {
    let report = [lcov("src/a.py", &[(1, 1)])];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[5])]), &[]);
    assert_eq!(got.result(100), ChangedResult::NotApplicable);
    assert!(got.result(100).ok());
    let none = gate(OWNS_A, &report, &AddedLines::new(), &[]);
    assert_eq!(none.result(100), ChangedResult::NotApplicable);
}

#[test]
fn changed_file_missing_from_report_is_incomplete() {
    let manifest = "v1 cell linux_x86_64\nprod source src/a.py\nprod source src/b.py\n";
    let report = [lcov("src/a.py", &[(1, 1)])];
    let got = gate(
        manifest,
        &report,
        &added(&[("src/a.py", &[1]), ("src/b.py", &[1])]),
        &[("src/b.py", "x = 1\n")],
    );
    assert_eq!(got.uninstrumented, vec!["src/b.py".to_string()]);
    assert_eq!(got.result(0), ChangedResult::Incomplete);
    assert!(!got.result(0).ok());
}

#[test]
fn case_differing_git_path_matches_report() {
    let manifest = "v1 cell linux_x86_64\nprod source pkg/Foo.c\n";
    let report = [lcov("pkg/Foo.c", &[(1, 0), (2, 1)])];
    let got = gate(manifest, &report, &added(&[("pkg/foo.c", &[1, 2])]), &[]);
    assert_eq!((got.covered_lines, got.eligible_lines), (1, 2));
}

#[test]
fn case_differing_absent_file_is_uninstrumented() {
    let manifest = "v1 cell linux_x86_64\nprod source pkg/Foo.c\n";
    let got = gate(
        manifest,
        &[],
        &added(&[("pkg/foo.c", &[1])]),
        &[("pkg/Foo.c", "x\n")],
    );
    assert_eq!(got.uninstrumented, vec!["pkg/Foo.c".to_string()]);
}

#[test]
fn ambiguous_case_keeps_exact_matching() {
    let manifest = "v1 cell linux_x86_64\nprod source pkg/Foo.c\n";
    let report = [lcov("pkg/Foo.c", &[(1, 0)]), lcov("pkg/FOO.c", &[(1, 1)])];
    let got = gate(manifest, &report, &added(&[("pkg/foo.c", &[1])]), &[]);
    assert_eq!((got.covered_lines, got.eligible_lines), (0, 1));
}

#[test]
fn test_support_and_unowned_files_are_skipped() {
    let manifest = "v1 cell linux_x86_64\nprod source src/a.py\ntest source src/a_test.py\n";
    let report = [
        lcov("src/a.py", &[(1, 1)]),
        lcov("src/a_test.py", &[(1, 0)]),
    ];
    let got = gate(
        manifest,
        &report,
        &added(&[
            ("src/a_test.py", &[1]),
            ("src/other.py", &[1]),
            ("README.md", &[1]),
        ]),
        &[],
    );
    assert_eq!(got, ChangedAccount::default());
    assert_eq!(got.result(100), ChangedResult::NotApplicable);
}

#[test]
fn ignored_changed_line_is_not_counted() {
    let marker = ["LCOV", "_EXCL", "_LINE"].concat();
    let source = format!("a = 1\nb = 2  # {marker} - reason: glue, issue: 1.\n");
    let report = [lcov("src/a.py", &[(1, 1), (2, 0)])];
    let got = gate(
        OWNS_A,
        &report,
        &added(&[("src/a.py", &[1, 2])]),
        &[("src/a.py", &source)],
    );
    assert_eq!((got.covered_lines, got.eligible_lines), (1, 1));
    assert_eq!(got.result(100), ChangedResult::Pass);
}

#[test]
fn diff_and_report_paths_share_keys() {
    let report = [lcov("bazel-out/k8-fastbuild/bin/src/a.py", &[(1, 0)])];
    let got = gate(OWNS_A, &report, &added(&[("./src/a.py", &[1])]), &[]);
    assert_eq!(got.eligible_lines, 1);
    assert_eq!(got.result(50), ChangedResult::Fail);
}

#[test]
fn result_names_are_stable() {
    assert_eq!(ChangedResult::Pass.name(), "pass");
    assert_eq!(ChangedResult::Fail.name(), "fail");
    assert_eq!(ChangedResult::NotApplicable.name(), "not-applicable");
    assert_eq!(ChangedResult::Incomplete.name(), "incomplete");
}

#[test]
fn unreadable_source_is_an_error() {
    let report = [lcov("src/a.py", &[(1, 0)])];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[1])]), &[]);
    assert_eq!(got.errors.len(), 1);
    assert!(
        got.errors[0].contains("cannot read src/a.py"),
        "{:?}",
        got.errors
    );
}

#[test]
fn quoted_paths_decode_every_git_escape() {
    let diff = "--- \"a/\\n\\t\\a\\b\\f\\v\\r\\1x\\0412.py\"\n+++ \"b/\\n\\t\\a\\b\\f\\v\\r\\1x\\0412.py\"\n@@ -0,0 +1 @@\n+a\n";
    let parsed = parse_added_lines(diff);
    assert_eq!(
        parsed.keys().collect::<Vec<_>>(),
        ["\n\t\u{7}\u{8}\u{c}\u{b}\r\u{1}x!2.py"]
    );
}

#[test]
fn quoted_path_ending_in_a_backslash_reads_as_a_separator() {
    let diff = "--- \"a/x\\\"\n+++ \"b/x\\\"\n@@ -0,0 +1 @@\n+a\n";
    assert_eq!(parse_added_lines(diff).keys().collect::<Vec<_>>(), ["x/"]);
}

#[test]
fn malformed_ignore_marker_is_an_error() {
    let marker = ["LCOV", "_EXCL", "_LINE"].concat();
    let source = format!("a = 1  # {marker}\n");
    let report = [lcov("src/a.py", &[(1, 0)])];
    let got = gate(
        OWNS_A,
        &report,
        &added(&[("src/a.py", &[1])]),
        &[("src/a.py", &source)],
    );
    assert_eq!(got.errors.len(), 1, "{:?}", got.errors);
}

#[test]
fn unparsable_report_is_an_error() {
    let report = ["SF:src/a.py\nDA:not-a-number,1\nend_of_record\n".to_string()];
    let got = gate(OWNS_A, &report, &added(&[("src/a.py", &[1])]), &[]);
    assert_eq!(got.errors.len(), 1);
    assert!(got.errors[0].starts_with("invalid coverage report: "));
}
