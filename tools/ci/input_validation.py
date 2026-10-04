"""Validates the consumer workflow inputs before any matrix job starts.

The consumer workflow passes each workflow_call input through one environment
variable and runs this file from standard input. A rejected input prints one
diagnostic naming the accepted values, and an accepted input is written to
GITHUB_OUTPUT so the matrix, runner, threshold and disabled-check wiring all
read one validated source.
"""

import json
import os
import re
import sys

PLATFORMS_ENV = "DX_PLATFORMS"
DISABLED_ENV = "DX_DISABLED_CHECKS"
SCHEDULING_ENV = "DX_SCHEDULING_MODE"
SCANNING_ENV = "DX_CODE_SCANNING_OPT_IN"
MIN_COVERAGE_ENV = "DX_MIN_COVERAGE"
OUTPUT_ENV = "GITHUB_OUTPUT"

PLATFORMS = {
    "linux_x86_64": "ubuntu-latest",
    "linux_arm64": "ubuntu-24.04-arm",
    "macos_arm64": "macos-14",
    "windows_x86_64": "windows-latest",
    "windows_arm64": "windows-11-arm",
}
DEFAULT_PLATFORMS = ["linux_x86_64"]
CHECKS = (
    "lint",
    "typecheck",
    "format",
    "generate",
    "security-audit",
    "license-audit",
    "test",
    "build",
    "coverage",
)
SCHEDULING = "parallel"
WHOLE_PERCENT = re.compile(r"[0-9]+")


def fail(message):
    """Reports one diagnostic and returns a failing exit status."""
    print(f"dx-ci: {message}", file=sys.stderr)
    return 1


def accepted(values):
    """Returns the accepted values as one comma-separated list."""
    return ", ".join(values)


def json_type(value):
    """Returns the JSON type name of one parsed value."""
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    return "object"


def validate_platforms(text):
    """Returns the platform labels to run, or None after a diagnostic."""
    if text == "":
        return DEFAULT_PLATFORMS
    try:
        parsed = json.loads(text)
    except ValueError as error:
        fail(f"{PLATFORMS_ENV} is not a JSON array: {error}. {platform_hint()}")
        return None
    if not isinstance(parsed, list):
        fail(
            f"{PLATFORMS_ENV} is a JSON {json_type(parsed)}, not an array. {platform_hint()}"
        )
        return None
    if parsed == []:
        fail(f"{PLATFORMS_ENV} is an empty array. {platform_hint()}")
        return None
    for item in parsed:
        if not isinstance(item, str):
            fail(
                f"{PLATFORMS_ENV} holds a JSON {json_type(item)}, not a platform label "
                f"string. {platform_hint()}"
            )
            return None
    for item in parsed:
        if item not in PLATFORMS:
            fail(
                f"{PLATFORMS_ENV} names unknown platform {item!r}. "
                f"Accepted: {accepted(PLATFORMS)}."
            )
            return None
    seen = []
    for item in parsed:
        if item in seen:
            fail(f"{PLATFORMS_ENV} lists {item!r} twice. List every platform once.")
            return None
        seen.append(item)
    return parsed


def platform_hint():
    """Returns the one sentence naming the accepted platforms and the default."""
    return f"Accepted: {accepted(PLATFORMS)}. Omit it to run {DEFAULT_PLATFORMS[0]}."


def validate_disabled(text):
    """Returns the normalized disabled check IDs, or None after a diagnostic."""
    if text.strip() == "":
        return []
    ids = []
    for part in text.split(","):
        name = part.strip()
        if name == "":
            fail(
                f"{DISABLED_ENV} holds an empty check ID in {text!r}. "
                f"Accepted: {accepted(CHECKS)}."
            )
            return None
        if name not in CHECKS:
            fail(
                f"{DISABLED_ENV} names unknown check {name!r}. Accepted: {accepted(CHECKS)}."
            )
            return None
        if name in ids:
            fail(f"{DISABLED_ENV} lists {name!r} twice. List every check ID once.")
            return None
        ids.append(name)
    return ids


def validate_scheduling(text):
    """Returns the accepted scheduling mode, or None after a diagnostic."""
    if text != SCHEDULING:
        fail(
            f"{SCHEDULING_ENV}={text!r} is not implemented. Checks always run in "
            f"parallel, so pass {SCHEDULING!r}."
        )
        return None
    return text


def validate_scanning(text):
    """Returns the accepted code scanning opt-in, or None after a diagnostic."""
    if text == "false":
        return text
    if text == "true":
        fail(
            f"{SCANNING_ENV}=true is not implemented. No SARIF upload step ships "
            "in this workflow, so leave it false."
        )
        return None
    fail(f"{SCANNING_ENV}={text!r} is not true or false.")
    return None


def validate_min_coverage(text):
    """Returns the coverage threshold to enforce, or None after a diagnostic."""
    if text == "":
        return ""
    if WHOLE_PERCENT.fullmatch(text) is None or not 0 <= int(text) <= 100:
        fail(
            f"{MIN_COVERAGE_ENV}={text!r} is not a whole percent from 0 to 100. "
            "Write a whole number such as 80, or omit it to collect without a "
            "threshold."
        )
        return None
    return text


def compact(value):
    """Returns value as JSON without spaces."""
    return json.dumps(value, separators=(",", ":"))


def write_outputs(outputs):
    """Appends the validated values to the job output file."""
    path = os.environ.get(OUTPUT_ENV, "")
    if path == "":
        fail(f"{OUTPUT_ENV} is unset, so the validated inputs cannot reach the jobs.")
        return False
    with open(path, "a", encoding="utf-8") as handle:
        for key in sorted(outputs):
            print(f"{key}={outputs[key]}", file=handle)
    return True


def main():
    """Validates every input, then writes the validated values as job outputs."""
    accepted_values = {
        PLATFORMS_ENV: validate_platforms(os.environ.get(PLATFORMS_ENV, "")),
        DISABLED_ENV: validate_disabled(os.environ.get(DISABLED_ENV, "")),
        SCHEDULING_ENV: validate_scheduling(os.environ.get(SCHEDULING_ENV, SCHEDULING)),
        SCANNING_ENV: validate_scanning(os.environ.get(SCANNING_ENV, "false")),
        MIN_COVERAGE_ENV: validate_min_coverage(os.environ.get(MIN_COVERAGE_ENV, "")),
    }
    if any(value is None for value in accepted_values.values()):
        return 1
    outputs = {
        "platforms": compact(accepted_values[PLATFORMS_ENV]),
        "runners": compact(PLATFORMS),
        "disabled": ",".join(accepted_values[DISABLED_ENV]),
        "min_coverage": accepted_values[MIN_COVERAGE_ENV],
    }
    if not write_outputs(outputs):
        return 1
    print(f"{PLATFORMS_ENV} {outputs['platforms']}")
    print(f"{DISABLED_ENV} {outputs['disabled'] or '(none)'}")
    print(f"{SCHEDULING_ENV} {accepted_values[SCHEDULING_ENV]}")
    print(f"{SCANNING_ENV} {accepted_values[SCANNING_ENV]}")
    print(f"{MIN_COVERAGE_ENV} {outputs['min_coverage'] or '(none)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
