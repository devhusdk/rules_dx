"""Behavioral fixtures for the stale-run filters cancel-stale.yml runs."""

import json
import os
import subprocess
import sys

import pytest
from workflow_testing import ROOT, embedded_script

WORKFLOW = ".github/workflows/cancel-stale.yml"
FIXTURE_DIR = "tools/ci/cancel_stale_fixtures"
PR_HEREDOC = "python3 - runs.jsonl open.jsonl <<'DX_STALE_PR_RUNS' > stale.txt"
PR_MARKER = "DX_STALE_PR_RUNS"
MAIN_HEREDOC = "python3 - runs.jsonl <<'DX_STALE_MAIN_RUNS' > stale.txt"
MAIN_MARKER = "DX_STALE_MAIN_RUNS"
PR_FILES = {"runs.jsonl", "open.jsonl", "expect.txt"}
MAIN_FILES = {"runs.jsonl", "env.json", "expect.txt"}


def cases(kind):
    """Returns every fixture name of one kind, sorted."""
    directory = ROOT / FIXTURE_DIR / kind
    return sorted(path.name for path in directory.iterdir() if path.is_dir())


def start_filter(script, home, args, env=None):
    """Runs one filter in home and returns the finished process."""
    return subprocess.run(
        [sys.executable, "-", *args],
        input=script,
        cwd=home,
        env={**os.environ, **(env or {})},
        capture_output=True,
        text=True,
        check=False,
    )


def run_filter(script, home, args, env=None):
    """Runs one filter in home and returns the ids it prints."""
    done = start_filter(script, home, args, env)
    assert done.returncode == 0, done.stderr
    return done.stdout


def copy_fixture(tmp_path, kind, case, names):
    """Copies the named fixture files into tmp_path and returns the fixture dir."""
    source = ROOT / FIXTURE_DIR / kind / case
    for name in names:
        (tmp_path / name).write_bytes((source / name).read_bytes())
    return source


@pytest.mark.parametrize("case", cases("pr"))
def test_pr_fixtures(tmp_path, case):
    """Each PR fixture prints exactly the run ids it expects."""
    script = embedded_script(WORKFLOW, PR_HEREDOC, PR_MARKER)
    source = copy_fixture(tmp_path, "pr", case, ["runs.jsonl", "open.jsonl"])
    got = run_filter(script, tmp_path, ["runs.jsonl", "open.jsonl"])
    assert got == (source / "expect.txt").read_text(encoding="utf-8")


@pytest.mark.parametrize("case", cases("main"))
def test_main_fixtures(tmp_path, case):
    """Each main fixture prints exactly the run ids it expects."""
    script = embedded_script(WORKFLOW, MAIN_HEREDOC, MAIN_MARKER)
    source = copy_fixture(tmp_path, "main", case, ["runs.jsonl"])
    env = json.loads((source / "env.json").read_text(encoding="utf-8"))
    got = run_filter(script, tmp_path, ["runs.jsonl"], env)
    assert got == (source / "expect.txt").read_text(encoding="utf-8")


@pytest.mark.parametrize("case", ["pr_no_open", "pr_no_runs", "main_no_runs"])
def test_missing_listing_fails(tmp_path, case):
    """A missing listing fails the filter instead of cancelling runs."""
    if case == "main_no_runs":
        script = embedded_script(WORKFLOW, MAIN_HEREDOC, MAIN_MARKER)
        source = ROOT / FIXTURE_DIR / "main" / "new_sha_is_kept"
        env = json.loads((source / "env.json").read_text(encoding="utf-8"))
        done = start_filter(script, tmp_path, ["runs.jsonl"], env)
    else:
        script = embedded_script(WORKFLOW, PR_HEREDOC, PR_MARKER)
        if case == "pr_no_open":
            copy_fixture(tmp_path, "pr", "no_open_prs_cancels_all", ["runs.jsonl"])
        else:
            copy_fixture(tmp_path, "pr", "run_of_open_pr_is_kept", ["open.jsonl"])
        done = start_filter(script, tmp_path, ["runs.jsonl", "open.jsonl"])
    assert done.returncode != 0
    assert done.stdout == ""
    assert "FileNotFoundError" in done.stderr


def test_runs_are_listed_before_open_prs():
    """The closed-PR sweep lists runs before it lists open PRs."""
    text = (ROOT / WORKFLOW).read_text(encoding="utf-8")
    runs = text.index('"repos/${GH_REPO}/actions/runs" -f event=pull_request')
    prs = text.index("-f state=open")
    assert runs < prs
    assert text.index("done > runs.jsonl") < prs


@pytest.mark.parametrize(("kind", "files"), [("pr", PR_FILES), ("main", MAIN_FILES)])
def test_fixture_dirs_hold_exactly_their_files(kind, files):
    """Every fixture dir holds exactly the files its filter reads."""
    assert cases(kind)
    for case in cases(kind):
        directory = ROOT / FIXTURE_DIR / kind / case
        assert {path.name for path in directory.iterdir()} == files, case


def test_main_sweep_skips_runs_with_jobs():
    """The main sweep reads a run's jobs and skips it once it has any job."""
    text = (ROOT / WORKFLOW).read_text(encoding="utf-8")
    sweep = text[text.index(MAIN_MARKER) :]
    jobs = sweep.index('"repos/${GH_REPO}/actions/runs/${id}/jobs"')
    skip = sweep.index('echo "run ${id} already has jobs"')
    cancel = sweep.index('"repos/${GH_REPO}/actions/runs/${id}/cancel"')
    assert jobs < skip < cancel
    assert "--jq '.total_count'" in sweep
    assert "select(.status" not in sweep
