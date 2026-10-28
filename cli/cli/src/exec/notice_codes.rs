//! Notice codes, and which command emits each one.

use dx_plan::args::Command;

use super::common::{
    CODE_CLEAN_PLANNED, CODE_CLEAN_PRESERVED_LEASED, CODE_CLEAN_PRESERVED_UNOBSERVED,
    CODE_CLEAN_PRUNED, CODE_DISK_LOW,
};

pub(crate) const CODE_BUMP_WIDENED: &str = "bump_widened";
pub(crate) const CODE_BUMP_NO_CHANGE: &str = "bump_no_change";
pub(crate) const CODE_ADD_DECLARED: &str = "add_declared";
pub(crate) const CODE_UPDATE_SET_SUCCESS: &str = "update_set_success";
pub(crate) const CODE_UPDATE_SET_CURRENT: &str = "update_set_current";
pub(crate) const CODE_UPDATE_SET_PINNED: &str = "update_set_pinned";
pub(crate) const CODE_UPDATE_SET_BLOCKED: &str = "update_set_blocked";
pub(crate) const CODE_BUILD_EVENTS_INCOMPLETE: &str = "build_events_incomplete";
pub(crate) const CODE_TESTS_FILTERED: &str = "tests_filtered";
pub(crate) const CODE_MANAGED_DRIFT: &str = "managed_drift";
pub(crate) const CODE_UMBRELLA_PHASE_SKIPPED: &str = "umbrella_phase_skipped";
pub(crate) const CODE_UMBRELLA_PHASE_FINDINGS: &str = "umbrella_phase_findings";
pub(crate) const CODE_AUDIT_SECURITY_CLEAN: &str = "audit_security_clean";
pub(crate) const CODE_AUDIT_LICENSE_CLEAN: &str = "audit_license_clean";

/// The info notice code for an audit family that came back clean.
pub(crate) fn audit_clean_code(family: dx_audit::AuditFamily) -> &'static str {
    match family {
        dx_audit::AuditFamily::Security => CODE_AUDIT_SECURITY_CLEAN,
        dx_audit::AuditFamily::License => CODE_AUDIT_LICENSE_CLEAN,
    }
}

/// Codes any command can emit, named in the commands README.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SHARED_NOTICE_CODES: &[&str] = &[
    CODE_DISK_LOW,
    dx_plan::slots::DISK_LOW_CODE,
    dx_plan::slots::WAITING_FOR_BUILD_SLOT_CODE,
    dx_plan::slots::BUILD_SLOT_SKIPPED_CODE,
    dx_plan::slots::BUILD_SLOTS_DISABLED_CODE,
    dx_process::groups::CODE_ORPHAN_PROCESS_GROUP_STOPPED,
];

const WORKFLOW_CODES: &[&str] = &[CODE_BUILD_EVENTS_INCOMPLETE];
const TEST_CODES: &[&str] = &[CODE_BUILD_EVENTS_INCOMPLETE, CODE_TESTS_FILTERED];
const AUDIT_CODES: &[&str] = &[CODE_AUDIT_SECURITY_CLEAN, CODE_AUDIT_LICENSE_CLEAN];
const UMBRELLA_CODES: &[&str] = &[CODE_UMBRELLA_PHASE_SKIPPED, CODE_UMBRELLA_PHASE_FINDINGS];

/// The notice codes `command` emits itself; umbrella phases list theirs under the phase command.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn notice_codes(command: Command) -> &'static [&'static str] {
    match command {
        Command::Security | Command::License => AUDIT_CODES,
        Command::Lint | Command::Typecheck | Command::Format => WORKFLOW_CODES,
        Command::Build | Command::Run | Command::Deploy => WORKFLOW_CODES,
        Command::Test | Command::Coverage => TEST_CODES,
        Command::Generate => &[dx_plan::generate::IGNORED_IMPORT_CODE],
        Command::Check | Command::Fix => UMBRELLA_CODES,
        Command::Clean => &[
            CODE_CLEAN_PLANNED,
            CODE_CLEAN_PRUNED,
            CODE_CLEAN_PRESERVED_LEASED,
            CODE_CLEAN_PRESERVED_UNOBSERVED,
        ],
        Command::Update => &[
            CODE_UPDATE_SET_SUCCESS,
            CODE_UPDATE_SET_CURRENT,
            CODE_UPDATE_SET_PINNED,
            CODE_UPDATE_SET_BLOCKED,
            dx_update::recovery::RECOVERY_CODE,
        ],
        Command::Bump => &[
            CODE_BUMP_WIDENED,
            CODE_BUMP_NO_CHANGE,
            CODE_UPDATE_SET_SUCCESS,
        ],
        Command::Add => &[CODE_ADD_DECLARED, CODE_UPDATE_SET_SUCCESS],
        Command::Codegen | Command::Env => &[CODE_MANAGED_DRIFT],
        Command::Worktree => &[],
        Command::Migrate
        | Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Status
        | Command::Version
        | Command::Watch
        | Command::Completion
        | Command::Docs
        | Command::ListTests
        | Command::Explain
        | Command::Install
        | Command::Rollback => &[],
    }
}
