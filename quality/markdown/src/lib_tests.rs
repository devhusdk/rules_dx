use super::check::FindingLine;
use super::*;
use crate::frontmatter::{headings, line_starts};
use std::collections::BTreeMap;

fn siblings(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn kinds(outcome: &CheckOutcome) -> Vec<(u32, FindingKind)> {
    outcome.findings.iter().map(|f| (f.line, f.kind)).collect()
}

#[test]
fn clean_document_has_no_findings() {
    let text =
        "# Title\n\nSee [other](other.md) and [section](#title).\n\n```rust\nlet x = 1;\n```\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/other.md", "# Other\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert!(outcome.skipped_remotes.is_empty());
}

#[test]
fn missing_file_target_fails_closed() {
    let outcome = check_markdown(
        "docs/page.md",
        "# Title\n\nSee [gone](gone.md).\n",
        &siblings(&[]),
    );
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
}

#[test]
fn empty_target_fails_closed() {
    let outcome = check_markdown("docs/page.md", "# Title\n\nSee [x]().\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
}

#[test]
fn missing_anchor_is_a_finding() {
    let outcome = check_markdown(
        "docs/page.md",
        "# Title\n\nSee [here](#nope).\n",
        &siblings(&[]),
    );
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingAnchor)]);
}

#[test]
fn sibling_anchor_resolves() {
    let text = "# Title\n\nSee [deep](sub/other.md#part-two).\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/sub/other.md", "# Part One\n\n## Part Two\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn sibling_missing_anchor_is_a_finding() {
    let text = "# Title\n\nSee [deep](sub/other.md#absent).\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/sub/other.md", "# Part One\n")]),
    );
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingAnchor)]);
}

#[test]
fn multiple_h1_is_a_finding() {
    let outcome = check_markdown("a.md", "# One\n\n# Two\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::HeadingHierarchy)]);
}

#[test]
fn missing_h1_is_a_finding() {
    let outcome = check_markdown("a.md", "## Only\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(1, FindingKind::HeadingHierarchy)]);
}

#[test]
fn skipped_level_is_a_finding() {
    let outcome = check_markdown("a.md", "# T\n\n### Deep\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::HeadingHierarchy)]);
}

#[test]
fn fence_without_language_is_a_finding() {
    let outcome = check_markdown("a.md", "# T\n\n```\ncode\n```\n", &siblings(&[]));
    assert_eq!(
        kinds(&outcome),
        vec![(3, FindingKind::MissingCodeFenceLanguage)]
    );
}

#[test]
fn unclosed_fence_is_a_finding() {
    let outcome = check_markdown("a.md", "# T\n\n```rust\ncode\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn fence_closed_by_longer_run_reopens_after() {
    let text = "# T\n\n```rust\n````\n```\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
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
    let outcome = check_markdown("a.md", "# T\n\n~~~\ncode\n", &siblings(&[]));
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
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_fence_hides_rest_of_document() {
    let text = "# T\n\n```rust\n[gone](gone.md)\n\n## Late\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn indented_code_content_is_suppressed() {
    let text = "# T\n\n    [gone](gone.md)\n\n    ## Fake\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn sibling_indented_code_headings_are_ignored() {
    let text = "# T\n\nSee [r](other.md#real).\n";
    let sibling = "# Part\n\n    ## Fake\n\n## Real\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("other.md", sibling)]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    let bad = "# T\n\nSee [f](other.md#fake).\n";
    let outcome = check_markdown("a.md", bad, &siblings(&[("other.md", sibling)]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingAnchor)]);
}

#[test]
fn remote_targets_are_skipped_never_fetched() {
    let text = "# T\n\nSee [web](https://example.com/page#frag).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty());
    assert_eq!(
        outcome.skipped_remotes,
        vec!["https://example.com/page#frag".to_string()]
    );
}

#[test]
fn email_autolink_is_out_of_scope() {
    let text = "# T\n\nWrite <dev@example.com>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert!(outcome.skipped_remotes.is_empty());
}

#[test]
fn relative_autolink_with_at_in_path_fails_closed_when_missing() {
    let text = "# T\n\nSee <./dir@name/file.md>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
}

#[test]
fn relative_autolink_with_at_in_path_resolves_when_declared() {
    let text = "# T\n\nSee <./dir@name/file.md>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("dir@name/file.md", "# O\n")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn links_inside_fences_and_code_spans_are_ignored() {
    let text = "# T\n\n```text\n[gone](gone.md)\n```\n\n`[gone](gone.md)`\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn duplicate_headings_get_numbered_slugs() {
    let text = "# T\n\n## Repeat\n\n## Repeat\n\nSee [second](#repeat-1).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn reference_link_resolves_through_definition() {
    let text = "# T\n\nSee [other][ref].\n\n[ref]: other.md\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/other.md", "# Other\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn undefined_reference_label_fails_closed() {
    let text = "# T\n\nSee [other][missing].\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
    assert_eq!(
        outcome.findings[0].message,
        "reference link label has no definition: [missing]"
    );
}

#[test]
fn undefined_collapsed_reference_fails_closed() {
    let text = "# T\n\nSee [gone][].\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
    assert_eq!(
        outcome.findings[0].message,
        "reference link label has no definition: [gone]"
    );
}

#[test]
fn shortcut_reference_resolves_through_definition() {
    let text = "# T\n\nSee [other].\n\n[other]: other.md\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/other.md", "# Other\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn image_with_undefined_reference_fails_closed() {
    let text = "# T\n\nSee ![alt][missing].\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
}

#[test]
fn uri_autolink_is_skipped_never_fetched() {
    let text = "# T\n\nSee <https://example.com/page#frag>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert_eq!(
        outcome.skipped_remotes,
        vec!["https://example.com/page#frag".to_string()]
    );
}

#[test]
fn multiline_link_target_resolves() {
    let text = "# T\n\nSee [multi\nline](other.md).\n";
    let outcome = check_markdown(
        "docs/page.md",
        text,
        &siblings(&[("docs/other.md", "# Other\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn html_block_content_is_ignored() {
    let text = "# T\n\n<div>\n# Fake\n[gone](gone.md)\n</div>\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn inline_html_leaves_line_links_visible() {
    let text = "# T\n\nPress <kbd>Ctrl</kbd> plus [gone](gone.md).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::MissingFileTarget)]);
    assert!(outcome.findings[0].message.contains("gone.md"));
}

#[test]
fn blockquote_heading_is_not_a_heading() {
    let outcome = check_markdown("a.md", "> # Quoted\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn bare_brackets_without_definition_are_literal_text() {
    let text = "# T\n\nA [bracket] aside.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn resolve_target_handles_relative_and_root_paths() {
    assert_eq!(resolve_target("docs/a/b.md", "../c.md"), "docs/c.md");
    assert_eq!(resolve_target("docs/b.md", "/x/y.md"), "x/y.md");
    assert_eq!(resolve_target("b.md", "./c.md"), "c.md");
}

#[test]
fn resolve_target_normalizes_dot_segments_and_clamps_excessive_dotdot() {
    assert_eq!(resolve_target("docs/a.md", "a/b/../c.md"), "docs/a/c.md");
    assert_eq!(resolve_target("docs/a.md", "./c.md"), "docs/c.md");
    assert_eq!(resolve_target("docs/a.md", "b/./c.md"), "docs/b/c.md");
    assert_eq!(resolve_target("docs/a.md", "b/c/"), "docs/b/c");
    assert_eq!(resolve_target("a/b.md", "../../c.md"), "c.md");
    assert_eq!(resolve_target("a.md", "../../../c.md"), "c.md");
    assert_eq!(resolve_target("docs/a.md", "/x/../y.md"), "y.md");
    assert_eq!(resolve_target("docs/a.md", "/./y.md"), "y.md");
    assert_eq!(resolve_target("docs/a.md", "/a/b/"), "a/b");
    assert_eq!(resolve_target("docs/a.md", "/../y.md"), "y.md");
    assert_eq!(resolve_target("docs/a/b.md", "x/../../c.md"), "docs/c.md");
}

#[test]
fn tilde_run_inside_backtick_fence_is_content() {
    let text = "# T\n\n```rust\n~~~\n```\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn empty_h1_counts_as_the_single_h1() {
    let outcome = check_markdown("a.md", "#\n\nText.\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn document_without_headings_has_no_hierarchy_finding() {
    let text = "See [here](other.md).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("other.md", "x")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn angle_bracket_target_strips() {
    let text = "# T\n\nSee [here](<other.md>).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("other.md", "# O\n")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn autolink_target_resolves() {
    let text = "# T\n\nSee <./other.md>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("other.md", "# O\n")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn directory_link_resolves_to_declared_readme_index() {
    let text = "# T\n\nSee the [CLI](../cli/) guide.\n";
    let outcome = check_markdown(
        "docs/architecture/notes.md",
        text,
        &siblings(&[("docs/cli/README.md", "# CLI\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn directory_link_anchor_checks_index_slugs() {
    let good = "# T\n\nSee [setup](../cli/#setup).\n";
    let outcome = check_markdown(
        "docs/architecture/notes.md",
        good,
        &siblings(&[("docs/cli/README.md", "# CLI\n\n## Setup\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    let bad = "# T\n\nSee [setup](../cli/#missing).\n";
    let outcome = check_markdown(
        "docs/architecture/notes.md",
        bad,
        &siblings(&[("docs/cli/README.md", "# CLI\n\n## Setup\n")]),
    );
    assert_eq!(
        kinds(&outcome),
        vec![(3, FindingKind::MissingAnchor)],
        "{:?}",
        outcome.findings
    );
}

#[test]
fn directory_link_without_declared_index_fails_closed() {
    let text = "# T\n\nSee the [CLI](../cli/) guide.\n";
    let outcome = check_markdown("docs/architecture/notes.md", text, &siblings(&[]));
    assert_eq!(
        kinds(&outcome),
        vec![(3, FindingKind::MissingFileTarget)],
        "{:?}",
        outcome.findings
    );
}

#[test]
fn declared_file_wins_over_directory_index() {
    let text = "# T\n\nSee [cli](cli).\n";
    let outcome = check_markdown(
        "notes.md",
        text,
        &siblings(&[("cli", "# Not a dir\n"), ("cli/README.md", "# Index\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn self_directory_link_resolves_to_root_readme_index() {
    let text = "# T\n\nSee the [index](./) page.\n";
    let outcome = check_markdown("notes.md", text, &siblings(&[("README.md", "# Root\n")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn multiline_code_span_hides_autolink() {
    let text = "# T\n\nRun (`dump <pkg> [-o out]\n[-f]`, more).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn span_closer_misread_as_opener_is_fixed_across_lines() {
    let text = "# T\n\nUnset under `bazel\ntest`. Resolve `$WS/<short_path>` here.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn span_carry_resets_on_blank_line() {
    let text = "# T\n\nStray ` opener hides [gone](gone.md).\n\nSee <also-gone.md>.\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(
        kinds(&outcome),
        vec![
            (3, FindingKind::MissingFileTarget),
            (5, FindingKind::MissingFileTarget),
        ],
        "{:?}",
        outcome.findings
    );
}

#[test]
fn bare_hash_anchor_links_to_top() {
    let outcome = check_markdown("a.md", "# T\n\nBack to [top](#).\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn sibling_fence_content_headings_are_ignored() {
    let text = "# T\n\nSee [r](other.md#real).\n";
    let sibling = "# Part\n\n```text\n## Fake\n```\n\n## Real\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("other.md", sibling)]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn slug_keeps_underscores_like_github() {
    let text = "# T\n\n## well-known_name\n\nSee [s](#well-known_name).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    let text = "# T\n\n## well-known_name\n\nSee [s](#well-known-name).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(5, FindingKind::MissingAnchor)]);
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
    assert_eq!(slug("dx fix --here"), "dx-fix---here");
}

#[test]
fn unicode_anchors_resolve() {
    let text = "# T\n\n## Привет\n\n## 你好\n\nSee [ru](#привет) and [zh](#你好).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    let text = "# T\n\n## 😄 emoji\n\n## Æúű\n\nSee [e](#-emoji) and [a](#æúű).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn sibling_underscore_anchor_resolves() {
    let text = "# Android\n\nSee [p](../patches/README.md#rules_android).\n";
    let outcome = check_markdown(
        "android/README.md",
        text,
        &siblings(&[("patches/README.md", "# Patches\n\n## rules_android\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn sibling_duplicate_headings_get_numbered_slugs() {
    let text = "# T\n\nSee [second](other.md#repeat-1).\n";
    let outcome = check_markdown(
        "a.md",
        text,
        &siblings(&[("other.md", "# O\n\n## Repeat\n\n## Repeat\n")]),
    );
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn duplicate_slug_skips_taken_suffix() {
    let text = "# T\n\n## Foo\n\n## Foo\n\n## Foo-1\n";
    let slugs: Vec<_> = headings(text, &line_starts(text))
        .into_iter()
        .map(|h| h.slug)
        .collect();
    assert_eq!(slugs, vec!["t", "foo", "foo-1", "foo-1-1"]);
}

#[test]
fn scheme_with_digit_is_remote() {
    let text = "# T\n\nSee [bucket](s3://bucket/key).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert_eq!(outcome.skipped_remotes, vec!["s3://bucket/key".to_string()]);
}

#[test]
fn indented_fence_and_heading_are_literal_text() {
    let text = "# T\n\n    ```\n    code\n    ```\n\n    ## Not a heading\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn backtick_in_info_string_is_not_a_fence() {
    let outcome = check_markdown("a.md", "# T\n\n```a`b\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn hash_without_space_is_not_a_heading() {
    let outcome = check_markdown("a.md", "# T\n\n#Nope\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn closing_hashes_strip_from_heading_text() {
    let text = "# T\n\n## Title ##\n\nSee [t](#title).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn hash_inside_heading_text_is_kept() {
    let text = "# T\n\n## C# basics\n\nSee [c](#c-basics).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn empty_definition_label_is_not_a_definition() {
    let outcome = check_markdown("a.md", "# T\n\n[]: other.md\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn definition_without_target_is_not_a_definition() {
    let outcome = check_markdown("a.md", "# T\n\n[ref]:\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn autolink_without_close_is_literal_text() {
    let outcome = check_markdown("a.md", "# T\n\na < b\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_bracket_is_literal_text() {
    let outcome = check_markdown("a.md", "# T\n\nSee [unclosed here.\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn nested_parens_in_target_resolve() {
    let text = "# T\n\nSee [x](sub(1).md).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("sub(1).md", "# S\n")]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_paren_target_is_literal_text() {
    let outcome = check_markdown("a.md", "# T\n\nSee [x](other.md\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn unclosed_reference_label_is_literal_text() {
    let outcome = check_markdown("a.md", "# T\n\nSee [x][label\n", &siblings(&[]));
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
}

#[test]
fn mismatched_code_span_run_skips_gracefully() {
    let text = "# T\n\n``code` and [ok](#t).\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
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
fn cli_clean_sources_emit_nothing() {
    let files = cli_files(&[
        ("/exec/page.md", "# T\n\nSee [o](other.md#o).\n"),
        ("/exec/other.md", "# O\n"),
    ]);
    let (code, out, err) = run_harness(
        &files,
        &[
            "--source",
            "page.md=/exec/page.md",
            "--sibling",
            "other.md=/exec/other.md",
        ],
    );
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
    assert!(err.is_empty(), "{err:?}");
}

#[test]
fn cli_sources_resolve_each_other_without_sibling_flags() {
    let files = cli_files(&[
        ("/exec/a.md", "# A\n\nSee [b](b.md).\n"),
        ("/exec/b.md", "# B\n"),
    ]);
    let (code, out, _) = run_harness(
        &files,
        &["--source", "a.md=/exec/a.md", "--source", "b.md=/exec/b.md"],
    );
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn cli_findings_are_json_lines_and_exit_zero() {
    let files = cli_files(&[("/exec/page.md", "# T\n\nSee [gone](gone.md).\n")]);
    let (code, out, err) = run_harness(&files, &["--source", "page.md=/exec/page.md"]);
    assert_eq!(code, 0);
    assert_eq!(
        out,
        vec![
            "{\"path\":\"page.md\",\"line\":3,\"kind\":\"missing-file-target\",\"message\":\"link target does not resolve to a declared sibling: gone.md\"}"
                .to_string()
        ]
    );
    assert!(err.is_empty(), "{err:?}");
}

#[test]
fn cli_findings_serialize_as_stable_json_lines() {
    let line = FindingLine {
        path: "p.md",
        line: 3,
        kind: "missing-file-target",
        message: "a\"b\\c\né✓",
    };
    assert_eq!(
        serde_json::to_string(&line).expect("serializes"),
        "{\"path\":\"p.md\",\"line\":3,\"kind\":\"missing-file-target\",\"message\":\"a\\\"b\\\\c\\né✓\"}"
    );
    assert_eq!(
        serde_json::to_string(&"bell\x07").expect("serializes"),
        "\"bell\\u0007\""
    );
}

#[test]
fn cli_kind_ids_are_stable() {
    assert_eq!(
        kind_id(FindingKind::UnclosedCodeFence),
        "unclosed-code-fence"
    );
    assert_eq!(kind_id(FindingKind::MissingAnchor), "missing-anchor");
    assert_eq!(kind_id(FindingKind::HeadingHierarchy), "heading-hierarchy");
    assert_eq!(
        kind_id(FindingKind::MissingCodeFenceLanguage),
        "missing-code-fence-language"
    );
    assert_eq!(
        kind_id(FindingKind::MissingFileTarget),
        "missing-file-target"
    );
}

#[test]
fn cli_remotes_are_noted_once_on_stderr() {
    let files = cli_files(&[(
        "/exec/page.md",
        "# T\n\nSee [a](https://example.com/x) and [b](https://example.com/x).\n",
    )]);
    let (code, out, err) = run_harness(&files, &["--source", "page.md=/exec/page.md"]);
    assert_eq!(code, 0);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(
        err,
        vec!["skipped remote target (never fetched): https://example.com/x".to_string()]
    );
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
fn cli_flag_as_value_is_malformed() {
    let files = cli_files(&[]);
    for (args, want) in [
        (
            vec!["--source", "--sibling"],
            "malformed --source \"--sibling\"",
        ),
        (
            vec!["--source", "--source"],
            "malformed --source \"--source\"",
        ),
        (
            vec!["--sibling", "--source"],
            "malformed --sibling \"--source\"",
        ),
    ] {
        let (code, _, err) = run_harness(&files, &args);
        assert_eq!(code, 2, "{args:?}");
        assert_eq!(err.len(), 2, "{args:?} {err:?}");
        assert_eq!(
            err[0],
            format!("{want}, want WS_PATH=EXEC_PATH"),
            "{args:?}"
        );
        assert!(err[1].starts_with("usage: quality_markdown"), "{err:?}");
    }
}

#[test]
fn cli_attached_forms_echo_whole_token() {
    let files = cli_files(&[]);
    for (args, want) in [
        (vec!["--bogus=x"], "unknown argument: --bogus=x"),
        (vec!["--source="], "unknown argument: --source="),
        (vec!["--sibling="], "unknown argument: --sibling="),
        (
            vec!["--sibling=no-equals"],
            "malformed --sibling \"no-equals\", want WS_PATH=EXEC_PATH",
        ),
    ] {
        let (code, _, err) = run_harness(&files, &args);
        assert_eq!(code, 2, "{args:?}");
        assert_eq!(err.len(), 2, "{args:?} {err:?}");
        assert_eq!(err[0], want, "{args:?}");
        assert!(err[1].starts_with("usage: quality_markdown"), "{err:?}");
    }
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
fn cli_empty_value_is_malformed() {
    let files = cli_files(&[]);
    for (args, want) in [
        (
            vec!["--source", ""],
            "malformed --source \"\", want WS_PATH=EXEC_PATH",
        ),
        (
            vec!["--sibling", ""],
            "malformed --sibling \"\", want WS_PATH=EXEC_PATH",
        ),
        (
            vec!["--source", "", "--source="],
            "malformed --source \"\", want WS_PATH=EXEC_PATH",
        ),
        (
            vec!["--source=", "--source", ""],
            "unknown argument: --source=",
        ),
    ] {
        let (code, _, err) = run_harness(&files, &args);
        assert_eq!(code, 2, "{args:?}");
        assert_eq!(err.len(), 2, "{args:?} {err:?}");
        assert_eq!(err[0], want, "{args:?}");
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
    let outcome = check_markdown("a.md", "# T\n\n> ```sh\n> echo\n> ```\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn fence_in_nested_list_is_closed() {
    let text =
        "# T\n\n- a\n    - b\n\n      ```sh\n      echo\n      ```\n- ```sh\n  echo\n  ```\n";
    let outcome = check_markdown("a.md", text, &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn unclosed_fence_in_blockquote_is_a_finding() {
    let outcome = check_markdown("a.md", "# T\n\n> ```sh\n> echo\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn fence_with_four_space_indented_close_text_is_unclosed() {
    let outcome = check_markdown("a.md", "# T\n\n```sh\necho\n    ```\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn quoted_fence_content_that_looks_like_fence_is_unclosed() {
    let outcome = check_markdown("a.md", "# T\n\n> ```sh\n> > ```\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![(3, FindingKind::UnclosedCodeFence)]);
}

#[test]
fn setext_h1_counts_as_h1() {
    let outcome = check_markdown(
        "a.md",
        "Title\n=====\n\n[x](#title)\n\n# Two\n",
        &siblings(&[]),
    );
    assert_eq!(kinds(&outcome), vec![(6, FindingKind::HeadingHierarchy)]);
}

#[test]
fn duplicate_sibling_heading_gets_counter_slug() {
    let text = "# T\n\n## A\n\n## A\n\n[x](#a-1)\n";
    assert_eq!(kinds(&check_markdown("a.md", text, &siblings(&[]))), vec![]);
}

#[test]
fn heading_link_text_slugs_rendered_text() {
    let text = "# T\n\n## See [a](b.md)\n\n[x](#see-a)\n";
    let outcome = check_markdown("a.md", text, &siblings(&[("b.md", "# B\n")]));
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn quoted_heading_skips_hierarchy() {
    let outcome = check_markdown("a.md", "# T\n\n> ### Q\n", &siblings(&[]));
    assert_eq!(kinds(&outcome), vec![]);
}

#[test]
fn duplicate_heading_counter_includes_quoted_heading() {
    let text = "# T\n\n> ## A\n\n## A\n\n[x](#a-1)\n";
    assert_eq!(kinds(&check_markdown("a.md", text, &siblings(&[]))), vec![]);
}
