#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod build_output;
pub mod defaults;
pub mod dependency_sets;
pub mod error;
pub mod families;
pub mod git_changes;
pub mod init_dir;
pub mod launcher;
pub mod legacy_layout;
pub mod load_graph;
pub mod lock_hubs;
pub mod module_pin;
pub mod overrides;
pub mod ownership;
pub mod preflight;
pub mod preset;
pub mod preset_fragment;
pub mod scaffold;
pub mod status;
mod rule_keys;
pub mod upgrade;
pub mod version;
pub mod watch;

#[cfg(test)]
mod local_paths_tests;

pub use error::AdoptError;
pub use families::{FamilyDeps, detected_families, plan_family_deps};
pub use git_changes::{
    ChangeKind, DEFAULT_BRANCH_CANDIDATES, GitChange, dedupe_changes, dx_config_git_ignored,
    is_dx_state, nearest_package_pattern, parse_name_status_nul,
};
pub use init_dir::{
    DEFAULT_INIT_MODULE, FRESH_BAZEL_VERSION, fresh_blocker, init_dir_module, is_fresh_dir,
    plan_fresh_files, validate_init_dir,
};
pub use launcher::{
    LAUNCHER_CMD_REL, LAUNCHER_SH_REL, LauncherPins, LauncherState, RELEASE_ASSETS,
    init_apply_command, launcher_state, plan_launcher_files, refresh_launcher_files, release_pins,
    render_launcher_cmd, render_launcher_sh, stale_launchers, write_launcher_files,
};
pub use load_graph::{BzlImpact, bzl_impact, module_inputs, repo_impact};
pub use lock_hubs::{
    LockHubs, MavenLocks, plan_lock_hubs, plan_maven_locks, plan_pnpm_tool, unlocked_manifests,
};
pub use module_pin::{ModuleEdit, plan_rules_dx_version, write_edit_or_create, write_module_edit};
pub use overrides::{
    OVERRIDES_INCLUDE, OverridesState, apply_overrides, check_overrides, check_overrides_in,
    is_rules_dx_root, module_deps, preflight_overrides, refresh_overrides, render_overrides,
};
pub use ownership::Ownership;
pub use preflight::{Finding, Preflight, preflight, render_plan_text};
pub use preset::{
    PresetDrift, PresetState, inspect_preset, preflight_preset, preset_drift, preset_files,
    preset_paths, write_preset,
};
pub use scaffold::{
    Applied, CONSUMER_WORKFLOW_PIN, CiProvider, FileKind, bazelignore_covers_dx,
    owned_file_drift, plan_init_files, plan_init_files_at,
    preflight_destinations, refresh_scaffold, render_consumer_caller, render_consumer_module_setup,
    render_gitlab_caller, with_overrides_include,
};
pub use status::{StatusCheck, observed_status_checks, render_status_text};
pub use upgrade::{
    Selections, ToolSwap, UpgradeIntent, base_changes, merge_override, override_conflict_message,
    override_missing_base_message, suppression_lines, tool_swaps, upgrade_recovery_message, upgrade_retry_command,
};
pub use version::{
    DX_VERSION, MODULE_VERSION, local_path_overrides, module_dependency_status,
    module_rules_dx_version, pin_is_valid, pin_rewrite_hint, read_module_dependency,
    read_version_pin, read_version_pins, refresh_stale_pins, refresh_stale_pins_from_env,
    render_module_dependency, version_pin_matches_module, write_version_pin, write_version_pins,
};
pub use watch::{WATCHABLE_COMMANDS, plan_watch};
