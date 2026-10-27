use std::path::Path;

use dx_codegen::quote_label;
use dx_plan::resolve::{QueryFailure, QueryRunner, ResolveError, SelectionContext};

const ARG_TOLERANT_KINDS: [&str; 2] = ["rust_clippy_test", "rustfmt_test"];

/// Test rule kinds whose binaries take libtest arguments such as `--exact`.
const SUPPORTED_CASE_KINDS: [&str; 2] = ["rust_test", "_rust_forward_test"];

pub(crate) fn last_test_filter(options: &[String]) -> Option<String> {
    let mut found = None;
    let mut index = 0;
    while index < options.len() {
        let arg = &options[index];
        if let Some(value) = arg.strip_prefix("--test_filter=") {
            found = Some(value.to_owned());
        } else if arg == "--test_filter" {
            found = options.get(index + 1).cloned();
            index += 1;
        }
        index += 1;
    }
    found.filter(|value| !value.is_empty())
}

pub(crate) fn tests_expression(targets: &[String]) -> Option<String> {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    for target in targets {
        match target.strip_prefix('-') {
            Some(negative) => exclude.push(quote_label(negative)),
            None => include.push(quote_label(target)),
        }
    }
    if include.is_empty() {
        return None;
    }
    let mut expr = include.join(" + ");
    for negative in exclude {
        expr.push_str(" - ");
        expr.push_str(&negative);
    }
    Some(format!("tests({expr})"))
}

pub(crate) fn rust_filter_args(
    options: &[String],
    targets: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    selection: &SelectionContext,
) -> Result<Vec<String>, ResolveError> {
    let Some(filter) = last_test_filter(options) else {
        return Ok(Vec::new());
    };
    let Some(expr) = tests_expression(targets) else {
        return Ok(Vec::new());
    };
    let argv = selection
        .unconfigured_view()
        .selection_argv("query", &["--output=label_kind".to_owned()], &expr)
        .map_err(|error| {
            ResolveError::query(
                &expr,
                QueryFailure::Prepare {
                    detail: error.to_string(),
                },
            )
        })?;
    let label = format!("bazel query {expr}");
    let result = ResolveError::run(runner, &argv, workspace, &label)?;
    let kinds = parse_label_kind(&result.stdout).unwrap_or_default();
    let is_rust = |kind: &str| SUPPORTED_CASE_KINDS.contains(&kind);
    let all_rust = kinds.iter().any(|(kind, _)| is_rust(kind))
        && kinds
            .iter()
            .all(|(kind, _)| is_rust(kind) || ARG_TOLERANT_KINDS.contains(&kind.as_str()));
    if !all_rust {
        return Ok(Vec::new());
    }
    let rust_labels: Vec<String> = kinds
        .into_iter()
        .filter(|(kind, _)| is_rust(kind))
        .map(|(_, label)| label)
        .collect();
    if !without_libtest_harness(&rust_labels, workspace, runner, selection)?.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![format!("--test_arg={filter}")])
}

/// Returns the targets among `labels` and their `upstream` tests that set
/// `use_libtest_harness = False`. Such binaries do not parse libtest filters.
pub(crate) fn without_libtest_harness(
    labels: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    selection: &SelectionContext,
) -> Result<Vec<String>, ResolveError> {
    if labels.is_empty() {
        return Ok(Vec::new());
    }
    let set = dx_plan::resolve::set_expression(labels);
    let expr = format!("attr(use_libtest_harness, 0, {set} + labels(upstream, {set}))");
    let argv = selection
        .unconfigured_view()
        .selection_argv("query", &["--output=label".to_owned()], &expr)
        .map_err(|error| {
            ResolveError::query(
                &expr,
                dx_plan::resolve::QueryFailure::Prepare {
                    detail: error.to_string(),
                },
            )
        })?;
    let label = format!("bazel query {expr}");
    let result = ResolveError::run(runner, &argv, workspace, &label)?;
    Ok(String::from_utf8_lossy(&result.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Parses `--output=label_kind` query lines into sorted, unique (kind, label) pairs.
fn parse_label_kind(stdout: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = std::str::from_utf8(stdout).map_err(|_| "query output is not UTF-8".to_owned())?;
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next(), parts.next()) {
            (Some(kind), Some(rule), Some(label)) if rule == "rule" && label.starts_with("//") => {
                pairs.push((kind.to_owned(), label.to_owned()));
            }
            _ => {
                return Err(format!(
                    "query returned a malformed test observation: {line}"
                ));
            }
        }
    }
    pairs.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
    pairs.dedup_by(|left, right| left.1 == right.1);
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_testing::cli::strings;

    #[test]
    fn last_test_filter_reads_both_forms_and_keeps_the_last() {
        assert_eq!(last_test_filter(&strings(&["--jobs=2"])), None);
        assert_eq!(
            last_test_filter(&strings(&["--test_filter=slots"])).as_deref(),
            Some("slots")
        );
        assert_eq!(
            last_test_filter(&strings(&["--test_filter", "a", "--test_filter=b"])).as_deref(),
            Some("b")
        );
        assert_eq!(
            last_test_filter(&strings(&["--test_filter=b", "--test_filter", "a"])).as_deref(),
            Some("a")
        );
        assert_eq!(last_test_filter(&strings(&["--test_filter="])), None);
        assert_eq!(last_test_filter(&strings(&["--test_filter"])), None);
    }

    #[test]
    fn tests_expression_subtracts_negative_patterns() {
        assert_eq!(
            tests_expression(&strings(&["//a/...", "//b:t", "-//a/skip/..."])).as_deref(),
            Some("tests(\"//a/...\" + \"//b:t\" - \"//a/skip/...\")")
        );
        assert_eq!(tests_expression(&strings(&["-//a/..."])), None);
    }

    #[test]
    fn label_kind_lines_sort_dedup_and_reject_malformed_output() {
        let pairs = parse_label_kind(
            b"rust_test rule //b:two\n\nsh_test rule //a:one\nrust_test rule //b:two\n",
        )
        .expect("parse");
        assert_eq!(
            pairs,
            [
                ("sh_test".to_owned(), "//a:one".to_owned()),
                ("rust_test".to_owned(), "//b:two".to_owned()),
            ]
        );
        let error = parse_label_kind(b"rust_test //a:one\n").expect_err("malformed");
        assert!(error.contains("malformed test observation"), "{error}");
        assert!(parse_label_kind(&[0xff]).is_err());
    }
}
