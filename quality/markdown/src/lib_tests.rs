use super::check::FindingLine;
use super::*;
use std::collections::BTreeMap;

fn kinds(outcome: &CheckOutcome) -> Vec<(u32, FindingKind)> {
    outcome.findings.iter().map(|f| (f.line, f.kind)).collect()
}

#[test]
fn multiple_h1_is_a_finding() {
    let outcome = check_markdown("# One\n\n# Two\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::HeadingHierarchy)]);
}

#[test]
fn missing_h1_is_a_finding() {
    let outcome = check_markdown("## Only\n");
    assert_eq!(kinds(&outcome), vec![(1, FindingKind::HeadingHierarchy)]);
}

#[test]
fn skipped_level_is_a_finding() {
    let outcome = check_markdown("# T\n\n### Deep\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::HeadingHierarchy)]);
}

#[test]
fn fence_without_language_is_a_finding() {
    let outcome = check_markdown("# T\n\n```\ncode\n```\n");
    assert_eq!(
        kinds(&outcome),
        vec![(3, FindingKind::MissingCodeFenceLanguage)]
    );
}

#[test]
fn unclosed_fence_is_a_finding() {
    let outcome = check_markdown("# T\n\n```rust\ncode\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn fence_closed_by_longer_run_reopens_after() {
    let text = "# T\n\n```rust\n````\n```\n";
    let outcome = check_markdown(text);
    assert_eq!(
        kinds(&outcome),
        vec![
            (5, FindingKind::MissingCodeFenceLanguage),
            (5, FindingKind::UnclosedCodeFence),
        ]
    );
}

#[test]
fn unclosed_tilde_fence_is_a_finding() {
    let outcome = check_markdown("# T\n\n~~~\ncode\n");
    assert_eq!(
        kinds(&outcome),
        vec![
            (3, FindingKind::MissingCodeFenceLanguage),
            (3, FindingKind::UnclosedCodeFence),
        ]
    );
}

#[test]
fn fence_info_with_extra_words_has_language() {
    let text = "# T\n\n```rust foo\ncode\n```\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_fence_hides_rest_of_document() {
    let text = "# T\n\n```rust\n[gone](gone.md)\n\n## Late\n";
    let outcome = check_markdown(text);
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn indented_code_content_is_suppressed() {
    let text = "# T\n\n    [gone](gone.md)\n\n    ## Fake\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn links_inside_fences_and_code_spans_are_ignored() {
    let text = "# T\n\n```text\n[gone](gone.md)\n```\n\n`[gone](gone.md)`\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn html_block_content_is_ignored() {
    let text = "# T\n\n<div>\n# Fake\n[gone](gone.md)\n</div>\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn blockquote_heading_is_not_a_heading() {
    let outcome = check_markdown("> # Quoted\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn bare_brackets_without_definition_are_literal_text() {
    let text = "# T\n\nA [bracket] aside.\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn tilde_run_inside_backtick_fence_is_content() {
    let text = "# T\n\n```rust\n~~~\n```\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn empty_h1_counts_as_the_single_h1() {
    let outcome = check_markdown("#\n\nText.\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn multiline_code_span_hides_autolink() {
    let text = "# T\n\nRun (`dump <pkg> [-o out]\n[-f]`, more).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn span_closer_misread_as_opener_is_fixed_across_lines() {
    let text = "# T\n\nUnset under `bazel\ntest`. Resolve `$WS/<short_path>` here.\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn bare_hash_anchor_links_to_top() {
    let outcome = check_markdown("# T\n\nBack to [top](#).\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn slug_matches_github_fixtures() {
    assert_eq!(slug("Привет"), "привет");
    assert_eq!(slug("你好"), "你好");
    assert_eq!(slug("😄 emoji"), "-emoji");
    assert_eq!(slug("Æúű"), "æúű");
    assert_eq!(slug("a   b"), "a---b");
    assert_eq!(slug("rules_android"), "rules_android");
    assert_eq!(slug("C# basics"), "c-basics");
    assert_eq!(slug("x²"), "x");
    assert_eq!(slug("dx check --here"), "dx-check---here");
}

#[test]
fn unicode_anchors_resolve() {
    let text = "# T\n\n## Привет\n\n## 你好\n\nSee [ru](#привет) and [zh](#你好).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    let text = "# T\n\n## 😄 emoji\n\n## Æúű\n\nSee [e](#-emoji) and [a](#æúű).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn indented_fence_and_heading_are_literal_text() {
    let text = "# T\n\n    ```\n    code\n    ```\n\n    ## Not a heading\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn backtick_in_info_string_is_not_a_fence() {
    let outcome = check_markdown("# T\n\n```a`b\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn hash_without_space_is_not_a_heading() {
    let outcome = check_markdown("# T\n\n#Nope\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn closing_hashes_strip_from_heading_text() {
    let text = "# T\n\n## Title ##\n\nSee [t](#title).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn hash_inside_heading_text_is_kept() {
    let text = "# T\n\n## C# basics\n\nSee [c](#c-basics).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn empty_definition_label_is_not_a_definition() {
    let outcome = check_markdown("# T\n\n[]: other.md\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn definition_without_target_is_not_a_definition() {
    let outcome = check_markdown("# T\n\n[ref]:\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn autolink_without_close_is_literal_text() {
    let outcome = check_markdown("# T\n\na < b\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_bracket_is_literal_text() {
    let outcome = check_markdown("# T\n\nSee [unclosed here.\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_paren_target_is_literal_text() {
    let outcome = check_markdown("# T\n\nSee [x](other.md\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_reference_label_is_literal_text() {
    let outcome = check_markdown("# T\n\nSee [x][label\n");
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn mismatched_code_span_run_skips_gracefully() {
    let text = "# T\n\n``code` and [ok](#t).\n";
    let outcome = check_markdown(text);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

fn cli_files(files: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.as_bytes().to_vec()))
        .collect()
}

fn run_harness(
    files: &BTreeMap<String, Vec<u8>>,
    args: &[&str],
) -> (i32, Vec<String>, Vec<String>) {
    let owned_args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_cli(
        &owned_args,
        &|path| {
            files.get(path).cloned().ok_or_else(|| MarkdownError::Io {
                message: "missing fixture".to_string(),
            })
        },
        &mut |line| out.push(line.to_string()),
        &mut |line| err.push(line.to_string()),
    );
    (code, out, err)
}

#[test]
fn cli_duplicate_source_keeps_first_mapping() {
    let files = cli_files(&[
        ("/exec/first.md", "# First\n"),
        ("/exec/second.md", "# Second\n"),
    ]);
    let (code, out, _) = run_harness(
        &files,
        &[
            "--source",
            "page.md=/exec/first.md",
            "--source",
            "page.md=/exec/second.md",
        ],
    );
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn cli_no_sources_is_a_no_op() {
    let files = cli_files(&[]);
    let (code, out, err) = run_harness(&files, &[]);
    assert_eq!(code, 0);
    assert!(out.is_empty());
    assert!(err.is_empty());
}

#[test]
fn cli_unknown_argument_fails() {
    let files = cli_files(&[]);
    let (code, out, err) = run_harness(&files, &["--bogus", "x"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert_eq!(err.len(), 2, "{err:?}");
    assert!(err[0].contains("unknown argument: --bogus"), "{err:?}");
}

#[test]
fn cli_missing_value_fails() {
    let files = cli_files(&[]);
    let (code, _, err) = run_harness(&files, &["--source"]);
    assert_eq!(code, 2);
    assert!(err[0].contains("missing value for --source"), "{err:?}");
}

#[test]
fn cli_malformed_mapping_fails() {
    let files = cli_files(&[]);
    for spec in ["no-equals", "=no-ws", "no-exec="] {
        let (code, _, err) = run_harness(&files, &["--source", spec]);
        assert_eq!(code, 2, "{spec}");
        assert!(err[0].contains("malformed --source"), "{err:?}");
    }
}

#[test]
fn cli_unreadable_file_fails_closed() {
    let files = cli_files(&[]);
    let (code, out, err) = run_harness(&files, &["--source", "page.md=/exec/page.md"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert_eq!(
        err,
        vec!["cannot read page.md: missing fixture".to_string()]
    );
}

#[test]
fn cli_non_utf8_reports_path() {
    let mut files = cli_files(&[("/exec/messy.md", "# T\n\nSee [gone](gone.md).\n")]);
    files.insert("/exec/bad.md".to_string(), vec![0xff, 0xfe]);
    let (code, out, err) = run_harness(
        &files,
        &[
            "--source",
            "messy.md=/exec/messy.md",
            "--source",
            "bad.md=/exec/bad.md",
        ],
    );
    assert_eq!(code, 2);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(err, vec!["not UTF-8: bad.md".to_string()]);
}

#[test]
fn cli_bare_positional_is_unknown_argument() {
    let files = cli_files(&[]);
    for token in ["oops", "-x", "--help"] {
        let (code, out, err) = run_harness(&files, &[token]);
        assert_eq!(code, 2, "{token}");
        assert!(out.is_empty());
        assert_eq!(err.len(), 2, "{token} {err:?}");
        assert_eq!(err[0], format!("unknown argument: {token}"), "{token}");
        assert!(err[1].starts_with("usage: quality_markdown"), "{err:?}");
    }
}

#[test]
fn cli_unknown_flag_after_valid_source_still_fails() {
    let files = cli_files(&[]);
    let (code, _, err) = run_harness(&files, &["--source", "a.md=/exec/a.md", "--bogus"]);
    assert_eq!(code, 2);
    assert_eq!(err[0], "unknown argument: --bogus", "{err:?}");
}

#[test]
fn fence_in_blockquote_is_closed() {
    let outcome = check_markdown("# T\n\n> ```sh\n> echo\n> ```\n");
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn fence_in_nested_list_is_closed() {
    let text =
        "# T\n\n- a\n    - b\n\n      ```sh\n      echo\n      ```\n- ```sh\n  echo\n  ```\n";
    let outcome = check_markdown(text);
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn unclosed_fence_in_blockquote_is_a_finding() {
    let outcome = check_markdown("# T\n\n> ```sh\n> echo\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn fence_with_four_space_indented_close_text_is_unclosed() {
    let outcome = check_markdown("# T\n\n```sh\necho\n    ```\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn quoted_fence_content_that_looks_like_fence_is_unclosed() {
    let outcome = check_markdown("# T\n\n> ```sh\n> > ```\n");
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn setext_h1_counts_as_h1() {
    let outcome = check_markdown("Title\n=====\n\n[x](#title)\n\n# Two\n");
    assert_eq!(kinds(&outcome), vec![(6, FindingKind::HeadingHierarchy)]);
}

#[test]
fn duplicate_heading_gets_counter_slug() {
    let text = "# T\n\n## A\n\n## A\n\n[x](#a-1)\n";
    assert_eq!(kinds(&check_markdown(text)), vec![]);
}

#[test]
fn quoted_heading_skips_hierarchy() {
    let outcome = check_markdown("# T\n\n> ### Q\n");
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn duplicate_heading_counter_includes_quoted_heading() {
    let text = "# T\n\n> ## A\n\n## A\n\n[x](#a-1)\n";
    assert_eq!(kinds(&check_markdown(text)), vec![]);
}

#[test]
fn cli_findings_are_json_lines_and_exit_zero() {
    let files = cli_files(&[("/exec/page.md", "# T\n\n```\ncode\n```\n")]);
    let (code, out, err) = run_harness(&files, &["--source", "page.md=/exec/page.md"]);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        vec![
            "{\"path\":\"page.md\",\"line\":3,\"kind\":\"missing-code-fence-language\",\"message\":\"fenced code block without a language tag\"}"
                .to_string()
        ]
    );
    assert!(err.is_empty(), "{err:?}");
}

#[test]
fn cli_kind_ids_are_stable() {
    assert_eq!(
        kind_id(FindingKind::UnclosedCodeFence),
        "unclosed-code-fence"
    );
    assert_eq!(kind_id(FindingKind::HeadingHierarchy), "heading-hierarchy");
    assert_eq!(
        kind_id(FindingKind::MissingCodeFenceLanguage),
        "missing-code-fence-language"
    );
}

#[test]
fn cli_links_are_not_checked() {
    let files = cli_files(&[("/exec/page.md", "# T\n\nSee [gone](gone.md#nope).\n")]);
    let (code, out, _) = run_harness(&files, &["--source", "page.md=/exec/page.md"]);
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn cli_findings_serialize_as_stable_json_lines() {
    let line = FindingLine {
        path: "p.md",
        line: 3,
        kind: "heading-hierarchy",
        message: "a\"b\\c\né✓",
    };
    assert_eq!(
        serde_json::to_string(&line).expect("serializes"),
        "{\"path\":\"p.md\",\"line\":3,\"kind\":\"heading-hierarchy\",\"message\":\"a\\\"b\\\\c\\né✓\"}"
    );
}

#[test]
fn cli_kind_limits_the_reported_findings() {
    let files = cli_files(&[("/exec/a.md", "# A\n\n```\ncode\n```\n")]);
    let source = "a.md=/exec/a.md";
    let (code, out, _) = run_harness(&files, &["--source", source]);
    assert_eq!((code, out.len()), (0, 1), "{out:?}");
    let (code, out, _) = run_harness(&files, &["--source", source, "--kind", "heading-hierarchy"]);
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
    let (code, out, _) = run_harness(
        &files,
        &["--source", source, "--kind", "missing-code-fence-language"],
    );
    assert_eq!((code, out.len()), (0, 1), "{out:?}");
}

#[test]
fn cli_unknown_kind_is_a_usage_error() {
    let files = cli_files(&[]);
    let (code, _, err) = run_harness(&files, &["--kind", "nope"]);
    assert_eq!(code, 2);
    assert_eq!(err[0], "unknown kind \"nope\"");
}
