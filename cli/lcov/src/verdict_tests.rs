use super::*;
use crate::{FileHits, LcovError, ELIGIBLE, SUPPORT};
use std::collections::BTreeMap;

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
    let verdict = evaluate(&inventory, &["elf.rs".to_string()], &report, &loader(files));
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
            &[owned.clone()],
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
