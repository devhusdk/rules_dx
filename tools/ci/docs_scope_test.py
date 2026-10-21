"""Process-boundary tests for the docs workflow scope validator."""

import os
import pathlib
import stat
import subprocess
import sys

WORKFLOW = ".github/workflows/reusable-docs.yml"
SCRIPT = "tools/ci/docs_scope.py"
OPENING = "python3 - <<'DX_DOCS_SCOPE_SCRIPT'"
MARKER = "DX_DOCS_SCOPE_SCRIPT"
PROBE = "DX_SCOPE_EXECUTED"
RECORDER = """#!/bin/sh
for arg in "$@"; do printf '%s\\n' "$arg"; done > "$DX_ARGV_LOG"
"""


def workspace_root():
    """Returns the runfiles root this test reads its own tree from."""
    return pathlib.Path(__file__).resolve().parents[2]


ROOT = workspace_root()


def embedded_script():
    """Returns the scope script the workflow runs, dedented to its own file text."""
    lines = (ROOT / WORKFLOW).read_text(encoding="utf-8").splitlines()
    openings = [i for i, line in enumerate(lines) if line.strip() == OPENING]
    assert len(openings) == 1, f"{WORKFLOW} runs {len(openings)} scope scripts, want 1"
    base = len(lines[openings[0]]) - len(lines[openings[0]].lstrip(" "))
    body = []
    for line in lines[openings[0] + 1 :]:
        if line.strip() == MARKER:
            return "\n".join(body) + "\n"
        body.append(line[base:])
    raise AssertionError(f"{WORKFLOW} has no {MARKER} terminator")


def run(tmp_path, scope, config=""):
    """Runs the workflow script against a recording bazel and returns the result."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    bazel = bin_dir / "bazel"
    bazel.write_text(RECORDER, encoding="utf-8")
    bazel.chmod(bazel.stat().st_mode | stat.S_IEXEC)
    script = tmp_path / "scope.py"
    script.write_text(embedded_script(), encoding="utf-8")
    log = tmp_path / "argv.log"
    log.unlink(missing_ok=True)
    env = dict(os.environ)
    env.update(
        DX_DOCS_SCOPE=scope,
        DX_ARGV_LOG=str(log),
        BAZEL_CONFIG=config,
        BB_ARGS="",
        PATH=f"{bin_dir}{os.pathsep}{env.get('PATH', '')}",
    )
    done = subprocess.run(
        [sys.executable, str(script)],
        capture_output=True,
        cwd=tmp_path,
        encoding="utf-8",
        env=env,
    )
    argv = log.read_text(encoding="utf-8").splitlines() if log.exists() else None
    return done, argv


def test_the_workflow_runs_the_canonical_script():
    canonical = (ROOT / SCRIPT).read_text(encoding="utf-8")
    assert embedded_script() == canonical


def test_valid_scopes_reach_bazel_as_separate_arguments(tmp_path):
    done, argv = run(tmp_path, "//... //docs:all @dep//pkg:tgt", "--config=ci")
    assert done.returncode == 0, done.stderr
    assert argv == [
        "run",
        "--config=ci",
        "@rules_dx//:dx",
        "--",
        "lint",
        "--check",
        "//...",
        "//docs:all",
        "@dep//pkg:tgt",
        "--",
        "--config=ci",
    ]


def test_substitution_is_rejected_and_never_runs(tmp_path):
    done, argv = run(tmp_path, f"//...$(printf {PROBE} >&2)")
    assert done.returncode == 1
    assert argv is None
    assert "not a target pattern" in done.stderr
    assert PROBE not in (done.stdout + done.stderr).splitlines()


def test_separators_quotes_and_globs_are_rejected(tmp_path):
    for scope in (
        "//...;id",
        "//...\n//other",
        "//...\t//other",
        "//... && id",
        "//a|b",
        '"//..."',
        "'//...'",
        "//*",
        "//...`id`",
        "--fix",
        "-//foo",
        "foo",
        "",
        "   ",
    ):
        done, argv = run(tmp_path, scope)
        assert done.returncode == 1, scope
        assert argv is None, scope
        assert done.stderr.startswith("dx-ci: "), scope
