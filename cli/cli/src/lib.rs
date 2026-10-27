// LCOV_EXCL_START - reason: re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod adopt;
mod args;
mod disk;
mod exec;
mod finalize;
mod generate;
mod plan;
mod platform;
mod reports;
mod resolve;
pub mod skew;
mod slots;
#[cfg(test)]
mod test_support;

pub use args::help::{help_command_in, usage_banner};
pub use args::{
    apply_here, ci_ignores_env, early_workspace_flag, is_discovery_exempt, is_help_request,
    json_output_intent, load_startup_defaults, man_page, parse_with_ci, try_complete, ArgsError,
    Command, FileDefaults, Profile, DX_WORKSPACE_ENV,
};
pub use exec::common::{emit_startup_outcome, flush_out};
pub use exec::{execute, hook_kind, run_worktree_hook, set_usage_command, Env, AUDIT_TODAY_ENV};
pub use plan::{
    clean_for_stop, create_run_temp_dir, plan_build, plan_deploy_build, plan_deploy_run,
    plan_generate, plan_managed_with_roots, plan_run_build, plan_run_targets, plan_workflow,
    remove_on_stop, remove_stale_run_dirs, set_rules_dx_repo, WorkflowVerb,
};
pub use platform::host_refusal;
pub use resolve::{QueryResult, QueryRunner, ResolvedScope};
pub use slots::{inherit_share, with_gate, AfterFirstLine, Gate, SHARE_ENV_VAR, SLOT_ENV_VAR};
// LCOV_EXCL_STOP - reason: end re-export only, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
