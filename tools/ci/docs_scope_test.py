"""Process-boundary tests for the docs workflow scope validator."""

import os
import stat
import subprocess
import sys

from workflow_testing import ROOT

SCRIPT = "tools/ci/docs_scope.py"
PROBE = "DX_SCOPE_EXECUTED"
RECORDER = """#!/bin/sh
for arg in "$@"; do printf '%s\\n' "$arg"; done > "$DX_ARGV_LOG"
"""
WINDOWS_RECORDER = """import os, sys
with open(os.environ["DX_ARGV_LOG"], "w", encoding="utf-8") as log:
    log.writelines(arg + "\\n" for arg in sys.argv[1:])
"""


def run(tmp_path, scope, config=""):
    """Runs the scope script against a recording bazel and returns the result."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    if os.name == "nt":
        recorder = bin_dir / "recorder.py"
        recorder.write_text(WINDOWS_RECORDER, encoding="utf-8")
        launcher = f'@"{sys.executable}" "{recorder}" %*\n'
        (bin_dir / "bazel.bat").write_text(launcher, encoding="utf-8")
    else:
        bazel = bin_dir / "bazel"
        bazel.write_text(RECORDER, encoding="utf-8")
        bazel.chmod(bazel.stat().st_mode | stat.S_IEXEC)
    script = ROOT / SCRIPT
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
