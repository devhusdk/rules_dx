#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use dx_release_tools::{bcr_submit_record, ledger_exit, render_ledger};

fn run(args: &[String]) -> (String, i32) {
    let [_, subject, result, pr_url] = args else {
        return (
            "usage: bcr_ledger <subject> <result> <pr_url>\n".to_owned(),
            2,
        );
    };
    let records = [bcr_submit_record(subject, result, pr_url)];
    (render_ledger(&records), ledger_exit(&records))
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let (text, code) = run(&std::env::args().collect::<Vec<_>>());
    print!("{text}");
    std::process::exit(code);
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::run;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn a_bcr_pull_request_is_recorded_as_executed() {
        let url = "https://github.com/bazelbuild/bazel-central-registry/pull/9";
        let (text, code) = run(&args(&["bcr_ledger", "rules_dx@1.2.3", "success", url]));
        assert_eq!(code, 0);
        assert!(text.contains("\"state\": \"executed\""));
    }

    #[test]
    fn a_failed_job_exits_nonzero() {
        let (text, code) = run(&args(&["bcr_ledger", "rules_dx@1.2.3", "failure", ""]));
        assert_eq!(code, 1);
        assert!(text.contains("\"state\": \"failed\""));
    }

    #[test]
    fn wrong_argument_count_is_a_usage_error() {
        assert_eq!(run(&args(&["bcr_ledger"])).1, 2);
    }
}
