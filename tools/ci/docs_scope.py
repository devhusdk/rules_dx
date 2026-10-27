"""Validates the docs scope input and runs dx lint --check over it.

The docs workflow passes the docs_scope input through the DX_DOCS_SCOPE
environment variable and runs this with bazel run. The value is
space-separated target patterns. Every pattern is checked, then bazel starts
with the patterns as separate arguments and no shell.
"""

import os
import re
import shutil
import subprocess
import sys

SCOPE_ENV = "DX_DOCS_SCOPE"
CONFIG_ENVS = ("BAZEL_CONFIG", "BB_ARGS")

PATTERN = re.compile(
    r"(?:@@?[A-Za-z0-9_.~+-]*)?//[A-Za-z0-9_.+@=,~/-]*(?::[A-Za-z0-9_.+@=,~/-]+)?"
)


def fail(message):
    """Reports one diagnostic and returns a failing exit status."""
    print(f"dx-ci: {message}", file=sys.stderr)
    return 1


def parse_scope(text):
    """Returns the ordered scope patterns, or None after a diagnostic."""
    if text.strip(" ") == "":
        fail(f"{SCOPE_ENV} is empty. Write patterns such as //... separated by spaces.")
        return None
    patterns = [part for part in text.split(" ") if part != ""]
    for pattern in patterns:
        if PATTERN.fullmatch(pattern) is None:
            fail(
                f"{SCOPE_ENV} holds {pattern!r}, which is not a target pattern. "
                "Write patterns such as //... or //pkg:target separated by spaces."
            )
            return None
    return patterns


def main():
    """Runs dx lint --check over the validated scope and returns its exit status."""
    os.chdir(os.environ.get("BUILD_WORKING_DIRECTORY", "."))
    patterns = parse_scope(os.environ.get(SCOPE_ENV, ""))
    if patterns is None:
        return 1
    config = []
    for name in CONFIG_ENVS:
        config.extend(os.environ.get(name, "").split())
    bazel = shutil.which("bazel") or "bazel"
    argv = [bazel, "run", *config, "@rules_dx//:dx", "--", "lint", "--check"]
    argv += [*patterns, "--", *config]
    try:
        return subprocess.run(argv, check=False).returncode
    except OSError as error:
        return fail(f"bazel cannot start: {error}")


if __name__ == "__main__":
    sys.exit(main())
