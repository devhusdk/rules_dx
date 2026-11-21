//! Policy: JVM tools must read JavaInfo classpaths only through
//! `jvm_aux_jars`, which keeps compile-only (neverlink) jars. A tool that reads
//! `transitive_runtime_jars` directly silently loses them.
//!
//! Scans `core/quality/*.bzl`, `core/quality/families/*.bzl`, `java/private/quality`,
//! `kotlin/private/quality` and `scala/private/quality` (the data); JVM aspect
//! code moved into another subpackage must extend the globs.

use std::path::Path;

use dx_testing::runfiles_root;

/// Functions allowed to read JavaInfo classpaths. Add a function here only with
/// a justification; prefer calling `jvm_aux_jars(java_info, owned)`.
const ALLOWED_FUNCTIONS: &[&str] = &[
    "jvm_aux_jars",
    "_jacoco_runtime_jars",
    // Dependency analysis reads the compile classpath to attribute used jars to owners.
    "_indirect_jars",
    "unused_deps_map",
    "_analyzer",
    // Compiler and plugin jars come from toolchain targets.
    "_kotlinc_check",
    "_scalac_check",
    "scalac_toolchain_jars",
    "_scala_plugin_jars",
];

const FORBIDDEN: &[&str] = &[
    "transitive_runtime_jars",
    "transitive_compile_time_jars",
    "compilation_info.runtime_classpath",
    "compilation_info.compilation_classpath",
];

/// Returns `(line, token)` for each forbidden token outside allowed functions.
fn violations(src: &str) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let mut allowed = false;
    for (i, line) in src.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("def ") {
            let name = rest.split('(').next().unwrap_or("").trim();
            allowed = ALLOWED_FUNCTIONS.contains(&name);
        } else if !line.is_empty()
            && !line.starts_with(char::is_whitespace)
            && !line.starts_with('#')
            && !line.starts_with(')')
            && !line.starts_with(']')
            && !line.starts_with('}')
        {
            allowed = false;
        }
        if allowed || line.trim_start().starts_with('#') {
            continue;
        }
        for token in FORBIDDEN {
            if line.contains(token) {
                out.push((i + 1, *token));
            }
        }
    }
    out
}

fn bzl_sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sub in [
        "core/quality",
        "core/quality/families",
        "java/private/quality",
        "kotlin/private/quality",
        "scala/private/quality",
    ] {
        let dir = runfiles_root().join(sub);
        for entry in std::fs::read_dir(&dir).expect("readable quality dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|e| e == "bzl") {
                let file = name(&path);
                let file = match sub {
                    "core/quality" => file,
                    "core/quality/families" => format!("families/{file}"),
                    "java/private/quality" => format!("java/{file}"),
                    "kotlin/private/quality" => format!("kotlin/{file}"),
                    _ => format!("scala/{file}"),
                };
                out.push((file, std::fs::read_to_string(&path).expect("readable bzl")));
            }
        }
    }
    out
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn jvm_classpaths_are_read_only_through_jvm_aux_jars() {
    let mut failures = Vec::new();
    for (file, src) in bzl_sources() {
        for (line, token) in violations(&src) {
            failures.push(format!("core/quality/{file}:{line}: `{token}`"));
        }
    }
    assert!(
        failures.is_empty(),
        "JavaInfo classpaths must be read via jvm_aux_jars(java_info, owned) so compile-only \
         jars are kept, or add a justified function to ALLOWED_FUNCTIONS:\n{}",
        failures.join("\n")
    );
}

#[test]
fn jvm_aux_jars_helper_exists_and_is_used() {
    let sources = bzl_sources();
    let source = |wanted: &str| {
        sources
            .iter()
            .find(|(f, _)| f == wanted)
            .map(|(_, src)| src.as_str())
            .unwrap_or_else(|| panic!("{wanted} scanned"))
    };
    let jvm = source("java/jvm.bzl");
    assert!(
        jvm.contains("def jvm_aux_jars("),
        "jvm_aux_jars helper was removed"
    );
    for (file, user) in [
        ("java/jvm.bzl", "spotbugs"),
        ("scala/scala.bzl", "scalafix"),
    ] {
        let calls = source(file).matches("= jvm_aux_jars(").count();
        assert!(
            calls >= 1,
            "expected {user} in {file} to call jvm_aux_jars, found {calls}"
        );
    }
}

#[test]
fn scanned_sources_are_not_empty() {
    let files = bzl_sources();
    assert!(!files.is_empty(), "no .bzl files in runfiles");
    assert!(files.iter().any(|(f, _)| f == "java/jvm.bzl"));
    assert!(files.iter().any(|(f, _)| f == "kotlin/kotlinc_check.bzl"));
    assert!(files.iter().any(|(f, _)| f == "scala/scala.bzl"));
}

#[test]
fn splitter_scopes_by_top_level_function() {
    let src = "def jvm_aux_jars(a):\n    return a.transitive_runtime_jars\n\ndef other(a):\n    # transitive_runtime_jars\n    return a.transitive_runtime_jars\n";
    assert_eq!(violations(src), vec![(6, "transitive_runtime_jars")]);
    assert_eq!(violations("x = y.transitive_compile_time_jars\n").len(), 1);
}
