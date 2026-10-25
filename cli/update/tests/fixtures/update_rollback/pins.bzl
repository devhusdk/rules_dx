"""Atomic dx update rollback-plan pins."""

ATOMICITY_BOUNDARY = "per-set commit"
REPO_WIDE_ATOMIC = False
AUTOMATIC_ROLLBACK = False

RETRY_COMMAND = "dx update --apply <failed/blocked sets>"
CHECK_RETRY_COMMAND = "dx update <failed sets>"
RESTORE_COMMAND = "git checkout -- <kept locks>"
RETRY_IDEMPOTENT = True
DX_RUNS_GIT_RESTORE = False

RECOVERY_CODE = "update_recovery"
RECOVERY_TEXT_PREFIX = "dx: update_recovery:"
RECOVERY_JSON_CODE = "update_recovery"
NO_SILENT_PARTIAL_SUCCESS = True

INTERRUPTED_KEEPS_PRECEDING = True
INTERRUPTED_UNATTEMPTED_EMITS_NOTHING = True
INTERRUPTED_NO_COMMAND_FINISHED_ON_SIGNAL = True
INTERRUPTED_RETRY = "dx update --apply <unattempted sets>"

REJECTED_AUTOMATIC_ROLLBACK = "automatic rollback rejected (no backend committed-change manifest)"
REJECTED_GIT_SCAN = "git scan rejected"
REJECTED_PRIVATE_ROLLBACK = "private lockfile surgery rejected"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
