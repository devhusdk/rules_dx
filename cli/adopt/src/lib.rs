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
pub mod load_graph;
pub mod lock_hubs;
pub mod module_pin;
pub mod overrides;
pub mod preflight;
pub mod preset;
pub mod preset_fragment;
pub mod profiles;
pub mod scaffold;
pub mod status;
pub mod upgrade;
pub mod version;
pub mod watch;

pub use defaults::join_quality_baseline;
pub use error::AdoptError;
pub use families::{plan_family_deps, FamilyDeps};
pub use git_changes::{
    dedupe_changes, dx_config_git_ignored, is_dx_state, nearest_package_pattern,
    parse_name_status_nul, ChangeKind, GitChange, DEFAULT_BRANCH_CANDIDATES, DX_CONFIG_DIR,
};
pub use init_dir::{
    fresh_blocker, init_dir_module, is_fresh_dir, plan_fresh_files, validate_init_dir,
    DEFAULT_INIT_MODULE, FRESH_BAZEL_VERSION,
};
pub use launcher::{
    init_apply_command, launcher_state, plan_launcher_files, refresh_launcher_files, release_pins,
    render_launcher_cmd, render_launcher_sh, stale_launchers, write_launcher_files, LauncherPins,
    LauncherState, LAUNCHER_CMD_REL, LAUNCHER_SH_REL, RELEASE_ASSETS,
};
pub use load_graph::{bzl_impact, module_inputs, repo_impact, BzlImpact};
pub use lock_hubs::{plan_lock_hubs, LockHubs};
pub use module_pin::{plan_rules_dx_version, write_module_edit, ModuleEdit};
pub use overrides::{
    apply_overrides, check_overrides, check_overrides_in, is_rules_dx_root, module_deps,
    preflight_overrides, refresh_overrides, render_overrides, OverridesState, OVERRIDES_INCLUDE,
};
pub use preflight::{preflight, render_plan_text, Finding, Preflight};
pub use preset::{
    inspect_preset, inspect_preset_as, layer_profile, preflight_preset, preset_drift, preset_files,
    preset_paths, read_profile, refresh_preset, sync_preset, write_preset, PresetDrift,
    PresetState, WERROR_NOTE,
};
pub use profiles::{infer_profile, plan_profile_files, profile_notes, Profile};
pub use scaffold::{
    bazelignore_covers_dx, bazelignore_has_bare_dx, owned_file_drift, plan_init_files,
    plan_init_files_at, preflight_destinations, refresh_scaffold, render_consumer_caller,
    render_consumer_module_setup, with_overrides_include, Applied, FileKind, CONSUMER_WORKFLOW_PIN,
};
pub use status::{observed_status_checks, render_status_text, StatusCheck};
pub use upgrade::{upgrade_recovery_message, upgrade_retry_command, UpgradeIntent};
pub use version::{
    local_path_overrides, module_dependency_status, module_rules_dx_version, pin_is_valid,
    pin_rewrite_hint, read_module_dependency, read_version_pin, read_version_pins,
    refresh_stale_pins, refresh_stale_pins_from_env, render_module_dependency,
    version_pin_matches_module, write_version_pin, write_version_pins, DX_VERSION, MODULE_VERSION,
};
pub use watch::{plan_watch, WATCHABLE_COMMANDS};
