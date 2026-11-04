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
pub mod git_changes;
pub mod init_dir;
pub mod launcher;
pub mod load_graph;
pub mod module_pin;
pub mod overrides;
pub mod preflight;
pub mod preset;
pub mod preset_fragment;
pub mod scaffold;
pub mod status;
pub mod upgrade;
pub mod version;
pub mod watch;

pub use defaults::join_quality_baseline;
pub use error::AdoptError;
pub use git_changes::{
    dedupe_changes, is_dx_state, nearest_package_pattern, parse_name_status_nul, ChangeKind,
    GitChange, DEFAULT_BRANCH_CANDIDATES,
};
pub use init_dir::{
    init_dir_module, is_fresh_dir, plan_fresh_files, validate_init_dir, DEFAULT_INIT_MODULE,
    FRESH_BAZEL_VERSION,
};
pub use launcher::{
    launcher_state, plan_launcher_files, refresh_launcher_files, release_pins, render_launcher_cmd,
    render_launcher_sh, stale_launchers, write_launcher_files, LauncherPins, LauncherState,
    LAUNCHER_CMD_REL, LAUNCHER_SH_REL, RELEASE_ASSETS,
};
pub use load_graph::{bzl_impact, module_inputs, repo_impact, BzlImpact};
pub use module_pin::{plan_rules_dx_version, write_module_edit, ModuleEdit};
pub use overrides::{
    apply_overrides, check_overrides, check_overrides_in, preflight_overrides, render_overrides, OverridesState,
    OVERRIDES_INCLUDE,
};
pub use preflight::{preflight, render_plan_text, Finding, Preflight};
pub use preset::{
    inspect_preset, preflight_preset, preset_drift, preset_paths, sync_preset, write_preset,
    PresetDrift, PresetState,
};
pub use scaffold::{
    apply_init, bazelignore_covers_dx, plan_init_files, plan_init_files_at, preflight_destinations,
    refresh_owned_files, render_consumer_caller, render_consumer_module_setup, Applied, FileKind,
};
pub use status::{observed_status_checks, render_status_text, StatusCheck};
pub use upgrade::{upgrade_recovery_message, upgrade_retry_command, UpgradeIntent};
pub use version::{
    local_path_overrides, module_dependency_status, pin_fix_hint, pin_is_valid, pin_rewrite_hint,
    read_module_dependency, read_version_pin, read_version_pins, render_module_dependency,
    version_pin_matches_module, write_version_pin, write_version_pins, DX_VERSION, MODULE_VERSION,
};
pub use watch::{plan_watch, WATCHABLE_COMMANDS};
