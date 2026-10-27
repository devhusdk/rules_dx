use super::*;
use crate::{find_ignores, parse_lcov, FileHits, LcovError, ELIGIBLE, SUPPORT};
use std::collections::BTreeMap;

type BranchHit = ((u32, u32, u32), Option<u64>);

fn marker(kind: &str) -> String {
    ["LCOV", "_EXCL", kind].concat()
}

fn file_lines(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn hits(pairs: &[(u32, u64)]) -> FileHits {
    let mut file = FileHits::default();
    for (line, count) in pairs {
        file.lines.insert(*line, *count);
    }
    file
}

fn branch_file(lines: &[(u32, u64)], branches: &[BranchHit]) -> FileHits {
    let mut file = hits(lines);
    for (id, taken) in branches {
        file.branches.insert(*id, *taken);
    }
    file.branch_data = true;
    file
}

fn report_of(path: &str, pairs: &[(u32, u64)]) -> BTreeMap<String, FileHits> {
    let mut report = BTreeMap::new();
    report.insert(path.to_string(), hits(pairs));
    report
}

fn loader<'a>(files: BTreeMap<&'a str, String>) -> impl Fn(&str) -> Result<String, LcovError> + 'a {
    move |path| {
        files.get(path).cloned().ok_or_else(|| LcovError::Io {
            message: format!("no such fixture: {path}"),
        })
    }
}

fn eligible_inventory(paths: &[&str]) -> BTreeMap<String, String> {
    let mut inventory = BTreeMap::new();
    for path in paths {
        inventory.insert(path.to_string(), ELIGIBLE.to_string());
    }
    inventory
}

#[test]
fn valid_ignore_shrinks_denominator_only_for_its_line() {
    let source = file_lines(&[
        "pub fn classify(n: u32) -> u32 {".to_string(),
        "    if n == 0 {".to_string(),
        format!(
            "        return 0; // {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "    }".to_string(),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1), (2, 1), (3, 0), (5, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 3);
    assert_eq!(verdict.eligible, 3);
    assert_eq!(verdict.files[0].ignored, 1);
}

#[test]
fn uncovered_line_fails_with_location() {
    let files = BTreeMap::from([("elf.rs", "fn f() {\n    1\n}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1), (2, 0)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.files[0].uncovered, vec![2]);
    let text = render(&verdict);
    assert!(text.contains("FAIL") && text.contains("elf.rs:2"), "{text}");
}

#[test]
fn python_valid_ignore_shrinks_denominator() {
    let source = file_lines(&[
        "def classify(n):".to_string(),
        "    if n == 0:".to_string(),
        format!(
            "        return 0  # {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "    return n".to_string(),
    ]);
    let files = BTreeMap::from([("elf.py", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.py"]),
        &["elf.py".to_string()],
        &report_of("elf.py", &[(1, 1), (2, 1), (3, 0), (4, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 3);
    assert_eq!(verdict.eligible, 3);
    assert_eq!(verdict.files[0].ignored, 1);
}

#[test]
fn python_routes_to_files_not_other_sources() {
    let files = BTreeMap::from([("elf.py", "x = 1\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.py"]),
        &["elf.py".to_string()],
        &report_of("elf.py", &[(1, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files.len(), 1);
}

#[test]
fn python_absent_eligible_source_fails() {
    let files = BTreeMap::from([("elf.py", "x = 1\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.py"]),
        &["elf.py".to_string()],
        &BTreeMap::new(),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("absent")),
        "{verdict:?}"
    );
}

#[test]
fn python_uncovered_line_fails_with_location() {
    let files = BTreeMap::from([("elf.py", "x = 1\ny = 2\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.py"]),
        &["elf.py".to_string()],
        &report_of("elf.py", &[(1, 1), (2, 0)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.files[0].uncovered, vec![2]);
    let text = render(&verdict);
    assert!(text.contains("FAIL") && text.contains("elf.py:2"), "{text}");
}

#[test]
fn javascript_routes_to_files_with_slash_markers() {
    let source = file_lines(&[
        "export function f() {".to_string(),
        format!(
            "  return 0; // {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.js", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.js"]),
        &["elf.js".to_string()],
        &report_of("elf.js", &[(1, 1), (2, 0), (3, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.covered, 2);
    assert_eq!(verdict.eligible, 2);
    assert_eq!(verdict.files[0].ignored, 1);
}

#[test]
fn absent_eligible_source_fails() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &BTreeMap::new(),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("absent")),
        "{verdict:?}"
    );
    assert!(
        verdict.errors.iter().any(|e| e.contains("no SF records")),
        "{verdict:?}"
    );
}

#[test]
fn unowned_bazel_source_fails() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string(), "ghost.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("ghost.rs")),
        "{verdict:?}"
    );
}

#[test]
fn stale_eligible_inventory_fails() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs", "stale.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("stale.rs")),
        "{verdict:?}"
    );
}

#[test]
fn unknown_disposition_fails() {
    let mut inventory = BTreeMap::new();
    inventory.insert("elf.rs".to_string(), "maybe".to_string());
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &inventory,
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|e| e.contains("unknown disposition")),
        "{verdict:?}"
    );
}

#[test]
fn zero_da_lines_without_ignores_fails() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|e| e.contains("no instrumented lines")),
        "{verdict:?}"
    );
}

#[test]
fn fully_ignored_zero_da_file_passes_with_zero_counts() {
    let source = file_lines(&[
        format!("// {} - reason: shim, issue: 1055.", marker("_START")),
        "fn main() {}".to_string(),
        format!("// {} - reason: shim end, issue: 1055.", marker("_STOP")),
    ]);
    let live = file_lines(&["fn f() {}".to_string()]);
    let files = BTreeMap::from([("shim.rs", source), ("elf.rs", live)]);
    let mut report = report_of("elf.rs", &[(1, 2)]);
    report.insert("shim.rs".to_string(), FileHits::default());
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs", "shim.rs"]),
        &["elf.rs".to_string(), "shim.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 1);
    assert_eq!(verdict.eligible, 1);
}

#[test]
fn empty_denominator_is_never_a_pass() {
    let source = file_lines(&[
        format!("// {} - reason: shim, issue: 1055.", marker("_START")),
        "fn main() {}".to_string(),
        format!("// {} - reason: shim end, issue: 1055.", marker("_STOP")),
    ]);
    let files = BTreeMap::from([("shim.rs", source)]);
    let mut report = BTreeMap::new();
    report.insert("shim.rs".to_string(), FileHits::default());
    let verdict = evaluate(
        &eligible_inventory(&["shim.rs"]),
        &["shim.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|e| e.contains("empty denominator")),
        "{verdict:?}"
    );
    let text = render(&verdict);
    assert!(text.contains("n/a (no eligible lines)"), "{text}");
}

#[test]
fn support_hits_are_skipped_and_others_listed() {
    let live = "fn f() {}\n".to_string();
    let files = BTreeMap::from([("elf.rs", live)]);
    let mut inventory = eligible_inventory(&["elf.rs"]);
    inventory.insert("tool.rs".to_string(), SUPPORT.to_string());
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("tool.rs".to_string(), hits(&[(1, 0)]));
    report.insert("notes.txt".to_string(), hits(&[(7, 1)]));
    let verdict = evaluate(
        &inventory,
        &["elf.rs".to_string(), "tool.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 1);
    assert_eq!(verdict.eligible, 1);
    assert_eq!(verdict.other_sources, vec!["notes.txt".to_string()]);
    let text = render(&verdict);
    assert!(
        text.contains("PASS") && text.contains("not counted"),
        "{text}"
    );
}

#[test]
fn cc_routes_to_files_with_slash_markers() {
    let source = file_lines(&[
        "int Add(int a, int b) {".to_string(),
        "  return a + b;".to_string(),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.cc", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.cc"]),
        &["elf.cc".to_string()],
        &report_of("elf.cc", &[(1, 1), (2, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files.len(), 1);
}

#[test]
fn cc_uncovered_line_fails_with_location() {
    let files = BTreeMap::from([("elf.cc", "int f() {\n  return 1;\n}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.cc"]),
        &["elf.cc".to_string()],
        &report_of("elf.cc", &[(1, 1), (2, 0)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.files[0].uncovered, vec![2]);
}

#[test]
fn cc_header_routes_to_files() {
    let files = BTreeMap::from([("elf.h", "int Add(int a, int b);\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.h"]),
        &["elf.h".to_string()],
        &report_of("elf.h", &[(1, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
}

#[test]
fn uninventoried_cc_in_report_fails() {
    let files = BTreeMap::from([("elf.cc", "int f() { return 1; }\n".to_string())]);
    let mut report = report_of("elf.cc", &[(1, 1)]);
    report.insert("rogue.cc".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["elf.cc"]),
        &["elf.cc".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("rogue.cc")),
        "{verdict:?}"
    );
}

#[test]
fn uninventoried_rust_in_report_fails() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("rogue.rs".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("rogue.rs")),
        "{verdict:?}"
    );
}

#[test]
fn starlark_line_data_fails_until_route_exists() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("defs.bzl".to_string(), hits(&[(3, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(
        verdict.errors,
        ["unexpected Starlark line data for defs.bzl: the gate has no Starlark line route"],
        "{verdict:?}"
    );
}

#[test]
fn starlark_entry_without_lines_is_listed_only() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("defs.bzl".to_string(), FileHits::default());
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.other_sources, vec!["defs.bzl".to_string()]);
}

#[test]
fn unreadable_source_fails() {
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(BTreeMap::new()),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("no such fixture")),
        "{verdict:?}"
    );
}

#[test]
fn invalid_source_markers_fail_evaluation() {
    let source = file_lines(&[format!(
        "// {} - reason: stray stop, issue: 1055.",
        marker("_STOP")
    )]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|e| e.contains("without START")),
        "{verdict:?}"
    );
}

#[test]
fn covered_languages_include_wrapped_langs() {
    for path in [
        "elf.rs",
        "elf.go",
        "elf.py",
        "elf.js",
        "elf.jsx",
        "elf.mjs",
        "elf.cjs",
        "elf.ts",
        "elf.tsx",
        "elf.mts",
        "elf.cts",
        "elf.java",
        "elf.kt",
        "elf.scala",
        "elf.cs",
        "elf.fs",
        "elf.fsi",
        "elf.c",
        "elf.cc",
        "elf.cpp",
        "elf.cxx",
        "elf.h",
        "elf.hh",
        "elf.hpp",
        "elf.hxx",
        "elf.kts",
        "elf.rb",
        "elf.erl",
        "elf.hrl",
    ] {
        assert!(is_covered_language(path), "{path}");
    }
    for path in [
        "elf.pyi",
        "elf.bzl",
        "elf.sh",
        "elf.md",
        "notes.txt",
        "elf.svelte",
        "elf.vue",
        "elf.d.ts",
    ] {
        assert!(!is_covered_language(path), "{path}");
    }
}

#[test]
fn every_covered_language_is_scanned_with_its_own_comment_marker() {
    fn marker_for(syntax: LineComment) -> &'static str {
        match syntax {
            LineComment::SlashSlash => "//",
            LineComment::Hash => "#",
            LineComment::Percent => "%",
        }
    }
    for (extension, syntax) in COVERED_LANGUAGES {
        let path = format!("elf{extension}");
        let source = file_lines(&[format!(
            "{} {} - reason: fixture, issue: 1055.",
            marker_for(*syntax),
            marker("_LINE")
        )]);
        assert!(is_covered_language(&path), "{path}");
        let ignores = find_ignores(&path, &source).unwrap();
        assert!(ignores.singles.contains_key(&1), "{path}");
    }
    for path in ["elf.d.ts", "elf.d.mts", "elf.d.cts"] {
        let source = file_lines(&[format!(
            "// {} - reason: fixture, issue: 1055.",
            marker("_LINE")
        )]);
        assert!(!is_covered_language(path), "{path}");
        assert!(find_ignores(path, &source).unwrap().singles.is_empty());
    }
}

#[test]
fn java_routes_to_files_with_slash_markers() {
    let source = file_lines(&[
        "public class Elf {".to_string(),
        format!(
            "  int f() {{ return 0; }} // {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.java", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.java"]),
        &["elf.java".to_string()],
        &report_of("elf.java", &[(1, 1), (2, 0), (3, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.covered, 2);
    assert_eq!(verdict.eligible, 2);
    assert_eq!(verdict.files[0].ignored, 1);
}

#[test]
fn kotlin_routes_to_files() {
    let files = BTreeMap::from([("elf.kt", "fun f(): Int { return 1; }\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.kt"]),
        &["elf.kt".to_string()],
        &report_of("elf.kt", &[(1, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files.len(), 1);
}

#[test]
fn scala_routes_to_files() {
    let files = BTreeMap::from([("elf.scala", "object Elf { def f = 1 }\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.scala"]),
        &["elf.scala".to_string()],
        &report_of("elf.scala", &[(1, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files.len(), 1);
}

#[test]
fn csharp_routes_to_files_with_slash_markers() {
    let source = file_lines(&[
        "public static class Elf {".to_string(),
        format!(
            "  public static int F() => 0; // {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.cs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.cs"]),
        &["elf.cs".to_string()],
        &report_of("elf.cs", &[(1, 1), (2, 0), (3, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files[0].ignored, 1);
}

#[test]
fn fsharp_routes_to_files() {
    let files = BTreeMap::from([("elf.fs", "module Elf\nlet f = 1\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.fs"]),
        &["elf.fs".to_string()],
        &report_of("elf.fs", &[(1, 1), (2, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
    assert_eq!(verdict.files.len(), 1);
}

#[test]
fn fsharp_signature_routes_to_files() {
    let files = BTreeMap::from([("elf.fsi", "module Elf\nval f : int\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.fsi"]),
        &["elf.fsi".to_string()],
        &report_of("elf.fsi", &[(1, 1), (2, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.other_sources.is_empty());
}

#[test]
fn modern_js_ts_extensions_route_to_files() {
    for path in ["elf.mjs", "elf.cjs", "elf.mts", "elf.cts"] {
        let files = BTreeMap::from([(path, "x = 1\n".to_string())]);
        let owned = path.to_string();
        let verdict = evaluate(
            &eligible_inventory(&[path]),
            std::slice::from_ref(&owned),
            &report_of(path, &[(1, 1)]),
            &loader(files),
        );
        assert!(verdict.passed, "{path}: {verdict:?}");
        assert!(verdict.other_sources.is_empty(), "{path}");
    }
}

#[test]
fn java_uncovered_line_fails_with_location() {
    let files = BTreeMap::from([("elf.java", "class Elf {}\nint x;\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.java"]),
        &["elf.java".to_string()],
        &report_of("elf.java", &[(1, 1), (2, 0)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.files[0].uncovered, vec![2]);
    let text = render(&verdict);
    assert!(
        text.contains("FAIL") && text.contains("elf.java:2"),
        "{text}"
    );
}

#[test]
fn python_stub_stays_other_sources() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("elf.pyi".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.other_sources, vec!["elf.pyi".to_string()]);
}

#[test]
fn omitted_split_module_fails_as_bazel_source_missing_from_inventory() {
    let files = BTreeMap::from([
        ("cli/lcov/src/lib.rs", "fn f() {}\n".to_string()),
        ("cli/lcov/src/verdict_lang.rs", "fn g() {}\n".to_string()),
    ]);
    let verdict = evaluate(
        &eligible_inventory(&["cli/lcov/src/lib.rs"]),
        &[
            "cli/lcov/src/lib.rs".to_string(),
            "cli/lcov/src/verdict_lang.rs".to_string(),
        ],
        &report_of("cli/lcov/src/lib.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|error| error.contains(
            "Bazel-declared source missing from inventory: cli/lcov/src/verdict_lang.rs"
        )),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn orphan_instrumented_source_fails_as_not_in_inventory() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("orphan.rs".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|error| error.contains("instrumented source not in inventory: orphan.rs")),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn stale_inventory_row_fails_as_not_declared_by_bazel() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs", "stale.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(
            |error| error.contains("inventory eligible source not declared by Bazel: stale.rs")
        ),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn zero_hit_source_fails_with_missing_instrumentation() {
    let files = BTreeMap::from([("empty.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["empty.rs"]),
        &["empty.rs".to_string()],
        &report_of("empty.rs", &[]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|error| error.contains(
            "no instrumented lines and no validated ignores for eligible source: empty.rs"
        )),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn generated_support_source_is_declared_but_not_counted() {
    let mut inventory = eligible_inventory(&["elf.rs"]);
    inventory.insert("gen/bindings.rs".to_string(), SUPPORT.to_string());
    let files = BTreeMap::from([
        ("elf.rs", "fn f() {}\n".to_string()),
        ("gen/bindings.rs", "fn g() {}\n".to_string()),
    ]);
    let mut report = report_of("elf.rs", &[(1, 1)]);
    report.insert("gen/bindings.rs".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &inventory,
        &["elf.rs".to_string(), "gen/bindings.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.files.len(), 1);
    assert_eq!(verdict.covered, 1);
    assert_eq!(verdict.eligible, 1);
}

#[test]
fn arm_only_source_passes_in_its_own_cell_and_fails_cross_cell() {
    let arm_files = BTreeMap::from([
        ("elf.rs", "fn f() {}\n".to_string()),
        ("arm/only.rs", "fn g() {}\n".to_string()),
    ]);
    let mut arm_report = report_of("elf.rs", &[(1, 1)]);
    arm_report.insert("arm/only.rs".to_string(), hits(&[(1, 1)]));
    let arm_sources = vec!["elf.rs".to_string(), "arm/only.rs".to_string()];
    let arm_verdict = evaluate(
        &eligible_inventory(&["elf.rs", "arm/only.rs"]),
        &arm_sources,
        &arm_report,
        &loader(arm_files),
    );
    assert!(arm_verdict.passed, "{arm_verdict:?}");
    assert_eq!(arm_verdict.files.len(), 2);
    let seed_files = BTreeMap::from([
        ("elf.rs", "fn f() {}\n".to_string()),
        ("arm/only.rs", "fn g() {}\n".to_string()),
    ]);
    let seed_verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &arm_sources,
        &arm_report,
        &loader(seed_files),
    );
    assert!(!seed_verdict.passed);
    assert!(
        seed_verdict.errors.iter().any(
            |error| error.contains("Bazel-declared source missing from inventory: arm/only.rs")
        ),
        "{:?}",
        seed_verdict.errors
    );
}

#[test]
fn stale_single_ignore_fails_naming_file_and_line() {
    let source = file_lines(&[
        "pub fn classify(n: u32) -> u32 {".to_string(),
        format!(
            "    let _ = n; // {} - reason: fixture dead marker, issue: 1055.",
            marker("_LINE")
        ),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1), (3, 1), (4, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|error| error.contains("stale coverage ignore at elf.rs:2")),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn stale_range_ignore_fails_naming_start_and_end() {
    let source = file_lines(&[
        "fn f() {}".to_string(),
        format!(
            "// {} - reason: fixture dead range, issue: 1055.",
            marker("_START")
        ),
        "// nothing executable here".to_string(),
        format!(
            "// {} - reason: fixture dead range end, issue: 1055.",
            marker("_STOP")
        ),
    ]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict
            .errors
            .iter()
            .any(|error| error.contains("stale coverage range at elf.rs:2-4")),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn fully_ignored_instrumented_file_fails_as_broad() {
    let source = file_lines(&[
        format!(
            "// {} - reason: fixture whole-file range, issue: 1055.",
            marker("_START")
        ),
        "fn hidden() {}".to_string(),
        format!(
            "// {} - reason: fixture whole-file range end, issue: 1055.",
            marker("_STOP")
        ),
    ]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(2, 0)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    let broad = "broad coverage ignore at elf.rs: all 1 instrumented lines ignored";
    assert!(
        verdict.errors.iter().any(|error| error.contains(broad)),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn orphan_support_row_fails_as_not_declared_by_bazel() {
    let mut inventory = eligible_inventory(&["elf.rs"]);
    inventory.insert("ghost.rs".to_string(), SUPPORT.to_string());
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &inventory,
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(
            |error| error.contains("inventory support source not declared by Bazel: ghost.rs")
        ),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn render_reports_raw_ignored_and_eligible_counts() {
    let source = file_lines(&[
        "pub fn classify(n: u32) -> u32 {".to_string(),
        "    if n == 0 {".to_string(),
        format!(
            "        return 0; // {} - reason: fixture defensive branch, issue: 1055.",
            marker("_LINE")
        ),
        "    }".to_string(),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let files = BTreeMap::from([("elf.rs", source)]);
    let mut inventory = eligible_inventory(&["elf.rs"]);
    inventory.insert("tool.rs".to_string(), SUPPORT.to_string());
    let verdict = evaluate(
        &inventory,
        &["elf.rs".to_string(), "tool.rs".to_string()],
        &report_of("elf.rs", &[(1, 1), (2, 1), (3, 0), (5, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    let text = render(&verdict);
    assert!(
        text.contains("measured lines: 4 raw, 1 ignored, 3 eligible"),
        "{text}"
    );
    assert!(
        text.contains("uncounted support files:\n  - tool.rs"),
        "{text}"
    );
}

#[test]
fn measured_go_block_without_functions_parses_and_passes() {
    let text =
        "SF:gazelle/vue/naming.go\nFNF:0\nFNH:0\nDA:7,13\nDA:9,8\nLH:2\nLF:2\nend_of_record\n";
    let report = parse_lcov(text).unwrap();
    assert_eq!(report["gazelle/vue/naming.go"].lines.len(), 2);
    let files = BTreeMap::from([("gazelle/vue/naming.go", "package vue\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["gazelle/vue/naming.go"]),
        &["gazelle/vue/naming.go".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 2);
    assert!(verdict.other_sources.is_empty());
}

#[test]
fn measured_go_zero_hit_lines_parse_and_fail_with_locations() {
    let text = "SF:gazelle/scala/parser.go\nFNF:0\nFNH:0\nDA:15,11\nDA:16,11\nDA:17,11\nDA:18,11\nDA:19,10\nDA:20,10\nDA:21,10\nDA:22,10\nDA:23,10\nDA:25,11\nDA:26,11\nDA:27,9\nDA:28,9\nDA:29,11\nDA:30,11\nDA:33,10\nDA:34,10\nDA:35,10\nDA:36,10\nDA:37,1\nDA:38,1\nDA:39,9\nDA:40,1\nDA:41,1\nDA:42,8\nDA:47,0\nDA:48,0\nDA:49,0\nDA:51,1\nDA:53,12\nDA:54,12\nDA:55,12\nDA:57,10\nDA:58,10\nDA:59,10\nLH:32\nLF:35\nend_of_record\n";
    let report = parse_lcov(text).unwrap();
    let lines = &report["gazelle/scala/parser.go"].lines;
    assert_eq!(lines.len(), 35);
    assert_eq!(lines[&47], 0);
    assert_eq!(lines[&48], 0);
    assert_eq!(lines[&49], 0);
    let files = BTreeMap::from([("gazelle/scala/parser.go", "package scala\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["gazelle/scala/parser.go"]),
        &["gazelle/scala/parser.go".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert_eq!(verdict.files[0].uncovered, vec![47, 48, 49]);
    let text = render(&verdict);
    assert!(
        text.contains("gazelle/scala/parser.go:47, gazelle/scala/parser.go:48"),
        "{text}"
    );
}

#[test]
fn measured_python_empty_block_errors_as_uninstrumented() {
    let text =
        "SF:quality/tools/python/tool_bootstrap.py\nFNF:0\nFNH:0\nLH:0\nLF:0\nend_of_record\n";
    let report = parse_lcov(text).unwrap();
    assert!(report["quality/tools/python/tool_bootstrap.py"]
        .lines
        .is_empty());
    let files = BTreeMap::from([(
        "quality/tools/python/tool_bootstrap.py",
        "import os\n".to_string(),
    )]);
    let verdict = evaluate(
        &eligible_inventory(&["quality/tools/python/tool_bootstrap.py"]),
        &["quality/tools/python/tool_bootstrap.py".to_string()],
        &report,
        &loader(files),
    );
    assert!(!verdict.passed);
    assert!(
        verdict.errors.iter().any(|error| error.contains(
            "no instrumented lines and no validated ignores for eligible source: quality/tools/python/tool_bootstrap.py"
        )),
        "{:?}",
        verdict.errors
    );
}

#[test]
fn branch_data_with_both_outcomes_reports_taken_counts() {
    let files = BTreeMap::from([("elf.rs", "fn f(n: u32) -> u32 {\n    n\n}\n".to_string())]);
    let mut report = BTreeMap::new();
    report.insert(
        "elf.rs".to_string(),
        branch_file(
            &[(1, 1), (2, 1)],
            &[((2, 0, 0), Some(3)), ((2, 0, 1), Some(1))],
        ),
    );
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_eligible, 2);
    assert_eq!(verdict.branch_ignored, 0);
    assert_eq!(verdict.branch_hit, 2);
    assert_eq!(verdict.branch_files, 1);
    assert_eq!(verdict.branch_bare, 0);
    assert!(verdict.files[0].missed_branches.is_empty());
    let text = render(&verdict);
    assert!(
        text.contains(
            "measured branches: 2 raw, 0 ignored, 2 eligible, 2 taken (1 files with branch data, 0 without)"
        ),
        "{text}"
    );
    assert!(
        text.contains("elf.rs branches(rs): 2/2 taken (0 ignored)"),
        "{text}"
    );
    assert!(
        text.contains("informational branch rate: 100.00% (measured files only; never a gate)"),
        "{text}"
    );
}

#[test]
fn line_gate_passes_with_missed_branch_outcomes() {
    let files = BTreeMap::from([("elf.rs", "fn f(n: u32) -> u32 {\n    n\n}\n".to_string())]);
    let mut report = BTreeMap::new();
    report.insert(
        "elf.rs".to_string(),
        branch_file(
            &[(1, 1), (2, 1)],
            &[
                ((2, 0, 0), Some(1)),
                ((2, 0, 1), None),
                ((2, 0, 2), Some(0)),
            ],
        ),
    );
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_eligible, 3);
    assert_eq!(verdict.branch_hit, 1);
    assert_eq!(verdict.files[0].missed_branches, vec![(2, 0, 1), (2, 0, 2)]);
    let text = render(&verdict);
    assert!(
        text.contains("measured branches: 3 raw, 0 ignored, 3 eligible, 1 taken"),
        "{text}"
    );
    assert!(
        text.contains("missed: elf.rs:2 block 0 branch 1, elf.rs:2 block 0 branch 2"),
        "{text}"
    );
    assert!(
        text.contains("informational branch rate: 33.33% (measured files only; never a gate)"),
        "{text}"
    );
}

#[test]
fn unavailable_branch_data_reports_unknown_never_full() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report_of("elf.rs", &[(1, 1)]),
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_files, 0);
    assert_eq!(verdict.branch_bare, 1);
    let text = render(&verdict);
    assert!(
        text.contains("measured branches: unavailable (no branch records in report; branch coverage unknown, never 100%)"),
        "{text}"
    );
    assert!(!text.contains("informational branch rate"), "{text}");
}

#[test]
fn summary_only_branch_section_counts_zero_eligible() {
    let files = BTreeMap::from([("elf.rs", "fn f() {}\n".to_string())]);
    let mut measured = hits(&[(1, 1)]);
    measured.branch_data = true;
    let mut report = BTreeMap::new();
    report.insert("elf.rs".to_string(), measured);
    let verdict = evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_files, 1);
    assert_eq!(verdict.branch_eligible, 0);
    let text = render(&verdict);
    assert!(
        text.contains(
            "measured branches: 0 raw, 0 ignored, 0 eligible, 0 taken (1 files with branch data, 0 without)"
        ),
        "{text}"
    );
    assert!(
        text.contains("elf.rs branches(rs): 0/0 taken (0 ignored)"),
        "{text}"
    );
    assert!(
        text.contains("informational branch rate: n/a (no eligible branches)"),
        "{text}"
    );
}

fn run_branch_fixture(source: String, branches: &[BranchHit], lines: &[(u32, u64)]) -> GateVerdict {
    let files = BTreeMap::from([("elf.rs", source)]);
    let mut report = BTreeMap::new();
    report.insert("elf.rs".to_string(), branch_file(lines, branches));
    evaluate(
        &eligible_inventory(&["elf.rs"]),
        &["elf.rs".to_string()],
        &report,
        &loader(files),
    )
}

#[test]
fn excluded_line_branch_is_ignored_not_missed() {
    let source = file_lines(&[
        "pub fn f(n: u32) -> u32 {".to_string(),
        format!(
            "    if n == 0 {{ return 0; }} // {} - reason: fixture defensive branch, issue: 2516.",
            marker("_LINE")
        ),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let verdict = run_branch_fixture(
        source,
        &[
            ((2, 0, 0), Some(4)),
            ((2, 0, 1), Some(0)),
            ((3, 0, 0), Some(1)),
        ],
        &[(1, 1), (2, 1), (3, 1)],
    );
    assert!(verdict.passed, "{verdict:?}");
    assert!(verdict.files[0].missed_branches.is_empty());
    assert_eq!(verdict.branch_ignored, 2);
    assert_eq!(verdict.branch_eligible, 1);
    assert_eq!(verdict.branch_hit, 1);
    let text = render(&verdict);
    assert!(
        text.contains("3 raw, 2 ignored, 1 eligible, 1 taken"),
        "{text}"
    );
    assert!(text.contains("100.00%"), "{text}");
}

#[test]
fn range_excluded_branches_are_ignored() {
    let source = file_lines(&[
        "pub fn f(n: u32) -> u32 {".to_string(),
        format!("    // {} - reason: shim, issue: 2516.", marker("_START")),
        "    if n == 0 { return 0; }".to_string(),
        format!(
            "    // {} - reason: shim end, issue: 2516.",
            marker("_STOP")
        ),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let verdict = run_branch_fixture(
        source,
        &[
            ((3, 0, 0), Some(0)),
            ((3, 0, 1), Some(1)),
            ((5, 0, 0), Some(0)),
        ],
        &[(1, 1), (3, 1), (5, 1)],
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_ignored, 2);
    assert_eq!(verdict.branch_eligible, 1);
    assert_eq!(verdict.branch_hit, 0);
    assert_eq!(verdict.files[0].missed_branches, vec![(5, 0, 0)]);
}

#[test]
fn all_branches_ignored_reports_no_eligible_rate() {
    let source = file_lines(&[
        "pub fn f(n: u32) -> u32 {".to_string(),
        format!(
            "    if n == 0 {{ return 0; }} // {} - reason: fixture defensive branch, issue: 2516.",
            marker("_LINE")
        ),
        "    n".to_string(),
        "}".to_string(),
    ]);
    let verdict = run_branch_fixture(
        source,
        &[((2, 0, 0), Some(0)), ((2, 0, 1), Some(1))],
        &[(1, 1), (2, 1), (3, 1)],
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_eligible, 0);
    assert_eq!(verdict.branch_ignored, 2);
    let text = render(&verdict);
    assert!(text.contains("n/a (no eligible branches)"), "{text}");
}

#[test]
fn mixed_files_split_branch_provenance_counts() {
    let files = BTreeMap::from([
        ("taken.rs", "fn f() {}\n".to_string()),
        ("bare.rs", "fn g() {}\n".to_string()),
    ]);
    let mut report = BTreeMap::new();
    report.insert(
        "taken.rs".to_string(),
        branch_file(&[(1, 1)], &[((1, 0, 0), Some(1))]),
    );
    report.insert("bare.rs".to_string(), hits(&[(1, 1)]));
    let verdict = evaluate(
        &eligible_inventory(&["taken.rs", "bare.rs"]),
        &["taken.rs".to_string(), "bare.rs".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.branch_files, 1);
    assert_eq!(verdict.branch_bare, 1);
    let text = render(&verdict);
    assert!(
        text.contains("(1 files with branch data, 1 without)"),
        "{text}"
    );
}

#[test]
fn branch_language_labels_covered_extensions() {
    assert_eq!(language_label("elf.rs"), "rs");
    assert_eq!(language_label("elf.py"), "py");
    assert_eq!(language_label("elf.cc"), "cc");
    assert_eq!(language_label("elf.mts"), "mts");
    assert_eq!(language_label("notes.txt"), "other");
}

#[test]
fn measured_python_pytest_block_with_functions_parses_and_passes() {
    let text = "SF:python/tests/fixtures/hello/hello.py\nFN:4,greet\nFNDA:1,greet\nFNF:1\nFNH:1\nDA:4,1\nDA:6,1\nLH:2\nLF:2\nend_of_record\n";
    let report = parse_lcov(text).unwrap();
    assert_eq!(
        report["python/tests/fixtures/hello/hello.py"].lines.len(),
        2
    );
    let files = BTreeMap::from([(
        "python/tests/fixtures/hello/hello.py",
        "x = 1\n".to_string(),
    )]);
    let verdict = evaluate(
        &eligible_inventory(&["python/tests/fixtures/hello/hello.py"]),
        &["python/tests/fixtures/hello/hello.py".to_string()],
        &report,
        &loader(files),
    );
    assert!(verdict.passed, "{verdict:?}");
    assert_eq!(verdict.covered, 2);
    assert!(verdict.other_sources.is_empty());
}

fn split_module_fixture() -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("lib/src/lib.rs", "mod split;\nfn f() {}\n".to_string()),
        ("lib/src/split.rs", "fn g() {}\n".to_string()),
    ])
}

fn paths(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn unexecuted_split_module_is_counted_and_fails() {
    let mut report = report_of("lib/src/lib.rs", &[(1, 1), (2, 1)]);
    report.insert("lib/src/split.rs".to_string(), hits(&[(1, 0)]));
    let verdict = evaluate(
        &eligible_inventory(&["lib/src/lib.rs", "lib/src/split.rs"]),
        &paths(&["lib/src/lib.rs", "lib/src/split.rs"]),
        &report,
        &loader(split_module_fixture()),
    );
    assert!(!verdict.passed);
    assert_eq!((verdict.covered, verdict.eligible), (2, 3));
}

#[test]
fn dropping_split_module_from_inventory_and_report_still_fails() {
    let verdict = evaluate(
        &eligible_inventory(&["lib/src/lib.rs"]),
        &paths(&["lib/src/lib.rs", "lib/src/split.rs"]),
        &report_of("lib/src/lib.rs", &[(1, 1), (2, 1)]),
        &loader(split_module_fixture()),
    );
    assert!(!verdict.passed);
    assert!(verdict
        .errors
        .contains(&"Bazel-declared source missing from inventory: lib/src/split.rs".to_string()));
}

#[test]
fn tracked_reconciliation_catches_module_dropped_everywhere() {
    let errors = reconcile_tracked(
        &eligible_inventory(&["lib/src/lib.rs"]),
        &paths(&["lib/src/lib.rs"]),
        &paths(&["lib/src/lib.rs", "lib/src/split.rs"]),
    );
    assert_eq!(
        errors,
        vec![
            "tracked production source has no disposition: lib/src/split.rs; add it to the inventory as eligible or support"
                .to_string()
        ]
    );
}

#[test]
fn tracked_reconciliation_accepts_complete_accounting() {
    let mut inventory = eligible_inventory(&["lib/src/lib.rs"]);
    inventory.insert("lib/src/split.rs".to_string(), SUPPORT.to_string());
    let errors = reconcile_tracked(
        &inventory,
        &paths(&["lib/src/lib.rs", "lib/src/split.rs"]),
        &paths(&["lib/src/lib.rs", "lib/src/split.rs", "docs/readme.md"]),
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn tracked_reconciliation_flags_stale_inventory_rows() {
    let errors = reconcile_tracked(
        &eligible_inventory(&["lib/src/lib.rs", "gone.rs"]),
        &paths(&["lib/src/lib.rs"]),
        &paths(&["lib/src/lib.rs"]),
    );
    assert_eq!(
        errors,
        vec!["inventory source is not a tracked file: gone.rs; remove the stale row".to_string()]
    );
}

#[test]
fn tracked_reconciliation_flags_path_aliases_and_duplicates() {
    let errors = reconcile_tracked(
        &eligible_inventory(&["a.rs", "./a.rs"]),
        &paths(&["a.rs", "a.rs"]),
        &paths(&["a.rs", "sub\\b.rs", "sub/b.rs"]),
    );
    assert!(
        errors.contains(&"path alias in inventory: ./a.rs and a.rs name the same file".to_string())
    );
    assert!(errors.contains(&"duplicate path in Bazel source list: a.rs".to_string()));
    assert!(errors.contains(
        &"path alias in tracked source list: sub\\b.rs and sub/b.rs name the same file".to_string()
    ));
}

#[test]
fn language_label_names_new_covered_extensions() {
    for (path, label) in [
        ("a.kts", "kts"),
        ("a.rb", "rb"),
        ("a.erl", "erl"),
        ("a.hrl", "hrl"),
    ] {
        assert_eq!(language_label(path), label, "{path}");
    }
}
