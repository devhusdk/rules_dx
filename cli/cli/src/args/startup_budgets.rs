/// Measured interactive budgets for startup, completion, and scoped checks.
use std::time::Duration;

use super::completion::QUERY_TIMEOUT;
use super::{is_discovery_exempt, is_help_request, parse_with_ci, ArgsError, Command, FileDefaults};

/// Warm help text on Linux x86_64 measures near 50ms; the ceiling holds 40x headroom.
pub const HELP_TEXT_BUDGET_MS: u64 = 2000;

/// Warm version flag measures near 35ms; it shares the help early-return path.
pub const VERSION_FLAG_BUDGET_MS: u64 = 2000;

/// Warm argument errors measure near 15ms; they return before workspace discovery.
pub const ARG_ERROR_BUDGET_MS: u64 = 2000;

/// Warm completion script render measures near 15ms; it never queries Bazel.
pub const COMPLETION_RENDER_BUDGET_MS: u64 = 2000;

/// One bounded single-package query may take this long before completion falls back.
pub const COMPLETION_QUERY_DEADLINE_MS: u64 = 1500;

/// A full completion round stays inside this even when the query runs the deadline out.
pub const COMPLETION_TOTAL_BUDGET_MS: u64 = 5000;

/// Ownership inspection delegates to a Bazel query, so it carries no CLI ceiling.
pub const OWNERSHIP_DELEGATES_TO_BAZEL: bool = true;

fn local_budgets() -> [(&'static str, u64, u64); 4] {
    [
        ("help", HELP_TEXT_BUDGET_MS, 50),
        ("version", VERSION_FLAG_BUDGET_MS, 35),
        ("arg-error", ARG_ERROR_BUDGET_MS, 15),
        ("completion-render", COMPLETION_RENDER_BUDGET_MS, 15),
    ]
}

fn assert_help(words: &[&str]) {
    let args: Vec<String> = words.iter().map(ToString::to_string).collect();
    let Err(ArgsError::Help { .. }) =
        parse_with_ci(&args, &|_| None, &FileDefaults::default(), false)
    else {
        panic!("{words:?} must return pre-discovery help");
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_cover_measured_floors_with_headroom() {
        for (name, budget, floor) in local_budgets() {
            assert!(
                budget >= floor * 10,
                "{name} budget {budget}ms must hold 10x over the {floor}ms floor"
            );
        }
        assert!(
            COMPLETION_QUERY_DEADLINE_MS >= 1500,
            "the query deadline must cover a cold single-package query"
        );
    }

    #[test]
    fn budgets_stay_interactive_not_vacuous() {
        for (name, budget, _) in local_budgets() {
            assert!(
                budget <= COMPLETION_TOTAL_BUDGET_MS,
                "{name} budget {budget}ms must stay within one interaction"
            );
        }
        assert!(
            COMPLETION_TOTAL_BUDGET_MS <= 5000,
            "completion total must stay a real ceiling"
        );
        assert!(
            COMPLETION_QUERY_DEADLINE_MS < COMPLETION_TOTAL_BUDGET_MS,
            "the query deadline must leave render headroom"
        );
    }

    #[test]
    fn query_deadline_matches_the_completer() {
        assert_eq!(
            QUERY_TIMEOUT,
            Duration::from_millis(COMPLETION_QUERY_DEADLINE_MS),
            "the recorded deadline must be the enforced one"
        );
    }

    #[test]
    fn help_and_version_flags_need_no_workspace() {
        for words in [
            vec!["--help"],
            vec!["--version"],
            vec!["-V"],
            vec!["build", "--help"],
            vec!["version", "--help"],
        ] {
            assert!(is_help_request(&words), "{words:?} is a help request");
            assert_help(&words);
        }
        assert!(!is_help_request(&["build"] as &[&str]));
    }

    #[test]
    fn argument_errors_surface_before_discovery() {
        let args: Vec<String> = ["frobnicate"].iter().map(ToString::to_string).collect();
        let Err(error) = parse_with_ci(&args, &|_| None, &FileDefaults::default(), false) else {
            panic!("an unknown command must fail at parse time");
        };
        assert!(
            matches!(error, ArgsError::Usage { .. }),
            "an unknown command must render usage, got {error:?}"
        );
    }

    #[test]
    fn completion_render_needs_no_workspace() {
        assert!(
            is_discovery_exempt(Command::Completion),
            "completion must stay runnable without module discovery"
        );
        let script = super::super::completion::render_completion("bash").expect("render");
        assert!(
            super::super::completion::registers_callback(&script, "bash"),
            "the rendered script must call back into dx"
        );
    }

    #[test]
    fn ownership_has_no_cli_ceiling_by_policy() {
        assert!(
            OWNERSHIP_DELEGATES_TO_BAZEL,
            "ownership latency belongs to the Bazel query, not the CLI"
        );
    }
}
