"""Behavioral fixtures for the rules_dx MODULE.bazel pin validator."""

import os
import pathlib
import shutil
import subprocess
import sys

FIXTURE_DIR = "tools/ci/module_pin_fixtures"
MODULE_FIXTURE = "MODULE.bazel.fixture"
HEREDOC = "python3 - <<'"
MARKER = "DX_MODULE_PIN"
PIN_ENV = "RULES_DX_PIN"


def workspace_root():
    """Returns the runfiles root this test reads its own tree from."""
    return pathlib.Path(__file__).resolve().parents[2]


ROOT = workspace_root()


def workflow_validator(tmp_path, name):
    """Writes the validator the named workflow runs, and returns its path."""
    workflow = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
    lines = workflow.splitlines()
    openings = [i for i, line in enumerate(lines) if HEREDOC + MARKER in line]
    assert len(openings) > 0, f"{name} runs no {MARKER} validator"
    base = len(lines[openings[0]]) - len(lines[openings[0]].lstrip(" "))
    body = []
    for line in lines[openings[0] + 1 :]:
        if line.strip() == MARKER:
            break
        body.append(line[base:])
    else:
        raise AssertionError(f"{name} has no {MARKER} terminator")
    path = tmp_path / f"{pathlib.Path(name).stem}.py"
    path.write_text("\n".join(body) + "\n", encoding="utf-8")
    return path


def expectation(case):
    """Returns the pin, exit status, texts and absent files of one fixture."""
    lines = (ROOT / FIXTURE_DIR / case / "expect.txt").read_text(encoding="utf-8")
    pin = ""
    status = None
    says = []
    absent = []
    for line in lines.splitlines():
        if line == "pin":
            continue
        if line.startswith("pin "):
            pin = line[len("pin ") :]
        elif line.startswith("exit "):
            status = int(line[len("exit ") :])
        elif line.startswith("says "):
            says.append(line[len("says ") :])
        elif line.startswith("absent "):
            absent.append(line[len("absent ") :])
    return pin, status, says, absent


def cases():
    """Returns every fixture name, sorted."""
    return sorted(path.name for path in (ROOT / FIXTURE_DIR).iterdir() if path.is_dir())


def scratch_case(tmp_path, case):
    """Copies one fixture module into a private directory and returns it."""
    home = tmp_path / case
    home.mkdir()
    shutil.copyfile(ROOT / FIXTURE_DIR / case / MODULE_FIXTURE, home / "MODULE.bazel")
    return home


def run(validator, home, pin):
    """Runs the validator over the module in home and returns code and text."""
    done = subprocess.run(
        [sys.executable, str(validator), "MODULE.bazel"],
        capture_output=True,
        cwd=home,
        encoding="utf-8",
        env=dict(os.environ, **{PIN_ENV: pin}),
    )
    return done.returncode, done.stdout + done.stderr


def repository_pin():
    """Returns the version this repository passes to its own self-call."""
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    for line in workflow.splitlines():
        if line.strip().startswith("rules_dx_version:"):
            return line.split(":", 1)[1].strip().strip('"')
    raise AssertionError("ci.yml passes no rules_dx_version")


def test_every_fixture_states_one_expected_outcome():
    names = cases()
    assert len(names) > 0, f"{FIXTURE_DIR} holds no fixture"
    for case in names:
        _, status, says, _ = expectation(case)
        assert status in (0, 1), f"{case} declares no exit status"
        assert len(says) > 0, f"{case} expects no validator text"


def test_fixtures_report_the_expected_diagnostic(tmp_path):
    validator = workflow_validator(tmp_path, "reusable-consumer.yml")
    for case in cases():
        pin, status, says, absent = expectation(case)
        home = scratch_case(tmp_path, case)
        code, output = run(validator, home, pin)
        assert code == status, f"{case} exited {code}, expected {status}: {output}"
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"
        for name in absent:
            assert not (home / name).exists(), f"{case} created {name}"


def test_the_docs_workflow_runs_the_same_validator(tmp_path):
    consumer = workflow_validator(tmp_path, "reusable-consumer.yml")
    docs = workflow_validator(tmp_path, "reusable-docs.yml")
    assert docs.read_bytes() == consumer.read_bytes()


def test_the_repository_module_declares_the_version_its_workflow_passes(tmp_path):
    validator = workflow_validator(tmp_path, "reusable-consumer.yml")
    pin = repository_pin()
    home = tmp_path / "repository"
    home.mkdir()
    shutil.copyfile(ROOT / "MODULE.bazel", home / "MODULE.bazel")
    code, output = run(validator, home, pin)
    assert code == 0, f"MODULE.bazel must declare rules_dx at {pin}: {output}"
    declared = f'module(name = "rules_dx", version = "{pin}") matches the workflow pin'
    assert declared in output
    wrong = "0.0.0-wrong"
    other, message = run(validator, home, wrong)
    assert other == 1, message
    assert f'the workflow expects "{wrong}"' in message
