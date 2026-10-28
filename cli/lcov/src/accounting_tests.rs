use super::*;

fn marker(kind: &str) -> String {
    ["LCOV", "_EXCL", kind].concat()
}

fn manifest(text: &str) -> SourceManifest {
    parse_source_manifest(text).unwrap()
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

fn lcov(path: &str, lines: &[(u32, u64)]) -> String {
    let mut out = format!("SF:{path}\n");
    for (line, count) in lines {
        out.push_str(&format!("DA:{line},{count}\n"));
    }
    out.push_str("end_of_record\n");
    out
}

fn excluded_source() -> String {
    format!(
        "// {} - reason: generated glue, issue: 1.\nfn a() {{}}\n// {} - reason: end, issue: 1.\n",
        marker("_START"),
        marker("_STOP")
    )
}

#[test]
fn parses_rows_with_kind_and_origin() {
    let parsed = manifest("v1 cell linux_x86_64\nprod source a.rs\ntest generated b/c d.rs\n");
    assert_eq!(parsed.cell, "linux_x86_64");
    assert_eq!(
        parsed.rows,
        vec![
            SourceRow {
                path: "a.rs".to_string(),
                test: false,
                generated: false
            },
            SourceRow {
                path: "b/c d.rs".to_string(),
                test: true,
                generated: true
            },
        ]
    );
}

#[test]
fn rejects_bad_headers_and_unknown_cells() {
    for text in [
        "",
        "v2 cell linux_x86_64\n",
        "v1 cell\n",
        "v1 cell \n",
        "v1 os linux\n",
    ] {
        let error = parse_source_manifest(text).unwrap_err();
        assert!(
            error.contains("unsupported source manifest header"),
            "{error}"
        );
    }
    let error = parse_source_manifest("v1 cell unknown\nprod source a.rs\n").unwrap_err();
    assert!(error.contains("unknown cell"), "{error}");
}

#[test]
fn rejects_malformed_rows() {
    let cases = [
        ("prod a.rs", "malformed source manifest row 2"),
        ("prod source ", "malformed source manifest row 2"),
        ("lib source a.rs", "unknown source kind \"lib\" at row 2"),
        (
            "prod vendored a.rs",
            "unknown source origin \"vendored\" at row 2",
        ),
        ("cell linux arm64", "more than one cell at row 2"),
    ];
    for (row, want) in cases {
        let error = parse_source_manifest(&format!("v1 cell linux_x86_64\n{row}\n")).unwrap_err();
        assert!(error.contains(want), "{row}: {error}");
    }
}

#[test]
fn rejects_duplicates_and_aliases() {
    let error = parse_source_manifest("v1 cell linux_x86_64\nprod source a.rs\ntest source a.rs\n")
        .unwrap_err();
    assert!(
        error.contains("duplicate path in source manifest: a.rs"),
        "{error}"
    );
    let error =
        parse_source_manifest("v1 cell linux_x86_64\nprod source a.rs\nprod source ./a.rs\n")
            .unwrap_err();
    assert!(error.contains("path alias in source manifest"), "{error}");
}

#[test]
fn unexecuted_module_is_reported_even_when_every_reported_line_is_hit() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source lib/lib.rs\nprod source lib/split.rs\n",
    )];
    let load = sources(&[
        ("lib/lib.rs", "fn a() {}\n"),
        ("lib/split.rs", "fn b() {}\n"),
    ]);
    let result = account(&manifests, &[lcov("lib/lib.rs", &[(1, 3)])], &load);
    assert_eq!(
        result.errors,
        vec!["production source absent from coverage report: lib/split.rs"]
    );
    assert_eq!(result.eligible_files, 1);
    assert_eq!(result.files["lib/lib.rs"], Disposition::Eligible);
    assert!(!result.files.contains_key("lib/split.rs"));
    assert_eq!((result.covered_lines, result.eligible_lines), (1, 1));
    assert!(!result.meets(100));
    assert!(!result.meets(0));
}

#[test]
fn orphan_report_path_is_an_error() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let documents = [lcov("a.rs", &[(1, 1)]), lcov("stray.rs", &[(1, 1)])];
    let result = account(&manifests, &documents, &sources(&[("a.rs", "fn a() {}\n")]));
    assert_eq!(
        result.errors,
        vec!["coverage report path not owned by any target: stray.rs"]
    );
}

#[test]
fn zero_hit_file_is_eligible_and_counted() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let load = sources(&[("a.rs", "fn a() {\n}\n")]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 0), (2, 0)])], &load);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["a.rs"], Disposition::Eligible);
    assert_eq!((result.raw_lines, result.eligible_lines), (2, 2));
}

#[test]
fn excluded_lines_are_counted_apart() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let source = format!(
        "fn a() {{}}\nfn b() {{}} // {} - reason: defensive, issue: 1.\n",
        marker("_LINE")
    );
    let load = sources(&[("a.rs", source.as_str())]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1), (2, 0)])], &load);
    assert_eq!(
        (
            result.raw_lines,
            result.excluded_lines,
            result.eligible_lines
        ),
        (2, 1, 1)
    );
}

#[test]
fn unreadable_reported_source_counts_every_line() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1)])], &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.eligible_lines, 1);
}

#[test]
fn header_without_lines_is_support() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source cc/a.h\nprod source cc/a.cc\n",
    )];
    let documents = [lcov("cc/a.h", &[]), lcov("cc/a.cc", &[(1, 1)])];
    let result = account(&manifests, &documents, &sources(&[("cc/a.cc", "int a;\n")]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["cc/a.h"], Disposition::Support);
    assert_eq!(result.support_files, 1);
    assert_eq!(result.eligible_files, 1);
}

#[test]
fn header_absent_from_the_report_is_support() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source cc/decl.hpp\nprod source cc/a.cc\n",
    )];
    let documents = [lcov("cc/a.cc", &[(1, 1)])];
    let result = account(
        &manifests,
        &documents,
        &sources(&[("cc/decl.hpp", "int a();\n"), ("cc/a.cc", "int a;\n")]),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["cc/decl.hpp"], Disposition::Support);
    assert_eq!(result.support_files, 1);
}

#[test]
fn declaration_stubs_are_support() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source ts/a.d.ts\nprod source ts/b.d.mts\n",
    )];
    let result = account(&manifests, &[], &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["ts/a.d.ts"], Disposition::Support);
    assert_eq!(result.files["ts/b.d.mts"], Disposition::Support);
    assert_eq!(result.unsupported_files, 0);
    assert_eq!(result.support_files, 2);
}

#[test]
fn generated_report_path_matches_its_short_path() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod generated gen/out.cc\n",
    )];
    let documents = [lcov("bazel-out/k8-fastbuild/bin/gen/out.cc", &[(1, 1)])];
    let result = account(&manifests, &documents, &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["gen/out.cc"], Disposition::Eligible);
}

#[test]
fn bazel_out_path_without_bin_keeps_its_spelling() {
    assert_eq!(
        source_key("bazel-out/stable-status.txt"),
        "bazel-out/stable-status.txt"
    );
    assert_eq!(source_key("other/a.rs"), "other/a.rs");
}

#[test]
fn dot_slash_report_path_matches() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let documents = [lcov("./a.rs", &[(1, 1)])];
    let result = account(&manifests, &documents, &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.eligible_files, 1);
}

#[test]
fn cell_only_file_is_required_in_that_cell_only() {
    let x86 = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let arm = [manifest(
        "v1 cell linux_arm64\nprod source a.rs\nprod source arm.rs\n",
    )];
    let documents = [lcov("a.rs", &[(1, 1)])];
    assert!(account(&x86, &documents, &sources(&[])).errors.is_empty());
    let result = account(&arm, &documents, &sources(&[]));
    assert_eq!(result.cell, "linux_arm64");
    assert_eq!(
        result.errors,
        vec!["production source absent from coverage report: arm.rs"]
    );
}

#[test]
fn mixed_cells_are_an_error() {
    let manifests = [
        manifest("v1 cell linux_x86_64\nprod source a.rs\n"),
        manifest("v1 cell linux_arm64\nprod source b.rs\n"),
    ];
    let documents = [lcov("a.rs", &[(1, 1)]), lcov("b.rs", &[(1, 1)])];
    let result = account(&manifests, &documents, &sources(&[]));
    assert_eq!(
        result.errors,
        vec!["coverage source manifests name more than one cell: linux_arm64, linux_x86_64"]
    );
}

#[test]
fn empty_manifest_set_is_an_error() {
    let result = account(&[], &[lcov("a.rs", &[(1, 1)])], &sources(&[]));
    assert_eq!(result.cell, "unknown");
    assert_eq!(
        result.errors,
        vec![
            "no coverage source manifests were reported",
            "coverage report path not owned by any target: a.rs",
        ]
    );
}

#[test]
fn invalid_report_is_an_error() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let result = account(&manifests, &["DA:1,1\n".to_string()], &sources(&[]));
    assert_eq!(result.errors.len(), 2, "{:?}", result.errors);
    assert!(result.errors[0].starts_with("invalid coverage report: "));
    assert_eq!(
        result.errors[1],
        "production source absent from coverage report: a.rs"
    );
}

#[test]
fn prod_wins_over_test_across_manifests() {
    let manifests = [
        manifest("v1 cell linux_x86_64\ntest source a.rs\n"),
        manifest("v1 cell linux_x86_64\nprod source a.rs\n"),
        manifest("v1 cell linux_x86_64\nprod source b.rs\n"),
        manifest("v1 cell linux_x86_64\ntest source b.rs\n"),
    ];
    let result = account(&manifests, &[], &sources(&[]));
    assert_eq!(
        result.errors,
        vec![
            "production source absent from coverage report: a.rs",
            "production source absent from coverage report: b.rs",
        ]
    );
}

#[test]
fn testonly_sources_are_support_even_when_absent() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\ntest source helper.rs\ntest source helper_test.rs\n",
    )];
    let result = account(&manifests, &[lcov("helper.rs", &[(1, 0)])], &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.support_files, 2);
    assert_eq!(result.eligible_lines, 0);
}

#[test]
fn unsupported_files_are_listed_and_counted() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source tool.sh\nprod source rules.bzl\n",
    )];
    let result = account(&manifests, &[], &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.unsupported_files, 2);
    assert_eq!(result.files["tool.sh"], Disposition::Unsupported);
    assert_eq!(result.files["rules.bzl"], Disposition::Unsupported);
}

#[test]
fn whole_file_exclusion_accounts_an_absent_source() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let source = format!("\n{}", excluded_source());
    let result = account(&manifests, &[], &sources(&[("a.rs", source.as_str())]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.files["a.rs"], Disposition::Excluded);
    assert_eq!(result.excluded_files, 1);
}

#[test]
fn whole_file_exclusion_reads_each_comment_syntax() {
    for (path, comment) in [("a.py", "#"), ("a.erl", "%")] {
        let source = format!(
            "{comment} {} - reason: glue, issue: 1.\nx = 1\n{comment} {} - reason: end, issue: 1.\n",
            marker("_START"),
            marker("_STOP")
        );
        let manifests = [manifest(&format!(
            "v1 cell linux_x86_64\nprod source {path}\n"
        ))];
        let result = account(&manifests, &[], &sources(&[(path, source.as_str())]));
        assert_eq!(result.excluded_files, 1, "{path}: {:?}", result.errors);
    }
}

#[test]
fn partial_or_invalid_exclusion_does_not_account_an_absent_source() {
    let partial = format!("{}fn outside() {{}}\n", excluded_source());
    let unmarked = "fn a() {}\n".to_string();
    let invalid = format!("fn a() {{}} // {}\n", marker("_LINE"));
    for source in [partial, unmarked, invalid] {
        let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
        let result = account(&manifests, &[], &sources(&[("a.rs", source.as_str())]));
        assert_eq!(
            result.errors,
            vec!["production source absent from coverage report: a.rs"],
            "{source}"
        );
    }
}

#[test]
fn summary_names_every_count() {
    let result = CellAccount {
        cell: "linux_x86_64".to_string(),
        raw_lines: 10,
        excluded_lines: 2,
        eligible_lines: 8,
        covered_lines: 7,
        eligible_files: 3,
        support_files: 4,
        excluded_files: 1,
        unsupported_files: 5,
        ..CellAccount::default()
    };
    assert_eq!(
        result.summary(),
        "coverage sources linux_x86_64: 10 raw, 2 excluded, 8 eligible, 7 covered lines; 3 eligible, 4 support, 1 excluded, 5 unsupported files"
    );
}

#[test]
fn absent_lists_only_missing_production_sources() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source a.rs\nprod source missing.rs\ntest source t_test.rs\nprod source cc/decl.hpp\n",
    )];
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1)])], &sources(&[]));
    assert_eq!(result.absent, vec!["missing.rs".to_string()]);
}

#[test]
fn whole_file_excluded_source_is_not_absent() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let source = format!("\n{}", excluded_source());
    let result = account(&manifests, &[], &sources(&[("a.rs", source.as_str())]));
    assert!(result.absent.is_empty());
}

#[test]
fn zero_hit_eligible_line_fails_only_a_nonzero_minimum() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let load = sources(&[("a.rs", "fn a() {\n}\n")]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1), (2, 0)])], &load);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!((result.covered_lines, result.eligible_lines), (1, 2));
    assert!(!result.meets(100));
    assert!(result.meets(50));
    assert!(result.meets(0));
}

#[test]
fn fully_covered_cell_meets_one_hundred() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let load = sources(&[("a.rs", "fn a() {\n}\n")]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1), (2, 4)])], &load);
    assert_eq!((result.covered_lines, result.eligible_lines), (2, 2));
    assert!(result.meets(100));
}

#[test]
fn empty_denominator_never_meets() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source tool.sh\n")];
    let result = account(&manifests, &[], &sources(&[]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.eligible_lines, 0);
    assert!(!result.meets(0));
}

#[test]
fn invalid_marker_in_a_reported_source_is_an_error() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let source = format!("// {}\nfn a() {{}}\n", marker("_LINE"));
    let load = sources(&[("a.rs", source.as_str())]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1), (2, 1)])], &load);
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert!(result.errors[0].contains("a.rs"), "{:?}", result.errors);
    assert_eq!((result.covered_lines, result.eligible_lines), (2, 2));
    assert!(!result.meets(100));
}

#[test]
fn unloadable_source_counts_raw_with_hits() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let result = account(
        &manifests,
        &[lcov("a.rs", &[(1, 1), (2, 0)])],
        &sources(&[]),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        (
            result.raw_lines,
            result.eligible_lines,
            result.covered_lines
        ),
        (2, 2, 1)
    );
}

#[test]
fn reported_unsupported_language_stays_out_of_the_denominator() {
    let manifests = [manifest(
        "v1 cell linux_x86_64\nprod source a.rs\nprod source tool.sh\n",
    )];
    let documents = [lcov("a.rs", &[(1, 1)]), lcov("tool.sh", &[(1, 0), (2, 0)])];
    let result = account(&manifests, &documents, &sources(&[("a.rs", "fn a() {}\n")]));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!((result.covered_lines, result.eligible_lines), (1, 1));
    assert_eq!(result.files["tool.sh"], Disposition::Unsupported);
    assert!(result.meets(100));
}

#[test]
fn excluded_lines_leave_the_covered_count() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let source = format!(
        "fn a() {{}}\nfn b() {{}} // {} - reason: defensive, issue: 1.\n",
        marker("_LINE")
    );
    let load = sources(&[("a.rs", source.as_str())]);
    let result = account(&manifests, &[lcov("a.rs", &[(1, 1), (2, 3)])], &load);
    assert_eq!((result.covered_lines, result.eligible_lines), (1, 1));
}

#[test]
fn duplicate_records_union_with_max_hits() {
    let manifests = [manifest("v1 cell linux_x86_64\nprod source a.rs\n")];
    let documents = [
        lcov("a.rs", &[(1, 0), (2, 1)]),
        lcov("a.rs", &[(1, 3), (3, 0)]),
    ];
    let load = sources(&[("a.rs", "fn a() {}\nfn b() {}\nfn c() {}\n")]);
    let result = account(&manifests, &documents, &load);
    assert_eq!((result.covered_lines, result.eligible_lines), (2, 3));
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn orphan_candidates_keep_covered_languages_only() {
    let tracked = strings(&[
        "b/lib.rs",
        "a/main.go",
        "a/main.go",
        "include/api.h",
        "types/api.d.ts",
        "types/api.ts",
        "README.md",
        ".dx/state/x.rs",
        "vendor/dep/x.rs",
        "vendored/x.rs",
    ]);
    let ignored = strings(&["vendor"]);
    assert_eq!(
        orphan_candidates(&tracked, &ignored),
        strings(&[
            "a/main.go",
            "b/lib.rs",
            "include/api.h",
            "types/api.ts",
            "vendored/x.rs"
        ])
    );
}

#[test]
fn recorded_orphans_become_sorted_deduped_errors() {
    let mut result = CellAccount {
        eligible_lines: 1,
        covered_lines: 1,
        ..CellAccount::default()
    };
    assert!(result.meets(0));
    result.record_orphans(strings(&["z.rs", "a.rs", "z.rs"]));
    assert_eq!(result.orphans, strings(&["a.rs", "z.rs"]));
    assert_eq!(
        result.errors,
        vec![
            "tracked source not owned by any Bazel target: a.rs",
            "tracked source not owned by any Bazel target: z.rs"
        ]
    );
    assert!(!result.meets(0));
}
