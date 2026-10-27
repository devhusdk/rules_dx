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
pub mod cuda_stanza;
pub mod defaults;
pub mod dependency_sets;
pub mod error;
pub mod git_changes;
pub mod launcher;
pub mod load_graph;
pub mod migrate;
pub mod new;
mod new_locks;
pub mod overrides;
pub mod preset_fragment;
pub mod scaffold;
pub mod status;
pub mod upgrade;
pub mod version;
pub mod watch;

pub use defaults::join_quality_baseline;
pub use error::AdoptError;
pub use git_changes::{
    dedupe_changes, nearest_package_pattern, parse_name_status_nul, ChangeKind, GitChange,
    DEFAULT_BRANCH_CANDIDATES,
};
pub use launcher::{
    launcher_version, plan_launcher_files, refresh_launcher_files, release_pins,
    render_launcher_cmd, render_launcher_sh, stale_launchers, write_launcher_files, LauncherPins,
    LAUNCHER_CMD_REL, LAUNCHER_SH_REL, RELEASE_ASSETS,
};
pub use load_graph::{bzl_impact, module_inputs, repo_impact, BzlImpact};
pub use migrate::plan_migrate;
pub use new::{
    apply_new, default_new_name, new_is_known_language, new_language_name_list, plan_new_files,
    removed_new_language, validate_new_destination, NEW_LANGUAGES, SUPPORTED_NEW_LANGUAGES,
};
pub use overrides::{
    apply_overrides, check_overrides, preflight_overrides, OverridesState, OVERRIDES_INCLUDE,
};
pub use scaffold::{
    apply_init, bazelignore_covers_dx, plan_init_files, preflight_destinations,
    render_consumer_module_setup, Applied,
};
pub use status::{observed_status_checks, render_status_text, StatusCheck};
pub use upgrade::{plan_upgrade, UpgradeIntent};
pub use version::{
    local_path_overrides, module_dependency_status, pin_fix_hint, pin_is_valid, pin_rewrite_hint,
    read_module_dependency, read_pin_record, read_version_pin, record_pin_operation,
    render_module_dependency, version_pin_matches_module, write_version_pin, DX_VERSION,
    MODULE_VERSION, VERSION_HISTORY_REL,
};
pub use watch::{plan_watch, WATCHABLE_COMMANDS};
