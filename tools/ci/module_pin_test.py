"""Behavioral fixtures for the rules_dx MODULE.bazel pin validator."""

import os
import shutil
import subprocess
import sys

from workflow_testing import ROOT

FIXTURE_DIR = "tools/ci/module_pin_fixtures"
MODULE_FIXTURE = "MODULE.bazel.fixture"
VALIDATOR = "tools/ci/module_pin.py"
PIN_ENV = "RULES_DX_PIN"


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
    source = ROOT / FIXTURE_DIR / case
    for path in source.rglob("*"):
        if not path.is_file() or path.name == "expect.txt":
            continue
        name = path.relative_to(source)
        if name.name.endswith(".fixture"):
            name = name.with_name(name.name[: -len(".fixture")])
        target = home / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
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
    validator = ROOT / VALIDATOR
    for case in cases():
        pin, status, says, absent = expectation(case)
        home = scratch_case(tmp_path, case)
        code, output = run(validator, home, pin)
        assert code == status, f"{case} exited {code}, expected {status}: {output}"
        for text in says:
            assert text in output, f"{case} never reported [{text}]: {output}"
        for name in absent:
            assert not (home / name).exists(), f"{case} created {name}"


def test_the_repository_module_declares_the_version_its_workflow_passes(tmp_path):
    validator = ROOT / VALIDATOR
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


def test_bazel_run_reads_the_module_in_the_calling_directory(tmp_path):
    case = "pass_external_bazel_dep"
    home = scratch_case(tmp_path, case)
    pin, _, _, _ = expectation(case)
    elsewhere = tmp_path / "runfiles"
    elsewhere.mkdir()
    done = subprocess.run(
        [sys.executable, str(ROOT / VALIDATOR)],
        capture_output=True,
        cwd=elsewhere,
        encoding="utf-8",
        env=dict(os.environ, **{PIN_ENV: pin, "BUILD_WORKING_DIRECTORY": str(home)}),
    )
    assert done.returncode == 0, done.stdout + done.stderr


BAD_LABELS = [
    "@//:x.MODULE.bazel",
    "@other//:x.MODULE.bazel",
    "//../a:x.MODULE.bazel",
    "//:../x.MODULE.bazel",
    "//a\\\\b:x.MODULE.bazel",
    "//a//b:x.MODULE.bazel",
    "//pkg",
    ":x.MODULE.bazel",
    "//:x.bzl",
    "//:.h.MODULE.bazel",
]


def test_bad_include_labels_are_rejected_and_never_read(tmp_path):
    validator = ROOT / VALIDATOR
    (tmp_path / "x.MODULE.bazel").write_text(
        'bazel_dep(name = "rules_dx", version = "1.2.3")\n', encoding="utf-8"
    )
    for index, label in enumerate(BAD_LABELS):
        home = tmp_path / f"home{index}"
        home.mkdir()
        (home / "MODULE.bazel").write_text(
            f'include("{label}")\n', encoding="utf-8"
        )
        code, output = run(validator, home, "1.2.3")
        assert code == 1, f"{label} was accepted: {output}"
        assert "matches" not in output, f"{label} was read: {output}"
        assert "include " + label in output, f"{label}: {output}"
