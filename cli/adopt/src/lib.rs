#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod defaults;
pub mod dependency_sets;
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
pub mod verify_sets;
pub mod version;
pub mod watch;

pub use defaults::{
    env_bool, env_string, find_config, find_config_sources, find_new_configs, is_truthy,
    load_defaults, load_defaults_with_mode, merge_defaults, parse_bool, parse_file_text,
    resolve_bool, resolve_string, resolve_workspace, shares_file_with_other_tables, ConfigSources,
    FileDefaults, BOOL_SPELLINGS, CONFIG_REL, CONFIG_TOML_REL, CONSUMER_SCHEMA_VERSION,
    DX_DRY_RUN_ENV, DX_FAIL_ON_ENV, DX_LOCAL_TOML_REL, DX_OUTPUT_ENV, DX_QUIET_ENV, DX_TOML_REL,
    DX_VERBOSE_ENV, DX_WORKSPACE_ENV, FALSEY,
};
pub use error::AdoptError;
pub use hooks::{
    change_source_name, checks_for_trigger, dedupe_changes, default_hooks_config,
    display_hooks_path, hook_check_timed_out, hook_git_is_hermetic, hook_git_path_is_hermetic,
    hook_status_shows_merged, hook_trigger_pipe, hook_verb_pipe, install_hooks, install_hooks_into,
    is_hook_trigger, is_zero_sha, join_hooks_path, load_hook_timings, load_hooks_config,
    nearest_package_pattern, parse_git_path_output, parse_name_status_nul, parse_push_refs,
    push_diff_base, push_ref_is_deletion, render_hook_shim, render_hook_timings,
    render_hooks_status, render_hooks_status_merged, render_local_overlay, render_selection_line,
    uninstall_hooks, uninstall_hooks_from, ChangeKind, ChangeSource, GitChange, HookTimings,
    HooksConfig, PushRef, EMPTY_TREE_SHA, HOOK_BASELINE_REL, HOOK_BUDGET_SECS, HOOK_GIT_ENV_VAR,
    HOOK_MANAGED_MARKER, HOOK_OVERLAY_REL, HOOK_TIMINGS_REL, HOOK_TRIGGERS, HOOK_VERBS,
    LOCAL_OVERLAY_COMMENT, ZERO_SHA,
};
pub use inspect::{inspect_scope_allowed, plan_inspect, plan_somepath, InspectPlan};
pub use migrate::{
    migrate_is_major_bump, migrate_is_upgrade, migrate_manifest_name, migrate_manifest_name_full,
    plan_migrate, plan_qualified_migrate, plan_released_migrate, MigratePlan, QUALIFIED_MIGRATIONS,
};
pub use new::{
    apply_new, default_new_name, derive_new_identity, new_is_known_language,
    new_language_name_list, normalize_new_language, plan_new_files, validate_new_destination,
    NEW_LANGUAGE_ALIASES, SUPPORTED_NEW_LANGUAGES,
};
pub use policy::{devcontainer_is_admissible, diagnostics_command_allowed};
pub use preset_fragment::{
    check_preset, owned_collisions_in_content, preset_paths, render_preset_fragment, update_preset,
    PresetError, PRESET_BAZEL_VERSION,
};
pub use scaffold::{
    apply_init, editor_disposition, editor_language_supported, plan_init_files,
    scaffold_dest_within_root, validate_init_module, ScaffoldFile, DEVCONTAINER_JSON,
    ENVRC_CONTENT,
};
pub use status::{default_status_checks, render_status_json, render_status_text, StatusCheck};
pub use upgrade::{
    plan_qualified_upgrade, plan_released_upgrade, plan_upgrade, upgrade_recovery_message,
    upgrade_restore_command_for, upgrade_retry_command, UpgradePlan,
};
pub use verify_sets::{
    load_verify_set, ResolvedSet, ResolvedStep, VerifySetError, VERIFY_SCHEMA_VERSION,
    VERIFY_STEP_COMMANDS, VERIFY_TOML_REL,
};
pub use version::{
    parse_module_dependency, parse_pin_record, read_module_dependency, read_pin_record,
    read_version_pin, record_pin_operation, render_module_dependency, render_pin_record,
    rollback_re_pins_previous, version_pin_matches_module, write_version_pin, ModuleDependency,
    PinRecord, DX_VERSION, MODULE_VERSION, PREVIOUS_VERSION, VERSION_HISTORY_REL,
};
pub use watch::{
    coalesce_watch_paths, plan_watch, should_watch_path, watch_for_change, WATCHABLE_COMMANDS,
    WATCH_DEBOUNCE_MS,
};
