use super::*;

fn marker(kind: &str) -> String {
    ["LCOV", "_EXCL", kind].concat()
}

fn file_lines(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[test]
fn single_line_ignore_needs_same_line_reason() {
    let source = file_lines(&[
        "pub fn f() -> u32 {".to_string(),
        format!("    // {} - reason: fixture, issue: 1055.", marker("_LINE")),
        "    1".to_string(),
        "}".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert_eq!(ignores.singles.len(), 1);
    assert!(ignores.singles.contains_key(&2));
    assert!(is_ignored(&ignores, 2));
    assert!(!is_ignored(&ignores, 1));
    assert!(!is_ignored(&ignores, 3));
}

#[test]
fn single_line_ignore_accepts_previous_line_reason() {
    let source = file_lines(&[
        "    // reason: fixture explains the next line, issue: 1055.".to_string(),
        format!("    // {}", marker("_LINE")),
        "    1".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
}

#[test]
fn single_line_ignore_accepts_marker_at_end_of_line() {
    let source = file_lines(&[
        "    // reason: fixture, issue: 1055.".to_string(),
        format!("    // {}", marker("_LINE")),
        "    1".to_string(),
    ]);
    assert!(find_ignores("t.rs", &source)
        .unwrap()
        .singles
        .contains_key(&2));
}

#[test]
fn missing_reason_fails_on_first_line() {
    let source = file_lines(&[format!("// {}", marker("_LINE")), "fn f() {}".to_string()]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(
        err.to_string().contains("t.rs:1") && err.to_string().contains("reason"),
        "{err}"
    );
}

#[test]
fn missing_reason_fails_with_unrelated_previous_line() {
    let source = file_lines(&[
        "fn f() {".to_string(),
        "    // nothing here.".to_string(),
        format!("    // {} - oops, no reason key.", marker("_LINE")),
        "}".to_string(),
    ]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn empty_reason_fails() {
    let source = file_lines(&[
        "    // reason:   ".to_string(),
        format!("    // {}", marker("_LINE")),
        "    1".to_string(),
    ]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn distant_reason_fails() {
    let source = file_lines(&[
        "    // reason: too far above.".to_string(),
        "    // filler.".to_string(),
        format!("    // {}", marker("_LINE")),
    ]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn range_excludes_interior_and_boundaries() {
    let source = file_lines(&[
        "fn f() {".to_string(),
        format!(
            "    // {} - reason: range opens, issue: 1055.",
            marker("_START")
        ),
        "    1".to_string(),
        format!(
            "    // {} - reason: range closes, issue: 1055.",
            marker("_STOP")
        ),
        "}".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert_eq!(ignores.ranges.len(), 1);
    assert!(is_ignored(&ignores, 2));
    assert!(is_ignored(&ignores, 3));
    assert!(is_ignored(&ignores, 4));
    assert!(!is_ignored(&ignores, 1));
    assert!(!is_ignored(&ignores, 5));
}

#[test]
fn stop_without_reason_fails() {
    let source = file_lines(&[
        format!("// {} - reason: opens, issue: 1055.", marker("_START")),
        "code();".to_string(),
        format!("// {}", marker("_STOP")),
    ]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn stop_without_start_fails() {
    let source = file_lines(&[format!(
        "// {} - reason: stray stop, issue: 1055.",
        marker("_STOP")
    )]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("without START"), "{err}");
}

#[test]
fn nested_start_fails() {
    let source = file_lines(&[
        format!("// {} - reason: outer, issue: 1055.", marker("_START")),
        format!("// {} - reason: inner, issue: 1055.", marker("_START")),
        format!("// {} - reason: close, issue: 1055.", marker("_STOP")),
        format!("// {} - reason: close, issue: 1055.", marker("_STOP")),
    ]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("nested"), "{err}");
}

#[test]
fn unclosed_start_fails() {
    let source = file_lines(&[
        "fn f() {".to_string(),
        format!(
            "    // {} - reason: never closed, issue: 1055.",
            marker("_START")
        ),
        "}".to_string(),
    ]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(
        err.to_string().contains("unclosed") && err.to_string().contains("t.rs:2"),
        "{err}"
    );
}

#[test]
fn unrecognized_suffix_fails() {
    let source = file_lines(&[format!("// {} - reason: typo.", marker("_RANGE"))]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("unrecognized"), "{err}");
}

#[test]
fn bare_prefix_fails() {
    let source = file_lines(&[format!("// {} - reason: bare.", marker(""))]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn word_boundary_suffix_fails() {
    let source = file_lines(&[format!("// {}X - reason: glued.", marker("_LINE"))]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn markers_inside_string_literals_are_ignored() {
    let tricky = format!("let s = \"code with // {} inside\";", marker("_LINE"));
    let source = file_lines(&[tricky, "real();".to_string()]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn lexer_survives_escapes_and_char_literals() {
    let source = file_lines(&[
        "let s = \"a\\\"b\";".to_string(),
        "let q = '\\'';".to_string(),
        "let c = '/';".to_string(),
        "code();".to_string(),
    ]);
    assert!(find_ignores("t.rs", &source).unwrap().singles.is_empty());
}

#[test]
fn marker_after_string_state_is_recognized() {
    let source = file_lines(&[
        "let s = \"a\\\"b\";".to_string(),
        format!(
            "// {} - reason: after strings, issue: 1055.",
            marker("_LINE")
        ),
        "code();".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
}

#[test]
fn hash_comment_markers_are_honored_for_python() {
    let source = file_lines(&[
        "def f():".to_string(),
        format!(
            "    pass  # {} - reason: fixture defensive line, issue: 1055.",
            marker("_LINE")
        ),
        "    return 1".to_string(),
    ]);
    let ignores = find_ignores("t.py", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
    assert!(!is_ignored(&ignores, 3));
}

#[test]
fn hash_markers_inside_python_strings_are_ignored() {
    let tricky = format!("s = \"code with # {} inside\";", marker("_LINE"));
    let source = file_lines(&[tricky, "real();".to_string()]);
    let ignores = find_ignores("t.py", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn slash_markers_are_not_honored_for_python() {
    let source = file_lines(&[format!("// {} - reason: wrong style.", marker("_LINE"))]);
    let ignores = find_ignores("t.py", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn hash_marker_without_reason_fails_for_python() {
    let source = file_lines(&[format!("# {}", marker("_LINE")), "x = 1".to_string()]);
    let err = find_ignores("t.py", &source).unwrap_err();
    assert!(
        err.to_string().contains("t.py:1") && err.to_string().contains("reason"),
        "{err}"
    );
}

#[test]
fn hash_malformed_directive_fails_for_python() {
    let source = file_lines(&[format!("# {} - reason: typo.", marker("_RANGE"))]);
    let err = find_ignores("t.py", &source).unwrap_err();
    assert!(err.to_string().contains("unrecognized"), "{err}");
}

#[test]
fn hash_range_excludes_boundaries_for_starlark() {
    let source = file_lines(&[
        "def f():".to_string(),
        format!(
            "    # {} - reason: range opens, issue: 1055.",
            marker("_START")
        ),
        "    pass".to_string(),
        format!(
            "    # {} - reason: range closes, issue: 1055.",
            marker("_STOP")
        ),
    ]);
    let ignores = find_ignores("t.bzl", &source).unwrap();
    assert_eq!(ignores.ranges.len(), 1);
    assert!(is_ignored(&ignores, 2));
    assert!(is_ignored(&ignores, 3));
    assert!(is_ignored(&ignores, 4));
    assert!(!is_ignored(&ignores, 1));
}

#[test]
fn html_comment_markers_are_honored_for_markdown() {
    let source = file_lines(&[
        "# Title".to_string(),
        format!(
            "<!-- {} - reason: fixture prose, issue: 1055. -->",
            marker("_LINE")
        ),
        "Body.".to_string(),
    ]);
    let ignores = find_ignores("t.md", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
}

#[test]
fn html_second_comment_on_line_keeps_separator() {
    let line = format!(
        "prose <!-- dropped --> more <!-- {} - reason: second segment, issue: 1055. -->",
        marker("_LINE")
    );
    let source = file_lines(&[line]);
    let ignores = find_ignores("t.md", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
}

#[test]
fn html_unterminated_comment_after_content_keeps_prefix() {
    let line = format!(
        "prose <!-- dropped --> tail <!-- {} - reason: unterminated, issue: 1055.",
        marker("_LINE")
    );
    let source = file_lines(&[line]);
    let ignores = find_ignores("t.md", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
}

#[test]
fn html_code_outside_segments_is_not_scanned_for_markdown() {
    let source = file_lines(&["real `code` here.".to_string(), "More.".to_string()]);
    let ignores = find_ignores("t.md", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn regex_comment_scan_skips_string_then_finds_real_marker() {
    let line = format!(
        "let s = \"code with // {} inside\"; // {} - reason: real, issue: 1055.",
        marker("_LINE"),
        marker("_LINE")
    );
    let source = file_lines(&[line]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
}

#[test]
fn regex_hash_scan_skips_char_literal_hash() {
    let line = format!(
        "let c = '#'; # {} - reason: after char, issue: 1055.",
        marker("_LINE")
    );
    let source = file_lines(&[line]);
    let ignores = find_ignores("t.py", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
}

#[test]
fn regex_word_boundary_rejects_glued_suffixes() {
    for suffix in ["_LINES", "_LINE2", "_LINE_", "_STARTX", "_STOPPED"] {
        let source = file_lines(&[format!("// {} - reason: glued.", marker(suffix))]);
        let err = find_ignores("t.rs", &source).unwrap_err();
        assert!(err.to_string().contains("unrecognized"), "{suffix}: {err}");
    }
}

#[test]
fn regex_word_boundary_accepts_punctuation_suffix() {
    for suffix in ["_LINE", "_START", "_STOP"] {
        let open = marker(suffix);
        let source = if suffix == "_STOP" {
            file_lines(&[
                format!("// {} - reason: opens, issue: 1055.", marker("_START")),
                "code();".to_string(),
                format!("// {open} - reason: closes, issue: 1055."),
            ])
        } else if suffix == "_START" {
            file_lines(&[
                format!("// {open} - reason: opens, issue: 1055."),
                "code();".to_string(),
                format!("// {} - reason: closes, issue: 1055.", marker("_STOP")),
            ])
        } else {
            file_lines(&[format!("// {open} - reason: ok, issue: 1055.")])
        };
        assert!(find_ignores("t.rs", &source).is_ok(), "{suffix}");
    }
}

#[test]
fn block_comment_markers_are_inert_wontfix() {
    let source = file_lines(&[
        "fn f() {".to_string(),
        format!("    /* {} - reason: block. */", marker("_LINE")),
        "    1".to_string(),
        "}".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn raw_string_markers_are_inert_wontfix() {
    let tricky = format!("let s = r#\"code with // {} inside\"#;", marker("_LINE"));
    let source = file_lines(&[tricky, "real();".to_string()]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.is_empty());
    assert!(ignores.ranges.is_empty());
}

#[test]
fn hash_comment_markers_are_honored_for_pyi_stubs() {
    let source = file_lines(&[
        "def f() -> int: ...".to_string(),
        format!(
            "    pass  # {} - reason: fixture stub line, issue: 1055.",
            marker("_LINE")
        ),
    ]);
    let ignores = find_ignores("t.pyi", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
}

#[test]
fn bare_policy_without_reason_is_rejected() {
    let source = file_lines(&[format!(
        "// {} - policy: docs/cli/commands/build-test-coverage.md",
        marker("_LINE")
    )]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("bare policy"), "{err}");
}

#[test]
fn bare_policy_on_previous_line_is_rejected() {
    let source = file_lines(&[
        "    // policy: docs/cli/commands/build-test-coverage.md".to_string(),
        format!("    // {}", marker("_LINE")),
        "    1".to_string(),
    ]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("bare policy"), "{err}");
}

#[test]
fn reason_without_issue_is_rejected() {
    let source = file_lines(&[format!(
        "// {} - reason: specific but untracked.",
        marker("_LINE")
    )]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("issue"), "{err}");
}

#[test]
fn issue_without_digit_is_rejected() {
    let source = file_lines(&[format!(
        "// {} - reason: fixture, issue: no-digit.",
        marker("_LINE")
    )]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("issue"), "{err}");
}

#[test]
fn reason_plus_issue_plus_policy_is_accepted() {
    let source = file_lines(&[format!(
        "// {} - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md",
        marker("_LINE")
    )]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
}

#[test]
fn split_reason_and_issue_across_lines_is_accepted() {
    let source = file_lines(&[
        "    // reason: thin shim.".to_string(),
        format!("    // {} - issue: 1055.", marker("_LINE")),
        "    1".to_string(),
    ]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&2));
}

#[test]
fn long_reason_fails_for_single_line() {
    let long = "x".repeat(MAX_REASON_LEN + 1);
    let source = file_lines(&[format!(
        "// {} - reason: {long}, issue: 1055",
        marker("_LINE")
    )]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("too long"), "{err}");
    assert!(err.to_string().contains("t.rs:1"), "{err}");
}

#[test]
fn long_reason_fails_for_range_start() {
    let long = "y".repeat(MAX_REASON_LEN + 40);
    let source = file_lines(&[
        format!("// {} - reason: {long}, issue: 1055", marker("_START")),
        "code();".to_string(),
        format!("// {} - reason: closes, issue: 1055.", marker("_STOP")),
    ]);
    let err = find_ignores("t.rs", &source).unwrap_err();
    assert!(err.to_string().contains("too long"), "{err}");
}

#[test]
fn max_length_reason_passes_at_boundary() {
    let suffix = ", issue: 1055";
    let exact = "z".repeat(MAX_REASON_LEN - suffix.len());
    let source = file_lines(&[format!("// {} - reason: {exact}{suffix}", marker("_LINE"))]);
    assert!(find_ignores("t.rs", &source)
        .unwrap()
        .singles
        .contains_key(&1));
}

#[test]
fn empty_policy_reason_fails() {
    let source = file_lines(&[
        "    // policy:   ".to_string(),
        format!("    // {}", marker("_LINE")),
        "    1".to_string(),
    ]);
    assert!(find_ignores("t.rs", &source).is_err());
}

#[test]
fn slash_markers_are_honored_for_wrapped_langs() {
    for path in [
        "t.java", "t.kt", "t.scala", "t.cs", "t.fs", "t.fsi", "t.mjs", "t.cjs", "t.mts", "t.cts",
    ] {
        let source = file_lines(&[format!(
            "// {} - reason: fixture, issue: 1055.",
            marker("_LINE")
        )]);
        let ignores = find_ignores(path, &source).unwrap();
        assert!(ignores.singles.contains_key(&1), "{path}");
    }
}

#[test]
fn reason_wins_when_line_carries_both_keys() {
    let source = file_lines(&[format!(
        "// {} - issue: 1055 - reason: first policy: second.",
        marker("_LINE")
    )]);
    let ignores = find_ignores("t.rs", &source).unwrap();
    assert!(ignores.singles.contains_key(&1));
    assert_eq!(ignores.singles[&1], "first policy: second.".to_string());
}
