"""Behavioral cases for the base pin in gate.yml."""

import os
import subprocess
import sys

import pytest
from workflow_testing import embedded_script

WORKFLOW = ".github/workflows/gate.yml"
HEREDOC = "python3 - <<'DX_GATE_BASE'"
MARKER = "DX_GATE_BASE"
ZEROS = "0" * 40


def git(repo, *args):
    """Runs git in repo and returns its trimmed stdout."""
    done = subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=True,
        env={
            **os.environ,
            "GIT_AUTHOR_NAME": "t",
            "GIT_AUTHOR_EMAIL": "t@example.com",
            "GIT_COMMITTER_NAME": "t",
            "GIT_COMMITTER_EMAIL": "t@example.com",
        },
    )
    return done.stdout.strip()


def commit(repo, name):
    """Commits one new file and returns the commit id."""
    (repo / name).write_text(name, encoding="utf-8")
    git(repo, "add", name)
    git(repo, "commit", "-q", "-m", name)
    return git(repo, "rev-parse", "HEAD")


@pytest.fixture(name="repo")
def make_repo(tmp_path):
    """Returns a repo with commits a, b, c on main and a stale branch."""
    path = tmp_path / "repo"
    path.mkdir()
    git(path, "init", "-q", "-b", "main")
    ids = {"a": commit(path, "a"), "b": commit(path, "b"), "c": commit(path, "c")}
    git(path, "checkout", "-q", "-b", "other", ids["a"])
    ids["x"] = commit(path, "x")
    git(path, "checkout", "-q", "main")
    return path, ids


def run_pin(repo, tmp_path, **env):
    """Runs the pin script and returns the process and the GATE_BASE it wrote."""
    out = tmp_path / "env"
    out.write_text("", encoding="utf-8")
    done = subprocess.run(
        [sys.executable, "-"],
        input=embedded_script(WORKFLOW, HEREDOC, MARKER),
        cwd=repo,
        env={**os.environ, "GITHUB_ENV": str(out), **env},
        capture_output=True,
        text=True,
        check=False,
    )
    text = out.read_text(encoding="utf-8").strip()
    return done, text.removeprefix("GATE_BASE=")


def test_pull_request_uses_the_first_parent(repo, tmp_path):
    """A PR merge ref pins its first parent."""
    path, ids = repo
    git(path, "merge", "-q", "--no-ff", "-m", "m", "other")
    done, base = run_pin(path, tmp_path, EVENT_NAME="pull_request")
    assert done.returncode == 0, done.stderr
    assert base == ids["c"]
    assert git(path, "rev-parse", "refs/remotes/origin/main") == ids["c"]


def test_push_uses_before(repo, tmp_path):
    """A push pins the commit before it."""
    path, ids = repo
    done, base = run_pin(path, tmp_path, EVENT_NAME="push", PUSH_BEFORE=ids["a"])
    assert done.returncode == 0, done.stderr
    assert base == ids["a"]


def test_push_with_zero_before_uses_first_parent(repo, tmp_path):
    """A push with an all-zero before falls back to HEAD^1."""
    path, ids = repo
    done, base = run_pin(path, tmp_path, EVENT_NAME="push", PUSH_BEFORE=ZEROS)
    assert done.returncode == 0, done.stderr
    assert base == ids["b"]


def test_force_push_resolves_through_the_merge_base(repo, tmp_path):
    """A before that is not an ancestor pins the merge base."""
    path, ids = repo
    done, base = run_pin(path, tmp_path, EVENT_NAME="push", PUSH_BEFORE=ids["x"])
    assert done.returncode == 0, done.stderr
    assert base == ids["a"]


def test_merge_group_uses_base_sha(repo, tmp_path):
    """A merge group pins its base_sha."""
    path, ids = repo
    done, base = run_pin(
        path, tmp_path, EVENT_NAME="merge_group", MERGE_GROUP_BASE=ids["a"]
    )
    assert done.returncode == 0, done.stderr
    assert base == ids["a"]


def test_base_equal_to_head_fails(repo, tmp_path):
    """A base that equals HEAD fails."""
    path, ids = repo
    done, _ = run_pin(path, tmp_path, EVENT_NAME="push", PUSH_BEFORE=ids["c"])
    assert done.returncode != 0
    assert "equals HEAD" in done.stderr


def test_missing_merge_group_value_fails(repo, tmp_path):
    """A merge_group event without base_sha fails."""
    path, _ = repo
    done, _ = run_pin(path, tmp_path, EVENT_NAME="merge_group", MERGE_GROUP_BASE="")
    assert done.returncode != 0
    assert "base_sha" in done.stderr
