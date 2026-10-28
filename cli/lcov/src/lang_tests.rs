use crate::lang::{LineComment, COVERED_LANGUAGES};
use crate::{find_ignores, is_covered_language};

fn marker(kind: &str) -> String {
    ["LCOV", "_EXCL", kind].concat()
}

fn file_lines(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
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
