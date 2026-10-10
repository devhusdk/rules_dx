"""CLI execution/reporting gaps fixture."""

WATCHABLE_COMMANDS = [
    "build",
    "test",
    "run",
    "lint",
    "typecheck",
    "format",
    "check",
    "fix",
]
WATCHABLE_COUNT = 8

NOT_WATCHABLE_COMMANDS = [
    "bazel",
    "bump",
    "capabilities",
    "clean",
    "codegen",
    "completion",
    "coverage",
    "deps",
    "deploy",
    "docs",
    "env",
    "generate",
    "hooks",
    "init",
    "license",
    "migrate",
    "new",
    "owners",
    "rerun",
    "security",
    "setup",
    "status",
    "tests",
    "update",
    "upgrade",
    "verify",
    "version",
    "watch",
    "why",
]
NOT_WATCHABLE_COUNT = 29

WATCH_REFUSES_CI = True
WATCH_DEBOUNCE_MS = 200
WATCH_REUSES_WRAPPED_VERBATIM = True

STARTUP_OPTIONS_REJECTED = [
    "bazelrc",
    "home_rc",
    "nohome_rc",
    "system_rc",
    "nosystem_rc",
    "output_base",
    "output_user_root",
    "host_jvm_args",
    "server_jvm_out",
]
STARTUP_DISPOSITION = "wont-fix"

TEST_BINARY_ARGS_REJECTED = ["test_arg"]
TEST_BINARY_DISPOSITION = "wont-fix"

BAZEL_FORWARDS_UNCHANGED = True
RUN_FORWARDS_TO_APP = True
OTHER_WORKFLOW_FORWARDS_TO_BAZEL_COMMAND_OPTIONS = True

REPORT_LINT = ["sarif"]
REPORT_TYPECHECK = ["sarif"]
REPORT_TEST = ["junit"]
REPORT_COVERAGE = ["lcov"]
REPORT_AUDIT = ["sarif", "spdx"]
REPORT_CHECK = ["sarif"]
REPORT_FIX = ["sarif"]
REPORT_NONE = [
    "build",
    "run",
    "deploy",
    "docs",
    "format",
    "generate",
    "clean",
    "update",
    "bump",
    "migrate",
    "codegen",
    "env",
    "setup",
    "init",
    "new",
    "hooks",
    "status",
    "version",
    "watch",
    "owners",
    "deps",
    "why",
    "upgrade",
    "completion",
    "bazel",
    "capabilities",
    "verify",
    "rerun",
    "tests",
]
REPORT_DISPOSITION = "wont-fix"

PARALLEL_CHECK_FIX = "sequential format then lint then typecheck then generate"
PARALLEL_WATCH = "one iteration at a time"
PARALLEL_UPDATE = "sequential per-set with continuation, no parallel"
PARALLEL_RUN = "sequential multirun in scope order"
PARALLEL_DISPOSITION = "wont-fix"

REJECTED_SILENT_SUBSTITUTION = "silent substitution across commands rejected"
REJECTED_WATCH_DAEMON = "watch daemon rejected"
REJECTED_PARALLEL_UMBRELLA = "parallel umbrella rejected"
REJECTED_INFERRED_FORWARDS = "inferred option-class forwarding rejected"
REJECTED_INVENTED_REPORTS = "invented standard reports rejected"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
