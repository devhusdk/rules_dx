#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod defaults;
pub mod error;
pub mod hooks;
pub mod inspect;
pub mod migrate;
pub mod new;
pub mod policy;
pub mod preset_fragment;
pub mod scaffold;
pub mod status;
pub mod upgrade;
pub mod version;
pub mod watch;

pub use defaults::{
    env_bool, env_string, find_config, is_truthy, load_defaults, parse_file_text, resolve_bool,
    resolve_string, resolve_workspace, FileDefaults, CONFIG_REL, CONFIG_TOML_REL, DX_DRY_RUN_ENV,
    DX_FAIL_ON_ENV, DX_OUTPUT_ENV, DX_QUIET_ENV, DX_VERBOSE_ENV, DX_WORKSPACE_ENV,
};
pub use error::AdoptError;
pub use hooks::{
    checks_for_trigger, default_hooks_config, hook_check_timed_out, hook_git_is_hermetic,
    hook_git_path_is_hermetic, hook_shim_overwrite_allowed, hook_status_shows_merged,
    hook_trigger_pipe, hook_verb_pipe, install_hooks, is_hook_trigger, load_hook_timings,
    load_hooks_config, render_hook_shim, render_hook_timings, render_hooks_status,
    render_hooks_status_merged, render_local_overlay, uninstall_hooks, HookTimings, HooksConfig,
    HOOK_BASELINE_REL, HOOK_BUDGET_SECS, HOOK_GIT_ENV_VAR, HOOK_MANAGED_MARKER, HOOK_OVERLAY_REL,
    HOOK_TIMINGS_REL, HOOK_TRIGGERS, HOOK_VERBS, LOCAL_OVERLAY_COMMENT,
};
pub use inspect::{inspect_scope_allowed, plan_inspect, plan_somepath, InspectPlan};
pub use migrate::{
    migrate_is_major_bump, migrate_is_upgrade, migrate_manifest_name, migrate_manifest_name_full,
    plan_migrate, MigratePlan,
};
pub use new::{
    apply_new, default_new_name, new_is_known_language, new_language_name_list,
    normalize_new_language, plan_new_files, NEW_LANGUAGE_ALIASES, SUPPORTED_NEW_LANGUAGES,
};
pub use policy::{devcontainer_is_admissible, diagnostics_command_allowed};
pub use preset_fragment::{
    check_preset, owned_collisions_in_content, preset_paths, render_preset_fragment, update_preset,
    PresetError, PRESET_BAZEL_VERSION,
};
pub use scaffold::{
    absent_only_write_allowed, apply_init, editor_disposition, editor_language_supported,
    init_must_refuse, plan_init_files, ScaffoldFile, DEVCONTAINER_JSON, ENVRC_CONTENT,
};
pub use status::{default_status_checks, render_status_json, render_status_text, StatusCheck};
pub use upgrade::{
    plan_upgrade, upgrade_recovery_message, upgrade_restore_command, upgrade_retry_command,
    UpgradePlan,
};
pub use version::{
    read_version_pin, rollback_re_pins_previous, version_pin_matches_module, write_version_pin,
    DX_VERSION, MODULE_VERSION, PREVIOUS_VERSION,
};
pub use watch::{
    coalesce_watch_paths, plan_watch, should_watch_path, watch_for_change, watch_iteration_accepts,
    WATCHABLE_COMMANDS, WATCH_DEBOUNCE_MS,
};
