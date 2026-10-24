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
pub mod launcher;
pub mod load_graph;
pub mod migrate;
pub mod new;
mod new_locks;
pub mod policy;
pub mod preset_fragment;
pub mod scaffold;
pub mod status;
pub mod upgrade;
pub mod version;
pub mod watch;

pub use defaults::{
    check_quality_baseline, env_bool, env_string, find_config_sources, find_new_configs, is_truthy,
    join_quality_baseline, load_defaults, load_defaults_with_mode, merge_defaults, parse_bool,
    parse_file_text, resolve_bool, resolve_string, resolve_workspace,
    shares_file_with_other_tables, ConfigSources, FileDefaults, BOOL_SPELLINGS,
    CONSUMER_SCHEMA_VERSION, DX_FAIL_ON_ENV, DX_LOCAL_TOML_REL, DX_OUTPUT_ENV, DX_QUIET_ENV,
    DX_TOML_REL, DX_VERBOSE_ENV, DX_WORKSPACE_ENV, FALSEY,
};
pub use error::AdoptError;
pub use git_changes::{
    dedupe_changes, nearest_package_pattern, parse_name_status_nul, ChangeKind, GitChange,
};
pub use launcher::{
    render_launcher_cmd, render_launcher_sh, write_launcher_files, LAUNCHER_CMD_REL,
    LAUNCHER_SH_REL,
};
pub use load_graph::{bzl_impact, BzlImpact};
pub use migrate::{
    migrate_is_major_bump, migrate_is_upgrade, migrate_manifest_name, migrate_manifest_name_full,
    plan_migrate, plan_qualified_migrate, plan_released_migrate, MigratePlan, QUALIFIED_MIGRATIONS,
};
pub use new::{
    apply_new, default_new_name, derive_new_identity, new_is_known_language,
    new_language_identity_rule, new_language_name_list, normalize_new_language, plan_new_files,
    removed_new_language, validate_new_destination, IdentityRule, NewLanguage, NEW_LANGUAGES,
    SUPPORTED_NEW_LANGUAGES,
};
pub use policy::{devcontainer_is_admissible, diagnostics_command_allowed};
pub use preset_fragment::{
    check_preset, owned_collisions_in_content, preset_paths, render_preset_fragment, update_preset,
    PresetError, PRESET_BAZEL_VERSION,
};
pub use scaffold::{
    apply_init, apply_scaffold, editor_disposition, editor_language_supported, plan_init_files,
    plan_init_files_for_host, render_consumer_caller, render_consumer_module_setup,
    render_consumer_own_toolchain_setup, render_consumer_renamed_setup, render_editor_settings,
    scaffold_dest_within_root, validate_init_module, ScaffoldFile, CONSUMER_LLVM_VERSION,
    CONSUMER_RULES_CC_VERSION, CONSUMER_RULES_RUST_VERSION, CONSUMER_RUSTFMT_VERSION,
    CONSUMER_RUST_EDITION, CONSUMER_RUST_VERSION, CONSUMER_WORKFLOW_OWNER, CONSUMER_WORKFLOW_PATH,
    CONSUMER_WORKFLOW_PIN, CONSUMER_WORKFLOW_REPO, DEVCONTAINER_JSON, ENVRC_CONTENT,
};
pub use status::{
    default_status_checks, observed_status_checks, render_status_json, render_status_text,
    StatusCheck,
};
pub use upgrade::{
    plan_qualified_upgrade, plan_released_upgrade, plan_upgrade, upgrade_recovery_message,
    upgrade_refresh_commands, upgrade_restore_command_for, upgrade_retry_command, UpgradeIntent,
    UpgradePlan,
};
pub use version::{
    local_path_overrides, module_dependency_status, parse_consumer_setup, parse_module_dependency, parse_pin_record,
    pin_fix_hint, read_module_dependency, read_pin_record, read_version_pin, record_pin_operation,
    render_module_dependency, render_pin_record, rollback_re_pins_previous,
    version_pin_matches_module, write_version_pin, ConsumerSetup, ModuleDependency, PinRecord,
    DX_VERSION, MODULE_VERSION, PREVIOUS_VERSION, VERSION_HISTORY_REL,
};
pub use watch::{
    coalesce_watch_paths, plan_watch, should_watch_path, watch_for_change, WATCHABLE_COMMANDS,
    WATCH_DEBOUNCE_MS,
};
