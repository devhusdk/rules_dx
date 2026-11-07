//! Notice codes shared across commands.

pub(crate) const CODE_UPDATE_WIDENED: &str = "update_widened";
pub(crate) const CODE_UPDATE_NO_CHANGE: &str = "update_no_change";
pub(crate) const CODE_ADD_DECLARED: &str = "add_declared";
pub(crate) const CODE_UPDATE_SET_SUCCESS: &str = "update_set_success";
pub(crate) const CODE_UPDATE_SET_CURRENT: &str = "update_set_current";
pub(crate) const CODE_UPDATE_SET_PINNED: &str = "update_set_pinned";
pub(crate) const CODE_UPDATE_SET_BLOCKED: &str = "update_set_blocked";
pub(crate) const CODE_BUILD_EVENTS_INCOMPLETE: &str = "build_events_incomplete";
pub(crate) const CODE_CHANGES_PROPOSED: &str = "changes_proposed";
pub(crate) const CODE_TESTS_FILTERED: &str = "tests_filtered";
pub(crate) const CODE_TARGETS_SKIPPED: &str = "targets_skipped";
pub(crate) const CODE_INIT_PLAN_PARTIAL: &str = "init_plan_partial";
pub(crate) const CODE_INIT_ABSENT: &str = "init_absent";
pub(crate) const CODE_IDE_REFRESH_FAILED: &str = "ide_refresh_failed";
pub(crate) const CODE_PRUNE_FAILED: &str = "prune_failed";
pub(crate) const CODE_SYNC_FAILED: &str = "sync_failed";
pub(crate) const CODE_UMBRELLA_PHASE_SKIPPED: &str = "umbrella_phase_skipped";
pub(crate) const CODE_UMBRELLA_PHASE_FINDINGS: &str = "umbrella_phase_findings";
pub(crate) const CODE_COMMIT_NOT_CONVENTIONAL: &str = "commit_not_conventional";
pub(crate) const CODE_AUDIT_SECURITY_CLEAN: &str = "audit_security_clean";
pub(crate) const CODE_AUDIT_LICENSE_CLEAN: &str = "audit_license_clean";
pub(crate) const CODE_AUDIT_ACCEPTED: &str = "audit_accepted";

/// The info notice code for an audit family that came back clean.
pub(crate) fn audit_clean_code(family: dx_audit::AuditFamily) -> &'static str {
    match family {
        dx_audit::AuditFamily::Security => CODE_AUDIT_SECURITY_CLEAN,
        dx_audit::AuditFamily::License => CODE_AUDIT_LICENSE_CLEAN,
    }
}
