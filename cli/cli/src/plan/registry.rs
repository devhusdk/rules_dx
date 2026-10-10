use super::{CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG};
use crate::args::Command;
use crate::reports::StandardFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub aspects: &'static [&'static str],
    pub reports: &'static [StandardFormat],
    pub settings: &'static [&'static str],
}

impl CommandSpec {
    pub fn accepts_report(&self, format: &str) -> bool {
        StandardFormat::parse(format).is_some_and(|parsed| self.reports.contains(&parsed))
    }
}

pub fn keep_sorted_support_state() -> &'static str {
    "qualified"
}

pub fn keep_sorted_cli_gap(aspects: &[&str]) -> Option<&'static str> {
    if aspects
        .iter()
        .any(|aspect| aspect.contains("real_text_lint"))
    {
        None
    } else {
        Some("support: keep_sorted lint is qualified but has no CLI lint aspect (missing real_text_lint_aspect)")
    }
}

pub fn spec(command: Command) -> CommandSpec {
    match command {
        Command::Lint => CommandSpec {
            command,
            aspects: &[
                "//quality:real_aspects.bzl%real_lint_aspect",
                "//quality:real_aspects.bzl%real_js_lint_aspect",
                "//quality:real_aspects.bzl%real_python_lint_aspect",
                "//quality:real_aspects.bzl%real_jvm_lint_aspect",
                "//quality:real_aspects.bzl%real_rust_lint_aspect",
                "//quality:real_aspects.bzl%real_shell_lint_aspect",
                "//quality:real_aspects.bzl%real_text_lint_aspect",
            ],
            reports: &[StandardFormat::Sarif],
            settings: &[CLIPPY_DIAGNOSTICS_FLAG],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: &[
                "//quality:real_aspects.bzl%real_typecheck_aspect",
                "//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ],
            reports: &[StandardFormat::Sarif],
            settings: &[RUSTC_DIAGNOSTICS_FLAG],
        },
        Command::Format => CommandSpec {
            command,
            aspects: &[
                "//quality:real_aspects.bzl%real_format_aspect",
                "//quality:real_aspects.bzl%real_js_format_aspect",
                "//quality:real_aspects.bzl%real_jvm_format_aspect",
                "//quality:real_aspects.bzl%real_rust_format_aspect",
                "//quality:real_aspects.bzl%real_shell_format_aspect",
                "//quality:real_aspects.bzl%real_cpp_format_aspect",
            ],
            reports: &[],
            settings: &[],
        },
        Command::Generate => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Build => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Test => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Junit],
            settings: &[],
        },
        Command::Coverage => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Lcov],
            settings: &[],
        },
        Command::Run => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Deploy => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Check => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
            settings: &[],
        },
        Command::Fix => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
            settings: &[],
        },
        Command::Verify => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Rerun => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Tests => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Clean => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Codegen | Command::Env | Command::Setup => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Hooks
        | Command::Status
        | Command::Capabilities
        | Command::Version
        | Command::Watch
        | Command::Owners
        | Command::Deps
        | Command::Why
        | Command::Completion => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Security => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
            settings: &[],
        },
        Command::License => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif, StandardFormat::Spdx],
            settings: &[],
        },
        Command::Update => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Bump => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Migrate => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Docs => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
        Command::Bazel => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
            settings: &[],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_entries_match_command_contract() {
        let lint = spec(Command::Lint);
        assert_eq!(
            lint.aspects,
            &[
                "//quality:real_aspects.bzl%real_lint_aspect",
                "//quality:real_aspects.bzl%real_js_lint_aspect",
                "//quality:real_aspects.bzl%real_python_lint_aspect",
                "//quality:real_aspects.bzl%real_jvm_lint_aspect",
                "//quality:real_aspects.bzl%real_rust_lint_aspect",
                "//quality:real_aspects.bzl%real_shell_lint_aspect",
                "//quality:real_aspects.bzl%real_text_lint_aspect",
            ]
        );
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        assert_eq!(lint.settings, &[CLIPPY_DIAGNOSTICS_FLAG]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "//quality:real_aspects.bzl%real_typecheck_aspect",
                "//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ]
        );
        assert_eq!(typecheck.reports, &[StandardFormat::Sarif]);
        assert_eq!(typecheck.settings, &[RUSTC_DIAGNOSTICS_FLAG]);
        let format = spec(Command::Format);
        assert_eq!(
            format.aspects,
            &[
                "//quality:real_aspects.bzl%real_format_aspect",
                "//quality:real_aspects.bzl%real_js_format_aspect",
                "//quality:real_aspects.bzl%real_jvm_format_aspect",
                "//quality:real_aspects.bzl%real_rust_format_aspect",
                "//quality:real_aspects.bzl%real_shell_format_aspect",
                "//quality:real_aspects.bzl%real_cpp_format_aspect",
            ]
        );
        assert!(format.reports.is_empty());
        let build = spec(Command::Build);
        assert!(build.aspects.is_empty());
        assert!(build.reports.is_empty());
        let test = spec(Command::Test);
        assert_eq!(test.reports, &[StandardFormat::Junit]);
        let coverage = spec(Command::Coverage);
        assert_eq!(coverage.reports, &[StandardFormat::Lcov]);
        let run = spec(Command::Run);
        assert!(run.aspects.is_empty());
        assert!(run.reports.is_empty());
        let generate = spec(Command::Generate);
        assert!(generate.aspects.is_empty());
        assert!(generate.reports.is_empty());
        let clean = spec(Command::Clean);
        assert!(clean.aspects.is_empty());
        assert!(clean.reports.is_empty());
        let bazel = spec(Command::Bazel);
        assert!(bazel.aspects.is_empty());
        assert!(bazel.reports.is_empty());
        for command in [Command::Codegen, Command::Env, Command::Setup] {
            let entry = spec(command);
            assert!(entry.aspects.is_empty());
            assert!(entry.reports.is_empty());
            assert!(command.is_managed());
        }
        for command in [Command::Security, Command::License] {
            let entry = spec(command);
            assert!(entry.aspects.is_empty());
            assert!(command.is_audit_update());
        }
        assert_eq!(spec(Command::Security).reports, &[StandardFormat::Sarif]);
        assert_eq!(
            spec(Command::License).reports,
            &[StandardFormat::Sarif, StandardFormat::Spdx]
        );
        let update = spec(Command::Update);
        assert!(update.aspects.is_empty());
        assert!(update.reports.is_empty());
        assert!(Command::Update.is_audit_update());
        let bump = spec(Command::Bump);
        assert!(bump.aspects.is_empty());
        assert!(bump.reports.is_empty());
        assert!(Command::Bump.is_audit_update());
        let migrate = spec(Command::Migrate);
        assert!(migrate.aspects.is_empty());
        assert!(migrate.reports.is_empty());
        assert!(!Command::Migrate.is_audit_update());
    }

    #[test]
    fn standard_format_names_and_parsing_are_inverses() {
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Spdx,
        ] {
            assert_eq!(StandardFormat::parse(format.name()), Some(format));
        }
        for text in ["sraif", "SARIF", "Junit", "", "sarif "] {
            assert_eq!(StandardFormat::parse(text), None, "{text:?} must not parse");
        }
    }

    #[test]
    fn every_standard_format_reaches_at_least_one_command() {
        use clap::ValueEnum;
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Spdx,
        ] {
            assert!(
                Command::value_variants()
                    .iter()
                    .any(|command| spec(*command).accepts_report(format.name())),
                "no command accepts the {format:?} format"
            );
        }
    }

    #[test]
    fn keep_sorted_cli_gap_is_explicit() {
        assert_eq!(keep_sorted_support_state(), "qualified");
        let lint = spec(Command::Lint);
        assert_eq!(keep_sorted_cli_gap(lint.aspects), None);
        assert!(
            lint.aspects
                .iter()
                .any(|aspect| aspect.contains("real_text_lint")),
            "lint wires the text aspect"
        );
        assert_eq!(
            keep_sorted_cli_gap(&["//quality:real_aspects.bzl%real_text_lint_aspect"]),
            None
        );
    }
}
