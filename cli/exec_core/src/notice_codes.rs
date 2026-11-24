pub const CODE_UPDATE_WIDENED: &str = "update_widened";
pub const CODE_UPDATE_NO_CHANGE: &str = "update_no_change";
pub const CODE_ADD_DECLARED: &str = "add_declared";
pub const CODE_UPDATE_SET_SUCCESS: &str = "update_set_success";
pub const CODE_UPDATE_SET_CURRENT: &str = "update_set_current";
pub const CODE_UPDATE_SET_PINNED: &str = "update_set_pinned";
pub const CODE_UPDATE_SET_BLOCKED: &str = "update_set_blocked";
pub const CODE_BUILD_EVENTS_INCOMPLETE: &str = "build_events_incomplete";
pub const CODE_CHANGES_PROPOSED: &str = "changes_proposed";
pub const CODE_TOOL_SKIPPED: &str = "tool_skipped";
pub const CODE_TESTS_FILTERED: &str = "tests_filtered";
pub const CODE_TARGETS_SKIPPED: &str = "targets_skipped";
pub const CODE_WORKSPACE_WITHOUT_TESTS: &str = "workspace_without_tests";
pub const CODE_INIT_PLAN_PARTIAL: &str = "init_plan_partial";
pub const CODE_INIT_ABSENT: &str = "init_absent";
pub const CODE_IDE_REFRESH_FAILED: &str = "ide_refresh_failed";
pub const CODE_PRUNE_FAILED: &str = "prune_failed";
pub const CODE_SYNC_FAILED: &str = "sync_failed";
pub const CODE_UMBRELLA_PHASE_SKIPPED: &str = "umbrella_phase_skipped";
pub const CODE_UMBRELLA_PHASE_FINDINGS: &str = "umbrella_phase_findings";
pub const CODE_COMMIT_NOT_CONVENTIONAL: &str = "commit_not_conventional";
pub const CODE_AUDIT_SECURITY_CLEAN: &str = "audit_security_clean";
pub const CODE_AUDIT_LICENSE_CLEAN: &str = "audit_license_clean";
pub const CODE_AUDIT_ACCEPTED: &str = "audit_accepted";
pub const CODE_SECURITY_VERIFIED: &str = "security_verified";
pub const CODE_DX_CACHED: &str = "dx_cached";
pub const CODE_DX_NOT_CACHED: &str = "dx_not_cached";

pub fn audit_clean_code(family: dx_audit::AuditFamily) -> &'static str {
    match family {
        dx_audit::AuditFamily::Security => CODE_AUDIT_SECURITY_CLEAN,
        dx_audit::AuditFamily::License => CODE_AUDIT_LICENSE_CLEAN,
    }
}
