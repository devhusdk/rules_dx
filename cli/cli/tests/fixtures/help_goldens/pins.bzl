"""Help-output goldens fixture for the dx CLI surface."""

HELP_TOP_GOLDEN = "top_help.golden"
HELP_REPRESENTATIVE_GOLDENS = [
    "lint_help.golden",
    "build_help.golden",
    "clean_help.golden",
    "bazel_help.golden",
    "docs_help.golden",
]

HELP_TOP_INVARIANTS = [
    "dx - Run Bazel workflows",
    "Commands:",
    "--workspace",
    "--output",
    "--fail-on",
    "Exit codes",
]

HELP_COMMAND_INVARIANTS = [
    "Usage:",
    "Scopes:",
    "Exit codes:",
    "Output:",
    "Per-command flags:",
]

HELP_FLAG_ONLY = ["--help", "-h"]
HELP_VERB_REDIRECT = True
HELP_BAZEL_VERBATIM = True

HELP_COMMAND_COUNT = 33
HELP_GENERATED_FROM_GRAMMAR = True
