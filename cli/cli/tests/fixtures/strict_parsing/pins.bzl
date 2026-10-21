"""Strict clap parsing fixture for the dx CLI surface."""

STRICT_UNKNOWN_REJECTED = [
    "--jobs=4",
    "-q",
    "--dry-run=yes",
    "--quiet=x",
    "--check=x",
    "--bazel=yes",
    "--force",
    "-",
    "--bogus",
    "--bogus=1",
]
STRICT_UNKNOWN_ECHO_WHOLE_TOKEN = True

STRICT_MISSING_REJECTED = [
    "--output",
    "--workspace",
    "--min-coverage",
    "--port",
    "--host",
    "--color",
    "--from",
]
STRICT_HYPHEN_VALUES_NOT_CONSUMED = True
STRICT_MISSING_NAMES_BARE_FLAG = True

STRICT_BAD_OUTPUT_REJECTED = ["yaml"]
STRICT_BAD_FAIL_ON_REJECTED = ["never"]
STRICT_BAD_COLOR_REJECTED = ["bright"]
STRICT_BAD_REPORT_REJECTED = ["sarif", "=out.sarif", "sarif=", ""]
STRICT_BAD_MIN_COVERAGE_REJECTED = ["eighty", "101"]
STRICT_BAD_PORT_REJECTED = ["0"]

STRICT_COMMAND_SUGGESTION = {"lintt": "lint"}
STRICT_OPTION_SUGGESTION = {"--ouptut=json": "--output"}

STRICT_UNSUPPORTED_REJECTED = [
    "build --serve",
    "build --port=8080",
    "build --host=example.test",
    "build --open",
    "lint --min-coverage",
    "build --check",
    "lint --bazel",
]

STRICT_BAZEL_FORWARDS_VERBATIM = True
STRICT_BAZEL_PREFIX_OPTIONS_REJECTED = True

STRICT_HELP_VERB_REDIRECT = True
STRICT_HELP_FLAG_ONLY = ["--help", "-h"]

STRICT_COMMAND_COUNT = 32
STRICT_GENERATED_HELP_FROM_GRAMMAR = True

REJECTED_SILENT_IGNORE = "silent ignore rejected"
REJECTED_PREFIX_INFERENCE = "prefix inference rejected"
REJECTED_HYPHEN_CONSUMPTION = "hyphen-value consumption rejected"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
