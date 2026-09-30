use super::{CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG};
use crate::args::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub aspects: &'static [&'static str],
    pub reports: &'static [&'static str],
    pub settings: &'static [&'static str],
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
            ],
            reports: &["sarif"],
            settings: &[CLIPPY_DIAGNOSTICS_FLAG],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: &[
                "//quality:real_aspects.bzl%real_typecheck_aspect",
                "//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ],
            reports: &["sarif"],
            settings: &[RUSTC_DIAGNOSTICS_FLAG],
        },
        Command::Format => CommandSpec {
            command,
            aspects: &[
                "//quality:real_aspects.bzl%real_format_aspect",
                "//quality:real_aspects.bzl%real_js_format_aspect",
                "//quality:real_aspects.bzl%real_jvm_format_aspect",
                "//quality:real_aspects.bzl%real_rust_format_aspect",
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
            reports: &["junit"],
            settings: &[],
        },
        Command::Coverage => CommandSpec {
            command,
            aspects: &[],
            reports: &["lcov"],
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
            reports: &["sarif"],
            settings: &[],
        },
        Command::Fix => CommandSpec {
            command,
            aspects: &[],
            reports: &["sarif"],
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
            reports: &["sarif"],
            settings: &[],
        },
        Command::License => CommandSpec {
            command,
            aspects: &[],
            reports: &["sarif", "spdx"],
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
            ]
        );
        assert_eq!(lint.reports, &["sarif"]);
        assert_eq!(lint.settings, &[CLIPPY_DIAGNOSTICS_FLAG]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "//quality:real_aspects.bzl%real_typecheck_aspect",
                "//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ]
        );
        assert_eq!(typecheck.reports, &["sarif"]);
        assert_eq!(typecheck.settings, &[RUSTC_DIAGNOSTICS_FLAG]);
        let format = spec(Command::Format);
        assert_eq!(
            format.aspects,
            &[
                "//quality:real_aspects.bzl%real_format_aspect",
                "//quality:real_aspects.bzl%real_js_format_aspect",
                "//quality:real_aspects.bzl%real_jvm_format_aspect",
                "//quality:real_aspects.bzl%real_rust_format_aspect",
            ]
        );
        assert!(format.reports.is_empty());
        let build = spec(Command::Build);
        assert!(build.aspects.is_empty());
        assert!(build.reports.is_empty());
        let test = spec(Command::Test);
        assert_eq!(test.reports, &["junit"]);
        let coverage = spec(Command::Coverage);
        assert_eq!(coverage.reports, &["lcov"]);
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
        assert_eq!(spec(Command::Security).reports, &["sarif"]);
        assert_eq!(spec(Command::License).reports, &["sarif", "spdx"]);
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
}
