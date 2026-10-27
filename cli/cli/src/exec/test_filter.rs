use std::path::Path;

use crate::exec::test_inventory::{parse_label_kind, SUPPORTED_CASE_KINDS};
use dx_codegen::quote_label;
use dx_plan::resolve::{QueryRunner, ResolveError};

const ARG_TOLERANT_KINDS: [&str; 2] = ["rust_clippy_test", "rustfmt_test"];

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
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    let Some(filter) = last_test_filter(options) else {
        return Ok(Vec::new());
    };
    let Some(expr) = tests_expression(targets) else {
        return Ok(Vec::new());
    };
    let mut argv = dx_process::startup_argv(startup_options);
    argv.push("query".to_owned());
    argv.push("--output=label_kind".to_owned());
    argv.push("--".to_owned());
    argv.push(expr.clone());
    let label = format!("bazel query {expr}");
    let result = ResolveError::run(runner, &argv, workspace, &label)?;
    let kinds = parse_label_kind(&result.stdout).unwrap_or_default();
    let is_rust = |kind: &str| SUPPORTED_CASE_KINDS.contains(&kind);
    let all_rust = kinds.iter().any(|(kind, _)| is_rust(kind))
        && kinds
            .iter()
            .all(|(kind, _)| is_rust(kind) || ARG_TOLERANT_KINDS.contains(&kind.as_str()));
    if all_rust {
        Ok(vec![format!("--test_arg={filter}")])
    } else {
        Ok(Vec::new())
    }
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
}
