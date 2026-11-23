use std::collections::BTreeMap;
use std::io::Write as _;

use comment_budget::{
    BUDGET, INPUTS, KIND, MIN_FILES, UPDATE, comment_lines, found, lowered, over_budget, scanned,
    skipped, under_budget,
};
use dx_guard::allowlist::{Counts, add, parse, render};
use dx_guard::snapshot::read_input;
use starlark_syntax as _;
use yaml_serde as _;

#[test]
fn no_file_goes_over_its_comment_budget() {
    let mut files = BTreeMap::new();
    for variable in INPUTS {
        let part = serde_json::from_str::<BTreeMap<String, String>>(&read_input(variable));
        assert!(part.is_ok(), "{variable}: {part:?}");
        files.extend(part.unwrap_or_default());
    }
    let files = scanned(files);
    assert!(
        files.len() >= MIN_FILES,
        "only {} files scanned",
        files.len()
    );
    let found = found(&files);
    let (budget, mut problems) = parse(&read_input("COMMENT_BUDGET"), &[KIND]);
    let lower = under_budget(&found, &budget);
    if !lower.is_empty() {
        let written = writeln!(
            std::io::stderr(),
            "{}\nThese files are under their budget in {BUDGET}. Lower it with `{UPDATE}`.",
            lower.join("\n")
        );
        assert!(written.is_ok(), "{written:?}");
    }
    problems.extend(over_budget(&found, &budget));
    assert!(
        problems.is_empty(),
        "{}\nDelete the new comments. Never raise a budget; `{UPDATE}` only lowers it.",
        problems.join("\n")
    );
}

#[test]
fn counts_a_plain_comment() {
    assert_eq!(comment_lines("a.rs", "// Adds one.\nfn f() {}\n"), 1);
    assert_eq!(comment_lines("a.rs", "let a = 1; // one\n"), 1);
    assert_eq!(comment_lines("a.rs", "/* two\n   lines */\nfn f() {}\n"), 2);
    assert_eq!(comment_lines("a/BUILD.bazel", "# Builds a.\nx = 1\n"), 1);
    assert_eq!(comment_lines("a.go", "// Package a.\npackage a\n"), 1);
    assert_eq!(comment_lines("a.py", "x = 1  # one\n"), 1);
    assert_eq!(comment_lines("a.sh", "# Runs a.\necho a # b\n"), 2);
    assert_eq!(comment_lines("a.ts", "const a = 1; // one\n"), 1);
    assert_eq!(comment_lines("a.cc", "int a; /* one */\n"), 1);
    assert_eq!(comment_lines("a.yml", "# Top.\nkey: value # why\n"), 2);
}

#[test]
fn counts_a_doc_comment() {
    assert_eq!(
        comment_lines(
            "a.rs",
            "//! The crate.\n\n/// Adds one.\n/// Twice.\nfn f() {}\n"
        ),
        3
    );
    assert_eq!(comment_lines("a.go", "// F adds one.\nfunc F() {}\n"), 1);
}

#[test]
fn skips_keep_markers() {
    let rust = "// SAFETY: the pointer is valid.\nunsafe { f() }\n\
                // LCOV_EXCL_START reason: r issue: 1\n";
    assert_eq!(comment_lines("a.rs", rust), 0);
    let starlark = "# gazelle:exclude x\nbazel_dep(name = \"a\")  # floor: needs b\n\
                    srcs = [\"a\"],  # keep\n";
    assert_eq!(comment_lines("a/BUILD.bazel", starlark), 0);
    assert_eq!(comment_lines("a.go", "//go:build linux\n\npackage a\n"), 0);
    let parsed = "# gazelle:ignore\n# Why: gazelle merges a and b.\n\
                  # Upstream: a/b#1\n# dx: keep\n";
    assert_eq!(comment_lines("a/BUILD.bazel", parsed), 0);
    assert_eq!(
        comment_lines("a.py", "#!/usr/bin/env python3\n# -*- coding: utf-8 -*-\n"),
        0
    );
    assert_eq!(
        comment_lines("a.sh", "echo $a # shellcheck disable=SC2086\n"),
        0
    );
    let pin = "    - uses: actions/checkout@0123456789abcdef0123456789abcdef01234567 # v4.1.0\n";
    assert_eq!(comment_lines("a.yml", pin), 0);
    let license =
        "// Copyright 2024 A.\n// Licensed under the Apache License.\n\n// Plain.\nint a;\n";
    assert_eq!(comment_lines("a.cc", license), 1);
}

#[test]
fn skips_clap_help_lines() {
    let source = "/// The tool.\n#[derive(Debug, Parser)]\n#[command(name = \"x\")]\nstruct Cli {\n\
                  \x20   /// Print more.\n    #[arg(long)]\n    verbose: bool,\n    // Plain.\n}\n\n\
                  /// Not help.\nfn f() {}\n\n\
                  #[derive(\n    Clone,\n    ValueEnum,\n)]\nenum Mode {\n    /// Fast.\n    Fast,\n}\n";
    assert_eq!(comment_lines("a.rs", source), 2);
}

#[test]
fn skips_strings_that_hold_comment_markers() {
    assert_eq!(
        comment_lines(
            "a.rs",
            "let a = \"https://x\";\nlet b = r#\"a // \"b\" c\"#;\nlet c = '\"'; let d = \"/* e\";\n"
        ),
        0
    );
    assert_eq!(comment_lines("a.rs", "let a = \"//\"; // real\n"), 1);
    assert_eq!(
        comment_lines(
            "a/BUILD.bazel",
            "deps = [\"//a:b\"]\nx = \"\"\"\n# not\n\"\"\"\n"
        ),
        0
    );
    assert_eq!(comment_lines("a.go", "var a = `\n// not\n`\n"), 0);
    assert_eq!(
        comment_lines(
            "a.sh",
            "echo \"a # b\" $# ${#x} 'c # d'\ncat <<'EOF'\n# not\nEOF\n# real\n"
        ),
        1
    );
    assert_eq!(
        comment_lines("a.yml", "run: |\n  # not\n  echo a\nname: \"a # b\"\n"),
        0
    );
    assert_eq!(
        comment_lines("a.js", "const r = /\\/\\//g; const s = '//';\n"),
        0
    );
    assert_eq!(comment_lines("a.cc", "auto s = R\"x(// not)x\";\n"), 0);
}

#[test]
fn skips_generated_files_and_skipped_paths() {
    assert_eq!(
        comment_lines(
            "a.go",
            "// Code generated by x. DO NOT EDIT.\n\n// F.\nfunc F() {}\n"
        ),
        0
    );
    for path in [
        "a/testdata/b.rs",
        "a/fixtures/b.py",
        "tools/docs/go/fixture/money.go",
        "tools/ci/input_validation_fixtures/a.yml",
        "tools/testing/hermetic/consumers/cc_fixture/c_only.cc",
        "third_party/b.go",
        "a/goldens/b.yaml",
        "a/pnpm-lock.yaml",
        ".dx/BUILD.bazel",
    ] {
        assert!(skipped(path), "{path}");
    }
    assert!(!skipped("tools/testing/hermetic/manifest_golden_test.rs"));
}

fn counts(entries: &[(&str, usize)]) -> Counts {
    let mut counts = Counts::new();
    for (path, count) in entries {
        add(&mut counts, path, KIND, *count);
    }
    counts
}

#[test]
fn fails_a_file_over_its_budget() {
    let found = counts(&[("cli/a.rs", 3), ("cli/b.rs", 1), ("go/c.go", 5)]);
    let budget = counts(&[("cli/a.rs", 1), ("cli/b.rs", 2), ("go/c.go", 6)]);
    assert_eq!(
        over_budget(&found, &budget),
        ["cli/a.rs: 3 comment lines, budget 1"]
    );
}

#[test]
fn an_unlisted_file_has_no_budget() {
    let found = counts(&[("cli/a.rs", 1), ("cli/new.rs", 2)]);
    let budget = counts(&[("cli/a.rs", 4)]);
    assert_eq!(
        over_budget(&found, &budget),
        ["cli/new.rs: 2 comment lines, budget 0"]
    );
}

#[test]
fn a_decrease_passes_and_notes_the_lower_budget() {
    let found = counts(&[("cli/a.rs", 1), ("cli/b.rs", 2)]);
    let budget = counts(&[("cli/a.rs", 4), ("cli/b.rs", 2), ("cli/gone.rs", 3)]);
    assert!(over_budget(&found, &budget).is_empty());
    assert_eq!(
        under_budget(&found, &budget),
        [
            "cli/a.rs: 1 comment lines, budget 4",
            "cli/gone.rs: 0 comment lines, budget 3",
        ]
    );
}

#[test]
fn update_only_lowers_or_drops_lines() {
    let found = counts(&[("cli/a.rs", 1), ("cli/b.rs", 5), ("cli/new.rs", 2)]);
    let budget = counts(&[("cli/a.rs", 4), ("cli/b.rs", 2), ("cli/gone.rs", 3)]);
    assert_eq!(
        render(&lowered(&found, &budget)),
        "cli/a.rs comment 1\ncli/b.rs comment 2\n"
    );
}
